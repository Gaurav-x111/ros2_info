//! AI assistant module for the TUI.
//! Handles `ai auto`, `ai`, `ai scan`, `ai fix`, etc.
//! Uses Ollama as the backend (localhost:11434).

use std::fs;
use std::io::Write;
use std::process::Command as StdCommand;
use std::sync::mpsc;

/// Default Ollama model used when the user hasn't explicitly chosen one.
pub const DEFAULT_MODEL: &str = "qwen2.5-coder:7b";

/// Base URL of the local Ollama API.
///
/// Defaults to `http://127.0.0.1:11434` — note `127.0.0.1` (IPv4), *not*
/// `localhost`: Ollama frequently binds only to IPv4, and `localhost` can
/// resolve to IPv6 `::1` first, causing curl to fail to connect even though a
/// browser (which tries `127.0.0.1`) reaches the server fine. Override with the
/// `OLLAMA_HOST` env var (e.g. `http://127.0.0.1:11434` or a remote host).
pub fn ollama_base() -> String {
    std::env::var("OLLAMA_HOST")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:11434".to_string())
}

/// List installed Ollama models by querying the local API.
/// Returns `(model_names, error_message)`.
pub fn list_ollama_models() -> (Vec<String>, Option<String>) {
    let base = ollama_base();
    let tags_url = format!("{}/api/tags", base);
    let (stdout, stderr, success) = run_cmd(
        "curl",
        &[
            "-s",
            tags_url.as_str(),
            "--connect-timeout",
            "10",
            "--max-time",
            "15",
        ],
    );
    if !success {
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "curl exited non-zero with no output".to_string()
        };
        return (
            Vec::new(),
            Some(format!(
                "Cannot reach Ollama at {} ({}). Start it with: ollama serve",
                ollama_base(),
                detail
            )),
        );
    }
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&stdout) {
        let models: Vec<String> = val["models"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        if models.is_empty() {
            return (
                Vec::new(),
                Some(format!(
                    "No models installed. Install one with:\n  ollama pull {}",
                    DEFAULT_MODEL
                )),
            );
        }
        (models, None)
    } else {
        (Vec::new(), Some("Failed to parse Ollama response.".into()))
    }
}

/// AI command types parsed from terminal input.
#[derive(Debug)]
pub enum AiCommand {
    /// Full autonomous mode: scan → fix → rebuild → loop → report.
    Auto,
    /// Interactive mode: scan → show issues → user decides.
    Interactive,
    /// Just scan for errors, don't fix.
    Scan,
    /// Fix a specific file.
    Fix(String),
    /// Explain an error message.
    Explain(String),
    /// Chat with AI about the codebase.
    Chat(String),
    /// Start web chat server on given port.
    WebChat(u16),
    /// Stop the running web chat server.
    WebChatStop,
    /// List available Ollama models.
    ModelList,
    /// Set the active model by name.
    ModelSet(String),
    /// Show available commands.
    Help,
    /// Unknown command.
    Unknown(String),
}

impl AiCommand {
    pub fn parse(input: &str) -> Self {
        let input = input.trim();
        if input == "ai auto" || input == "ai auto --full" || input == "ai solve" {
            AiCommand::Auto
        } else if input == "ai" || input == "ai --interactive" {
            AiCommand::Interactive
        } else if input == "ai scan" {
            AiCommand::Scan
        } else if let Some(rest) = input.strip_prefix("ai fix ") {
            AiCommand::Fix(rest.trim().to_string())
        } else if let Some(rest) = input.strip_prefix("ai explain ") {
            AiCommand::Explain(rest.trim().to_string())
        } else if let Some(rest) = input.strip_prefix("ai chat ") {
            AiCommand::Chat(rest.trim().to_string())
        } else if let Some(rest) = input.strip_prefix("ai web ") {
            let port: u16 = rest.trim().parse().unwrap_or(8899);
            AiCommand::WebChat(port)
        } else if let Some(rest) = input.strip_prefix("ai webchat ") {
            let port: u16 = rest.trim().parse().unwrap_or(8899);
            AiCommand::WebChat(port)
        } else if input == "ai web" || input == "ai webchat" {
            AiCommand::WebChat(8899)
        } else if input == "ai web stop" || input == "ai webchat stop" {
            AiCommand::WebChatStop
        } else if input == "ai model" || input == "ai model list" {
            AiCommand::ModelList
        } else if let Some(rest) = input.strip_prefix("ai model ") {
            AiCommand::ModelSet(rest.trim().to_string())
        } else if input == "ai help" || input == "ai --help" {
            AiCommand::Help
        } else {
            AiCommand::Unknown(input.to_string())
        }
    }

    pub fn is_ai_command(input: &str) -> bool {
        let t = input.trim();
        t == "ai" || t.starts_with("ai ")
    }
}

/// Run a shell command and capture output.
pub fn run_cmd(cmd: &str, args: &[&str]) -> (String, String, bool) {
    match StdCommand::new(cmd).args(args).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let success = output.status.success();
            (stdout, stderr, success)
        }
        Err(e) => (
            String::new(),
            format!("Failed to run {}: {}", cmd, e),
            false,
        ),
    }
}

/// Run a command and merge stdout+stderr. cargo emits compile diagnostics to
/// stderr; some notices go to stdout — merging is the robust choice.
pub fn run_cmd_merged(cmd: &str, args: &[&str]) -> (String, bool) {
    let (stdout, stderr, success) = run_cmd(cmd, args);
    let mut merged = stdout;
    if !stderr.is_empty() {
        if !merged.is_empty() {
            merged.push('\n');
        }
        merged.push_str(&stderr);
    }
    (merged, success)
}

/// Scan for build errors (cargo build).
pub fn scan_build_errors() -> Vec<String> {
    let (merged, success) = run_cmd_merged("cargo", &["build"]);
    if success {
        return Vec::new();
    }
    merged
        .lines()
        .filter(|l| l.contains("error["))
        .map(|l| l.trim().to_string())
        .collect()
}

/// Scan for clippy warnings.
#[allow(dead_code)]
pub fn scan_clippy_warnings() -> Vec<String> {
    let (merged, _) = run_cmd_merged("cargo", &["clippy", "--quiet"]);
    merged
        .lines()
        .filter(|l| l.contains("warning:") && !l.contains("generated"))
        .map(|l| l.trim().to_string())
        .collect()
}

/// Get the current file contents (for context).
#[allow(dead_code)]
pub fn get_file_contents(path: &str) -> Option<String> {
    fs::read_to_string(path).ok()
}

/// Build a prompt for the AI with context.
pub fn build_prompt(
    errors: &[String],
    file_path: Option<&str>,
    file_content: Option<&str>,
) -> String {
    let mut prompt = String::new();
    prompt.push_str(
        "You are a Rust/ROS2 expert. Analyze the following errors and provide fixes.\n\n",
    );

    if let Some(path) = file_path {
        prompt.push_str(&format!("=== Current file: {} ===\n", path));
        if let Some(content) = file_content {
            // Truncate to 200 lines max
            let lines: Vec<&str> = content.lines().take(200).collect();
            prompt.push_str(&lines.join("\n"));
            prompt.push('\n');
        }
        prompt.push('\n');
    }

    prompt.push_str("=== Build Errors ===\n");
    if errors.is_empty() {
        prompt.push_str("No errors found.\n");
    } else {
        for e in errors {
            prompt.push_str(e);
            prompt.push('\n');
        }
    }

    prompt.push_str("\n=== Instructions ===\n");
    prompt.push_str("For each error, provide:\n");
    prompt.push_str("1. The file and line number\n");
    prompt.push_str("2. What's wrong\n");
    prompt.push_str("3. The exact fix (show the changed code)\n");
    prompt.push_str("Use this format:\n");
    prompt.push_str("FILE: path/to/file.rs\n");
    prompt.push_str("LINE: 42\n");
    prompt.push_str("FIX: the replacement code\n");
    prompt.push_str("---\n");

    prompt
}

/// Resolve a usable Ollama model.
///
/// Returns `preferred` if it is installed; otherwise falls back to the first
/// available model (preferring a "coder" variant). Returns a helpful error if
/// Ollama is unreachable or has no models installed.
pub fn resolve_ollama_model(preferred: &str) -> Result<String, String> {
    let base = ollama_base();
    let tags_url = format!("{}/api/tags", base);
    let (stdout, stderr, success) = run_cmd(
        "curl",
        &[
            "-s",
            tags_url.as_str(),
            "--connect-timeout",
            "10",
            "--max-time",
            "15",
        ],
    );
    if !success {
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "curl exited non-zero with no output".to_string()
        };
        return Err(format!(
            "Cannot reach Ollama at {} ({}). Start it with: ollama serve",
            base, detail
        ));
    }
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&stdout) {
        let models: Vec<String> = val["models"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        if models.is_empty() {
            return Err(format!(
                "Ollama has no models installed. Install one with:\n  ollama pull {}",
                preferred
            ));
        }
        if models.iter().any(|m| m == preferred) {
            return Ok(preferred.to_string());
        }
        // Prefer a coder model, otherwise fall back to the first available one.
        for m in &models {
            if m.contains("coder") {
                return Ok(m.clone());
            }
        }
        return Ok(models[0].clone());
    }
    Ok(preferred.to_string())
}

/// Call Ollama API for chat completion.
pub fn call_ollama(prompt: &str, model: &str) -> Result<String, String> {
    let resolved = resolve_ollama_model(model)?;
    let base = ollama_base();
    let endpoint = format!("{}/api/chat", base);
    let payload = serde_json::json!({
        "model": resolved,
        "messages": [{"role": "user", "content": prompt}],
        "stream": false
    });

    let (stdout, stderr, success) = run_cmd(
        "curl",
        &[
            "-s",
            "-X",
            "POST",
            endpoint.as_str(),
            "-H",
            "Content-Type: application/json",
            "-d",
            &payload.to_string(),
            "--connect-timeout",
            "10",
            "--max-time",
            "180",
        ],
    );

    if !success {
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "curl exited non-zero with no output".to_string()
        };
        return Err(format!(
            "Ollama request failed: {} (endpoint {}). Is Ollama running and reachable?",
            detail, endpoint
        ));
    }

    // Parse response
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&stdout) {
        // Ollama returns {"error": "..."} on failure (e.g. model missing).
        if let Some(err) = val["error"].as_str() {
            if err.contains("not found") {
                return Err(format!(
                    "Ollama model '{}' not found. Install it with:\n  ollama pull {}\n\
                     (or run 'ollama list' to see installed models)",
                    resolved, resolved
                ));
            }
            return Err(format!("Ollama error: {}", err));
        }
        if let Some(content) = val["message"]["content"].as_str() {
            return Ok(content.to_string());
        }
    }
    Err(format!("Failed to parse Ollama response: {}", stdout))
}

/// Parse AI response to extract fixes.
pub fn parse_fixes(response: &str) -> Vec<Fix> {
    let mut fixes = Vec::new();
    let blocks: Vec<&str> = response.split("---").collect();

    for block in blocks {
        let mut file = String::new();
        let mut line = 0;
        let mut fix_code = String::new();
        let mut in_fix = false;

        for l in block.lines() {
            if let Some(f) = l.strip_prefix("FILE: ") {
                file = f.trim().to_string();
            } else if let Some(ln) = l.strip_prefix("LINE: ") {
                line = ln.trim().parse().unwrap_or(0);
            } else if let Some(rest) = l.strip_prefix("FIX: ") {
                fix_code = rest.trim().to_string();
                in_fix = true;
            } else if in_fix && !l.trim().is_empty() && !l.starts_with("FILE:") {
                fix_code.push('\n');
                fix_code.push_str(l);
            }
        }

        if !file.is_empty() && line > 0 && !fix_code.is_empty() {
            fixes.push(Fix {
                file,
                line,
                code: fix_code,
            });
        }
    }
    fixes
}

/// A parsed code fix.
#[derive(Debug, Clone)]
pub struct Fix {
    pub file: String,
    pub line: usize,
    pub code: String,
}

/// Apply a fix to a file: replace the 1-indexed `line` with `code`.
///
/// `code` may itself span multiple physical lines, so it is split and spliced
/// in rather than crammed into a single element. The original file's trailing
/// newline is preserved -- the old `lines.join("\n")` path silently dropped a
/// final newline, leaving the file without an EOF terminator.
pub fn apply_fix(fix: &Fix) -> Result<(), String> {
    let content =
        fs::read_to_string(&fix.file).map_err(|e| format!("Failed to read {}: {}", fix.file, e))?;
    let ends_with_newline = content.ends_with('\n');
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

    if fix.line == 0 || fix.line > lines.len() {
        return Err(format!("Line {} out of range for {}", fix.line, fix.file));
    }

    let replacement: Vec<String> = fix.code.split('\n').map(|s| s.to_string()).collect();
    lines.splice(fix.line - 1..fix.line, replacement);

    let mut out = lines.join("\n");
    if ends_with_newline {
        out.push('\n');
    }

    let mut file =
        fs::File::create(&fix.file).map_err(|e| format!("Failed to write {}: {}", fix.file, e))?;
    file.write_all(out.as_bytes())
        .map_err(|e| format!("Failed to write {}: {}", fix.file, e))?;

    Ok(())
}

/// Generate a report of what the AI did.
pub fn generate_report(attempts: &[Attempt]) -> String {
    let mut report = String::new();
    report.push_str("# Autonomous Fix Report\n\n");
    report.push_str(&format!(
        "Generated: {}\n\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));

    for (i, attempt) in attempts.iter().enumerate() {
        report.push_str(&format!("## Attempt {}\n\n", i + 1));
        report.push_str(&format!("**Errors found:** {}\n", attempt.errors.len()));
        report.push_str(&format!("**Fixes applied:** {}\n", attempt.fixes.len()));
        report.push_str(&format!(
            "**Build succeeded:** {}\n\n",
            attempt.build_passed
        ));

        if !attempt.errors.is_empty() {
            report.push_str("### Errors\n");
            for e in &attempt.errors {
                report.push_str(&format!("- {}\n", e));
            }
            report.push('\n');
        }

        if !attempt.fixes.is_empty() {
            report.push_str("### Fixes Applied\n");
            for f in &attempt.fixes {
                report.push_str(&format!(
                    "- **{}:{}** — replaced line with `{}`\n",
                    f.file, f.line, f.code
                ));
            }
            report.push('\n');
        }

        if let Some(ref ai_response) = attempt.ai_response {
            report.push_str("### AI Response\n");
            report.push_str(ai_response);
            report.push_str("\n\n");
        }
    }

    report
}

/// Record of a single fix attempt.
#[derive(Debug, Clone)]
pub struct Attempt {
    pub errors: Vec<String>,
    pub fixes: Vec<Fix>,
    pub build_passed: bool,
    pub ai_response: Option<String>,
}

/// Run the autonomous mode. Returns the final report and the session index.
pub fn run_autonomous(_terminal_idx: usize, tx: mpsc::Sender<AiEvent>, model: String) -> String {
    // Run the real loop in a guard so any unexpected panic is reported to the
    // UI as an error (instead of silently killing the worker thread and leaving
    // the AI session frozen with no "complete" message).
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_autonomous_inner(&tx, &model)
    }));

    match result {
        Ok(report) => report,
        Err(_) => {
            let _ = tx.send(AiEvent::Status(
                "💥 Autonomous mode panicked. See AUTONOMOUS_REPORT.md / logs.".to_string(),
            ));
            let _ = tx.send(AiEvent::Done);
            String::new()
        }
    }
}

fn run_autonomous_inner(tx: &mpsc::Sender<AiEvent>, model: &str) -> String {
    let mut attempts: Vec<Attempt> = Vec::new();
    let max_attempts = 3;

    for attempt_num in 0..max_attempts {
        let _ = tx.send(AiEvent::Status(format!(
            "🤖 Attempt {}/{}: Scanning...",
            attempt_num + 1,
            max_attempts
        )));

        // Scan for errors
        let errors = scan_build_errors();
        if errors.is_empty() {
            let _ = tx.send(AiEvent::Status(
                "✅ No build errors found. Done!".to_string(),
            ));
            break;
        }

        let _ = tx.send(AiEvent::Status(format!(
            "🤖 Found {} errors. Analyzing...",
            errors.len()
        )));

        // Build context
        let prompt = build_prompt(&errors, None, None);

        // Call AI
        let ai_response = match call_ollama(&prompt, model) {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(AiEvent::Status(format!("❌ AI error: {}", e)));
                break;
            }
        };

        let _ = tx.send(AiEvent::Output(format!(
            "\n🤖 AI Response:\n{}\n",
            ai_response
        )));

        // Parse fixes
        let fixes = parse_fixes(&ai_response);
        if fixes.is_empty() {
            let _ = tx.send(AiEvent::Status(
                "🤖 No actionable fixes found. Done.".to_string(),
            ));
            break;
        }

        // Apply fixes
        let mut applied = Vec::new();
        for fix in &fixes {
            match apply_fix(fix) {
                Ok(()) => {
                    let _ = tx.send(AiEvent::Output(format!(
                        "✅ Applied: {}:{}\n",
                        fix.file, fix.line
                    )));
                    applied.push(fix.clone());
                }
                Err(e) => {
                    let _ = tx.send(AiEvent::Output(format!("❌ Failed to apply: {}\n", e)));
                }
            }
        }

        // Rebuild
        let _ = tx.send(AiEvent::Status("🤖 Rebuilding...".to_string()));
        let (_merged, success) = run_cmd_merged("cargo", &["build"]);
        let build_passed = success;

        attempts.push(Attempt {
            errors,
            fixes: applied,
            build_passed,
            ai_response: Some(ai_response),
        });

        if build_passed {
            let _ = tx.send(AiEvent::Status("✅ Build passes!".to_string()));
            break;
        } else {
            let _ = tx.send(AiEvent::Status(format!(
                "❌ Build still fails. {}/{} attempts.",
                attempt_num + 1,
                max_attempts
            )));
        }
    }

    // Generate report
    let report = generate_report(&attempts);

    // Write report to file
    let report_path = "AUTONOMOUS_REPORT.md";
    if let Ok(mut f) = fs::File::create(report_path) {
        let _ = f.write_all(report.as_bytes());
    }

    let _ = tx.send(AiEvent::Status(format!(
        "📄 Report saved to {}",
        report_path
    )));
    let _ = tx.send(AiEvent::Done);

    report
}

/// Events sent from AI operations back to the terminal.
#[derive(Debug)]
pub enum AiEvent {
    Status(String),
    Output(String),
    Done,
}
