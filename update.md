# ROS2 Info v2.0.0 — Update Log

**Date:** 2026-04-12  
**Version:** 2.0.0  
**Install:** `uv pip install -e src/ros2_fastfetch/ click flask`  
**Run:** `.venv/bin/ros2_info <command>`

---

## Summary

This update transforms `ros2_info` from a static display tool into a **full interactive ROS2 developer workstation** with REPL terminal, RQT-style graph, new CLI subcommands, new themes, and a web dashboard with built-in terminal + graph panel.

---

## NEW: `fetch_info/terminal.py`

### What was added
A complete interactive terminal REPL (`run_interactive_terminal()`) and ASCII graph renderer (`render_ascii_graph()`).

### Terminal commands
| Command | What it does |
|---|---|
| `nodes` / `topics` / `services` / `actions` | Live discovery lists |
| `env` | ROS2 environment variables |
| `node info <name>` | Pub/sub/service detail (syntax-highlighted YAML) |
| `echo <topic> [--once]` | Stream topic messages |
| `hz <topic>` / `bw <topic>` | Rate and bandwidth measurement |
| `pub <topic> <type> <yaml>` | Publish messages |
| `service call <srv> <type> [yaml]` | Call a service |
| `param get/set/list <node> ...` | Parameter operations |
| `bag record/play/info ...` | Rosbag operations |
| `launch <pkg> <file> [args]` | Launch files |
| `run <pkg> <exe> [args]` | Run node executables |
| `graph [timeout]` | ASCII RQT-like node/topic graph |
| `watch [interval]` | Live-refresh node list |
| `ping <node>` | Check node reachability |
| `interface show <type>` | Show message definitions |
| `shell <cmd>` | Arbitrary shell command |
| `history` / `clear` / `help` / `quit` | Utility commands |

### Why
The old interactive mode was just a numbered menu. Developers need a real shell-like REPL to inspect and manipulate ROS2 without leaving the tool.

### How
- `readline` for history (persisted to `~/.ros2_info_history`, 500 entries), line editing, Tab autocomplete
- `ROS2Completer` class: fetches live topics+nodes every 5s for completions
- Streaming commands use `subprocess.Popen` line-by-line; `Ctrl+C` cleanly kills child
- `render_ascii_graph()` calls `ros2 node info` on each node to build the pub→topic→sub map, then renders as Rich table + arrow diagram

---

## UPDATED: `fetch_info/cli.py`

### New subcommands (+10)
| Command | Description |
|---|---|
| `ros2_info terminal` | Full REPL interactive terminal |
| `ros2_info graph` | ASCII RQT-like graph |
| `ros2_info services` | List active services |
| `ros2_info actions` | List active actions |
| `ros2_info bag record\|play\|info` | Rosbag operations |
| `ros2_info param get\|set\|list` | Parameter operations |
| `ros2_info launch <pkg> <file>` | Launch files |
| `ros2_info run <pkg> <exe>` | Run node executables |
| `ros2_info interface <type>` | Show message definitions |
| `ros2_info nodes --info` | Show pub/sub detail per node |

### Interactive menu updated
Now 11 entries; option 7 = RQT Graph, option 8 = Interactive Terminal, W = Web UI. Case-insensitive input.

---

## UPDATED: `fetch_info/display/themes.py`

Added two new themes:

| Theme | Colors | Feel |
|---|---|---|
| `neon` | Pink `#F0ABFC` + lime `#A3E635` | Cyberpunk dark club |
| `solar` | Amber `#FCD34D` + gold `#F59E0B` | Warm solar flare |

Total: 7 themes. Preview with `ros2_info themes`.

---

## UPDATED: `fetch_info/web.py`

### New API endpoints

| Endpoint | What it does |
|---|---|
| `GET /api/graph` | Returns JSON node/topic connection map |
| `POST /api/exec {cmd}` | Safely runs a read-only ROS2 command |

`/api/exec` allowlist: `nodes`, `topics`, `services`, `actions`, `env`, `node info`, `param list`, `bag info`, `interface show`. All write/streaming commands rejected.

---

## UPDATED: `fetch_info/templates/index.html`

### Graph Panel — `📊 Node / Topic Graph`
- Full-width SVG panel; fetches `/api/graph`
- Pure-JS 60-iteration spring force layout (no D3 — works offline on robots)
- Blue = nodes, amber = topics, arrows = publish/subscribe direction
- "Rebuild Graph" button; auto-loads 2s after page open

### Web Terminal Panel — `🖥 Web Terminal`
- Green-on-black monospace output, auto-scroll
- Arrow Up/Down history, quick-chip buttons (nodes/topics/services/actions/env)
- Posts to `/api/exec`, shows result inline
- Directs users to `ros2_info terminal` for streaming commands

---

## Setup (uv)

```bash
uv venv .venv
uv pip install -e src/ros2_fastfetch/ click flask
source .venv/bin/activate

ros2_info --help
ros2_info terminal        # full REPL
ros2_info graph           # ASCII graph
ros2_info -i              # interactive menu
ros2_info web             # web UI → http://localhost:8099
```

---

## Files Changed

| File | Status |
|---|---|
| `fetch_info/terminal.py` | NEW — REPL + graph engine |
| `fetch_info/cli.py` | UPDATED — +10 subcommands |
| `fetch_info/display/themes.py` | UPDATED — neon + solar themes |
| `fetch_info/web.py` | UPDATED — graph + exec API |
| `fetch_info/templates/index.html` | UPDATED — graph panel + web terminal |
| `update.md` | NEW — this file |

## Nothing Removed
All original commands (`--live`, `--watch`, `--json`, `nodes`, `topics`, `packages`, `workspace`, `env`, `themes`, `web`, `-i`) are fully preserved.

## 2.1 UI Hotfix: Fastfetch & Terminal Fixes
- **Fastfetch Layout:** `ros2_info --info` and the terminal `info` command now use a strict two-column fastfetch/neofetch style with aligned colons and robust ASCII art.
- **Terminal Fixes:** Replaced `console.clear()` with built-in `os.system('clear')` for native terminal rendering.
- **Tmux Fixes:** Tmux `attach-session` now uses `os.system()` to completely yield the terminal processes properly, preventing input hangings with readline.

## 2.3 Web Dashboard & Hardware Stats Update
- **Live Navbar Stats:** Added real-time CPU, Memory, and Battery indicators to the web dashboard top bar (polls via new `/api/status` endpoint).
- **Auto-Executing Terminal:** Dashboard terminal chips (nodes/topics/etc) now execute immediately on click.
- **Improved Fastfetch:** Terminal snapshot now includes CPU model, GPU, Memory/Disk usage percentages, and hardware sensor temperatures.
- **Robustness Fixes:** Reduced `/api/exec` timeout to 5s to prevent hangs when the ROS2 daemon is unresponsive.
- **UX Polish:** Added `Ctrl+L` clear shortcut to web terminal and `/` keyboard shortcut to focus input.
- **Package Integrity:** Updated `package.xml` with complete dependencies (`flask`, `climage`, `Pillow`) and updated maintainer info.

---

## 2026-05-11 Script Cleanup
- Moved the asset generator to [scripts/generate_climage_assets.py](scripts/generate_climage_assets.py).
- Added usage notes in [scripts/README.md](scripts/README.md).
