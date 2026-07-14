#![allow(dead_code)]
//! Editor state: open files, text buffers, cursor, dirty tracking, outline.
//! cc@zang aka Gaurav-x111

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditMode {
    Preview,
    Edit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChangeKind {
    None,
    Added,
    Modified,
    Deleted,
}

#[derive(Clone)]
pub struct OpenFile {
    pub path: PathBuf,
    pub lines: Vec<String>,
    /// Lines as originally loaded from disk (used to compute change indicators).
    pub saved: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub scroll_row: usize,
    pub scroll_col: usize,
    pub dirty: bool,
    /// Visual selection: (anchor_row, anchor_col). `None` = no selection.
    pub selection: Option<(usize, usize)>,
    /// Undo stack: (lines, cursor_row, cursor_col)
    undo_stack: Vec<(Vec<String>, usize, usize)>,
    /// Redo stack
    redo_stack: Vec<(Vec<String>, usize, usize)>,
    /// Current line highlight
    pub highlight_line: bool,
    /// True for the built-in, non-persisted "Welcome" buffer.
    pub welcome: bool,
    /// Neovim-mode chord prefix (e.g. the first `d` of `dd`). `None` = no
    /// pending operator.
    pub pending_key: Option<char>,
}

impl OpenFile {
    pub fn open(path: PathBuf) -> Self {
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else {
            content.split('\n').map(|s| s.to_string()).collect()
        };
        let saved = lines.clone();
        Self {
            path,
            lines,
            saved,
            cursor_row: 0,
            cursor_col: 0,
            scroll_row: 0,
            scroll_col: 0,
            dirty: false,
            selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            highlight_line: true,
            welcome: false,
            pending_key: None,
        }
    }

    pub fn new_buffer(path: PathBuf) -> Self {
        Self {
            path,
            lines: vec![String::new()],
            saved: Vec::new(),
            cursor_row: 0,
            cursor_col: 0,
            scroll_row: 0,
            scroll_col: 0,
            dirty: true,
            selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            highlight_line: true,
            welcome: false,
            pending_key: None,
        }
    }

    /// The built-in "Welcome — ROS2_INFO" buffer shown on first launch.
    /// Not persisted to disk; `save()` is a no-op for it.
    pub fn welcome() -> Self {
        let content = welcome_text();
        let lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else {
            content.split('\n').map(|s| s.to_string()).collect()
        };
        let saved = lines.clone();
        Self {
            path: PathBuf::from("Welcome — ROS2_INFO"),
            lines,
            saved,
            cursor_row: 0,
            cursor_col: 0,
            scroll_row: 0,
            scroll_col: 0,
            dirty: false,
            selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            highlight_line: false,
            welcome: true,
            pending_key: None,
        }
    }

    pub fn is_untitled(&self) -> bool {
        !self.path.exists() && !self.path.parent().is_some_and(|p| p.exists())
    }

    pub fn set_path(&mut self, path: PathBuf) {
        self.path = path;
    }

    pub fn filename(&self) -> String {
        let name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "<untitled>".to_string());
        if name.is_empty() || name == "." {
            "<untitled>".to_string()
        } else {
            name
        }
    }

    pub fn name(&self) -> String {
        self.filename()
    }

    pub fn language(&self) -> &'static str {
        let ext = self
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let name = self.filename().to_lowercase();
        if name.ends_with(".launch.py") || name.ends_with(".test.py") {
            return "python";
        }
        match ext.as_str() {
            "py" => "python",
            "rs" => "rust",
            "cpp" | "cc" | "cxx" | "h" | "hpp" => "cpp",
            "c" => "cpp",
            "yaml" | "yml" => "yaml",
            "xml" | "launch" | "urdf" | "xacro" => "xml",
            "md" => "markdown",
            "json" => "json",
            "toml" => "toml",
            "sh" | "bash" | "zsh" => "bash",
            "msg" | "srv" | "action" => "text",
            _ => "text",
        }
    }

    pub fn change_indicator(&self, row: usize) -> ChangeKind {
        if !self.dirty {
            return ChangeKind::None;
        }
        if row >= self.saved.len() && row < self.lines.len() {
            return ChangeKind::Added;
        }
        if row < self.saved.len() && row < self.lines.len() && self.saved[row] != self.lines[row] {
            return ChangeKind::Modified;
        }
        if row >= self.lines.len() && row < self.saved.len() {
            return ChangeKind::Deleted;
        }
        ChangeKind::None
    }

    pub fn save(&mut self) {
        // The Welcome buffer is informational only — never write it to disk.
        if self.welcome {
            return;
        }
        let content = self.lines.join("\n");
        let _ = std::fs::write(&self.path, content);
        self.saved = self.lines.clone();
        self.dirty = false;
    }

    /// Remove the active selection (if any), replacing it with nothing and
    /// moving the cursor to the anchor. Returns true if a selection was removed.
    pub fn delete_selection(&mut self) -> bool {
        let (ar, ac) = match self.selection.take() {
            Some(a) => a,
            None => return false,
        };
        let (br, bc) = (self.cursor_row, self.cursor_col);
        let ((sr, sc), (er, ec)) = if (ar, ac) <= (br, bc) {
            ((ar, ac), (br, bc))
        } else {
            ((br, bc), (ar, ac))
        };
        if sr == er {
            let line = &mut self.lines[sr];
            let s = line
                .char_indices()
                .nth(sc)
                .map(|(i, _)| i)
                .unwrap_or(line.len());
            let e = line
                .char_indices()
                .nth(ec)
                .map(|(i, _)| i)
                .unwrap_or(line.len());
            line.replace_range(s..e, "");
        } else {
            let head = self.lines[sr].clone();
            let tail = self.lines[er].clone();
            let s = head
                .char_indices()
                .nth(sc)
                .map(|(i, _)| i)
                .unwrap_or(head.len());
            let e = tail
                .char_indices()
                .nth(ec)
                .map(|(i, _)| i)
                .unwrap_or(tail.len());
            let mut merged = head;
            merged.replace_range(s.., "");
            let mut tail = tail;
            tail.replace_range(..e, "");
            merged.push_str(&tail);
            self.lines[sr] = merged;
            self.lines.drain(sr + 1..=er);
        }
        self.cursor_row = sr;
        self.cursor_col = sc;
        self.dirty = true;
        true
    }

    pub fn insert_char(&mut self, ch: char) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        let row = self.cursor_row.min(self.lines.len().saturating_sub(1));
        if row >= self.lines.len() {
            self.lines.push(String::new());
        }
        self.push_undo();
        self.delete_selection();
        let line = &mut self.lines[row];
        let col = self.cursor_col.min(line.chars().count());
        let byte_idx = line
            .char_indices()
            .nth(col)
            .map(|(i, _)| i)
            .unwrap_or(line.len());
        line.insert(byte_idx, ch);
        self.cursor_col += 1;
        self.dirty = true;
    }

    pub fn insert_newline(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.push_undo();
        self.delete_selection();
        let row = self.cursor_row.min(self.lines.len().saturating_sub(1));
        let line = std::mem::take(&mut self.lines[row]);
        let col = self.cursor_col.min(line.chars().count());
        let (left, right) = line.split_at(
            line.char_indices()
                .nth(col)
                .map(|(i, _)| i)
                .unwrap_or(line.len()),
        );
        self.lines.insert(row + 1, right.to_string());
        self.lines[row] = left.to_string();
        self.cursor_row = row + 1;
        self.cursor_col = 0;
        self.dirty = true;
    }

    pub fn backspace(&mut self) {
        if self.lines.is_empty() {
            return;
        }
        if self.selection.is_some() {
            self.push_undo();
            self.delete_selection();
            return;
        }
        let row = self.cursor_row.min(self.lines.len().saturating_sub(1));
        if self.cursor_col > 0 {
            self.push_undo();
            let line = &mut self.lines[row];
            let col = self.cursor_col.min(line.chars().count());
            let byte_idx = line
                .char_indices()
                .nth(col - 1)
                .map(|(i, _)| i)
                .unwrap_or(0);
            line.remove(byte_idx);
            self.cursor_col -= 1;
            self.dirty = true;
        } else if row > 0 {
            self.push_undo();
            let cur = std::mem::take(&mut self.lines[row]);
            let prev = self.lines[row - 1].chars().count();
            self.lines[row - 1].push_str(&cur);
            self.lines.remove(row);
            self.cursor_row -= 1;
            self.cursor_col = prev;
            self.dirty = true;
        }
    }

    pub fn push_undo(&mut self) {
        self.undo_stack
            .push((self.lines.clone(), self.cursor_row, self.cursor_col));
        self.redo_stack.clear();
        if self.undo_stack.len() > 200 {
            self.undo_stack.remove(0);
        }
    }

    pub fn undo(&mut self) {
        if let Some((lines, row, col)) = self.undo_stack.pop() {
            self.redo_stack
                .push((self.lines.clone(), self.cursor_row, self.cursor_col));
            self.lines = lines;
            self.cursor_row = row;
            self.cursor_col = col;
            self.dirty = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some((lines, row, col)) = self.redo_stack.pop() {
            self.undo_stack
                .push((self.lines.clone(), self.cursor_row, self.cursor_col));
            self.lines = lines;
            self.cursor_row = row;
            self.cursor_col = col;
            self.dirty = true;
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Return the currently selected text (or `None` if there is no selection).
    pub fn selected_text(&self) -> Option<String> {
        let ((sr, sc), (er, ec)) = self.selection_range()?;
        let mut out = String::new();
        for r in sr..=er {
            if let Some(line) = self.lines.get(r) {
                let len = line.chars().count();
                let (a, b) = if sr == er {
                    (sc, ec)
                } else if r == sr {
                    (sc, len)
                } else if r == er {
                    (0, ec)
                } else {
                    (0, len)
                };
                let a = a.min(len);
                let b = b.min(len);
                let s: String = line.chars().skip(a).take(b - a).collect();
                out.push_str(&s);
            }
            if r != er {
                out.push('\n');
            }
        }
        Some(out)
    }

    /// Ordered (inclusive) selection endpoints, or None.
    pub fn selection_range(&self) -> Option<((usize, usize), (usize, usize))> {
        let (ar, ac) = self.selection?;
        let (br, bc) = (self.cursor_row, self.cursor_col);
        Some(if (ar, ac) <= (br, bc) {
            ((ar, ac), (br, bc))
        } else {
            ((br, bc), (ar, ac))
        })
    }

    /// Is the character at (row, col) part of the active selection?
    pub fn char_selected(&self, row: usize, col: usize) -> bool {
        if let Some(((sr, sc), (er, ec))) = self.selection_range() {
            if row < sr || row > er {
                return false;
            }
            if row == sr && col < sc {
                return false;
            }
            if row == er && col >= ec {
                return false;
            }
            true
        } else {
            false
        }
    }

    /// Keep the cursor visually within `height` rows, adjusting scroll_row.
    /// scroll_row is interpreted as a *visual* row index (see build_visual_lines
    /// in ui.rs), so this works whether or not word-wrap is on.
    pub fn scroll_to_cursor(&mut self, height: usize) {
        if height == 0 {
            return;
        }
        if self.cursor_row < self.scroll_row {
            self.scroll_row = self.cursor_row;
        } else if self.cursor_row >= self.scroll_row + height {
            self.scroll_row = self.cursor_row - height + 1;
        }
    }

    pub fn select_all(&mut self) {
        if !self.lines.is_empty() {
            let last = self.lines.len() - 1;
            let last_col = self.lines[last].chars().count();
            self.selection = Some((0, 0));
            self.cursor_row = last;
            self.cursor_col = last_col;
        }
    }

    pub fn goto_line(&mut self, line: usize) {
        if line > 0 && line <= self.lines.len() {
            self.cursor_row = line - 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_left(&mut self) {
        self.clear_selection();
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].chars().count();
        }
    }

    pub fn move_right(&mut self) {
        self.clear_selection();
        let row = self.cursor_row.min(self.lines.len().saturating_sub(1));
        let len = self.lines.get(row).map(|l| l.chars().count()).unwrap_or(0);
        if self.cursor_col < len {
            self.cursor_col += 1;
        } else if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_up(&mut self) {
        self.clear_selection();
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
        }
    }

    pub fn move_down(&mut self) {
        self.clear_selection();
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
        }
    }

    /// Anchor the selection at the current cursor if one is not already set.
    /// Used by the Shift+Arrow "extend" family below.
    pub fn anchor_selection(&mut self) {
        if self.selection.is_none() {
            self.selection = Some((self.cursor_row, self.cursor_col));
        }
    }

    pub fn extend_left(&mut self) {
        self.anchor_selection();
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].chars().count();
        }
    }

    pub fn extend_right(&mut self) {
        self.anchor_selection();
        let row = self.cursor_row.min(self.lines.len().saturating_sub(1));
        let len = self.lines.get(row).map(|l| l.chars().count()).unwrap_or(0);
        if self.cursor_col < len {
            self.cursor_col += 1;
        } else if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
    }

    pub fn extend_up(&mut self) {
        self.anchor_selection();
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
        }
    }

    pub fn extend_down(&mut self) {
        self.anchor_selection();
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
        }
    }

    /// Find the next occurrence of `query` at/after (from_row, from_col),
    /// wrapping around to the top. Returns (row, char_col, match_len).
    pub fn find_next(
        &self,
        query: &str,
        from_row: usize,
        from_col: usize,
    ) -> Option<(usize, usize, usize)> {
        if query.is_empty() {
            return None;
        }
        let n = self.lines.len();
        let mut first = true;
        for i in from_row..n {
            let line = &self.lines[i];
            let byte_start = if first {
                line.char_indices()
                    .nth(from_col)
                    .map(|(b, _)| b)
                    .unwrap_or(line.len())
            } else {
                0
            };
            first = false;
            if let Some(pos) = line[byte_start..].find(query) {
                let match_byte = byte_start + pos;
                let char_idx = line[..match_byte].chars().count();
                return Some((i, char_idx, query.chars().count()));
            }
        }
        for i in 0..from_row.min(n) {
            let line = &self.lines[i];
            if let Some(pos) = line.find(query) {
                let char_idx = line[..pos].chars().count();
                return Some((i, char_idx, query.chars().count()));
            }
        }
        None
    }

    /// All match positions of `query` for highlighting.
    pub fn find_matches(&self, query: &str) -> Vec<(usize, usize, usize)> {
        let mut out = Vec::new();
        if query.is_empty() {
            return out;
        }
        for (i, line) in self.lines.iter().enumerate() {
            let mut search_from = 0;
            while let Some(pos) = line[search_from..].find(query) {
                let match_byte = search_from + pos;
                let char_idx = line[..match_byte].chars().count();
                out.push((i, char_idx, query.chars().count()));
                search_from = (match_byte + query.len()).min(line.len());
                if search_from >= line.len() {
                    break;
                }
            }
        }
        out
    }

    /// Replace `len` chars at (row, col) with `repl`.
    pub fn replace_at(&mut self, row: usize, col: usize, len: usize, repl: &str) {
        if let Some(line) = self.lines.get_mut(row) {
            let s = line
                .char_indices()
                .nth(col)
                .map(|(i, _)| i)
                .unwrap_or(line.len());
            let e = line
                .char_indices()
                .nth(col + len)
                .map(|(i, _)| i)
                .unwrap_or(line.len());
            line.replace_range(s..e, repl);
        }
    }

    /// Build the visual-row layout. Each entry is `(logical_row, char_start,
    /// char_end)`. With `wrap` on, a single logical line may produce several
    /// visual rows; otherwise it is a 1:1 mapping. `text_w` is the available
    /// character width (excluding the gutter).
    pub fn visual_lines(&self, text_w: usize, wrap: bool) -> Vec<(usize, usize, usize)> {
        let mut out = Vec::new();
        for (ri, line) in self.lines.iter().enumerate() {
            let n = line.chars().count();
            if !wrap || text_w == 0 {
                out.push((ri, 0, n));
            } else if n == 0 {
                out.push((ri, 0, 0));
            } else {
                let mut start = 0;
                while start < n {
                    let end = (start + text_w).min(n);
                    out.push((ri, start, end));
                    start = end;
                }
            }
        }
        out
    }

    /// First visual-row index whose logical row equals `logical`, if any.
    pub fn first_visual_of(
        &self,
        visual: &[(usize, usize, usize)],
        logical: usize,
    ) -> Option<usize> {
        visual.iter().position(|&(r, _, _)| r == logical)
    }
}

#[derive(Clone)]
pub struct Editor {
    pub files: Vec<OpenFile>,
    pub active: usize,
    pub mode: EditMode,
    pub last_heartbeat: Instant,
    pub word_wrap: bool,
    /// Goto-line input: `Some(text)` means the input is active.
    pub goto_line_input: Option<String>,
    /// Find-replace state
    pub find_query: Option<String>,
    pub replace_query: Option<String>,
    pub find_active: bool,
    /// When true, the find bar is in "replace" mode (Tab toggles).
    pub find_replace_mode: bool,
    /// Tracks whether the current find_query produced any matches.
    pub find_match_count: usize,
    /// Internal clipboard for cut/copy/paste (text only).
    pub clipboard: String,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            active: 0,
            mode: EditMode::Preview,
            last_heartbeat: Instant::now(),
            word_wrap: false,
            goto_line_input: None,
            find_query: None,
            replace_query: None,
            find_active: false,
            find_replace_mode: false,
            find_match_count: 0,
            clipboard: String::new(),
        }
    }
}

impl Editor {
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn active_file(&self) -> Option<&OpenFile> {
        self.files.get(self.active)
    }

    pub fn active_file_mut(&mut self) -> Option<&mut OpenFile> {
        self.files.get_mut(self.active)
    }

    pub fn open(&mut self, path: PathBuf) {
        if let Some(idx) = self.files.iter().position(|f| f.path == path) {
            self.active = idx;
            return;
        }
        self.files.push(OpenFile::open(path));
        self.active = self.files.len() - 1;
    }

    pub fn new_untitled(&mut self) {
        let mut count = 0;
        for f in &self.files {
            if f.filename() == "untitled" || f.filename().starts_with("untitled-") {
                count += 1;
            }
        }
        let name = if count == 0 {
            "untitled".to_string()
        } else {
            format!("untitled-{}", count)
        };
        let path = PathBuf::from(&name);
        self.files.push(OpenFile::new_buffer(path));
        self.active = self.files.len() - 1;
    }

    /// Open (or focus) the built-in Welcome buffer as the default landing tab.
    pub fn open_welcome(&mut self) {
        if let Some(idx) = self.files.iter().position(|f| f.welcome) {
            self.active = idx;
            return;
        }
        self.files.push(OpenFile::welcome());
        self.active = self.files.len() - 1;
    }

    pub fn close_active(&mut self) {
        if self.files.is_empty() {
            return;
        }
        self.files.remove(self.active);
        if self.active > 0 {
            self.active -= 1;
        }
        // Never leave an empty pane — closing the last tab returns to Welcome.
        if self.files.is_empty() {
            self.open_welcome();
        }
    }

    pub fn close_at(&mut self, idx: usize) {
        if idx < self.files.len() {
            self.files.remove(idx);
            if self.active >= self.files.len() && self.active > 0 {
                self.active -= 1;
            }
            // Never leave an empty pane — closing the last tab returns to Welcome.
            if self.files.is_empty() {
                self.open_welcome();
            }
        }
    }

    /// Copy the active selection into the internal clipboard.
    /// Return the current internal clipboard contents (for mirroring to the
    /// OS clipboard via OSC 52 from the main loop).
    pub fn get_clipboard(&self) -> &str {
        &self.clipboard
    }

    pub fn copy_selection(&mut self) {
        if let Some(f) = self.active_file() {
            if let Some(t) = f.selected_text() {
                self.clipboard = t;
            }
        }
    }

    /// Cut the active selection (copy + delete).
    pub fn cut_selection(&mut self) {
        self.copy_selection();
        if let Some(f) = self.active_file_mut() {
            f.push_undo();
            f.delete_selection();
        }
    }

    /// Paste the internal clipboard at the cursor, preserving newlines.
    pub fn paste(&mut self) {
        let clip = self.clipboard.clone();
        self.insert_clipboard(&clip);
    }

    /// Paste arbitrary text (e.g. from the system clipboard via bracketed
    /// paste or OSC 52) at the cursor, preserving newlines.
    pub fn paste_text(&mut self, text: &str) {
        self.insert_clipboard(text);
    }

    fn insert_clipboard(&mut self, clip: &str) {
        if clip.is_empty() {
            return;
        }
        if let Some(f) = self.active_file_mut() {
            let row = f.cursor_row.min(f.lines.len().saturating_sub(1));
            let col = f.cursor_col.min(f.lines[row].chars().count());
            let cur = std::mem::take(&mut f.lines[row]);
            let cut = cur
                .char_indices()
                .nth(col)
                .map(|(i, _)| i)
                .unwrap_or(cur.len());
            let (left, right) = cur.split_at(cut);
            let parts: Vec<&str> = clip.split('\n').collect();
            let mut new_lines: Vec<String> = Vec::with_capacity(parts.len());
            for (i, p) in parts.iter().enumerate() {
                if parts.len() == 1 {
                    new_lines.push(format!("{}{}{}", left, p, right));
                } else if i == 0 {
                    new_lines.push(format!("{}{}", left, p));
                } else if i == parts.len() - 1 {
                    new_lines.push(format!("{}{}", p, right));
                } else {
                    new_lines.push((*p).to_string());
                }
            }
            f.lines.splice(row..=row, new_lines);
            let last = (row + parts.len() - 1).min(f.lines.len().saturating_sub(1));
            f.cursor_row = last;
            f.cursor_col = f.lines.get(last).map(|l| l.chars().count()).unwrap_or(0);
            f.dirty = true;
            f.clear_selection();
        }
    }

    pub fn save_active(&mut self) {
        if let Some(f) = self.active_file_mut() {
            if !f.is_untitled() {
                f.save();
            }
        }
    }

    pub fn save_as(&mut self, path: PathBuf) {
        if let Some(f) = self.active_file_mut() {
            f.set_path(path);
            f.save();
        }
    }

    /// Replace the next occurrence of find_query with replace_query.
    pub fn replace_next(&mut self) -> bool {
        let q = self.find_query.clone().unwrap_or_default();
        let r = self.replace_query.clone().unwrap_or_default();
        if q.is_empty() {
            return false;
        }
        let found = if let Some(f) = self.active_file() {
            f.find_next(&q, f.cursor_row, f.cursor_col)
        } else {
            None
        };
        if let Some((row, col, len)) = found {
            if let Some(f) = self.active_file_mut() {
                f.push_undo();
                f.replace_at(row, col, len, &r);
                f.cursor_row = row;
                f.cursor_col = col + r.chars().count();
                f.dirty = true;
            }
            true
        } else {
            false
        }
    }

    /// Replace every occurrence of find_query with replace_query.
    pub fn replace_all(&mut self) {
        let q = self.find_query.clone().unwrap_or_default();
        let r = self.replace_query.clone().unwrap_or_default();
        if q.is_empty() {
            return;
        }
        if let Some(f) = self.active_file_mut() {
            f.push_undo();
            for line in f.lines.iter_mut() {
                *line = line.replace(&q, &r);
            }
            f.dirty = true;
        }
    }

    /// Recompute the match count for the current find_query (for live display).
    pub fn refresh_find_count(&mut self) {
        let q = self.find_query.clone().unwrap_or_default();
        if q.is_empty() {
            self.find_match_count = 0;
            return;
        }
        self.find_match_count = if let Some(f) = self.active_file() {
            f.find_matches(&q).len()
        } else {
            0
        };
    }

    /// Jump to the next match of find_query (wrapping), placing the cursor.
    /// Returns the number of matches found (for status display).
    pub fn find_next_match(&mut self) -> usize {
        let q = self.find_query.clone().unwrap_or_default();
        if q.is_empty() {
            return 0;
        }
        let count = if let Some(f) = self.active_file() {
            f.find_matches(&q).len()
        } else {
            0
        };
        self.find_match_count = count;
        if let Some(f) = self.active_file_mut() {
            let from = (f.cursor_row, f.cursor_col + 1);
            if let Some((row, col, _)) = f.find_next(&q, from.0, from.1) {
                f.cursor_row = row;
                f.cursor_col = col;
                f.clear_selection();
            } else if let Some((row, col, _)) = f.find_next(&q, 0, 0) {
                f.cursor_row = row;
                f.cursor_col = col;
                f.clear_selection();
            }
        }
        count
    }

    // ── Neovim-style motions / operators (used when `keybind_mode == Neovim`)
    // ─────────────────────────────────────────────────────────────────────

    /// Move to the start of the next whitespace-delimited word.
    pub fn next_word(&mut self) {
        if let Some(f) = self.active_file_mut() {
            f.clear_selection();
            let line = f.lines.get(f.cursor_row).cloned().unwrap_or_default();
            let chars: Vec<char> = line.chars().collect();
            let mut i = f.cursor_col.min(chars.len());
            while i < chars.len() && !chars[i].is_whitespace() {
                i += 1;
            }
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            f.cursor_col = i;
        }
    }

    /// Move to the start of the previous whitespace-delimited word.
    pub fn prev_word(&mut self) {
        if let Some(f) = self.active_file_mut() {
            f.clear_selection();
            let line = f.lines.get(f.cursor_row).cloned().unwrap_or_default();
            let chars: Vec<char> = line.chars().collect();
            let mut i = f.cursor_col;
            i = i.saturating_sub(1);
            while i > 0 && chars[i].is_whitespace() {
                i -= 1;
            }
            while i > 0 && !chars[i].is_whitespace() {
                i -= 1;
            }
            f.cursor_col = i;
        }
    }

    /// Move to the end of the current word.
    pub fn end_of_word(&mut self) {
        if let Some(f) = self.active_file_mut() {
            f.clear_selection();
            let line = f.lines.get(f.cursor_row).cloned().unwrap_or_default();
            let chars: Vec<char> = line.chars().collect();
            let mut i = f.cursor_col.min(chars.len());
            while i < chars.len() && !chars[i].is_whitespace() {
                i += 1;
            }
            i = i.saturating_sub(1);
            f.cursor_col = i;
        }
    }

    pub fn cursor_to_line_start(&mut self) {
        if let Some(f) = self.active_file_mut() {
            f.clear_selection();
            f.cursor_col = 0;
        }
    }

    pub fn cursor_to_line_end(&mut self) {
        if let Some(f) = self.active_file_mut() {
            f.clear_selection();
            let r = f.cursor_row.min(f.lines.len().saturating_sub(1));
            f.cursor_col = f.lines.get(r).map(|l| l.chars().count()).unwrap_or(0);
        }
    }

    pub fn goto_first_line(&mut self) {
        if let Some(f) = self.active_file_mut() {
            f.clear_selection();
            f.cursor_row = 0;
            f.cursor_col = 0;
        }
    }

    pub fn goto_last_line(&mut self) {
        if let Some(f) = self.active_file_mut() {
            f.clear_selection();
            let r = f.lines.len().saturating_sub(1);
            f.cursor_row = r;
            f.cursor_col = f.lines.get(r).map(|l| l.chars().count()).unwrap_or(0);
        }
    }

    /// `x` — delete the character under the cursor.
    pub fn delete_char_at_cursor(&mut self) {
        if let Some(f) = self.active_file_mut() {
            if f.lines.is_empty() {
                return;
            }
            let r = f.cursor_row.min(f.lines.len().saturating_sub(1));
            let len = f.lines.get(r).map(|l| l.chars().count()).unwrap_or(0);
            if f.cursor_col < len {
                f.push_undo();
                if let Some(line) = f.lines.get_mut(r) {
                    let byte = line
                        .char_indices()
                        .nth(f.cursor_col)
                        .map(|(i, _)| i)
                        .unwrap_or(line.len());
                    line.remove(byte);
                }
                f.dirty = true;
            } else if r + 1 < f.lines.len() {
                f.push_undo();
                let next = f.lines.remove(r + 1);
                if let Some(line) = f.lines.get_mut(r) {
                    line.push_str(&next);
                }
                f.dirty = true;
            }
        }
    }

    /// `dd` — delete the current line (joining an empty buffer to one line).
    pub fn delete_current_line(&mut self) {
        if let Some(f) = self.active_file_mut() {
            if f.lines.is_empty() {
                return;
            }
            f.push_undo();
            let r = f.cursor_row.min(f.lines.len().saturating_sub(1));
            f.lines.remove(r);
            if f.lines.is_empty() {
                f.lines.push(String::new());
            }
            f.cursor_row = r.min(f.lines.len().saturating_sub(1));
            f.cursor_col = 0;
            f.dirty = true;
        }
    }

    /// `yy` — yank (copy) the current line into the clipboard.
    pub fn yank_current_line(&mut self) {
        if let Some(f) = self.active_file() {
            let r = f.cursor_row.min(f.lines.len().saturating_sub(1));
            self.clipboard = f.lines.get(r).cloned().unwrap_or_default();
        }
    }

    /// `o` / `O` — open a new blank line below / above the cursor.
    pub fn open_below(&mut self) {
        if let Some(f) = self.active_file_mut() {
            f.push_undo();
            let r = (f.cursor_row + 1).min(f.lines.len());
            f.lines.insert(r, String::new());
            f.cursor_row = r;
            f.cursor_col = 0;
            f.dirty = true;
        }
    }

    pub fn open_above(&mut self) {
        if let Some(f) = self.active_file_mut() {
            f.push_undo();
            let r = f.cursor_row.min(f.lines.len());
            f.lines.insert(r, String::new());
            f.cursor_row = r;
            f.cursor_col = 0;
            f.dirty = true;
        }
    }

    /// `v` — toggle a visual selection anchored at the cursor.
    pub fn toggle_visual(&mut self) {
        if let Some(f) = self.active_file_mut() {
            if f.selection.is_some() {
                f.clear_selection();
            } else {
                f.selection = Some((f.cursor_row, f.cursor_col));
            }
        }
    }

    /// Outline symbols for the active file (function/class/variable), by line.
    pub fn outline(&self) -> Vec<OutlineItem> {
        let mut items = Vec::new();
        if let Some(f) = self.active_file() {
            for (i, line) in f.lines.iter().enumerate() {
                let trimmed = line.trim_start();
                if let Some(rest) = trimmed.strip_prefix("def ") {
                    let name = rest
                        .split(['(', ':'])
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if !name.is_empty() {
                        items.push(OutlineItem {
                            line: i,
                            kind: SymbolKind::Function,
                            name,
                        });
                    }
                } else if let Some(rest) = trimmed.strip_prefix("class ") {
                    let name = rest
                        .split(['(', ':'])
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if !name.is_empty() {
                        items.push(OutlineItem {
                            line: i,
                            kind: SymbolKind::Class,
                            name,
                        });
                    }
                } else if trimmed.starts_with("fn ")
                    || trimmed.starts_with("pub fn ")
                    || trimmed.starts_with("async fn ")
                {
                    let rest = trimmed
                        .trim_start_matches("pub ")
                        .trim_start_matches("async ")
                        .trim_start_matches("fn ");
                    let name = rest
                        .split(['(', '<', ':'])
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if !name.is_empty() {
                        items.push(OutlineItem {
                            line: i,
                            kind: SymbolKind::Function,
                            name,
                        });
                    }
                } else if let Some(rest) = trimmed.strip_prefix("struct ") {
                    let name = rest
                        .split(['{', '<', '('])
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if !name.is_empty() {
                        items.push(OutlineItem {
                            line: i,
                            kind: SymbolKind::Class,
                            name,
                        });
                    }
                } else if let Some(rest) = trimmed.strip_prefix("pub const ") {
                    let name = rest
                        .split([':', ' '])
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if !name.is_empty() {
                        items.push(OutlineItem {
                            line: i,
                            kind: SymbolKind::Variable,
                            name,
                        });
                    }
                } else if let Some(rest) = trimmed.strip_prefix("const ") {
                    let name = rest
                        .split([':', ' '])
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if !name.is_empty() {
                        items.push(OutlineItem {
                            line: i,
                            kind: SymbolKind::Variable,
                            name,
                        });
                    }
                } else if let Some(rest) = trimmed.strip_prefix("let ") {
                    let name = rest
                        .split(['=', ':', ' '])
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if !name.is_empty() && !name.contains('(') {
                        items.push(OutlineItem {
                            line: i,
                            kind: SymbolKind::Variable,
                            name,
                        });
                    }
                }
            }
        }
        items
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Class,
    Variable,
}

impl SymbolKind {
    pub fn icon(&self) -> &'static str {
        match self {
            SymbolKind::Function => "ƒ",
            SymbolKind::Class => "◉",
            SymbolKind::Variable => "◇",
        }
    }
}

pub struct OutlineItem {
    pub line: usize,
    pub kind: SymbolKind,
    pub name: String,
}

#[allow(dead_code)]
fn _is_path(p: &Path) -> bool {
    p.exists()
}

/// Seconds since last heartbeat, used for live-dot staleness.
pub fn heartbeat_age(hb: Instant) -> Duration {
    hb.elapsed()
}

/// Content of the default "Welcome" buffer — a fast, Rust-built ROS 2 dashboard.
fn welcome_text() -> &'static str {
    "\
╔══════════════════════════════════════════════════════════════════════════╗
║                  Welcome to ROS2_INFO  —  TUI                            ║
║            A fast, Rust-built ROS 2 developer dashboard                  ║
╚══════════════════════════════════════════════════════════════════════════╝

  Crafted by cc@zang aka Gaurav-x111

  This is a code editor at heart — open any file from the EXPLORER on the
  left (or press  Ctrl+P  to jump to a file). Click a file once to open it;
  click the explorer icon again to hide the sidebar. Press it once more to
  bring it back. Nothing here will close the app by accident.

  QUICK START
  ────────────────────────────────────────────────────────────────────────
    • Explorer icon (left rail)   → show / hide your files (toggle)
    • Terminal (bottom)           → a real shell; type  ros2 ...  directly
    • Top tabs                    → Overview · ROS2 · Workspace · Diagnostics
                                    Trends · Fleet
    • Ctrl+Shift+P                → Command Palette (ROS2 tools live here)
    • Ctrl+`                      → show / hide the terminal
    • Ctrl+Q                      → quit, from anywhere

  ROS 2 TOOLS (Command Palette → type \"ros\")
  ────────────────────────────────────────────────────────────────────────
    ros2 node list · topic list · topic echo · topic hz · doctor
    service / action / param list · bag record · launch · run

  TIP  Press  i  to edit,  Esc  to leave a panel,  ?  or F1  for all keys.

  Built in Rust with ratatui · zero-lag live telemetry · sandbox-safe.
"
}
