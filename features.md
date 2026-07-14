# ros2_info — Features, Subcommands & Commands

Inventory of every feature and how to invoke it. Two surface areas exist side by side:

- **`ros2_info` (Python)** — the complete, shipped feature set (PyPI-installable).
- **`ros2_info tui` (Rust)** — the in-progress terminal-IDE rewrite; a dashboard shell plus a passthrough command bar today.

The long-term goal (per the system-prompt spec) is for the Rust TUI to reach feature parity with the Python package. This file tracks both: what exists in Python is the spec-to-port; what exists in Rust is current state.

---

## 1. CLI subcommands — `ros2_info` (Python)

Default invocation with no subcommand prints the FastFetch-style overview.

### Global flags

| Flag | Effect |
|---|---|
| `-l` / `--live` | Collect live nodes/topics/services instead of cached |
| `--watch N` / `-w N` | Refresh every N seconds |
| `--json` | Output raw JSON |
| `-e` / `--env` | Show environment-variables panel |
| `-i` / `--interactive` | Interactive TUI menu |
| `--info` | System-info snapshot (legacy default) |
| `--verbose` | Expanded output with full lists |
| `--no-logo` | Skip logo rendering |
| `--ascii` | Force ASCII logo |
| `--image` | Force image rendering |
| `--width N` | Override terminal width (columns) |
| `--no-system` | Skip system collection |
| `--no-workspace` | Skip workspace collection |
| `--no-updates` | Skip ROS2 package-update check |
| `--logo` | Print ASCII logo only |
| `--timeout N` | Discovery timeout (default 3) |
| `--no-boot` | Skip auto-source / bootstrap |
| `-t` / `--theme NAME` | Select theme |
| `--web` | Launch the web dashboard |

### Subcommands

| # | Subcommand | Purpose |
|---|---|---|
| 1 | `ros2_info` | FastFetch-style overview (default) |
| 2 | `ros2_info terminal` | Full interactive REPL |
| 3 | `ros2_info graph` | ASCII node↔topic graph |
| 4 | `ros2_info nodes` | Live nodes list (add `--info/-I` for pub/sub) |
| 5 | `ros2_info topics` | Live topics list (`--hz` for publish rate, `-v` verbose) |
| 6 | `ros2_info services` | Live services list |
| 7 | `ros2_info actions` | Live actions list |
| 8 | `ros2_info packages [-f filter]` | Packages (with optional filter) |
| 9 | `ros2_info workspace` | Colcon workspaces + build status |
| 10 | `ros2_info env` | ROS2 / shell environment variables |
| 11 | `ros2_info themes` | List/select a theme |
| 12 | `ros2_info web [--port 8099]` | Web dashboard (Rust RT backend or legacy Flask; `--ssl`, `--auth`) |
| 13 | `ros2_info bag record\|play\|info ...` | rosbag2 record / play / info |
| 14 | `ros2_info param get\|set\|list ...` | Node parameters get / set / list |
| 15 | `ros2_info launch <pkg> <file> [args...]` | Launch a launch file |
| 16 | `ros2_info run <pkg> <exe> [args...]` | Run an executable |
| 17 | `ros2_info interface <type>` | Show an interface (msg/srv/action) definition |
| 18 | `ros2_info doctor` | Health checks (ROS not sourced, DDS mismatch, missing env, broken launch, bad YAML) |
| 19 | `ros2_info diagnose` | Alias of `doctor` |
| 20 | `ros2_info matrix` | Distro / version-compat matrix |
| 21 | `ros2_info benchmark [-t sec]` | Performance benchmark |
| 22 | `ros2_info trend [-H hours] [--record\|--chart\|--daemon]` | Historical trends / session capture |
| 23 | `ros2_info launch-verify <path>` (alias `lv`) | Validate a launch file + dependencies |
| 24 | `ros2_info bag-analyze <bag> [--compare other]` (alias `ba`) | rosbag forensics / diff |
| 25 | `ros2_info fleet <host...> [--discover] [--subnet S]` | Multi-host / multi-robot fleet view |
| 26 | `ros2_info tui` | Launch the Rust TUI |
| 27 | `ros2_info sandbox <action>` | Sandbox manager (create/clone/switch/export/import) |

---

## 2. REPL commands — inside `ros2_info terminal`

Tab-completion is live against discovered topics/nodes.

### Discovery & inspection
| Command | Action |
|---|---|
| `nodes` | List active nodes |
| `topics` | List active topics |
| `services` | List active services |
| `actions` | List active actions |
| `env` | Show ROS2 environment |
| `node info <n>` | Pub/sub info for a node |
| `interface show <type>` | Show an interface definition |
| `graph` | ASCII graph view |
| `watch` | Live auto-refresh view |

### Telemetry & publish
| Command | Action |
|---|---|
| `echo <topic>` | Stream topic messages |
| `hz <topic>` | Publish rate |
| `bw <topic>` | Bandwidth |
| `pub ...` | Publish to a topic |
| `ping <node>` | Ping a node |
| `service call <s> ...` | Call a service |
| `param get\|set\|list` | Node parameters |

### Execution & lifecycle
| Command | Action |
|---|---|
| `launch <pkg> <file>` | Launch file |
| `run <pkg> <exe>` | Run executable |
| `bag record\|play\|info` | Bag operations |
| `sandbox ...` | Sandbox isolation (`/sandbox` namespace) |

### Tooling passthroughs
| Command | Action |
|---|---|
| `shell <cmd>` | Run arbitrary shell command |
| `tmux` | Tmux control |
| `colcon ...` | colcon passthrough |
| `source <ws>` | Source an overlay |
| `cd` / `pwd` / `ls` | Shell navigation |
| `rqt` | Launch rqt |
| `web` | Start the web dashboard |

### Aliases & meta
| Command | Action |
|---|---|
| `lv` | `launch-verify` |
| `ba` | `bag-analyze` |
| `fleet` | Fleet view |
| `benchmark` | Performance bench |
| `doctor` / `diagnose` | Health checks |
| `matrix` | Distro matrix |
| `trend` | Trends |
| `history` | Show command history |
| `clear` | Clear screen |
| `help` | Command help |
| `quit` | Exit REPL |

---

## 3. Rust TUI — `ros2_info tui` (current state)

A live dashboard shell, not yet feature-parity with the Python REPL.

### Live tabs (6) — backed by a background collector thread

| Tab | Shows |
|---|---|
| Overview | System stats + ROS2 environment at a glance (FastFetch-style) |
| ROS2 | Live nodes/topics/services/actions + node↔topic map + telemetry log |
| Workspace | Colcon tree, build status, recently modified packages |
| Diagnostics | Health-check issues (ROS not sourced, DDS mismatch, broken launch, bad YAML) |
| Trends | Session-scoped historical averages (CPU/mem/nodes/topics) |
| Fleet | Multi-host / multi-robot fleet status |

### Integrated terminal
A real pty-backed terminal (`portable-pty` + `vt100`), not a one-shot shim. Each terminal tab owns an independent PTY session with a `vt100` parser on the main thread, its own writer, and scrollback (see `terminal.rs`). `ros2`, `colcon`, `ros2 launch`, and `ros2 topic echo` run live with streaming output; lines beginning with `ai` are intercepted by the AI session. Multiple sessions are supported.
- Built-ins: `clear`, `help`.

### AI assistant (local Ollama)

Backed by `src/tui/src/ai.rs`; all mutating actions go through the Preview → Apply gate (no silent file writes).

| Input | Action |
|---|---|
| `ai chat <msg>` | Ask Ollama a question about the codebase |
| `ai explain <error>` | Get an explanation + fix for an error message |
| `ai scan` | Scan for build errors only (no fix) |
| `ai fix <path>` | Lint/fix a specific file |
| `ai auto` / `ai solve` | Autonomous loop: scan → AI fix → rebuild → repeat (up to 3 attempts), writes `AUTONOMOUS_REPORT.md` |
| `ai model` / `ai model <name>` | List installed models / set the active model |
| `ai web [port]` | Start the Ollama-backed web chat server |
| `AI: Choose Model` (Command Palette) | Interactive picker for installed Ollama models |
| `AI: Solve (Autonomous Fix)` (Command Palette) | Launch `ai solve` from the palette |

Notes:
- The endpoint defaults to `http://127.0.0.1:11434` and is overridable via the `OLLAMA_HOST` env var (`ai.rs::ollama_base`).
- `DEFAULT_MODEL` is `qwen2.5-coder:7b`; if it isn't installed, `resolve_ollama_model` falls back to an installed model.
- The autonomous worker runs on a background thread wrapped in `catch_unwind`, so a model/parse failure surfaces as an error message rather than freezing the panel.

### Plugins (in-process API, example plugin ships)

The plugin API is **implemented** as an in-process Rust trait surface (`src/tui/src/plugin.rs`). A built-in **example plugin** is registered by default: it contributes an `Example` dashboard tab (rendered live in the Plugins sidebar), an `ai battery` terminal command, and a `FileSaved` event hook. See **`docs/plugins.md`** for the full "how to write a plugin" guide and `README.md → Plugins` for the user-facing overview. The `Plugins` activity bar entry renders the registered plugins and their tabs.

### Key bindings (every one also reachable by mouse where a UI element exists)

| Key | Action |
|---|---|
| `1`–`6` / `Alt+1`–`6` / `Tab` / `BackTab` | Switch tabs |
| `Ctrl+E` | Toggle file explorer |
| `Ctrl+B` | Toggle sidebar |
| `Ctrl+R` | Refresh data |
| `Ctrl+S` / `F6` | Toggle Sandbox / Global (F6 confirms entering Global) |
| `Ctrl+F` | Cycle telemetry log filter |
| `Ctrl+S` (file) | Save current file — *placeholder: "no file open"* |
| `Ctrl+P` | Go to File — fuzzy file open; selecting opens the file in the editor. The same overlay drives the Launch-file and Bag-file pickers |
| `Ctrl+Shift+P` | Command Palette (commands + ROS2 tools, incl. `AI: Choose Model` / `AI: Solve`); selecting a command runs it |
| `` Ctrl+` `` | Focus command bar / terminal |
| `?` | Help overlay |
| `Esc` | Close overlay → exit explorer → quit |
| `Ctrl+C` / `Ctrl+Q` | Quit |
| `↑` / `↓` | Scroll content / file-tree / command history |
| `Enter` | Submit command (or expand selected dir in explorer) |
| `Ctrl+A` / `Home`, `Ctrl+E` / `End` | Cursor to start / end of line |
| `Ctrl+L` | Clear command output |

### Mouse
- Click tabs (hit-test accounts for activity-bar offset + width-adaptive labels).
- Click the activity bar (Explorer · Search · ROS Graph · Diagnostics · Sandbox · Git · Plugins · Settings · Help).
- Scroll to navigate.

### Responsive layout
- Tab labels adapt: full `Overview · ROS2 · Workspace · Diagnostics · Trends · Fleet` at wide terminals; compact `O · R2 · WS · D · Tr · Fl` under ~50 cols.
- Title bar, activity rail, content panels, command bar, and status bar survive minimization down to ~40×12 without clipping or crashes.

---

## 4. Gap — what the Rust TUI does not yet have (verified against source, 2026-07-14)

### What IS built (so the gap discussion below isn't read against an empty screen)
- **Real PTY terminal** — `portable-pty` + `vt100`, multi-session, scrollback, live streaming (`terminal.rs`). `ros2 topic echo` and `ros2 launch` work.
- **Editor** — multi-tab, editable, with save/save-as, undo/redo, find + replace (+ replace-all), yank/paste, Neovim bindings, and a hand-written syntax highlighter (`editor.rs`, `syntax.rs`).
- **Go to File** (`Ctrl+P`) and **Command Palette** (`Ctrl+Shift+P`) — fuzzy overlay driving file-open, commands (incl. `AI: Choose Model` / `AI: Solve`), and the launch-file + bag-file pickers (`palette.rs`, `main.rs`).
- **Activity bar** — all 8 activities (Explorer · Search · ROS Graph · Diagnostics · Sandbox · Git · Plugins · Settings) render and switch views (`ui.rs:715-723`).
- **Git sidebar** — status, diff, commit, log, branches, checkout + `gh` issues/PRs (`git.rs`); rendered in the Git activity sidebar.
- **Python→Rust bridge** — `collector.rs` shells `python3` running inline scripts against `ros2`, `workspace`, `diagnostics`, `trends`, and `graph` collectors (5 of 8 wired).
- **AI** — local Ollama (`ai.rs`), scan/fix/auto/explain/chat/web, diff-gated.
- **Sandbox / Global** — confirmed binary toggle with a Global-entry confirmation.
- **Plugins** — in-process `Plugin` trait API (`plugin.rs`) + `PluginManager` + a built-in example plugin (Example tab, `ai battery`, FileSaved hook); the Plugins sidebar renders registered plugins live.

### What is NOT yet built (the honest remaining gap)
- **Fleet tab** — `build_fleet()` in `collector.rs:173` is a Rust-side stub; the Python `fleet` collector (multi-host SSH discovery) is not yet wired into the TUI.
- **bag_forensics** and **launch_verify** Python collectors — implemented in the Python package but not yet invoked from the Rust TUI.
- **Sandbox diff flow** — only a binary Sandbox/Global toggle exists; the planned Create/Clone/Switch/Export/Import + Preview→Apply export flow is not implemented.
- **Editor depth** — multi-cursor, split view, and breadcrumbs are not implemented (everything else is).
- **Session recorder** (`.cast` / GIF export) — not implemented.
- **Plugins — runtime loader** — the in-process API works (compile-in via `register_builtins`), but no external/FFI loader or manifest discovery yet; plugins are compiled into the binary, not dropped in at runtime.
- **Distribution** — no prebuilt `x86_64`/`aarch64` binaries, no `cargo install`, no cross-compile pipeline published.

Revised honest estimate: the Rust TUI implements the majority of the dashboard/editor/terminal/AI surface; the remaining gap is concentrated in the deep collectors (fleet, bag, launch-verify), the sandbox diff flow, and distribution.
