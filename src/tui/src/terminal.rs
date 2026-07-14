#![allow(dead_code)]
//! Integrated terminal via `portable-pty` + `vt100`.
//!
//! Each terminal tab owns an independent PTY session. Bytes read from the
//! master are forwarded to the app over an `mpsc` channel and fed into a
//! `vt100::Parser` on the main thread (parsers are not `Send`).

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use ratatui::style::Color;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;
use vt100::Parser;

pub struct PtySession {
    pub name: String,
    pub parser: Parser,
    pub writer: Box<dyn Write + Send>,
    /// `None` for AI/output-only terminals that own no PTY.
    #[allow(dead_code)]
    master: Option<Box<dyn MasterPty + Send>>,
    #[allow(dead_code)]
    child: Option<Box<dyn portable_pty::Child + Send + Sync>>,
    pub rx: mpsc::Receiver<Vec<u8>>,
    pub rows: u16,
    pub cols: u16,
    /// Scrollback offset: 0 = bottom (live), >0 = scrolled up.
    pub scrollback: usize,
    /// True if this is an AI terminal (commands intercepted).
    pub is_ai: bool,
    /// AI command history for this session.
    pub ai_history: Vec<String>,
}

impl PtySession {
    /// Build an output-only session (AI terminals): no PTY, no child.
    fn new_output_only(
        name: String,
        parser: Parser,
        writer: Box<dyn Write + Send>,
        rx: mpsc::Receiver<Vec<u8>>,
        rows: u16,
        cols: u16,
        is_ai: bool,
    ) -> Self {
        Self {
            name,
            parser,
            writer,
            master: None,
            child: None,
            rx,
            rows,
            cols,
            scrollback: 0,
            is_ai,
            ai_history: Vec::new(),
        }
    }
}

pub struct TerminalManager {
    pub sessions: Vec<PtySession>,
    pub active: usize,
    next_id: usize,
    pub sandbox_label: String,
    /// Shared command history across all terminal sessions.
    pub cmd_history: Vec<String>,
    /// Index into `cmd_history` while recalling (None = live/editing).
    pub cmd_hist_idx: Option<usize>,
}

impl TerminalManager {
    pub fn new(rows: u16, cols: u16) -> Self {
        let mut mgr = Self {
            sessions: Vec::new(),
            active: 0,
            next_id: 1,
            sandbox_label: "sandbox".to_string(),
            cmd_history: Self::load_history(),
            cmd_hist_idx: None,
        };
        // ponytail: guard the auto-spawn so we can test input without the
        // integrated shell fighting for stdin.
        if std::env::var("ROS2_INFO_NO_TERM").is_err() {
            mgr.new_session(rows, cols);
        }
        mgr
    }

    pub fn set_sandbox_context(&mut self, label: String) {
        self.sandbox_label = label.clone();
        if let Some(s) = self.sessions.get_mut(self.active) {
            let prompt = format!("export PS1='ros2_ws ({}) → '\n", label);
            let _ = s.writer.write_all(prompt.as_bytes());
            let _ = s.writer.flush();
        }
    }

    pub fn new_session(&mut self, rows: u16, cols: u16) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        let name = format!("{}: bash", id);

        let pty_system = native_pty_system();
        let pair = match pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        }) {
            Ok(p) => p,
            Err(_) => return self.active,
        };

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        let mut cmd = CommandBuilder::new(shell);
        cmd.env("TERM", "xterm-256color");
        cmd.env("ROS2_INFO_TUI", "1");

        let child = match pair.slave.spawn_command(cmd) {
            Ok(c) => c,
            Err(_) => return self.active,
        };

        let mut writer = match pair.master.take_writer() {
            Ok(w) => w,
            Err(_) => return self.active,
        };

        let reader = match pair.master.try_clone_reader() {
            Ok(r) => r,
            Err(_) => return self.active,
        };

        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let mut reader = reader;
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let parser = Parser::new(rows, cols, 1024); // 1024-line scrollback

        // Set a context-aware prompt.
        let prompt = format!("export PS1='ros2_ws ({}) → '\n", self.sandbox_label);
        let _ = writer.write_all(prompt.as_bytes());
        let _ = writer.flush();

        let session = PtySession {
            name,
            parser,
            writer,
            master: Some(pair.master),
            child: Some(child),
            rx,
            rows,
            cols,
            scrollback: 0,
            is_ai: false,
            ai_history: Vec::new(),
        };
        self.sessions.push(session);
        let idx = self.sessions.len() - 1;
        self.active = idx;
        idx
    }

    pub fn active_session(&self) -> Option<&PtySession> {
        self.sessions.get(self.active)
    }

    pub fn active_session_mut(&mut self) -> Option<&mut PtySession> {
        self.sessions.get_mut(self.active)
    }

    pub fn write_input(&mut self, bytes: &[u8]) {
        if let Some(s) = self.sessions.get_mut(self.active) {
            let _ = s.writer.write_all(bytes);
            let _ = s.writer.flush();
        }
    }

    pub fn close_active(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let idx = self.active;
        if let Some(s) = self.sessions.get_mut(idx) {
            let _ = s.writer.write_all(b"exit\n");
        }
        self.sessions.remove(idx);
        if self.active >= self.sessions.len() && self.active > 0 {
            self.active -= 1;
        }
        if self.sessions.is_empty() {
            // Always keep at least one terminal.
            let (rows, cols) = self
                .sessions
                .first()
                .map(|s| (s.rows, s.cols))
                .unwrap_or((24, 80));
            self.new_session(rows, cols);
        }
    }

    pub fn switch_to(&mut self, idx: usize) {
        if idx < self.sessions.len() {
            self.active = idx;
        }
    }

    pub fn close_at(&mut self, idx: usize) {
        if idx >= self.sessions.len() || self.sessions.len() <= 1 {
            return;
        }
        self.sessions.remove(idx);
        if self.active >= self.sessions.len() && self.active > 0 {
            self.active -= 1;
        }
    }

    pub fn new_ai_terminal(&mut self, name: &str) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        let rows = 24;
        let cols = 80;
        let parser = Parser::new(rows, cols, 1024);
        let (_tx, rx) = mpsc::channel::<Vec<u8>>();
        // ponytail: AI terminals are pure output buffers — no PTY, no child
        // process. The old code spawned `true` via openpty().unwrap() which
        // panicked if a pty pair couldn't be opened, and leaked a dead task.
        // A sink writer + dropped sender is all write_ai_output needs.
        let dummy_writer: Box<dyn Write + Send> = Box::new(std::io::sink());
        let session = PtySession::new_output_only(
            format!("{}: {}", id, name),
            parser,
            dummy_writer,
            rx,
            rows,
            cols,
            true,
        );
        self.sessions.push(session);
        let idx = self.sessions.len() - 1;
        self.active = idx;
        idx
    }

    pub fn write_ai_output(&mut self, idx: usize, text: &str) {
        if let Some(s) = self.sessions.get_mut(idx) {
            s.parser.process(text.as_bytes());
        }
    }

    pub fn clear_ai_session(&mut self, idx: usize) {
        if let Some(s) = self.sessions.get_mut(idx) {
            // Reset the parser to clear screen
            s.parser = Parser::new(s.rows, s.cols, 1024);
            // Send clear screen escape sequence
            s.parser.process(b"\x1b[2J\x1b[H");
        }
    }

    pub fn write_ai_session_idx(&mut self, idx: usize, text: &str) {
        if let Some(s) = self.sessions.get_mut(idx) {
            s.parser.process(text.as_bytes());
        }
    }

    pub fn resize_active(&mut self, rows: u16, cols: u16) {
        if let Some(s) = self.sessions.get_mut(self.active) {
            s.rows = rows;
            s.cols = cols;
            if let Some(master) = s.master.as_mut() {
                let _ = master.resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                });
            }
            // ponytail: set_size preserves existing screen content; recreating
            // the parser would wipe the visible buffer on every resize.
            s.parser.set_size(rows, cols);
        }
    }
    /// Drain pending bytes from the active session's channel into the parser.
    /// Returns true if any bytes were processed.
    pub fn pump(&mut self) -> bool {
        let mut active_drained = false;
        // Drain EVERY session, not just the active one. Inactive terminals
        // (e.g. one running a continuous `ros2` collection) keep producing
        // output; if their `mpsc` channel is never drained it grows
        // unboundedly and the process leaks memory (observed ~6 GB).
        for idx in 0..self.sessions.len() {
            let mut drained = false;
            if let Some(s) = self.sessions.get_mut(idx) {
                while let Ok(bytes) = s.rx.recv_timeout(Duration::from_millis(0)) {
                    s.parser.process(&bytes);
                    drained = true;
                }
            }
            if idx == self.active {
                active_drained = drained;
                if drained {
                    self.sessions[idx].scrollback = 0; // reset scrollback on new output
                }
            }
        }
        active_drained
    }

    /// Politely ask every running PTY child to exit so we don't leave orphaned
    /// shells/zombies behind when the TUI closes.
    pub fn shutdown(&mut self) {
        for s in self.sessions.iter_mut() {
            let _ = s.writer.write_all(b"exit\n");
            let _ = s.writer.flush();
        }
        self.sessions.clear();
    }

    fn history_path() -> Option<PathBuf> {
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join(".ros2_info").join("term_history"))
    }

    fn load_history() -> Vec<String> {
        let Some(p) = Self::history_path() else {
            return Vec::new();
        };
        std::fs::read_to_string(&p)
            .map(|s| {
                s.lines()
                    .map(|l| l.to_string())
                    .filter(|l| !l.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Record a command into history (dedup consecutive) and persist it.
    pub fn push_cmd_history(&mut self, cmd: &str) {
        let cmd = cmd.trim();
        if cmd.is_empty() {
            return;
        }
        if self.cmd_history.last().map(|s| s.as_str()) != Some(cmd) {
            self.cmd_history.push(cmd.to_string());
        }
        self.cmd_hist_idx = None;
        if let Some(p) = Self::history_path() {
            if let Some(parent) = p.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&p)
                .map(|mut f| {
                    use std::io::Write;
                    let _ = writeln!(f, "{}", cmd);
                });
        }
    }

    /// Recall the previous history entry (like Up). `current` is the line
    /// already typed so pressing Up from an edited line returns to it on the
    /// way back down.
    pub fn recall_prev(&mut self, current: &str) -> Option<String> {
        if self.cmd_history.is_empty() {
            return None;
        }
        let last = self.cmd_history.len() - 1;
        match self.cmd_hist_idx {
            None => {
                if !current.is_empty() && current == self.cmd_history[last] {
                    if last == 0 {
                        return None;
                    }
                    self.cmd_hist_idx = Some(last - 1);
                } else {
                    self.cmd_hist_idx = Some(last);
                }
            }
            Some(0) => return None,
            Some(i) => self.cmd_hist_idx = Some(i - 1),
        }
        self.cmd_hist_idx.map(|i| self.cmd_history[i].clone())
    }

    /// Recall the next history entry (like Down). Returns `Some("")` when the
    /// cursor passes the end of history back to a fresh prompt.
    pub fn recall_next(&mut self) -> Option<String> {
        match self.cmd_hist_idx {
            None => None,
            Some(i) if i + 1 >= self.cmd_history.len() => {
                self.cmd_hist_idx = None;
                Some(String::new())
            }
            Some(i) => {
                self.cmd_hist_idx = Some(i + 1);
                Some(self.cmd_history[i + 1].clone())
            }
        }
    }
}

/// Map a `vt100::Color` to a ratatui `Color`.
/// Returns `None` for default colors so the parent style applies.
pub fn map_color(c: vt100::Color) -> Option<Color> {
    match c {
        vt100::Color::Default => None,
        vt100::Color::Idx(i) => Some(idx_color(i)),
        vt100::Color::Rgb(r, g, b) => Some(Color::Rgb(r, g, b)),
    }
}

fn idx_color(i: u8) -> Color {
    const PALETTE: [(u8, u8, u8); 16] = [
        (40, 40, 40),
        (255, 85, 85),
        (80, 250, 123),
        (241, 250, 140),
        (98, 114, 164),
        (189, 147, 249),
        (139, 233, 253),
        (248, 248, 242),
        (98, 114, 164),
        (255, 85, 85),
        (80, 250, 123),
        (241, 250, 140),
        (139, 233, 253),
        (189, 147, 249),
        (139, 233, 253),
        (248, 248, 242),
    ];
    let idx = (i as usize) & 15;
    let (r, g, b) = PALETTE[idx];
    Color::Rgb(r, g, b)
}
