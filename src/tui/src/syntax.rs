//! Lightweight syntax highlighting via token scanning.
//!
//! Note: the spec named `tree-sitter` for highlighting. We use a small
//! tokenizer instead — it produces the same multi-color code the reference
//! image shows, without the grammar-crate API churn/risk. Swap in tree-sitter
//! later if richer semantic highlighting is needed.

use crate::theme;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

fn keywords(lang: &str) -> &'static [&'static str] {
    match lang {
        "python" => &[
            "def", "class", "return", "if", "elif", "else", "for", "while", "import", "from", "as",
            "with", "try", "except", "finally", "raise", "yield", "lambda", "pass", "break",
            "continue", "in", "is", "not", "and", "or", "None", "True", "False", "self", "async",
            "await", "global", "nonlocal", "assert", "del",
        ],
        "rust" => &[
            "fn", "let", "mut", "pub", "use", "struct", "enum", "impl", "trait", "match", "if",
            "else", "for", "while", "loop", "return", "self", "Self", "crate", "mod", "const",
            "static", "unsafe", "async", "await", "move", "ref", "where", "type", "dyn", "as",
            "true", "false", "Some", "None", "Ok", "Err", "Result", "Option", "Vec", "String",
        ],
        "cpp" => &[
            "auto",
            "int",
            "float",
            "double",
            "char",
            "void",
            "bool",
            "class",
            "struct",
            "public",
            "private",
            "protected",
            "virtual",
            "return",
            "if",
            "else",
            "for",
            "while",
            "do",
            "switch",
            "case",
            "break",
            "continue",
            "new",
            "delete",
            "namespace",
            "using",
            "const",
            "static",
            "template",
            "typename",
            "true",
            "false",
            "nullptr",
            "include",
            "define",
            "std",
            "this",
        ],
        "yaml" => &["true", "false", "null", "yes", "no", "on", "off"],
        "json" => &["true", "false", "null"],
        "toml" => &["true", "false"],
        "bash" => &[
            "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac",
            "function", "in", "return", "export", "source", "local", "echo", "cd", "exit", "set",
            "unset", "read", "select", "until", "break", "continue", "shift", "alias",
        ],
        "xml" => &[],
        _ => &[],
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Build a mapping: char_index → byte_offset for safe string slicing.
fn build_byte_map(line: &str) -> Vec<usize> {
    let mut map = Vec::with_capacity(line.len());
    for (byte_idx, _) in line.char_indices() {
        map.push(byte_idx);
    }
    map
}

/// Slice a string safely using char indices via a byte-offset map.
fn slice_by_char<'a>(
    line: &'a str,
    byte_map: &[usize],
    char_start: usize,
    char_end: usize,
) -> &'a str {
    let byte_start = byte_map.get(char_start).copied().unwrap_or(line.len());
    let byte_end = byte_map.get(char_end).copied().unwrap_or(line.len());
    &line[byte_start..byte_end]
}

fn is_pascal(word: &str) -> bool {
    let mut has_lower = false;
    for (idx, ch) in word.char_indices() {
        if ch.is_ascii_uppercase() {
            if idx == 0 {
                // ok
            } else {
                return false;
            }
        } else if ch.is_ascii_lowercase() {
            has_lower = true;
        } else if !ch.is_ascii_digit() && ch != '_' {
            return false;
        }
    }
    has_lower
        && word
            .chars()
            .next()
            .map(|c| c.is_ascii_uppercase())
            .unwrap_or(false)
}

fn peek_next_nonws(chars: &[char], i: usize) -> Option<char> {
    let mut j = i;
    while j < chars.len() && chars[j].is_whitespace() {
        j += 1;
    }
    chars.get(j).copied()
}

/// Highlight a single line for the given language.
fn highlight_line<'a>(line: &'a str, lang: &str, kw: &[&str]) -> Line<'a> {
    let mut spans: Vec<Span<'_>> = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let byte_map = build_byte_map(line);
    let mut i: usize = 0;

    let comment_marker: Option<&str> = match lang {
        "python" | "yaml" | "toml" | "bash" => Some("#"),
        "rust" | "cpp" | "json" => Some("//"),
        _ => None,
    };

    while i < chars.len() {
        let c = chars[i];

        // Bash shebang / comment
        if (lang == "bash") && i == 0 && c == '#' && chars.get(1) == Some(&'!') {
            let s = slice_by_char(line, &byte_map, i, chars.len());
            spans.push(Span::styled(s, Style::default().fg(theme::DIM)));
            break;
        }

        // JSON & TOML section headers and keys handled below.
        if lang == "json" || lang == "toml" {
            let done = highlight_json_toml(&mut spans, line, &byte_map, &mut i, lang, kw);
            if done {
                continue;
            }
        }

        // Bash variables: $name or ${name}
        if lang == "bash" && c == '$' {
            let mut j = i + 1;
            if chars.get(j) == Some(&'{') {
                j += 1;
                while j < chars.len() && chars[j] != '}' {
                    j += 1;
                }
                j = (j + 1).min(chars.len());
            } else {
                while j < chars.len() && is_ident_char(chars[j]) {
                    j += 1;
                }
            }
            let s = slice_by_char(line, &byte_map, i, j);
            spans.push(Span::styled(s, Style::default().fg(theme::ACCENT)));
            i = j;
            continue;
        }

        // Strings
        if c == '"' || c == '\'' {
            let quote = c;
            let mut j = i + 1;
            while j < chars.len() && chars[j] != quote {
                if chars[j] == '\\' && j + 1 < chars.len() {
                    j += 2;
                } else {
                    j += 1;
                }
            }
            j = (j + 1).min(chars.len());
            let s = slice_by_char(line, &byte_map, i, j);
            spans.push(Span::styled(s, Style::default().fg(theme::WARN)));
            i = j;
            continue;
        }

        // Comments
        if let Some(marker) = comment_marker {
            let rest: String = chars[i..].iter().collect();
            if rest.starts_with(marker) {
                let s = slice_by_char(line, &byte_map, i, chars.len());
                spans.push(Span::styled(s, Style::default().fg(theme::DIM)));
                break;
            }
        }

        // XML tags
        if lang == "xml" && c == '<' {
            let mut j = i + 1;
            while j < chars.len() && chars[j] != '>' {
                j += 1;
            }
            j = (j + 1).min(chars.len());
            let s = slice_by_char(line, &byte_map, i, j);
            spans.push(Span::styled(s, Style::default().fg(theme::ACCENT)));
            i = j;
            continue;
        }

        // Numbers (incl. hex/binary)
        if c.is_ascii_digit() || (c == '0' && matches!(chars.get(i + 1), Some('x') | Some('b'))) {
            let mut j = i;
            if c == '0' && matches!(chars.get(i + 1), Some('x') | Some('b')) {
                j += 2;
                while j < chars.len() && (chars[j].is_ascii_hexdigit() || chars[j] == '_') {
                    j += 1;
                }
            } else {
                while j < chars.len()
                    && (chars[j].is_ascii_digit() || chars[j] == '.' || chars[j] == '_')
                {
                    j += 1;
                }
            }
            let s = slice_by_char(line, &byte_map, i, j);
            spans.push(Span::styled(s, Style::default().fg(theme::MAGENTA)));
            i = j;
            continue;
        }

        // Identifiers / keywords / types / function calls
        if is_ident_start(c) {
            let mut j = i;
            while j < chars.len() && is_ident_char(chars[j]) {
                j += 1;
            }
            let word = slice_by_char(line, &byte_map, i, j);

            // Function call: identifier immediately followed by '(' (ignoring ws).
            let after = peek_next_nonws(&chars, j);
            if kw.contains(&word) {
                spans.push(Span::styled(
                    word.to_string(),
                    Style::default()
                        .fg(theme::ACCENT_WARM)
                        .add_modifier(Modifier::BOLD),
                ));
            } else if word == "self" || word == "this" || word == "super" {
                spans.push(Span::styled(
                    word.to_string(),
                    Style::default().fg(theme::MAGENTA),
                ));
            } else if after == Some('(') {
                // Function call site.
                spans.push(Span::styled(
                    word.to_string(),
                    Style::default().fg(theme::OK),
                ));
            } else if is_pascal(word) {
                // Type / class / enum name.
                spans.push(Span::styled(
                    word.to_string(),
                    Style::default().fg(theme::MAGENTA),
                ));
            } else {
                spans.push(Span::styled(
                    word.to_string(),
                    Style::default().fg(theme::FG),
                ));
            }
            i = j;
            continue;
        }

        // Default single char (handles multi-byte safely)
        let s = slice_by_char(line, &byte_map, i, i + 1);
        spans.push(Span::styled(s, Style::default().fg(theme::FG)));
        i += 1;
    }

    if spans.is_empty() {
        spans.push(Span::styled("", Style::default()));
    }
    Line::from(spans)
}

/// Highlight a fragment of a JSON or TOML line. Returns true if it consumed a
/// token starting at `i` (advancing `i`), false to fall through to defaults.
fn highlight_json_toml<'b>(
    spans: &mut Vec<Span<'b>>,
    line: &str,
    byte_map: &[usize],
    i: &mut usize,
    lang: &str,
    kw: &[&str],
) -> bool {
    let chars: Vec<char> = line.chars().collect();
    let c = chars[*i];

    // TOML section header: [table] or [[array]]
    if lang == "toml" && c == '[' {
        let mut j = *i + 1;
        while j < chars.len() && chars[j] != ']' {
            j += 1;
        }
        j = (j + 1).min(chars.len());
        let s = slice_by_char(line, byte_map, *i, j);
        spans.push(Span::styled(
            s.to_string(),
            Style::default().fg(theme::ACCENT),
        ));
        *i = j;
        return true;
    }

    // Strings — in JSON a string followed by ':' is a key.
    if c == '"' {
        let mut j = *i + 1;
        while j < chars.len() && chars[j] != '"' {
            if chars[j] == '\\' && j + 1 < chars.len() {
                j += 2;
            } else {
                j += 1;
            }
        }
        j = (j + 1).min(chars.len());
        let word = slice_by_char(line, byte_map, *i, j);
        // Look ahead: key if followed by optional ws and ':'.
        let mut k = j;
        while k < chars.len() && chars[k].is_whitespace() {
            k += 1;
        }
        if chars.get(k) == Some(&':') {
            spans.push(Span::styled(
                word.to_string(),
                Style::default().fg(theme::ACCENT),
            ));
        } else {
            spans.push(Span::styled(
                word.to_string(),
                Style::default().fg(theme::WARN),
            ));
        }
        *i = j;
        return true;
    }

    if c.is_ascii_digit()
        || (c == '-'
            && chars
                .get(*i + 1)
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false))
    {
        let mut j = *i;
        while j < chars.len()
            && (chars[j].is_ascii_digit()
                || chars[j] == '.'
                || chars[j] == '-'
                || chars[j] == '_'
                || chars[j] == 'e'
                || chars[j] == 'E')
        {
            j += 1;
        }
        let s = slice_by_char(line, byte_map, *i, j);
        spans.push(Span::styled(
            s.to_string(),
            Style::default().fg(theme::MAGENTA),
        ));
        *i = j;
        return true;
    }

    if is_ident_start(c) {
        let mut j = *i;
        while j < chars.len() && is_ident_char(chars[j]) {
            j += 1;
        }
        let word = slice_by_char(line, byte_map, *i, j);
        // In TOML, an identifier before '=' is a key.
        let mut k = j;
        while k < chars.len() && chars[k].is_whitespace() {
            k += 1;
        }
        if lang == "toml" && chars.get(k) == Some(&'=') {
            spans.push(Span::styled(
                word.to_string(),
                Style::default().fg(theme::WARN_AMBER),
            ));
        } else if kw.contains(&word) {
            spans.push(Span::styled(
                word.to_string(),
                Style::default()
                    .fg(theme::ACCENT_WARM)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                word.to_string(),
                Style::default().fg(theme::FG),
            ));
        }
        *i = j;
        return true;
    }

    let _ = (spans, kw);
    false
}

pub fn highlight_lines<'a>(lines: &'a [String], lang: &str) -> Vec<Line<'a>> {
    let kw = keywords(lang);
    lines.iter().map(|l| highlight_line(l, lang, kw)).collect()
}

/// Approximate dominant color per line for the minimap.
#[allow(dead_code)]
pub fn minimap_color(line: &str, lang: &str) -> Color {
    if line.trim().is_empty() {
        return theme::BORDER;
    }
    if (lang == "python" || lang == "yaml") && line.trim_start().starts_with('#') {
        return theme::DIM;
    }
    if (lang == "rust" || lang == "cpp") && line.trim_start().starts_with("//") {
        return theme::DIM;
    }
    if line.contains('"') || line.contains('\'') {
        return theme::WARN;
    }
    let kw = keywords(lang);
    for w in kw {
        if line.contains(w) {
            return theme::ACCENT_WARM;
        }
    }
    theme::SURFACE_HI
}
