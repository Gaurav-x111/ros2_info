//! Custom, non-blocking input reader.
//!
//! crossterm's `event::read()` performs a *blocking* read and can get stuck
//! forever if the terminal emits an escape sequence it cannot complete (a
//! known failure mode on double-clicks and on some terminal emulators). That
//! freezes the entire TUI. To avoid it we read raw stdin bytes and parse both
//! key presses and SGR (`?1006h`) mouse reports ourselves. A partial sequence
//! is simply buffered until the rest arrives, so the UI can never hang waiting
//! on input.
//!
//! We use `poll()` with a timeout (rather than `O_NONBLOCK`) so we never touch
//! the file-status flags of the shared pty fd — setting `O_NONBLOCK` on stdin
//! would also make stdout/stderr non-blocking and break writes elsewhere.

use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use std::io;
use std::os::unix::io::AsRawFd;
use std::time::{Duration, Instant};

pub struct Input {
    fd: i32,
    buf: Vec<u8>,
    /// Set when stdin hits EOF (parent terminal closed).
    pub eof: bool,
    esc_since: Option<Instant>,
    /// True while collecting a bracketed-paste payload (`ESC [ 200 ~ … ESC [ 201 ~`).
    paste_mode: bool,
}

impl Input {
    pub fn new() -> Self {
        Self {
            fd: io::stdin().as_raw_fd(),
            buf: Vec::with_capacity(256),
            eof: false,
            esc_since: None,
            paste_mode: false,
        }
    }

    /// Read whatever is available right now, then try to parse one complete
    /// event. Returns `None` when there is no complete event yet (a partial
    /// sequence is retained for the next tick).
    pub fn poll(&mut self, timeout: Duration) -> Option<crossterm::event::Event> {
        let mut fds = libc::pollfd {
            fd: self.fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let n = unsafe { libc::poll(&mut fds as *mut _, 1, timeout.as_millis() as i32) };
        if n > 0 {
            let ready = fds.revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR);
            if ready != 0 {
                let mut tmp = [0u8; 4096];
                let r = unsafe {
                    libc::read(self.fd, tmp.as_mut_ptr() as *mut libc::c_void, tmp.len())
                };
                if r > 0 {
                    self.buf.extend_from_slice(&tmp[..r as usize]);
                    // Safety valve: never let a stuck partial sequence grow forever.
                    if self.buf.len() > 8192 {
                        let drop = self.buf.len() - 1024;
                        self.buf.drain(0..drop);
                    }
                } else if r == 0 {
                    self.eof = true;
                }
                if fds.revents & (libc::POLLHUP | libc::POLLERR) != 0 {
                    self.eof = true;
                }
            }
        }
        self.parse_one()
    }

    fn parse_one(&mut self) -> Option<crossterm::event::Event> {
        if self.buf.is_empty() {
            return None;
        }
        // While collecting a bracketed paste, wait for the terminator
        // `ESC [ 2 0 1 ~`, then emit the payload as a Paste event.
        if self.paste_mode {
            if let Some(pos) = self.buf.windows(6).position(|w| w == b"\x1b[201~") {
                let content = self.buf[..pos].to_vec();
                self.buf.drain(0..pos + 6);
                self.paste_mode = false;
                let text = String::from_utf8_lossy(&content).into_owned();
                return Some(crossterm::event::Event::Paste(text));
            }
            return None;
        }
        let b = self.buf[0];
        if b == 0x1b {
            return self.parse_escape();
        }
        if b == 0x7f || b == 0x08 {
            self.buf.drain(0..1);
            return Some(key(KeyCode::Backspace, KeyModifiers::NONE));
        }
        if b == b'\n' || b == b'\r' {
            self.buf.drain(0..1);
            return Some(key(KeyCode::Enter, KeyModifiers::NONE));
        }
        if b == b'\t' {
            self.buf.drain(0..1);
            return Some(key(KeyCode::Tab, KeyModifiers::NONE));
        }
        // Ctrl+letter (0x01..=0x1a) -> Char('a'..='z') with CONTROL.
        if (0x01..=0x1a).contains(&b) {
            let c = (b + 0x60) as char; // 1->'a', 0x11->'q', ...
            self.buf.drain(0..1);
            return Some(key_mod(KeyCode::Char(c), KeyModifiers::CONTROL));
        }
        if b.is_ascii() && !b.is_ascii_control() {
            let c = b as char;
            self.buf.drain(0..1);
            return Some(key(KeyCode::Char(c), KeyModifiers::NONE));
        }
        // Unknown byte - skip it and retry.
        self.buf.drain(0..1);
        if self.esc_since.is_some() {
            self.esc_since = None;
        }
        self.parse_one()
    }

    fn parse_escape(&mut self) -> Option<crossterm::event::Event> {
        if self.buf.len() == 1 {
            // Lone ESC: wait briefly in case it is the prefix of a sequence.
            let now = Instant::now();
            match self.esc_since {
                None => {
                    self.esc_since = Some(now);
                    None
                }
                Some(t) => {
                    if now.duration_since(t) > Duration::from_millis(40) {
                        self.buf.drain(0..1);
                        self.esc_since = None;
                        Some(key(KeyCode::Esc, KeyModifiers::NONE))
                    } else {
                        None
                    }
                }
            }
        } else {
            self.esc_since = None;
            let second = self.buf[1];
            if second == b'[' {
                // Bracketed-paste start: `ESC [ 2 0 0 ~`. Enter paste mode and
                // collect the raw payload until `ESC [ 2 0 1 ~`.
                if self.buf.len() >= 6 && &self.buf[2..6] == b"200~" {
                    self.buf.drain(0..6);
                    self.paste_mode = true;
                    return None;
                }
                // SGR mouse reports are `ESC [ < ... M/m`. Detect the `<` and
                // hand off to the mouse parser instead of `parse_csi`, which
                // would otherwise swallow the whole sequence as an unknown CSI.
                if self.buf.len() > 2 && self.buf[2] == b'<' {
                    self.parse_sgr_mouse()
                } else {
                    self.parse_csi()
                }
            } else if second == b']' {
                // OSC sequence (clipboard read/write via OSC 52, etc.).
                self.parse_osc()
            } else if second == b'O' {
                self.buf.drain(0..2);
                self.parse_one()
            } else if second == b'<' {
                self.parse_sgr_mouse()
            } else if (33..=126).contains(&second) {
                // Alt+char (e.g. Alt+a -> ESC a).
                let c = second as char;
                self.buf.drain(0..2);
                Some(key_mod(KeyCode::Char(c), KeyModifiers::ALT))
            } else {
                self.buf.drain(0..2);
                self.parse_one()
            }
        }
    }

    fn parse_csi(&mut self) -> Option<crossterm::event::Event> {
        if self.buf.len() < 3 {
            return None;
        }
        let end = (2..self.buf.len()).find(|&i| (0x40..=0x7e).contains(&self.buf[i]))?;
        let seq = &self.buf[2..=end];
        let final_byte = seq[seq.len() - 1];
        let param_str = &seq[..seq.len() - 1];
        let params: Vec<u32> = param_str
            .split(|&b| b == b';')
            .map(|s| {
                std::str::from_utf8(s)
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0)
            })
            .collect();

        let mut modifiers = KeyModifiers::NONE;
        if params.len() >= 2 {
            match params[1] {
                2 => modifiers |= KeyModifiers::SHIFT,
                3 => modifiers |= KeyModifiers::ALT,
                4 => modifiers = modifiers.union(KeyModifiers::ALT | KeyModifiers::SHIFT),
                5 => modifiers |= KeyModifiers::CONTROL,
                6 => modifiers = modifiers.union(KeyModifiers::CONTROL | KeyModifiers::SHIFT),
                7 => modifiers = modifiers.union(KeyModifiers::CONTROL | KeyModifiers::ALT),
                8 => {
                    modifiers = modifiers
                        .union(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT)
                }
                _ => {}
            }
        }

        // `CSI u` modified-key encoding (`ESC [ code ; modifiers u`), enabled
        // via `ESC [ > 1 u` at startup. This is what makes Ctrl/Alt/Shift +
        // letter combos (e.g. Ctrl+Shift+P) distinguishable from their unmodified
        // forms. `code` is the key's ASCII/Unicode codepoint; `modifiers` is
        // already derived from the second parameter above (1=none,2=shift,
        // 3=alt,5=ctrl,6=ctrl+shift,…).
        if final_byte == b'u' {
            let kc = match params.first().copied().unwrap_or(0) {
                9 => {
                    if modifiers.contains(KeyModifiers::SHIFT) {
                        KeyCode::BackTab
                    } else {
                        KeyCode::Tab
                    }
                }
                13 => KeyCode::Enter,
                27 => KeyCode::Esc,
                32..=126 => KeyCode::Char((params[0] as u8) as char),
                127 => KeyCode::Backspace,
                _ => {
                    self.buf.drain(0..=end);
                    return self.parse_one();
                }
            };
            self.buf.drain(0..=end);
            return Some(key_mod(kc, modifiers));
        }

        let code = match final_byte {
            b'A' => Some(KeyCode::Up),
            b'B' => Some(KeyCode::Down),
            b'C' => Some(KeyCode::Right),
            b'D' => Some(KeyCode::Left),
            b'H' => Some(KeyCode::Home),
            b'F' => Some(KeyCode::End),
            b'Z' => {
                self.buf.drain(0..=end);
                return Some(key_mod(KeyCode::BackTab, KeyModifiers::SHIFT));
            }
            b'~' => match params.first().copied().unwrap_or(0) {
                1 | 7 => Some(KeyCode::Home),
                2 => Some(KeyCode::Insert),
                3 => Some(KeyCode::Delete),
                4 | 8 => Some(KeyCode::End),
                5 => Some(KeyCode::PageUp),
                6 => Some(KeyCode::PageDown),
                _ => None,
            },
            // xterm window-resize report: ESC [ 8 ; height ; width t
            b't' if params.first().copied() == Some(8) => {
                let h = params.get(1).copied().unwrap_or(0) as u16;
                let w = params.get(2).copied().unwrap_or(0) as u16;
                self.buf.drain(0..=end);
                return Some(crossterm::event::Event::Resize(w, h));
            }
            _ => None,
        };

        self.buf.drain(0..=end);
        match code {
            Some(c) => Some(key_mod(c, modifiers)),
            None => self.parse_one(),
        }
    }

    /// Parse an OSC (`ESC ] …`) sequence. We only care about OSC 52 clipboard
    /// replies: `ESC ] 5 2 ; c ; <base64> BEL` (or `ST`). A `?` payload is a
    /// query response with no data and is ignored. Decoded text is emitted as
    /// a `Paste` event so the app can insert system-clipboard content.
    fn parse_osc(&mut self) -> Option<crossterm::event::Event> {
        // An OSC sequence is `ESC ] …` terminated by BEL (0x07) or ST
        // (`ESC \`). `term_idx` is the last byte to drain; `content_end` is the
        // byte where the payload (the `]` ... data) ends.
        let (term_idx, content_end) =
            if let Some(rel) = self.buf[1..].iter().position(|&b| b == 0x07) {
                let bel = 1 + rel;
                (bel, bel)
            } else {
                let i = (1..self.buf.len().saturating_sub(1))
                    .find(|&i| self.buf[i] == 0x1b && self.buf.get(i + 1) == Some(&b'\\'))?;
                (i + 1, i)
            };
        let seq = self.buf[1..content_end].to_vec();
        self.buf.drain(0..=term_idx);
        // seq begins with ']' followed by "52;c;BASE64".
        if seq.first() == Some(&b']') && seq.len() > 6 && &seq[1..6] == b"52;c;" {
            let data = String::from_utf8_lossy(&seq[6..]);
            if data.trim() == "?" {
                return None;
            }
            if let Some(bytes) = b64_decode(&data) {
                let text = String::from_utf8_lossy(&bytes).into_owned();
                return Some(crossterm::event::Event::Paste(text));
            }
        }
        self.parse_one()
    }

    fn parse_sgr_mouse(&mut self) -> Option<crossterm::event::Event> {
        let end = (3..self.buf.len()).find(|&i| self.buf[i] == b'M' || self.buf[i] == b'm')?;
        let body = &self.buf[3..end]; // e.g. "0;12;34"
        let parts: Vec<&[u8]> = body.split(|&b| b == b';').collect();
        if parts.len() != 3 {
            self.buf.drain(0..=end);
            return self.parse_one();
        }
        let parse = |p: &[u8]| std::str::from_utf8(p).ok().and_then(|s| s.parse().ok());
        let (b_code, x, y): (u32, u32, u32) =
            match (parse(parts[0]), parse(parts[1]), parse(parts[2])) {
                (Some(a), Some(b), Some(c)) => (a, b, c),
                _ => {
                    self.buf.drain(0..=end);
                    return self.parse_one();
                }
            };
        self.buf.drain(0..=end);

        let letter = if self.buf.get(end) == Some(&b'm') {
            b'm'
        } else {
            b'M'
        };
        let btn = match b_code & 0x3 {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            _ => MouseButton::Right,
        };
        let kind = if letter == b'm' {
            MouseEventKind::Up(btn)
        } else if b_code & 64 != 0 {
            if b_code & 1 != 0 {
                MouseEventKind::ScrollDown
            } else {
                MouseEventKind::ScrollUp
            }
        } else if b_code & 32 != 0 {
            MouseEventKind::Drag(btn)
        } else {
            MouseEventKind::Down(btn)
        };
        let column: u16 = x.saturating_sub(1) as u16;
        let row: u16 = y.saturating_sub(1) as u16;
        Some(crossterm::event::Event::Mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }))
    }
}

fn key(code: KeyCode, mods: KeyModifiers) -> crossterm::event::Event {
    crossterm::event::Event::Key(KeyEvent {
        code,
        modifiers: mods,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    })
}

fn key_mod(code: KeyCode, mods: KeyModifiers) -> crossterm::event::Event {
    key(code, mods)
}

/// Minimal RFC 4648 base64 decoder (used for OSC 52 clipboard payloads).
fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let s = s.trim_end_matches('=');
    let val = |c: char| -> Option<u32> {
        match c {
            'A'..='Z' => Some(c as u32 - 'A' as u32),
            'a'..='z' => Some(c as u32 - 'a' as u32 + 26),
            '0'..='9' => Some(c as u32 - '0' as u32 + 52),
            '+' => Some(62),
            '/' => Some(63),
            _ => None,
        }
    };
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut i = 0;
    while i < bytes.len() {
        let mut n: u32 = 0;
        let mut cnt = 0;
        for (j, &b) in bytes[i..].iter().take(4).enumerate() {
            n |= val(b as char)? << (18 - 6 * j);
            cnt += 1;
        }
        // 4 chars → 3 bytes, 3 → 2, 2 → 1. A short final group ends input.
        if cnt >= 2 {
            out.push((n >> 16) as u8);
        }
        if cnt >= 3 {
            out.push(((n >> 8) & 0xff) as u8);
        }
        if cnt >= 4 {
            out.push((n & 0xff) as u8);
        }
        if cnt < 4 {
            break;
        }
        i += 4;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{Event, KeyCode, KeyModifiers};

    /// Feed raw bytes and parse exactly one event (the rest is ignored).
    fn parse_one_event(bytes: &[u8]) -> Option<Event> {
        let mut inp = Input::new();
        inp.buf.extend_from_slice(bytes);
        inp.parse_one()
    }

    #[test]
    fn ctrl_shift_p_is_reported_via_csi_u() {
        // Ctrl+Shift+P -> `ESC [ 80 ; 6 u` (P=80, modifier 6 = ctrl+shift).
        let e = parse_one_event(b"\x1b[80;6u").expect("event");
        match e {
            Event::Key(k) => {
                assert_eq!(k.code, KeyCode::Char('P'));
                assert!(k.modifiers.contains(KeyModifiers::CONTROL));
                assert!(k.modifiers.contains(KeyModifiers::SHIFT));
            }
            other => panic!("expected key, got {:?}", other),
        }
    }

    #[test]
    fn ctrl_p_is_ctrl_only() {
        // Ctrl+P (no shift) -> `ESC [ 112 ; 5 u` (p=112, modifier 5 = ctrl).
        let e = parse_one_event(b"\x1b[112;5u").expect("event");
        match e {
            Event::Key(k) => {
                assert_eq!(k.code, KeyCode::Char('p'));
                assert!(k.modifiers.contains(KeyModifiers::CONTROL));
                assert!(!k.modifiers.contains(KeyModifiers::SHIFT));
            }
            other => panic!("expected key, got {:?}", other),
        }
    }

    #[test]
    fn csi_u_enable_sequence_is_ignored() {
        // The startup `ESC [ > 1 u` must not produce a key event.
        assert!(parse_one_event(b"\x1b[>1u").is_none());
    }

    #[test]
    fn legacy_plain_and_arrows_still_work() {
        assert_eq!(
            parse_one_event(b"a").map(|e| match e {
                Event::Key(k) => k.code,
                _ => KeyCode::Null,
            }),
            Some(KeyCode::Char('a'))
        );
        assert_eq!(
            parse_one_event(b"\x1b[A").map(|e| match e {
                Event::Key(k) => k.code,
                _ => KeyCode::Null,
            }),
            Some(KeyCode::Up)
        );
    }
}
