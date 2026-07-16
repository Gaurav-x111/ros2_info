//! ╔════════════════════════════════════════════════════════════════════════╗
//! ║  ROS2_INFO TUI — full-screen Rust terminal dashboard for ROS 2 devs.   ║
//! ║                                                                        ║
//! ║  Author / maintainer: cc@zang aka Gaurav-x111                          ║
//! ╚════════════════════════════════════════════════════════════════════════╝
mod ai;
mod app;
mod collector;
mod editor;
mod file_tree;
mod git;
mod input;
mod palette;
mod plugin;
mod plugins;
mod syntax;
mod telemetry;
mod terminal;
mod theme;
mod ui;
mod web_chat;

use app::{
    Activity, App, CtxAction, Focus, HitTarget, KeybindMode, PaletteMode, ResizeEdge, SandboxMode,
};
use collector::run_background_collection;
use crossterm::event::{KeyCode, KeyModifiers, MouseEvent, MouseEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{cursor, ExecutableCommand};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{stdout, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use ui::ACTIVITY_W;

/// Set by an async signal handler (SIGINT/SIGTERM) or a panic so the main loop
/// can unwind and restore the terminal cleanly instead of leaving it frozen.
static QUIT: AtomicBool = AtomicBool::new(false);

// ── System clipboard (OSC 52) ────────────────────────────────────────────
// Copy/paste against the OS clipboard over a terminal uses the OSC 52 escape
// sequence. `set_clipboard` writes the selection; `request_clipboard` asks the
// terminal to send it back (the reply is parsed in `input.rs` as a Paste event).
// This works over SSH and in any OSC-52-capable terminal/emulator.

fn b64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((n >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(n & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Write `text` to the OS clipboard via OSC 52 (`ESC ] 52 ; c ; <b64> BEL`).
fn set_clipboard(text: &str) {
    if text.is_empty() {
        return;
    }
    let seq = format!("\x1b]52;c;{}\x07", b64_encode(text.as_bytes()));
    let _ = stdout().write_all(seq.as_bytes());
    let _ = stdout().flush();
}

/// Ask the terminal to send the OS clipboard contents back (OSC 52 query).
/// The reply arrives asynchronously and is surfaced as a `Paste` event.
fn request_clipboard() {
    let seq = "\x1b]52;c;?\x07";
    let _ = stdout().write_all(seq.as_bytes());
    let _ = stdout().flush();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::panic::set_hook(Box::new(move |panic_info| {
        QUIT.store(true, Ordering::SeqCst);
        let payload = panic_info
            .payload()
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                panic_info
                    .payload()
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
            })
            .unwrap_or_default();
        let msg = format!(
            "PANIC at {:?}: {}\n",
            panic_info
                .location()
                .map(|l| (l.file(), l.line(), l.column())),
            payload
        );
        let _ = std::fs::write("/tmp/tui_panic.log", &msg);
        let _ = restore_terminal();
        // Do NOT re-invoke the original hook: with stdin set to O_NONBLOCK the
        // shared pty fd (also the stderr target) is non-blocking, so printing
        // the backtrace would itself panic with EAGAIN. Just exit.
        std::process::exit(101);
    }));

    let mut stdout = stdout();
    enable_raw_mode()?;
    stdout.execute(EnterAlternateScreen)?;
    // Enable mouse reporting ourselves. We use basic + button-event tracking
    // (press/release/drag/scroll) with SGR encoding (?1006h) and parse the
    // bytes in `src/input.rs` so a partial escape sequence can never block the
    // UI the way crossterm's blocking `event::read()` does.
    stdout.write_all(b"\x1b[?1000h\x1b[?1002h\x1b[?1006h")?;
    // Report modified "other" keys (Ctrl/Alt/Shift + letter) in `CSI u` form
    // (`ESC [ code ; modifiers u`), so combos like Ctrl+Shift+P are
    // distinguishable from Ctrl+P. Without this most terminals send both as the
    // same control byte. Parsed in `input.rs` (`final_byte == b'u'`).
    stdout.write_all(b"\x1b[>1u")?;
    // Bracketed paste: the terminal wraps pasted text in ESC[200~ … ESC[201~
    // so multi-line pastes are inserted verbatim instead of being interpreted
    // as editor commands. Parsed in `input.rs`.
    stdout.write_all(b"\x1b[?2004h")?;
    stdout.execute(cursor::Hide)?;
    stdout.flush()?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut input = input::Input::new();

    let (tx, rx) = mpsc::channel();
    run_background_collection(tx.clone());

    let mut app = App::new(rx);

    'main: loop {
        if QUIT.load(Ordering::SeqCst) {
            app.quit = true;
        }
        app.tick_status();
        app.process_events();

        // Poll AI events from background autonomous task
        {
            let idx = app.ai_terminal_idx;
            let done = if let Some(ref rx) = app.ai_rx {
                let mut d = false;
                while let Ok(event) = rx.try_recv() {
                    if let Some(idx) = idx {
                        match event {
                            ai::AiEvent::Status(msg) => {
                                app.terminal_mgr
                                    .write_ai_session_idx(idx, &format!("{}\n", msg));
                            }
                            ai::AiEvent::Output(msg) => {
                                app.terminal_mgr.write_ai_session_idx(idx, &msg);
                            }
                            ai::AiEvent::Done => {
                                app.terminal_mgr
                                    .write_ai_session_idx(idx, "\n[Autonomous mode complete]\n");
                                d = true;
                            }
                        }
                    }
                }
                d
            } else {
                false
            };
            if done {
                app.ai_rx = None;
                app.ai_terminal_idx = None;
            }
        }

        terminal.draw(|f| ui::draw(f, &mut app))?;

        if input.eof {
            app.quit = true;
            break 'main;
        }

        match input.poll(Duration::from_millis(100)) {
            Some(crossterm::event::Event::Key(key)) => {
                handle_key(&mut app, key);
                if app.quit {
                    break 'main;
                }
            }
            Some(crossterm::event::Event::Mouse(m)) => handle_mouse(&mut app, m),
            Some(crossterm::event::Event::Paste(text)) => {
                if app.clipboard_read_pending.is_some() {
                    // Reply to our Ctrl+V OSC 52 query → system clipboard.
                    app.editor.paste_text(&text);
                    app.clipboard_read_pending = None;
                } else {
                    handle_paste(&mut app, &text);
                }
            }
            Some(crossterm::event::Event::Resize(_, _)) => {}
            Some(_) => {}
            None => {}
        }

        // Ctrl+V fallback: if the OSC 52 clipboard read didn't reply in time,
        // paste the editor's internal clipboard instead.
        if let Some(deadline) = app.clipboard_read_pending {
            if Instant::now() > deadline {
                app.editor.paste();
                app.clipboard_read_pending = None;
            }
        }
    }

    app.terminal_mgr.shutdown();
    restore_terminal()?;
    Ok(())
}

/// Fan a lifecycle event out to all registered plugins and apply the
/// actions they request (status notifications, terminal commands, …).
fn dispatch_plugin_event(app: &mut App, event: crate::plugin::AppEvent) {
    for action in app.plugin_manager.dispatch(&event) {
        match action {
            crate::plugin::PluginAction::Notify(msg) => app.set_status(msg, 3.0),
            crate::plugin::PluginAction::RunCommand(cmd) => {
                app.terminal_mgr.write_input(cmd.as_bytes());
                app.terminal_mgr.write_input(b"\r");
            }
            crate::plugin::PluginAction::OpenTab(_) => {}
        }
    }
}

fn handle_key(app: &mut App, key: crossterm::event::KeyEvent) {
    // Overlays first.
    if app.confirm.is_some() {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => app.confirm_action(),
            KeyCode::Char('n') | KeyCode::Esc | KeyCode::Char('q') => app.dismiss_confirm(),
            _ => {}
        }
        return;
    }
    if app.help_visible {
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('?') | KeyCode::Enter) {
            app.help_visible = false;
        }
        return;
    }

    if app.palette_open {
        handle_palette_key(app, key);
        return;
    }

    if app.model_picker_open {
        handle_model_picker_key(app, key);
        return;
    }

    if app.ctx_menu.is_some() {
        handle_ctx_menu_key(app, key);
        return;
    }

    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    // Global shortcuts (only when not typing into a panel that claims the key).
    match key.code {
        KeyCode::F(1) => {
            app.help_visible = !app.help_visible;
            return;
        }
        KeyCode::Char('?')
            if !ctrl
                && app.focus != Focus::ActivityBar
                && app.focus != Focus::Sidebar
                && app.focus != Focus::Terminal
                && app.focus != Focus::Editor =>
        {
            app.help_visible = !app.help_visible;
            return;
        }
        KeyCode::Char('c') if ctrl && app.web_chat_running => {
            // Ctrl+C stops the running AI web chat server (takes priority over
            // the global quit shortcut or PTY-forwarding while the server runs).
            stop_web_chat(app);
            return;
        }
        KeyCode::Char('z') if ctrl && app.web_chat_running => {
            // Ctrl+Z also stops the AI web chat server.
            stop_web_chat(app);
            return;
        }
        KeyCode::Char('c')
            if ctrl && app.focus != Focus::Terminal && app.focus != Focus::Editor =>
        {
            app.quit = true;
            return;
        }
        KeyCode::Char('q') if ctrl => {
            // Global force-quit — works from any focus, including the terminal
            // and editor. Plain Ctrl+C is still forwarded to the PTY so the
            // shell keeps its SIGINT semantics.
            app.quit = true;
            return;
        }
        KeyCode::Char('b') if ctrl => {
            app.toggle_sidebar();
            return;
        }
        KeyCode::Char('e') if ctrl => {
            // Force (re)open the file explorer even if the activity bar is
            // hard to click or the sidebar was toggled off.
            app.active_activity = Activity::Explorer;
            app.sidebar_visible = true;
            app.focus = Focus::Sidebar;
            return;
        }
        KeyCode::Char('`') if ctrl => {
            toggle_terminal(app);
            return;
        }
        KeyCode::Char('t') if ctrl => {
            if !app.terminal_visible {
                app.terminal_visible = true;
            }
            let rows = app
                .terminal_mgr
                .active_session()
                .map(|s| s.rows)
                .unwrap_or(24);
            let cols = app
                .terminal_mgr
                .active_session()
                .map(|s| s.cols)
                .unwrap_or(80);
            let before = app.terminal_mgr.sessions.len();
            let new_idx = app.terminal_mgr.new_session(rows, cols);
            if app.terminal_mgr.sessions.len() == before {
                app.set_status(
                    "Terminal error: could not spawn PTY. Check /dev/pts.".into(),
                    4.0,
                );
            } else {
                app.terminal_mgr.switch_to(new_idx);
            }
            app.focus = Focus::Terminal;
            app.terminal_input_buffer.clear();
            return;
        }
        KeyCode::Char(c)
            if c.eq_ignore_ascii_case(&'p')
                && ctrl
                && !key.modifiers.contains(KeyModifiers::SHIFT) =>
        {
            app.open_palette(PaletteMode::File);
            return;
        }
        KeyCode::Char(c)
            if c.eq_ignore_ascii_case(&'p')
                && ctrl
                && key.modifiers.contains(KeyModifiers::SHIFT) =>
        {
            app.open_palette(PaletteMode::Command);
            return;
        }
        KeyCode::Char('f') if ctrl => {
            app.active_activity = Activity::Search;
            app.sidebar_visible = true;
            app.search_input_active = true;
            app.focus = Focus::Sidebar;
            return;
        }
        KeyCode::Char('r') if ctrl => {
            app.set_status("Refreshing...".into(), 3.0);
            return;
        }
        KeyCode::F(6) => {
            match app.sandbox_mode {
                SandboxMode::Sandbox => app.request_enter_global(),
                SandboxMode::Global => app.toggle_sandbox(),
            }
            return;
        }
        KeyCode::Char('1') if alt => {
            app.current_tab = 0;
            return;
        }
        KeyCode::Char('2') if alt => {
            app.current_tab = 1;
            return;
        }
        KeyCode::Char('3') if alt => {
            app.current_tab = 2;
            return;
        }
        KeyCode::Char('4') if alt => {
            app.current_tab = 3;
            return;
        }
        KeyCode::Char('5') if alt => {
            app.current_tab = 4;
            return;
        }
        KeyCode::Char('6') if alt => {
            app.current_tab = 5;
            return;
        }
        // Number keys switch top tabs whenever the editor isn't being edited
        // (so they work with the Welcome tab and while just browsing files).
        KeyCode::Char('1')
            if !ctrl
                && !alt
                && app.focus != Focus::Terminal
                && app.focus != Focus::Editor
                && app.editor.mode != editor::EditMode::Edit =>
        {
            app.current_tab = 0;
            return;
        }
        KeyCode::Char('2')
            if !ctrl
                && !alt
                && app.focus != Focus::Terminal
                && app.focus != Focus::Editor
                && app.editor.mode != editor::EditMode::Edit =>
        {
            app.current_tab = 1;
            return;
        }
        KeyCode::Char('3')
            if !ctrl
                && !alt
                && app.focus != Focus::Terminal
                && app.focus != Focus::Editor
                && app.editor.mode != editor::EditMode::Edit =>
        {
            app.current_tab = 2;
            return;
        }
        KeyCode::Char('4')
            if !ctrl
                && !alt
                && app.focus != Focus::Terminal
                && app.focus != Focus::Editor
                && app.editor.mode != editor::EditMode::Edit =>
        {
            app.current_tab = 3;
            return;
        }
        KeyCode::Char('5')
            if !ctrl
                && !alt
                && app.focus != Focus::Terminal
                && app.focus != Focus::Editor
                && app.editor.mode != editor::EditMode::Edit =>
        {
            app.current_tab = 4;
            return;
        }
        KeyCode::Char('6')
            if !ctrl
                && !alt
                && app.focus != Focus::Terminal
                && app.focus != Focus::Editor
                && app.editor.mode != editor::EditMode::Edit =>
        {
            app.current_tab = 5;
            return;
        }
        _ => {}
    }

    // Escape: unfocus / back. VS Code-like: Esc never quits the TUI.
    // Use Ctrl+Q (or the palette "quit" command) to exit.
    if key.code == KeyCode::Esc {
        match app.focus {
            Focus::None => {
                if app.ros_graph_full {
                    app.ros_graph_full = false;
                } else {
                    // Instead of quitting, re-focus the editor (VS Code behavior).
                    app.focus = Focus::Editor;
                }
            }
            _ => app.focus = Focus::None,
        }
        return;
    }

    // Terminal focus consumes most keys.
    if app.focus == Focus::Terminal && !app.search_input_active {
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Char(c) if c.eq_ignore_ascii_case(&'t') && ctrl && shift => {
                app.terminal_mgr.close_active();
                return;
            }
            KeyCode::Char('w') if ctrl => {
                app.terminal_mgr.close_active();
                return;
            }
            _ => {}
        }
        send_to_terminal(app, &key);
        return;
    }

    // Search input mode: capture all typing into search query
    if app.search_input_active {
        match key.code {
            KeyCode::Esc => {
                app.search_input_active = false;
                app.search_query.clear();
                app.search_results.clear();
            }
            KeyCode::Enter => {
                app.search_input_active = false;
                app.search_files();
            }
            KeyCode::Backspace => {
                app.search_query.pop();
                app.search_files();
            }
            KeyCode::Char(c) => {
                app.search_query.push(c);
                app.search_files();
            }
            _ => {}
        }
        return;
    }

    // Save-as input mode: capture all typing
    if app.save_as_input.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.save_as_input = None;
            }
            KeyCode::Enter => {
                if let Some(ref text) = app.save_as_input.clone() {
                    let trimmed = text.trim().to_string();
                    if !trimmed.is_empty() {
                        let path = std::path::PathBuf::from(&trimmed);
                        app.editor.save_as(path);
                        app.set_status(format!("Saved as: {}", trimmed), 2.0);
                    }
                }
                app.save_as_input = None;
            }
            KeyCode::Backspace => {
                if let Some(ref mut text) = app.save_as_input {
                    text.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(ref mut text) = app.save_as_input {
                    text.push(c);
                }
            }
            _ => {}
        }
        return;
    }

    // Generic text-input prompt (New File / New Folder / Rename).
    if app.prompt.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.prompt = None;
            }
            KeyCode::Enter => {
                app.submit_prompt();
            }
            KeyCode::Backspace => {
                if let Some(ref mut p) = app.prompt {
                    p.value.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(ref mut p) = app.prompt {
                    p.value.push(c);
                }
            }
            _ => {}
        }
        return;
    }

    // Tab / Shift+Tab cycles focus.
    if key.code == KeyCode::Tab {
        cycle_focus(app, true);
        return;
    }
    if key.code == KeyCode::BackTab {
        cycle_focus(app, false);
        return;
    }

    match app.focus {
        Focus::Editor => handle_editor_key(app, key),
        Focus::Sidebar => handle_sidebar_key(app, key),
        Focus::ActivityBar => handle_activity_key(app, key),
        Focus::RightPanel => handle_right_key(app, key),
        Focus::Terminal => send_to_terminal(app, &key),
        Focus::None => handle_global_nav(app, key),
    }
}

fn handle_global_nav(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown => {
            if app.terminal_visible {
                if let Some(sess) = app.terminal_mgr.active_session_mut() {
                    let delta: i32 = match key.code {
                        KeyCode::Up => 1,
                        KeyCode::Down => -1,
                        KeyCode::PageUp => 10,
                        KeyCode::PageDown => -10,
                        _ => 0,
                    };
                    if delta > 0 {
                        sess.scrollback = sess.scrollback.saturating_add(delta as usize);
                    } else {
                        sess.scrollback = sess.scrollback.saturating_sub((-delta) as usize);
                    }
                }
            }
        }
        KeyCode::Enter => {
            // Open the selected tree item if any.
            if let Some(tree) = &mut app.file_tree {
                if let Some(sel) = &tree.selected.clone() {
                    if sel.is_file() {
                        app.open_file_in_editor(sel.clone());
                    } else {
                        tree.expand_selected();
                    }
                }
            }
        }
        KeyCode::Char('i') if !app.editor.is_empty() => {
            app.editor.mode = editor::EditMode::Edit;
            app.focus = Focus::Editor;
        }
        _ => {}
    }
}

fn handle_editor_key(app: &mut App, key: crossterm::event::KeyEvent) {
    if app.editor.is_empty() {
        return;
    }

    // Goto-line input mode
    if app.editor.goto_line_input.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.editor.goto_line_input = None;
            }
            KeyCode::Enter => {
                if let Some(ref text) = app.editor.goto_line_input.clone() {
                    if let Ok(n) = text.parse::<usize>() {
                        app.editor.active_file_mut().unwrap().goto_line(n);
                    }
                }
                app.editor.goto_line_input = None;
            }
            KeyCode::Backspace => {
                if let Some(ref mut text) = app.editor.goto_line_input {
                    text.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(ref mut text) = app.editor.goto_line_input {
                    text.push(c);
                }
            }
            _ => {}
        }
        return;
    }

    // Find / Replace input mode
    if app.editor.find_active {
        match key.code {
            KeyCode::Esc => {
                app.editor.find_active = false;
                app.editor.find_query = None;
                app.editor.replace_query = None;
            }
            KeyCode::Tab => {
                app.editor.find_replace_mode = !app.editor.find_replace_mode;
            }
            // Alt+Enter replaces all occurrences.
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                app.editor.replace_all();
                app.set_status("Replaced all occurrences.".into(), 2.0);
            }
            KeyCode::Enter => {
                let replace_mode = app.editor.find_replace_mode;
                if replace_mode {
                    app.editor.replace_next();
                } else {
                    app.editor.find_next_match();
                }
            }
            KeyCode::Backspace => {
                if app.editor.find_replace_mode {
                    if let Some(ref mut q) = app.editor.replace_query {
                        q.pop();
                    }
                } else if let Some(ref mut q) = app.editor.find_query {
                    q.pop();
                }
                app.editor.refresh_find_count();
            }
            KeyCode::Char(c) => {
                if app.editor.find_replace_mode {
                    app.editor
                        .replace_query
                        .get_or_insert_with(String::new)
                        .push(c);
                } else {
                    app.editor
                        .find_query
                        .get_or_insert_with(String::new)
                        .push(c);
                    app.editor.refresh_find_count();
                }
            }
            _ => {}
        }
        return;
    }

    // Shared editor shortcuts (undo/redo/save/find/word-wrap/...). Used by both
    // the default (Normal) and Neovim key schemes.
    if editor_common_key(app, key) {
        return;
    }

    // Neovim modal scheme: route to its own NORMAL/INSERT handler.
    if app.keybind_mode == KeybindMode::Neovim {
        handle_editor_neovim(app, key);
        return;
    }

    match key.code {
        KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.editor.active_file_mut().unwrap().extend_up()
        }
        KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.editor.active_file_mut().unwrap().extend_down()
        }
        KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.editor.active_file_mut().unwrap().extend_left()
        }
        KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.editor.active_file_mut().unwrap().extend_right()
        }
        KeyCode::Up => app.editor.active_file_mut().unwrap().move_up(),
        KeyCode::Down => app.editor.active_file_mut().unwrap().move_down(),
        KeyCode::Left => app.editor.active_file_mut().unwrap().move_left(),
        KeyCode::Right => app.editor.active_file_mut().unwrap().move_right(),
        KeyCode::Home => {
            let f = app.editor.active_file_mut().unwrap();
            f.cursor_col = 0;
        }
        KeyCode::End => {
            let f = app.editor.active_file_mut().unwrap();
            f.cursor_col = f
                .lines
                .get(f.cursor_row)
                .map(|l| l.chars().count())
                .unwrap_or(0);
        }
        KeyCode::PageUp => {
            let f = app.editor.active_file_mut().unwrap();
            for _ in 0..10 {
                f.move_up();
            }
        }
        KeyCode::PageDown => {
            let f = app.editor.active_file_mut().unwrap();
            for _ in 0..10 {
                f.move_down();
            }
        }
        KeyCode::Char(c) => {
            app.editor.mode = editor::EditMode::Edit;
            app.editor.active_file_mut().unwrap().insert_char(c);
        }
        KeyCode::Enter => {
            app.editor.mode = editor::EditMode::Edit;
            app.editor.active_file_mut().unwrap().insert_newline();
        }
        KeyCode::Backspace => {
            app.editor.mode = editor::EditMode::Edit;
            app.editor.active_file_mut().unwrap().backspace();
        }
        _ => {}
    }
}

/// Editor shortcuts shared by both key-binding schemes: undo/redo, select-all,
/// goto-line, find/replace, copy/cut/paste, save, close, word-wrap. Returns
/// `true` when the key was consumed.
fn editor_common_key(app: &mut App, key: crossterm::event::KeyEvent) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    if ctrl && !shift {
        match key.code {
            KeyCode::Char('z') => {
                app.editor.active_file_mut().unwrap().undo();
                return true;
            }
            KeyCode::Char('y') => {
                app.editor.active_file_mut().unwrap().redo();
                return true;
            }
            KeyCode::Char('a') => {
                app.editor.active_file_mut().unwrap().select_all();
                return true;
            }
            KeyCode::Char('g') => {
                app.editor.goto_line_input = Some(String::new());
                return true;
            }
            KeyCode::Char('f') => {
                app.editor.find_active = true;
                app.editor.find_replace_mode = false;
                app.editor.find_query = Some(String::new());
                return true;
            }
            KeyCode::Char('h') => {
                app.editor.find_active = true;
                app.editor.find_replace_mode = true;
                app.editor.find_query.get_or_insert_with(String::new);
                app.editor.replace_query.get_or_insert_with(String::new);
                return true;
            }
            KeyCode::Char('c') => {
                app.editor.copy_selection();
                set_clipboard(app.editor.get_clipboard());
                app.set_status("Copied selection.".into(), 2.0);
                return true;
            }
            KeyCode::Char('x') => {
                app.editor.cut_selection();
                set_clipboard(app.editor.get_clipboard());
                return true;
            }
            KeyCode::Char('v') => {
                // Ask the terminal for the OS clipboard; the reply is surfaced
                // as a Paste event. If no reply arrives we fall back to the
                // editor's internal clipboard (see the main loop).
                request_clipboard();
                app.clipboard_read_pending = Some(Instant::now() + Duration::from_millis(80));
                return true;
            }
            KeyCode::Char('s') => {
                if let Some(f) = app.editor.active_file() {
                    if f.is_untitled() {
                        app.save_as_input = Some(f.filename());
                    } else {
                        let path = f.path.clone();
                        app.editor.save_active();
                        app.set_status("Saved.".into(), 2.0);
                        dispatch_plugin_event(app, crate::plugin::AppEvent::FileSaved(path));
                    }
                }
                return true;
            }
            KeyCode::Char('w') => {
                if shift {
                    app.editor.files.clear();
                    app.editor.active = 0;
                } else {
                    app.editor.close_active();
                }
                return true;
            }
            _ => {}
        }
    }
    if key.modifiers.contains(KeyModifiers::ALT) && key.code == KeyCode::Char('z') {
        app.editor.word_wrap = !app.editor.word_wrap;
        let mode = if app.editor.word_wrap { "ON" } else { "OFF" };
        app.set_status(format!("Word Wrap: {}", mode), 2.0);
        return true;
    }
    false
}

/// Key handling for the Neovim-style modal scheme. `Preview` is treated as
/// vim's NORMAL mode (letters are commands); `Edit` is INSERT.
fn handle_editor_neovim(app: &mut App, key: crossterm::event::KeyEvent) {
    // INSERT mode: Esc returns to NORMAL; Ctrl+R redoes; otherwise type/edit.
    if app.editor.mode == editor::EditMode::Edit {
        if key.code == KeyCode::Esc {
            app.editor.mode = editor::EditMode::Preview;
            if let Some(f) = app.editor.active_file_mut() {
                f.pending_key = None;
                f.clear_selection();
            }
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('r') {
            app.editor.active_file_mut().unwrap().redo();
            return;
        }
        match key.code {
            KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                app.editor.active_file_mut().unwrap().extend_up()
            }
            KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                app.editor.active_file_mut().unwrap().extend_down()
            }
            KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) => {
                app.editor.active_file_mut().unwrap().extend_left()
            }
            KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => {
                app.editor.active_file_mut().unwrap().extend_right()
            }
            KeyCode::Up => app.editor.active_file_mut().unwrap().move_up(),
            KeyCode::Down => app.editor.active_file_mut().unwrap().move_down(),
            KeyCode::Left => app.editor.active_file_mut().unwrap().move_left(),
            KeyCode::Right => app.editor.active_file_mut().unwrap().move_right(),
            KeyCode::Home => {
                let f = app.editor.active_file_mut().unwrap();
                f.cursor_col = 0;
            }
            KeyCode::End => {
                let f = app.editor.active_file_mut().unwrap();
                f.cursor_col = f
                    .lines
                    .get(f.cursor_row)
                    .map(|l| l.chars().count())
                    .unwrap_or(0);
            }
            KeyCode::PageUp => {
                let f = app.editor.active_file_mut().unwrap();
                for _ in 0..10 {
                    f.move_up();
                }
            }
            KeyCode::PageDown => {
                let f = app.editor.active_file_mut().unwrap();
                for _ in 0..10 {
                    f.move_down();
                }
            }
            KeyCode::Char(c) => {
                app.editor.active_file_mut().unwrap().insert_char(c);
            }
            KeyCode::Enter => {
                app.editor.active_file_mut().unwrap().insert_newline();
            }
            KeyCode::Backspace => {
                app.editor.active_file_mut().unwrap().backspace();
            }
            _ => {}
        }
        return;
    }

    // NORMAL mode (Preview): letters are commands.
    let is_prefix = matches!(
        key.code,
        KeyCode::Char('d') | KeyCode::Char('y') | KeyCode::Char('g')
    );
    if !is_prefix {
        if let Some(f) = app.editor.active_file_mut() {
            f.pending_key = None;
        }
    }
    let visual = app
        .editor
        .active_file()
        .map(|f| f.selection.is_some())
        .unwrap_or(false);

    match key.code {
        KeyCode::Esc => {
            if let Some(f) = app.editor.active_file_mut() {
                f.pending_key = None;
                f.clear_selection();
            }
        }
        KeyCode::Char('h') | KeyCode::Left => {
            if visual {
                app.editor.active_file_mut().unwrap().extend_left();
            } else {
                app.editor.active_file_mut().unwrap().move_left();
            }
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if visual {
                app.editor.active_file_mut().unwrap().extend_down();
            } else {
                app.editor.active_file_mut().unwrap().move_down();
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if visual {
                app.editor.active_file_mut().unwrap().extend_up();
            } else {
                app.editor.active_file_mut().unwrap().move_up();
            }
        }
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Char(' ') => {
            if visual {
                app.editor.active_file_mut().unwrap().extend_right();
            } else {
                app.editor.active_file_mut().unwrap().move_right();
            }
        }
        KeyCode::Char('w') => app.editor.next_word(),
        KeyCode::Char('b') => app.editor.prev_word(),
        KeyCode::Char('e') => app.editor.end_of_word(),
        KeyCode::Char('0') => app.editor.cursor_to_line_start(),
        KeyCode::Char('$') => app.editor.cursor_to_line_end(),
        KeyCode::Char('G') => app.editor.goto_last_line(),
        KeyCode::Char('g') => {
            let pending = app.editor.active_file().and_then(|f| f.pending_key);
            if pending == Some('g') {
                app.editor.goto_first_line();
                if let Some(f) = app.editor.active_file_mut() {
                    f.pending_key = None;
                }
            } else if let Some(f) = app.editor.active_file_mut() {
                f.pending_key = Some('g');
            }
        }
        // Enter INSERT.
        KeyCode::Char('i') => app.editor.mode = editor::EditMode::Edit,
        KeyCode::Char('a') => {
            app.editor.active_file_mut().unwrap().move_right();
            app.editor.mode = editor::EditMode::Edit;
        }
        KeyCode::Char('A') => {
            app.editor.cursor_to_line_end();
            app.editor.mode = editor::EditMode::Edit;
        }
        KeyCode::Char('I') => {
            app.editor.cursor_to_line_start();
            app.editor.mode = editor::EditMode::Edit;
        }
        KeyCode::Char('o') => {
            app.editor.open_below();
            app.editor.mode = editor::EditMode::Edit;
        }
        KeyCode::Char('O') => {
            app.editor.open_above();
            app.editor.mode = editor::EditMode::Edit;
        }
        KeyCode::Char('x') => app.editor.delete_char_at_cursor(),
        KeyCode::Char('d') => {
            let pending = app.editor.active_file().and_then(|f| f.pending_key);
            if pending == Some('d') {
                let has_selection = app
                    .editor
                    .active_file()
                    .map(|f| f.selection.is_some())
                    .unwrap_or(false);
                if has_selection {
                    let f = app.editor.active_file_mut().unwrap();
                    f.push_undo();
                    f.delete_selection();
                    f.pending_key = None;
                } else {
                    app.editor.delete_current_line();
                    if let Some(f) = app.editor.active_file_mut() {
                        f.pending_key = None;
                    }
                }
            } else if let Some(f) = app.editor.active_file_mut() {
                f.pending_key = Some('d');
            }
        }
        KeyCode::Char('y') => {
            let pending = app.editor.active_file().and_then(|f| f.pending_key);
            if pending == Some('y') {
                let text = {
                    let f = app.editor.active_file().unwrap();
                    if f.selection.is_some() {
                        f.selected_text()
                    } else {
                        None
                    }
                };
                if let Some(t) = text {
                    app.editor.clipboard = t;
                } else {
                    app.editor.yank_current_line();
                }
                if let Some(f) = app.editor.active_file_mut() {
                    f.pending_key = None;
                    f.clear_selection();
                }
                set_clipboard(app.editor.get_clipboard());
            } else if let Some(f) = app.editor.active_file_mut() {
                f.pending_key = Some('y');
            }
        }
        KeyCode::Char('p') | KeyCode::Char('P') => {
            request_clipboard();
            app.clipboard_read_pending = Some(Instant::now() + Duration::from_millis(80));
        }
        KeyCode::Char('u') => {
            app.editor.active_file_mut().unwrap().undo();
        }
        KeyCode::Char('v') => app.editor.toggle_visual(),
        KeyCode::Char(':') => app.open_palette(PaletteMode::Command),
        _ => {}
    }
}

fn handle_sidebar_key(app: &mut App, key: crossterm::event::KeyEvent) {
    // Settings panel: the rows are actionable toggles.
    if app.active_activity == Activity::Settings {
        match key.code {
            KeyCode::Char('s') => {
                app.toggle_sidebar();
                return;
            }
            KeyCode::Char('r') => {
                app.toggle_right_panel();
                return;
            }
            KeyCode::Char('t') => {
                app.terminal_visible = !app.terminal_visible;
                return;
            }
            KeyCode::Char('m') => {
                app.toggle_sandbox();
                return;
            }
            KeyCode::Char('g') => {
                app.ros_graph_full = !app.ros_graph_full;
                return;
            }
            KeyCode::Char('k') => {
                app.toggle_keybind_mode();
                return;
            }
            _ => return,
        }
    }
    if app.active_activity == Activity::Plugins {
        if let KeyCode::Char('o') | KeyCode::Enter = key.code {
            let _ = std::process::Command::new("xdg-open").arg(".").spawn();
            app.set_status("Opening current folder…".into(), 2.0);
        }
        return;
    }
    let Some(tree) = &mut app.file_tree else {
        return;
    };
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    match key.code {
        KeyCode::Up => tree.select_prev(),
        KeyCode::Down => tree.select_next(),
        KeyCode::Enter | KeyCode::Right => {
            if let Some(sel) = &tree.selected.clone() {
                if sel.is_file() {
                    app.open_file_in_editor(sel.clone());
                } else {
                    tree.expand_selected();
                }
            }
        }
        KeyCode::Left => {
            if let Some(sel) = &tree.selected.clone() {
                if sel.is_dir() && tree.expanded.contains(sel) {
                    tree.toggle_expand(sel);
                }
            }
        }
        // New file
        KeyCode::Char('n') if !ctrl && !shift => {
            let target = tree.selected.clone().unwrap_or_else(|| tree.root.clone());
            app.run_ctx_action_target(CtxAction::NewFile, target);
        }
        // New folder
        KeyCode::Char('n') if !ctrl && shift => {
            let target = tree.selected.clone().unwrap_or_else(|| tree.root.clone());
            app.run_ctx_action_target(CtxAction::NewFolder, target);
        }
        // Rename
        KeyCode::F(2) => {
            if let Some(sel) = tree.selected.clone() {
                app.run_ctx_action_target(CtxAction::Rename, sel);
            }
        }
        // Delete
        KeyCode::Delete => {
            if let Some(sel) = tree.selected.clone() {
                app.request_delete(sel);
            }
        }
        // Refresh
        KeyCode::F(5) => {
            tree.refresh();
            app.set_status("Explorer refreshed.".into(), 1.5);
        }
        // Open the context menu on the selected item.
        KeyCode::Char('m') => {
            let target = tree.selected.clone().unwrap_or_else(|| tree.root.clone());
            app.open_ctx_menu(ACTIVITY_W + 2, 4, target);
        }
        KeyCode::Char('b') if ctrl => {
            app.toggle_sidebar();
        }
        _ => {}
    }
}

fn handle_activity_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Up => {
            // move selection upward among top activities or to pinned
            let idx = top_index(app.active_activity);
            if idx > 0 {
                app.active_activity = Activity::from_top_index(idx - 1);
            }
        }
        KeyCode::Down => {
            let idx = top_index(app.active_activity);
            if idx < Activity::TOP - 1 {
                app.active_activity = Activity::from_top_index(idx + 1);
            }
        }
        KeyCode::Enter => {
            app.focus = Focus::None;
        }
        _ => {}
    }
}

fn handle_right_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Up => {
            app.entities_tab = app.entities_tab.saturating_sub(1);
        }
        KeyCode::Down => {
            app.entities_tab = (app.entities_tab + 1).min(3);
        }
        _ => {}
    }
}

fn top_index(act: Activity) -> usize {
    match act {
        Activity::Explorer => 0,
        Activity::Search => 1,
        Activity::RosGraph => 2,
        Activity::Diagnostics => 3,
        Activity::Sandbox => 4,
        Activity::Git => 5,
        Activity::Plugins => 6,
        _ => 0,
    }
}

fn cycle_focus(app: &mut App, forward: bool) {
    let all = [
        Focus::ActivityBar,
        Focus::Sidebar,
        Focus::Editor,
        Focus::RightPanel,
        Focus::Terminal,
    ];
    let order: Vec<Focus> = all
        .into_iter()
        .filter(|f| match f {
            Focus::Sidebar => app.sidebar_visible,
            Focus::RightPanel => app.right_visible,
            Focus::Terminal => app.terminal_visible,
            _ => true,
        })
        .collect();
    if order.is_empty() {
        return;
    }
    let cur = order.iter().position(|f| *f == app.focus).unwrap_or(0);
    let next = if forward {
        (cur + 1) % order.len()
    } else {
        (cur + order.len() - 1) % order.len()
    };
    app.focus = order[next];
}

fn toggle_terminal(app: &mut App) {
    app.terminal_visible = !app.terminal_visible;
    if app.terminal_visible {
        app.focus = Focus::Terminal;
    } else {
        app.focus = Focus::None;
    }
}

/// Stop the AI web chat server (if running), freeing its port.
fn stop_web_chat(app: &mut App) {
    if !app.web_chat_running {
        app.set_status("No web chat server running".to_string(), 3.0);
        return;
    }
    if let Some(stop) = app.web_chat_stop.take() {
        stop.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    if let Some(handle) = app.web_chat_handle.take() {
        let _ = handle.join();
    }
    let port = app.web_chat_port;
    app.web_chat_running = false;
    app.set_status(
        format!("Web chat server stopped (was on port {})", port),
        3.0,
    );
}

fn send_to_terminal(app: &mut App, key: &crossterm::event::KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Enter => {
            let cmd = app.terminal_input_buffer.trim().to_string();
            app.terminal_input_buffer.clear();
            // Record the command (after an AI command is intercepted too, so
            // `ai ...` entries are recallable like any other line).
            app.terminal_mgr.push_cmd_history(&cmd);

            if ai::AiCommand::is_ai_command(&cmd) {
                // Intercept AI command
                let is_ai = app
                    .terminal_mgr
                    .sessions
                    .get(app.terminal_mgr.active)
                    .map(|s| s.is_ai)
                    .unwrap_or(false);

                // Reuse an existing AI terminal, or spin one up on the fly.
                let sess_idx = if !is_ai {
                    let new_idx = app.terminal_mgr.new_ai_terminal("AI");
                    app.terminal_mgr.switch_to(new_idx);
                    new_idx
                } else {
                    app.terminal_mgr.active
                };

                // Write the command to the AI terminal display
                app.terminal_mgr.write_ai_session_idx(sess_idx, &cmd);

                // Parse and execute AI command
                let parsed = ai::AiCommand::parse(&cmd);
                match parsed {
                    ai::AiCommand::Help => {
                        app.terminal_mgr.write_ai_session_idx(
                            sess_idx,
                            "\n=== AI Commands ===\n\
                             ai auto        — Full autonomous mode\n\
                             ai             — Interactive mode\n\
                             ai scan        — Scan for errors\n\
                             ai fix <file>  — Fix a specific file\n\
                             ai explain <e> — Explain an error\n\
                             ai chat <msg>  — Chat about code\n\
                             ai web [port]  — Start web chat UI (default: 8899)\n\
                             ai web stop    — Stop the running web chat server\n\
                             ai model       — List installed Ollama models\n\
                             ai model <n>   — Set the active model\n\
                             ai help        — Show this help\n\n",
                        );
                    }
                    ai::AiCommand::Scan => {
                        app.terminal_mgr
                            .write_ai_session_idx(sess_idx, "\n🔍 Scanning build errors...\n");
                        let errors = ai::scan_build_errors();
                        if errors.is_empty() {
                            app.terminal_mgr
                                .write_ai_session_idx(sess_idx, "✅ No build errors found.\n");
                        } else {
                            app.terminal_mgr.write_ai_session_idx(
                                sess_idx,
                                &format!("❌ Found {} errors:\n", errors.len()),
                            );
                            for e in &errors {
                                app.terminal_mgr
                                    .write_ai_session_idx(sess_idx, &format!("  {}\n", e));
                            }
                        }
                        app.terminal_mgr.write_ai_session_idx(sess_idx, "\n");
                    }
                    ai::AiCommand::Auto => {
                        app.terminal_mgr.write_ai_session_idx(
                            sess_idx,
                            "\n🤖 Starting autonomous mode (max 3 attempts)...\n",
                        );
                        let model = app.ai_model.clone();
                        let idx = sess_idx;
                        let (tx, rx) = std::sync::mpsc::channel();
                        std::thread::spawn(move || {
                            ai::run_autonomous(idx, tx, model);
                        });
                        app.ai_terminal_idx = Some(sess_idx);
                        app.ai_rx = Some(rx);
                    }
                    ai::AiCommand::Interactive => {
                        app.terminal_mgr.write_ai_session_idx(
                            sess_idx,
                            "\n🤖 Interactive mode — scanning...\n",
                        );
                        let errors = ai::scan_build_errors();
                        if errors.is_empty() {
                            app.terminal_mgr.write_ai_session_idx(
                                sess_idx,
                                "✅ No build errors found. Nothing to do.\n\n",
                            );
                        } else {
                            app.terminal_mgr.write_ai_session_idx(
                                sess_idx,
                                &format!("Found {} errors:\n", errors.len()),
                            );
                            for (i, e) in errors.iter().enumerate() {
                                app.terminal_mgr.write_ai_session_idx(
                                    sess_idx,
                                    &format!("  {}. {}\n", i + 1, e),
                                );
                            }
                            app.terminal_mgr.write_ai_session_idx(
                                sess_idx,
                                "\nType 'y' to fix, 'n' to skip, or 'q' to quit.\n",
                            );
                        }
                    }
                    ai::AiCommand::Fix(path) => {
                        app.terminal_mgr.write_ai_session_idx(
                            sess_idx,
                            &format!("🔍 Scanning {} for errors...\n", path),
                        );
                        // For now, just run clippy on the file
                        let (_, stderr, _) = ai::run_cmd("cargo", &["clippy", "--quiet", "2>&1"]);
                        let file_errors: Vec<String> = stderr
                            .lines()
                            .filter(|l| l.contains(&path) && l.contains("warning"))
                            .map(|l| l.trim().to_string())
                            .collect();
                        if file_errors.is_empty() {
                            app.terminal_mgr.write_ai_session_idx(
                                sess_idx,
                                &format!("✅ No issues found in {}\n\n", path),
                            );
                        } else {
                            app.terminal_mgr.write_ai_session_idx(
                                sess_idx,
                                &format!("Found {} issues in {}:\n", file_errors.len(), path),
                            );
                            for e in &file_errors {
                                app.terminal_mgr
                                    .write_ai_session_idx(sess_idx, &format!("  {}\n", e));
                            }
                        }
                    }
                    ai::AiCommand::Explain(msg) => {
                        app.terminal_mgr
                            .write_ai_session_idx(sess_idx, &format!("🤖 Explaining: {}\n\n", msg));
                        // Simple explanation via Ollama
                        let prompt =
                            format!("Explain this error in detail and how to fix it:\n{}", msg);
                        match ai::call_ollama(&prompt, &app.ai_model) {
                            Ok(response) => {
                                app.terminal_mgr
                                    .write_ai_session_idx(sess_idx, &format!("{}\n\n", response));
                            }
                            Err(e) => {
                                app.terminal_mgr.write_ai_session_idx(
                                    sess_idx,
                                    &format!("❌ AI error: {}\n\n", e),
                                );
                            }
                        }
                    }
                    ai::AiCommand::Chat(msg) => {
                        let prompt = format!(
                            "You are a Rust/ROS2 expert helping with a TUI project.\n\
                             User asks: {}\n\
                             Provide a concise, helpful answer.",
                            msg
                        );
                        match ai::call_ollama(&prompt, &app.ai_model) {
                            Ok(response) => {
                                app.terminal_mgr
                                    .write_ai_session_idx(sess_idx, &format!("{}\n\n", response));
                            }
                            Err(e) => {
                                app.terminal_mgr.write_ai_session_idx(
                                    sess_idx,
                                    &format!("❌ AI error: {}\n\n", e),
                                );
                            }
                        }
                    }
                    ai::AiCommand::WebChat(port) => {
                        if app.web_chat_running {
                            app.terminal_mgr.write_ai_session_idx(
                                sess_idx,
                                &format!(
                                    "⚠️  Web chat server already running on port {}\n\
                                 Stop it first with: ai web stop\n\n",
                                    app.web_chat_port
                                ),
                            );
                        } else {
                            let stop =
                                std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                            let server = web_chat::WebChatServer::new(port, stop.clone());
                            let handle = server.start();
                            app.web_chat_running = true;
                            app.web_chat_port = port;
                            app.web_chat_stop = Some(stop);
                            app.web_chat_handle = Some(handle);
                            app.terminal_mgr.write_ai_session_idx(
                                sess_idx,
                                &format!(
                                    "🌐 Web chat server started!\n\
                                     📡 Open: http://localhost:{}\n\
                                     🤖 Backend: Ollama (auto-picks installed model)\n\
                                     🛑 Stop it with: ai web stop\n\n",
                                    port
                                ),
                            );
                        }
                    }
                    ai::AiCommand::WebChatStop => {
                        if !app.web_chat_running {
                            app.terminal_mgr.write_ai_session_idx(
                                sess_idx,
                                "⚠️  No web chat server is running.\n\n",
                            );
                        } else {
                            if let Some(stop) = app.web_chat_stop.take() {
                                stop.store(true, std::sync::atomic::Ordering::SeqCst);
                            }
                            if let Some(handle) = app.web_chat_handle.take() {
                                let _ = handle.join();
                            }
                            app.web_chat_running = false;
                            app.terminal_mgr.write_ai_session_idx(
                                sess_idx,
                                &format!(
                                    "🛑 Web chat server stopped (was on port {}).\n\n",
                                    app.web_chat_port
                                ),
                            );
                        }
                    }
                    ai::AiCommand::ModelList => {
                        let (models, err) = ai::list_ollama_models();
                        if let Some(e) = err {
                            app.terminal_mgr
                                .write_ai_session_idx(sess_idx, &format!("❌ {}\n\n", e));
                        } else {
                            app.terminal_mgr.write_ai_session_idx(
                                sess_idx,
                                &format!(
                                    "📦 Installed Ollama models (current: {}):\n",
                                    app.ai_model
                                ),
                            );
                            for m in &models {
                                let marker = if *m == app.ai_model {
                                    " ← active"
                                } else {
                                    ""
                                };
                                app.terminal_mgr.write_ai_session_idx(
                                    sess_idx,
                                    &format!("  {}{}\n", m, marker),
                                );
                            }
                            app.terminal_mgr.write_ai_session_idx(
                                sess_idx,
                                "\nSet a model: ai model <name>\n\n",
                            );
                        }
                    }
                    ai::AiCommand::ModelSet(name) => {
                        // Quick-validate by checking if the model exists in the installed list.
                        let (models, _) = ai::list_ollama_models();
                        if models.iter().any(|m| m == &name) {
                            app.ai_model = name.clone();
                            app.terminal_mgr.write_ai_session_idx(
                                sess_idx,
                                &format!("✅ Model set to: {}\n\n", name),
                            );
                        } else if models.is_empty() {
                            app.terminal_mgr.write_ai_session_idx(
                                sess_idx,
                                &format!(
                                    "⚠️  Cannot verify model (Ollama unreachable or no models).\n\
                                     Set anyway: {}\n\n",
                                    name
                                ),
                            );
                            app.ai_model = name;
                        } else {
                            app.terminal_mgr.write_ai_session_idx(
                                sess_idx,
                                &format!("❌ Model '{}' not found. Available:\n", name),
                            );
                            for m in &models {
                                app.terminal_mgr
                                    .write_ai_session_idx(sess_idx, &format!("  {}\n", m));
                            }
                            app.terminal_mgr.write_ai_session_idx(sess_idx, "\n");
                        }
                    }
                    ai::AiCommand::Unknown(cmd) => {
                        // Fall back to plugin-contributed terminal commands
                        // (e.g. `ai battery`). Split into name + args.
                        let mut parts = cmd.splitn(2, char::is_whitespace);
                        let name = parts.next().unwrap_or("").trim();
                        let args = parts.next().unwrap_or("").trim();
                        match app.plugin_manager.run_command(name, args) {
                            Some(output) => {
                                app.terminal_mgr
                                    .write_ai_session_idx(sess_idx, &format!("{}\n\n", output));
                            }
                            None => {
                                app.terminal_mgr.write_ai_session_idx(
                                    sess_idx,
                                    &format!(
                                        "Unknown command: {}. Type 'ai help' for usage.\n\n",
                                        cmd
                                    ),
                                );
                            }
                        }
                    }
                }
            } else {
                // Regular command — send Enter + buffer to PTY
                app.terminal_mgr.write_input(b"\r");
                if !app.terminal_input_buffer.is_empty() {
                    app.terminal_mgr
                        .write_input(app.terminal_input_buffer.as_bytes());
                }
            }
        }
        KeyCode::Char(c) => {
            if ctrl {
                // Forward the actual control byte (Ctrl+C => 0x03 SIGINT, etc.)
                let byte = (c as u8) & 0x1f;
                app.terminal_mgr.write_input(&[byte]);
            } else {
                app.terminal_input_buffer.push(c);
                app.terminal_mgr.write_input(c.to_string().as_bytes());
            }
        }
        KeyCode::Backspace => {
            app.terminal_input_buffer.pop();
            app.terminal_mgr.write_input(b"\x7f");
        }
        KeyCode::Up => {
            // Hijack Up/Down for command history when live (not scrolled back)
            // and on the TERMINAL sub-tab; otherwise forward the arrow so the
            // shell/scrollback can use it.
            let live = app
                .terminal_mgr
                .active_session()
                .map(|s| s.scrollback == 0)
                .unwrap_or(true);
            if app.terminal_tab == 0 && live {
                if let Some(h) = app.terminal_mgr.recall_prev(&app.terminal_input_buffer) {
                    app.terminal_input_buffer = h.clone();
                    app.terminal_mgr.write_input(b"\x15"); // Ctrl+U: clear line
                    app.terminal_mgr.write_input(h.as_bytes());
                }
            } else {
                app.terminal_mgr.write_input(b"\x1b[A");
            }
        }
        KeyCode::Down => {
            let live = app
                .terminal_mgr
                .active_session()
                .map(|s| s.scrollback == 0)
                .unwrap_or(true);
            if app.terminal_tab == 0 && live {
                match app.terminal_mgr.recall_next() {
                    Some(h) => {
                        app.terminal_input_buffer = h.clone();
                        app.terminal_mgr.write_input(b"\x15");
                        app.terminal_mgr.write_input(h.as_bytes());
                    }
                    None => {
                        app.terminal_input_buffer.clear();
                        app.terminal_mgr.write_input(b"\x15");
                    }
                }
            } else {
                app.terminal_mgr.write_input(b"\x1b[B");
            }
        }
        KeyCode::Right => app.terminal_mgr.write_input(b"\x1b[C"),
        KeyCode::Left => app.terminal_mgr.write_input(b"\x1b[D"),
        KeyCode::Tab => app.terminal_mgr.write_input(b"\t"),
        _ => {}
    };
}

/// Current filtered palette results, recomputed each call.
fn palette_results(app: &App) -> Vec<(usize, palette::PaletteItem)> {
    palette::filter_commands(&app.palette_query)
}

fn palette_file_results(app: &App) -> Vec<(usize, (String, PathBuf))> {
    let cands = palette::file_candidates(&app.palette_files());
    let filtered = palette::filter_files(&cands, &app.palette_query);
    match app.palette_mode {
        PaletteMode::Launch => filtered
            .into_iter()
            .filter(|(_, (_, p))| palette::is_launch_file(p))
            .collect(),
        PaletteMode::Bag => filtered
            .into_iter()
            .filter(|(_, (_, p))| palette::is_bag_file(p))
            .collect(),
        _ => filtered,
    }
}

fn handle_palette_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.close_palette();
        }
        KeyCode::Enter => match app.palette_mode {
            PaletteMode::Command => {
                if let Some((_, item)) = palette_results(app).into_iter().nth(app.palette_sel) {
                    run_palette_command(app, item.id);
                    app.close_palette();
                }
            }
            PaletteMode::File => {
                if let Some((_, (_, path))) =
                    palette_file_results(app).into_iter().nth(app.palette_sel)
                {
                    app.open_file_in_editor(path);
                    app.close_palette();
                }
            }
            PaletteMode::Launch => {
                if let Some((_, (_, path))) =
                    palette_file_results(app).into_iter().nth(app.palette_sel)
                {
                    run_ros_command(app, &format!("ros2 launch {}", path.display()));
                    app.close_palette();
                }
            }
            PaletteMode::Bag => {
                if let Some((_, (_, path))) =
                    palette_file_results(app).into_iter().nth(app.palette_sel)
                {
                    run_ros_command(app, &format!("ros2 bag play {}", path.display()));
                    app.close_palette();
                }
            }
        },
        KeyCode::Up => {
            app.palette_sel = app.palette_sel.saturating_sub(1);
        }
        KeyCode::Down => {
            app.palette_sel = app.palette_sel.saturating_add(1);
        }
        KeyCode::Backspace => {
            app.palette_query.pop();
            app.palette_sel = 0;
        }
        KeyCode::Char(c) => {
            app.palette_query.push(c);
            app.palette_sel = 0;
        }
        _ => {}
    }
    // Clamp selection to the available result count.
    let count = if matches!(app.palette_mode, PaletteMode::Command) {
        palette_results(app).len()
    } else {
        palette_file_results(app).len()
    };
    if count > 0 {
        app.palette_sel = app.palette_sel.min(count - 1);
    } else {
        app.palette_sel = 0;
    }
}

fn handle_ctx_menu_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let n = app.ctx_menu.as_ref().map(|m| m.items.len()).unwrap_or(0);
    match key.code {
        KeyCode::Esc => {
            app.ctx_menu = None;
        }
        KeyCode::Up => {
            app.ctx_menu_sel = app.ctx_menu_sel.saturating_sub(1);
        }
        KeyCode::Down => {
            app.ctx_menu_sel = app.ctx_menu_sel.saturating_add(1).min(n.saturating_sub(1));
        }
        KeyCode::Enter => {
            let action = app
                .ctx_menu
                .as_ref()
                .and_then(|m| m.items.get(app.ctx_menu_sel).copied());
            if let Some(a) = action {
                app.ctx_menu_sel = 0;
                app.run_ctx_action(a);
            }
        }
        _ => {}
    }
}

fn handle_model_picker_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let n = app.available_models.len();
    match key.code {
        KeyCode::Esc | KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.close_model_picker();
        }
        KeyCode::Up => {
            app.model_picker_index = app.model_picker_index.saturating_sub(1);
        }
        KeyCode::Down => {
            app.model_picker_index = (app.model_picker_index + 1).min(n.saturating_sub(1));
        }
        KeyCode::Enter => {
            app.select_model();
        }
        _ => {}
    }
}

fn run_palette_command(app: &mut App, id: &str) {
    match id {
        "file.open" => {
            app.open_palette(PaletteMode::File);
        }
        "file.new" => {
            app.editor.new_untitled();
            app.editor.mode = editor::EditMode::Edit;
            app.focus = Focus::Editor;
        }
        "file.save" => {
            if let Some(f) = app.editor.active_file() {
                if f.is_untitled() {
                    app.save_as_input = Some(f.filename());
                } else {
                    app.editor.save_active();
                    app.set_status("Saved.".into(), 2.0);
                }
            }
        }
        "file.saveAs" => {
            if let Some(f) = app.editor.active_file() {
                app.save_as_input = Some(f.filename());
            }
        }
        "file.close" => {
            app.editor.close_active();
        }
        "term.new" => {
            if !app.terminal_visible {
                app.terminal_visible = true;
            }
            let rows = app
                .terminal_mgr
                .active_session()
                .map(|s| s.rows)
                .unwrap_or(24);
            let cols = app
                .terminal_mgr
                .active_session()
                .map(|s| s.cols)
                .unwrap_or(80);
            let before = app.terminal_mgr.sessions.len();
            let new_idx = app.terminal_mgr.new_session(rows, cols);
            if app.terminal_mgr.sessions.len() == before {
                app.set_status("Terminal error: could not spawn PTY.".into(), 4.0);
            } else {
                app.terminal_mgr.switch_to(new_idx);
            }
            app.focus = Focus::Terminal;
        }
        "term.toggle" => {
            app.terminal_visible = !app.terminal_visible;
            app.focus = if app.terminal_visible {
                Focus::Terminal
            } else {
                Focus::None
            };
        }
        "view.toggleSidebar" => app.toggle_sidebar(),
        "view.toggleRight" => app.toggle_right_panel(),
        "view.toggleWordWrap" => {
            app.editor.word_wrap = !app.editor.word_wrap;
            let m = if app.editor.word_wrap { "ON" } else { "OFF" };
            app.set_status(format!("Word Wrap: {}", m), 2.0);
        }
        "view.zen" => {
            let all_on = app.sidebar_visible && app.right_visible && app.terminal_visible;
            app.sidebar_visible = !all_on;
            app.right_visible = !all_on;
            app.terminal_visible = !all_on;
        }
        "nav.gotoLine" => {
            if !app.editor.is_empty() {
                app.editor.goto_line_input = Some(String::new());
                app.focus = Focus::Editor;
            }
        }
        "nav.find" => {
            if !app.editor.is_empty() {
                app.editor.find_active = true;
                app.editor.find_replace_mode = false;
                app.editor.find_query = Some(String::new());
                app.focus = Focus::Editor;
            }
        }
        "nav.overview" => app.current_tab = 0,
        "nav.ros2" => app.current_tab = 1,
        "nav.workspace" => app.current_tab = 2,
        "nav.diagnostics" => app.current_tab = 3,
        "nav.trends" => app.current_tab = 4,
        "nav.fleet" => app.current_tab = 5,
        "sandbox.toggle" => app.toggle_sandbox(),
        "settings.keybinds" => app.toggle_keybind_mode(),
        "help" => app.help_visible = true,
        "refresh" => app.set_status("Refreshing...".into(), 3.0),

        // ── AI assistant ─────────────────────────────────────────────────
        "ai.chooseModel" => app.open_model_picker(),
        "ai.auto" => {
            // Equivalent to typing `ai solve` in the integrated terminal.
            let sess_idx = app.terminal_mgr.active;
            app.terminal_mgr.write_ai_session_idx(
                sess_idx,
                "\n🤖 Starting autonomous mode (max 3 attempts)...\n",
            );
            let model = app.ai_model.clone();
            let idx = sess_idx;
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                ai::run_autonomous(idx, tx, model);
            });
            app.ai_terminal_idx = Some(sess_idx);
            app.ai_rx = Some(rx);
            app.terminal_visible = true;
            app.focus = Focus::Terminal;
        }

        // ── ROS 2 quick tools ────────────────────────────────────────────
        "ros.nodeList" => run_ros_command(app, "ros2 node list"),
        "ros.topicList" => run_ros_command(app, "ros2 topic list"),
        "ros.serviceList" => run_ros_command(app, "ros2 service list"),
        "ros.actionList" => run_ros_command(app, "ros2 action list"),
        "ros.paramList" => run_ros_command(app, "ros2 param list"),
        "ros.interfaceList" => run_ros_command(app, "ros2 interface list"),
        "ros.topicEcho" => {
            let sel = app.selected_node.clone().unwrap_or_default();
            if app.entities_tab == 1 && !sel.is_empty() {
                run_ros_command(app, &format!("ros2 topic echo {}", sel));
            } else {
                run_ros_command(app, "ros2 topic echo ");
            }
        }
        "ros.topicHz" => {
            let sel = app.selected_node.clone().unwrap_or_default();
            if app.entities_tab == 1 && !sel.is_empty() {
                run_ros_command(app, &format!("ros2 topic hz {}", sel));
            } else {
                run_ros_command(app, "ros2 topic hz ");
            }
        }
        "ros.topicInfo" => {
            let sel = app.selected_node.clone().unwrap_or_default();
            if app.entities_tab == 1 && !sel.is_empty() {
                run_ros_command(app, &format!("ros2 topic info {}", sel));
            } else {
                run_ros_command(app, "ros2 topic info ");
            }
        }
        "ros.nodeInfo" => {
            let sel = app.selected_node.clone().unwrap_or_default();
            if app.entities_tab == 0 && !sel.is_empty() {
                run_ros_command(app, &format!("ros2 node info {}", sel));
            } else {
                run_ros_command(app, "ros2 node info ");
            }
        }
        "ros.doctor" => run_ros_command(app, "ros2 doctor"),
        "ros.daemon" => run_ros_command(app, "ros2 daemon status"),
        "ros.bagRecord" => {
            let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
            run_ros_command(app, &format!("ros2 bag record -a -o ~/rosbag_{}", stamp));
        }
        "ros.bagPlay" => {
            app.open_palette(PaletteMode::Bag);
        }
        "ros.launch" => run_ros_command(app, "ros2 launch "),
        "ros.launchPicker" => {
            app.open_palette(PaletteMode::Launch);
        }
        "ros.run" => run_ros_command(app, "ros2 run "),

        "quit" => app.quit = true,
        _ => {}
    }
}

/// Run a `ros2` (or shell) command in the integrated terminal, creating a
/// session if needed and focusing it. A trailing carriage return makes the
/// shell execute the line immediately.
fn run_ros_command(app: &mut App, cmd: &str) {
    if !app.terminal_visible {
        app.terminal_visible = true;
    }
    if app.terminal_mgr.sessions.is_empty() {
        let (rows, cols) = (app.terminal_height.max(3), 80u16);
        app.terminal_mgr.new_session(rows, cols);
    }
    app.terminal_mgr
        .write_input(format!("{}\r", cmd).as_bytes());
    app.focus = Focus::Terminal;
    app.set_status(format!("▶ {}", cmd.trim_end()), 3.0);
}

/// Insert pasted text. When the editor has focus the text goes into the
/// buffer (surfaced as OSC 52 from the terminal's own paste path); when a
/// terminal pane is focused it is piped to that pty so shell/remote apps
/// receive it.
fn handle_paste(app: &mut App, text: &str) {
    if text.is_empty() {
        return;
    }
    if app.focus == Focus::Editor {
        app.editor.paste_text(text);
    } else if app.focus == Focus::Terminal {
        app.terminal_mgr.write_input(text.as_bytes());
    }
}

fn handle_mouse(app: &mut App, m: MouseEvent) {
    let MouseEvent {
        kind, column, row, ..
    } = m;
    match kind {
        MouseEventKind::ScrollDown => handle_scroll(app, -3),
        MouseEventKind::ScrollUp => handle_scroll(app, 3),
        MouseEventKind::Down(btn) => {
            // Right-click opens the file-tree context menu.
            if matches!(btn, crossterm::event::MouseButton::Right) {
                if app.sidebar_visible
                    && column >= ui::ACTIVITY_W
                    && column < ui::ACTIVITY_W + app.sidebar_width
                {
                    let target = sidebar_item_at(app, row).unwrap_or_else(|| {
                        app.file_tree
                            .as_ref()
                            .map(|t| t.root.clone())
                            .unwrap_or_default()
                    });
                    app.open_ctx_menu(column, row, target);
                } else {
                    app.ctx_menu = None;
                }
                return;
            }
            // Start a resize drag if the press landed on a resize edge;
            // otherwise treat the press as a click (responsive on Down).
            if let Some(edge) = hit_resize_edge(app, column, row) {
                app.resizing = Some(edge);
            } else {
                handle_click(app, column, row);
            }
        }
        MouseEventKind::Up(_) => {
            // End any resize drag. Activity-bar toggles happen on press (Down),
            // not here, so a click gesture can never toggle twice.
            app.resizing = None;
        }
        MouseEventKind::Drag(_) => {
            if let Some(edge) = app.resizing {
                match edge {
                    app::ResizeEdge::SidebarRight => {
                        let new_w = column.saturating_sub(ui::ACTIVITY_W);
                        app.sidebar_width = new_w.clamp(15, 60);
                    }
                    app::ResizeEdge::RightPanelLeft => {
                        let tw = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
                        let new_w = tw.saturating_sub(column);
                        app.right_panel_width = new_w.clamp(20, 60);
                    }
                    app::ResizeEdge::TerminalTop => {
                        let th = crossterm::terminal::size().map(|(_, h)| h).unwrap_or(24);
                        let new_h = th.saturating_sub(row).saturating_sub(2);
                        app.terminal_height = new_h.clamp(3, 20);
                    }
                }
            }
        }
        _ => {}
    }
}

fn hit_resize_edge(app: &mut App, col: u16, row: u16) -> Option<ResizeEdge> {
    let (tw, th) = crossterm::terminal::size().unwrap_or((80, 24));
    let activity_end = ui::ACTIVITY_W;
    let sidebar_end = if app.sidebar_visible {
        activity_end + app.sidebar_width
    } else {
        activity_end
    };
    let right_start = if app.right_visible {
        tw.saturating_sub(app.right_panel_width)
    } else {
        tw
    };
    let term_top = if app.terminal_visible {
        th.saturating_sub(app.terminal_height).saturating_sub(2)
    } else {
        th
    };

    if app.sidebar_visible
        && col >= sidebar_end.saturating_sub(1)
        && col <= sidebar_end + 1
        && row >= 2
    {
        return Some(ResizeEdge::SidebarRight);
    }
    if app.right_visible
        && col >= right_start.saturating_sub(1)
        && col <= right_start + 1
        && row >= 2
        && row < term_top
    {
        return Some(ResizeEdge::RightPanelLeft);
    }
    if app.terminal_visible
        && col >= activity_end
        && row >= term_top.saturating_sub(1)
        && row <= term_top
    {
        // Only the border row (and the body row just above it) start a resize;
        // the session-tab / "+" row (`term_top + 1`) must remain clickable.
        return Some(ResizeEdge::TerminalTop);
    }
    None
}

fn handle_scroll(app: &mut App, delta: i32) {
    match app.focus {
        Focus::Editor => {
            if let Some(f) = app.editor.active_file_mut() {
                if delta > 0 {
                    f.scroll_row = f.scroll_row.saturating_add(delta as usize);
                } else {
                    f.scroll_row = f.scroll_row.saturating_sub((-delta) as usize);
                }
            }
        }
        Focus::Terminal => {
            if let Some(sess) = app.terminal_mgr.active_session_mut() {
                if delta > 0 {
                    sess.scrollback = sess.scrollback.saturating_add(delta as usize);
                } else {
                    sess.scrollback = sess.scrollback.saturating_sub((-delta) as usize);
                }
            }
        }
        _ => {}
    }
}

/// Toggle the sidebar / panel state for an activity-bar icon. `icon` is the
/// activity index for `HitTarget::Activity`, `99` for Settings, `100` for Help.
/// Called once on mouse *release* so a full click gesture toggles exactly once.
fn toggle_activity_icon(app: &mut App, icon: u8) {
    match icon {
        99 => {
            if app.active_activity == Activity::Settings && app.sidebar_visible {
                app.sidebar_visible = false;
                app.focus = Focus::Editor;
            } else {
                app.active_activity = Activity::Settings;
                app.sidebar_visible = true;
                app.focus = Focus::ActivityBar;
            }
        }
        100 => {
            app.help_visible = !app.help_visible;
        }
        i => {
            let clicked = Activity::from_top_index(i as usize);
            if app.active_activity == clicked && app.sidebar_visible {
                // VS Code toggle: clicking active icon hides sidebar, returns to editor.
                app.sidebar_visible = false;
                app.focus = Focus::Editor;
            } else {
                app.active_activity = clicked;
                app.sidebar_visible = true;
                app.focus = Focus::ActivityBar;
            }
        }
    }
}

fn handle_click(app: &mut App, col: u16, row: u16) {
    // If a context menu is open, a left-click either picks an item or dismisses it.
    if let Some(menu) = &app.ctx_menu {
        let w = 26u16;
        let h = menu.items.len() as u16 + 2;
        let term_w = crossterm::terminal::size().map(|(tw, _)| tw).unwrap_or(80);
        let x = menu.x.min(term_w.saturating_sub(w));
        let y = menu.y;
        let in_menu = col >= x && col < x + w && row >= y && row < y + h;
        if in_menu && row > y {
            let idx = (row - y - 1) as usize;
            let action = menu.items.get(idx).copied();
            app.ctx_menu = None;
            app.ctx_menu_sel = 0;
            if let Some(a) = action {
                app.run_ctx_action(a);
            }
            return;
        }
        app.ctx_menu = None;
        return;
    }

    // Overlays must swallow mouse clicks; otherwise a click "falls through" to
    // the activity bar / sidebar underneath and corrupts UI state.
    if app.help_visible {
        app.help_visible = false;
        return;
    }
    if app.confirm.is_some() {
        return;
    }
    if app.palette_open {
        app.palette_open = false;
        app.palette_query.clear();
        app.palette_sel = 0;
        return;
    }

    if row == 0 {
        return;
    }

    // Unified input contract: resolve the click to a single widget via the
    // hit-test registry rebuilt every frame in `ui::draw`. Exactly one widget
    // wins (topmost), so a gesture mutates state at most once.
    let click_pt = ratatui::layout::Rect::new(col, row, 1, 1);
    let target = app
        .hit_regions
        .iter()
        .rev()
        .find(|(r, _)| r.intersects(click_pt))
        .map(|(_, t)| *t);

    match target {
        None => {
            // Unregistered area → treat as an editor-body click (focus + cursor).
            handle_editor_body_click(app, col, row);
        }
        Some(HitTarget::Activity(i)) => {
            // Toggle on press (like every other clickable in the app). The
            // 280 ms debounce collapses a genuine double-click so the sidebar
            // doesn't flip twice.
            let icon = i as u8;
            if !app.activity_click_debounced(icon) {
                toggle_activity_icon(app, icon);
            }
        }
        Some(HitTarget::ActivitySettings) => {
            if !app.activity_click_debounced(99) {
                toggle_activity_icon(app, 99);
            }
        }
        Some(HitTarget::ActivityHelp) => {
            if !app.activity_click_debounced(100) {
                toggle_activity_icon(app, 100);
            }
        }
        Some(HitTarget::TopTab(i)) => {
            app.current_tab = i;
        }
        Some(HitTarget::RightItem(_)) => {
            let total_w = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
            match ui::right_items_click(col, total_w) {
                Some(ui::RightItem::RosGraphToggle) => {
                    app.ros_graph_full = !app.ros_graph_full;
                }
                Some(ui::RightItem::Gear) => {
                    app.toggle_right_panel();
                }
                _ => {}
            }
        }
        Some(HitTarget::TerminalSession(i)) => {
            if i < app.terminal_mgr.sessions.len() {
                app.terminal_mgr.switch_to(i);
            }
            app.focus = Focus::Terminal;
        }
        Some(HitTarget::TerminalClose(i)) => {
            app.terminal_mgr.close_at(i);
            app.focus = Focus::Terminal;
        }
        Some(HitTarget::TerminalPlus) => {
            let before = app.terminal_mgr.sessions.len();
            let new_idx = app.terminal_mgr.new_session(
                app.terminal_mgr
                    .sessions
                    .first()
                    .map(|s| s.rows)
                    .unwrap_or(24),
                app.terminal_mgr
                    .sessions
                    .first()
                    .map(|s| s.cols)
                    .unwrap_or(80),
            );
            if app.terminal_mgr.sessions.len() == before {
                app.set_status("Terminal error: could not spawn PTY.".into(), 4.0);
            } else {
                app.terminal_mgr.switch_to(new_idx);
            }
            app.focus = Focus::Terminal;
        }
        Some(HitTarget::TerminalSubTab(i)) => {
            app.terminal_tab = i;
            app.focus = Focus::Terminal;
        }
        Some(HitTarget::TerminalBody) => {
            app.focus = Focus::Terminal;
        }
        Some(HitTarget::Sidebar) => {
            handle_sidebar_click(app, col, row);
        }
        Some(HitTarget::SidebarFile(idx)) => {
            if let Some(tree) = &mut app.file_tree {
                if let Some(item) = tree.items.get(idx) {
                    let path = item.path.clone();
                    let is_dir = item.is_dir;
                    tree.selected = Some(path.clone());
                    app.focus = Focus::Sidebar;
                    if is_dir {
                        tree.toggle_expand(&path);
                    } else {
                        app.open_file_in_editor(path);
                    }
                }
            }
        }
        Some(HitTarget::SidebarOpenEditor(i)) => {
            if i < app.editor.files.len() {
                app.editor.active = i;
                app.focus = Focus::Editor;
            }
        }
        Some(HitTarget::SidebarNew) => {
            app.editor.new_untitled();
            app.editor.mode = editor::EditMode::Edit;
            app.focus = Focus::Editor;
        }
        Some(HitTarget::SidebarSearchInput) => {
            app.search_input_active = true;
            app.focus = Focus::Sidebar;
        }
        Some(HitTarget::SidebarSearchResult(r)) => {
            if r < app.search_results.len() {
                let hit = &app.search_results[r];
                app.open_file_at_line(hit.path.clone(), hit.line);
            }
        }
        Some(HitTarget::RightPanelHeader(i)) => {
            if i < app.right_expanded.len() {
                app.right_expanded[i] = !app.right_expanded[i];
            }
        }
        Some(HitTarget::RightPanelEntity(_)) => {
            handle_right_panel_click(app, col, row);
        }
        Some(HitTarget::EditorTab(_)) => {
            handle_editor_tab_click(app, col, row);
        }
        Some(HitTarget::EditorTabClose(_)) => {
            handle_editor_tab_click(app, col, row);
        }
        Some(HitTarget::EditorNewTab) => {
            handle_editor_tab_click(app, col, row);
        }
        Some(HitTarget::Breadcrumb) => {
            if !app.editor.is_empty() {
                if let Some(f) = app.editor.active_file() {
                    let path = f.path.clone();
                    if let Some(parent) = path.parent() {
                        app.open_file_in_editor(parent.to_path_buf());
                    }
                }
                app.focus = Focus::Editor;
            }
        }
        Some(HitTarget::EditorBody) => {
            handle_editor_body_click(app, col, row);
        }
    }
}

/// Sidebar click handler retained for any generic `Sidebar` region that is not
/// covered by a more specific `HitTarget` (file-tree rows, OPEN EDITORS rows,
/// the Explorer "+", the search input, and search results are all handled by
/// their own dedicated arms in `handle_click`).
fn handle_sidebar_click(app: &mut App, _col: u16, _row: u16) {
    if !app.sidebar_visible {
        return;
    }
    app.focus = Focus::Sidebar;
}

/// Right-panel click handler (extracted from the old coordinate-based logic).
/// Toggles headers, switches entity sub-tabs, selects entity rows, and triggers
/// the Sandbox export button.
fn handle_right_panel_click(app: &mut App, col: u16, row: u16) {
    if !app.right_visible {
        return;
    }
    app.focus = Focus::RightPanel;
    let right_x = ui::ACTIVITY_W
        + (if app.sidebar_visible {
            app.sidebar_width
        } else {
            0
        })
        + center_width(app);
    let local_row = row.saturating_sub(2);
    let mut y = 0u16;
    for pi in 0..4 {
        let header_h = 1u16;
        let body_h = if app.right_expanded[pi] { 8u16 } else { 0u16 };
        let panel_h = header_h + body_h;
        if local_row < y + panel_h {
            if local_row == y {
                app.right_expanded[pi] = !app.right_expanded[pi];
            } else if pi == 1 && app.right_expanded[1] {
                if local_row == y + 1 {
                    let rel_col = col.saturating_sub(right_x);
                    let tab_count = 4u16;
                    let tab_w = app.right_panel_width / tab_count;
                    let idx = rel_col
                        .checked_div(tab_w)
                        .map_or(0, |q| q.min(tab_count - 1)) as usize;
                    app.entities_tab = idx;
                } else if local_row > y + 1 {
                    let row_idx = (local_row - y - 2) as usize;
                    let entities = match app.entities_tab {
                        0 => app.ros2.as_ref().map(|r| r.nodes.len()).unwrap_or(0),
                        1 => app.ros2.as_ref().map(|r| r.topics.len()).unwrap_or(0),
                        2 => app.ros2.as_ref().map(|r| r.services.len()).unwrap_or(0),
                        _ => app.ros2.as_ref().map(|r| r.actions.len()).unwrap_or(0),
                    };
                    if row_idx < entities {
                        app.selected_node = match app.entities_tab {
                            0 => app
                                .ros2
                                .as_ref()
                                .and_then(|r| r.nodes.get(row_idx).cloned()),
                            1 => app
                                .ros2
                                .as_ref()
                                .and_then(|r| r.topics.get(row_idx).map(|(n, _)| n.clone())),
                            2 => app
                                .ros2
                                .as_ref()
                                .and_then(|r| r.services.get(row_idx).cloned()),
                            _ => app
                                .ros2
                                .as_ref()
                                .and_then(|r| r.actions.get(row_idx).cloned()),
                        };
                    }
                }
            } else if pi == 3 {
                let export_y = y + panel_h - 1;
                if local_row == export_y {
                    app.request_enter_global();
                }
            }
            break;
        }
        y += panel_h;
    }
}

/// Editor tab-strip click handler (tab switch / close / new-buffer button).
fn handle_editor_tab_click(app: &mut App, col: u16, _row: u16) {
    if app.editor.is_empty() {
        return;
    }
    let center_start = if app.sidebar_visible {
        ui::ACTIVITY_W + app.sidebar_width
    } else {
        ui::ACTIVITY_W
    };
    let rel = col.saturating_sub(center_start);
    let (plus_x, plus_w) = ui::editor_new_tab_range(app);
    if rel >= plus_x && rel < plus_x + plus_w {
        app.editor.new_untitled();
        app.editor.mode = editor::EditMode::Edit;
        app.focus = Focus::Editor;
        return;
    }
    if let Some((idx, close)) = ui::editor_tab_hit(rel, app) {
        if close {
            app.editor.close_at(idx);
        } else {
            app.editor.active = idx;
        }
        app.focus = Focus::Editor;
    }
}

/// Editor body click handler: focus the editor and place the cursor.
fn handle_editor_body_click(app: &mut App, col: u16, row: u16) {
    app.focus = Focus::Editor;
    if app.editor.is_empty() {
        return;
    }
    // Map using the real editor code-area rect captured during draw, so clicks
    // resolve to the correct line regardless of layout/viewport offsets.
    let area = app.editor_area;
    if area.height == 0 {
        return;
    }
    let content_top: i32 = area.y as i32;
    let local_row: i32 = row as i32 - content_top;
    if local_row >= 0 {
        // Text starts after the 1-col border + 6-col gutter.
        let text_x: i32 = area.x as i32 + 1 + 6;
        let rel_col: i32 = col as i32 - text_x;
        let wrap = app.editor.word_wrap;
        if let Some(f) = app.editor.active_file_mut() {
            let text_w = area.width.saturating_sub(2 + 6).max(1) as usize;
            let visual = f.visual_lines(text_w, wrap);
            let scroll = f.scroll_row.min(f.lines.len().saturating_sub(1));
            let vis_start = visual
                .iter()
                .position(|&(r, _, _)| r >= scroll)
                .unwrap_or(0);
            let vidx = vis_start + local_row as usize;
            if vidx < visual.len() {
                let (logical, cstart, cend) = visual[vidx];
                let line_len = f.lines.get(logical).map(|l| l.chars().count()).unwrap_or(0);
                let mut cc = if rel_col < 0 {
                    cstart
                } else {
                    ((cstart as i32 + rel_col).max(cstart as i32)) as usize
                };
                cc = cc.min(cend).min(line_len);
                f.cursor_row = logical;
                f.cursor_col = cc;
            }
        }
    }
}

fn sidebar_item_at(app: &App, row: u16) -> Option<std::path::PathBuf> {
    let local = (row as i32 - 4) as usize;
    app.file_tree
        .as_ref()
        .and_then(|t| t.items.get(local))
        .map(|it| it.path.clone())
}

fn center_width(app: &App) -> u16 {
    let tw = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
    let mut w = tw.saturating_sub(ui::ACTIVITY_W);
    if app.sidebar_visible {
        w = w.saturating_sub(app.sidebar_width);
    }
    if app.right_visible {
        w = w.saturating_sub(app.right_panel_width);
    }
    w
}

fn restore_terminal() -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = stdout();
    stdout.write_all(b"\x1b[?2004l")?;
    stdout.write_all(b"\x1b[?1006l\x1b[?1002l\x1b[?1000l")?;
    stdout.write_all(b"\x1b[>0u")?; // disable modifyOtherKeys (CSI u) reporting
    stdout.execute(cursor::Show)?;
    stdout.execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
