# Contributing to ROS2 Info

Thanks for contributing! ROS2 Info has **two** components living side by side:

| Component | Path | Language | Status |
|---|---|---|---|
| Python feature package | `src/ros2_fastfetch/` | Python | Shipped / complete feature set |
| Rust terminal-IDE TUI | `src/tui/` | Rust | In-progress rewrite (dashboard, editor, terminal, AI) |

Pick the component you're touching and follow the matching setup below.

---

## Quick start — Python package

```bash
git clone <your-fork>
cd ros2_info
python -m venv .venv
source .venv/bin/activate
pip install -e src/ros2_fastfetch/ click rich psutil flask climage pillow pyyaml lark
```

## Quick start — Rust TUI

Requires Rust ≥ 1.77 and a `ros2` environment on `PATH` (the TUI shells out to
`ros2`/`colcon` for live data; it builds without ROS present).

```bash
cd src/tui
cargo build
cargo run -- tui        # or: cargo run --bin ros2-info-tui
```

---

## Running tests

```bash
# Python
source .venv/bin/activate
python -m pytest tests/ -v

# Rust
cd src/tui
cargo test
cargo clippy --all-targets -- -D warnings
```

CI (`.github/workflows/ci.yml`) builds and lints **both** components on every
push/PR.

---

## Code style

### Python (`src/ros2_fastfetch/fetch_info/`)
- Type hints on all public functions.
- Follow existing patterns: `collector/` gathers data (no UI imports); `display/` renders (no ROS2 imports).
- Run `flake8 src/ros2_fastfetch/fetch_info` before submitting.

### Rust (`src/tui/`)
- `cargo clippy --all-targets` must be clean (CI enforces `-D warnings`).
- Prefer the existing patterns: `ui.rs` for rendering, `main.rs` for event
  handling, `app.rs` for state. Keep `App` without a `Default` impl.
- No `unwrap()` on user/IO input; surface errors through the existing channels
  (status messages, `AiEvent`, `panic` hook → `/tmp/tui_panic.log`).
- Mouse/UI changes must register a `HitTarget` rect during draw and resolve via
  the unified hit-test in `handle_click` (see `Recent fixes` in `README.md`).

---

## Project structure

```
src/ros2_fastfetch/fetch_info/
  cli.py              — CLI entry point (click commands)
  terminal.py         — Interactive REPL
  web.py              — Web dashboard
  collector/          — Data gathering (no UI imports)
  display/            — Rendering (no ROS2 imports)
src/tui/
  src/
    main.rs           — event loop, key/mouse handling, palette commands
    ui.rs             — all rendering + hit-region registration
    app.rs            — App state, sidebar/editor/terminal managers
    ai.rs             — Ollama-backed AI (chat/scan/fix/auto)
    terminal.rs       — pty-backed terminal sessions
    editor.rs, syntax.rs, git.rs, palette.rs, input.rs
    plugin.rs         — Plugin trait, AppEvent/PluginAction, PluginManager
    plugins/          — built-in plugins (example.rs + register_builtins)
docs/
  plugins.md          — How to write a TUI plugin
tests/                — pytest suite (Python)
```

---

## Adding a plugin (Rust TUI)

The plugin API is **implemented in-process**. Read **`docs/plugins.md`** for the
full guide (trait, dashboard tabs, terminal commands, lifecycle events, build &
register). In short:

1. Open an issue describing the plugin and the `AppEvent`/`PluginAction` it needs.
2. Add a module under `src/tui/src/plugins/<name>/` (mirror `example.rs`).
3. Implement `ros2_info_tui::plugin::Plugin` and add a unit test.
4. Register it with one line in `src/tui/src/plugins/mod.rs::register_builtins`.
5. `cargo build` — the plugin is live. Open a PR; CI builds the TUI and every
   in-tree plugin.

---

## Pull request process

1. Branch from `main`; keep PRs focused.
2. Ensure **both** test suites pass (`pytest` + `cargo test`) and `clippy` is clean.
3. Add tests for new functionality.
4. Update `features.md` (and `README.md` where relevant) with user-facing changes.
5. Open the PR with a clear description of *what* and *why*.

## Reporting issues

Include:
- ROS2 distro and version
- Output of `ros2_info doctor`
- For TUI bugs: the panic log at `/tmp/tui_panic.log` (if any) and repro steps
- Steps to reproduce
