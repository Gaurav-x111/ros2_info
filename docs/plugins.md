# Writing a Plugin for `ros2_info` (Rust TUI)

> **Status: implemented (in-process).** The plugin *API* described here is live
> in `src/tui/src/plugin.rs` and wired into the running TUI via `PluginManager`
> (built-ins are registered in `src/tui/src/plugins/mod.rs::register_builtins`).
> A **reference example plugin** (`src/tui/src/plugins/example.rs`) ships with
> the TUI and is active by default — it contributes an `Example` dashboard tab,
> an `ai battery` terminal command, and a `FileSaved` toast. The next step is an
> external/FFI loader; today plugins are compiled into the binary. Open an issue
> to discuss the API before investing in a large plugin.

A *plugin* extends the TUI without forking the core crate. Plugins can:

- add **custom dashboard tabs** (sensor monitors, hardware dashboards, …),
- register **new terminal / AI commands** (`ai …`-style),
- react to **lifecycle events** (file saved, build failed, git push),
- feed **extra collectors** into the dashboard (battery, GPU, custom ROS 2 introspection).

Plugins **cannot**: mutate editor content without consent, reach the network
without permission, bypass the Preview → Apply gate, or read another plugin's
state.

---

## 1. Prerequisites

- Rust ≥ 1.77 and the normal TUI build toolchain (see `CONTRIBUTING.md`).
- A checkout of this repo (the plugin is developed against `src/tui`).
- Familiarity with `ratatui` (the dashboard tab is a `ratatui` widget).

## 2. Create the plugin

There are two ways to ship a plugin:

1. **In-tree (recommended / what ships today).** Add a module under
   `src/tui/src/plugins/<name>/` and register it in `register_builtins` (see
   §6). No extra `Cargo.toml`; it compiles with the TUI.
2. **Fork / external crate (future loader).** A separate crate that depends on
   `ros2_info_tui` and is wired in once the runtime loader lands.

For an in-tree plugin, create `src/tui/src/plugins/my_plugin.rs` (or a
subdirectory `my_plugin/mod.rs`):

```rust
// src/tui/src/plugins/my_plugin.rs
use ros2_info_tui::plugin::{Plugin, AppEvent, PluginAction};

pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn name(&self) -> &str { "my-plugin" }
    fn version(&self) -> &str { "0.1.0" }
    // ...see §3
}
```

> For an external crate, declare it as a `cdylib` + `rlib` and depend on
> `ros2_info_tui = { path = ".." }` with `ratatui = "0.29"`. The path form
> assumes the crate lives beside `src/tui`; adjust for your layout.

## 3. Implement the `Plugin` trait

```rust
// my_plugin/src/lib.rs
use ros2_info_tui::plugin::{Plugin, AppEvent, PluginAction};

pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn name(&self) -> &str { "my-dashboard" }
    fn version(&self) -> &str { "0.1.0" }

    fn on_event(&self, event: &AppEvent) -> Option<PluginAction> {
        match event {
            AppEvent::BuildFailed(errors) => Some(PluginAction::Notify(format!(
                "Build failed: {} errors",
                errors.len()
            ))),
            AppEvent::FileSaved(path) => {
                // e.g. re-run a linter, push a metric, etc.
                log::info!("saved {:?}", path);
                None
            }
            _ => None,
        }
    }

    fn dashboard_tabs(&self) -> Vec<Box<dyn DashboardTab>> {
        vec![Box::new(MyTab)]
    }

    fn terminal_commands(&self) -> Vec<Box<dyn TerminalCommand>> {
        vec![Box::new(MyCommand)]
    }
}
```

### Dashboard tab

```rust
use ros2_info_tui::plugin::DashboardTab;
use ratatui::Frame;

pub struct MyTab;

impl DashboardTab for MyTab {
    fn title(&self) -> &str { "My Tab" }
    fn render(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        // draw with ratatui widgets, same as core tabs
    }
}
```

### Terminal / AI command

```rust
use ros2_info_tui::plugin::TerminalCommand;

pub struct MyCommand;

impl TerminalCommand for MyCommand {
    fn name(&self) -> &str { "mytool" }
    fn run(&self, args: &str) -> String {
        format!("my_tool handled: {}", args)
    }
}
```

## 4. Plugin API reference

```rust
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn on_event(&self, event: &AppEvent) -> Option<PluginAction>;
    fn dashboard_tabs(&self) -> Vec<Box<dyn DashboardTab>>;
    fn terminal_commands(&self) -> Vec<Box<dyn TerminalCommand>>;
}
```

### `AppEvent` (lifecycle hooks)

| Variant | Fired when |
|---|---|
| `AppEvent::BuildFailed(Vec<String>)` | an `ai auto` / cargo build fails |
| `AppEvent::FileSaved(PathBuf)` | the editor saves a file |
| `AppEvent::GitPush` | a push is issued from the Git sidebar |
| `AppEvent::TopicReceived(String)` | a new topic message arrives (telemetry) |

### `PluginAction` (what a plugin may request)

| Variant | Effect |
|---|---|
| `PluginAction::Notify(String)` | surface a toast/status message |
| `PluginAction::OpenTab(String)` | focus/switch to a dashboard tab |
| `PluginAction::RunCommand(String)` | enqueue a terminal command |

> The exact variant set is **not yet frozen** — see `src/tui/src/plugin.rs`
> (once added) for the authoritative definition.

## 5. Build & test

```bash
cd src/tui
cargo build                        # builds the TUI + all in-tree plugins
cargo clippy --all-targets         # CI gate (-D warnings)
```

Because plugins are compiled into the binary via `register_builtins`, "building
the plugin" is just `cargo build` of the TUI. Validate the trait impl compiles
against the `ros2_info_tui::plugin` surface and add a unit test that constructs
your plugin and asserts `name()`/`version()`.

## 6. Register / contribute

1. Open an issue describing the plugin and its `AppEvent`/`PluginAction` needs.
2. Add the module under `src/tui/src/plugins/<name>/` (or keep it in your fork).
3. Register it with one line in `src/tui/src/plugins/mod.rs::register_builtins`:
   `mgr.register(Box::new(my_plugin::MyPlugin));`.
4. Add a unit test that constructs the plugin and asserts `name()`/`version()`.
5. Open a PR; CI builds the TUI **and** every in-tree plugin.

See `CONTRIBUTING.md` for the full PR process and `features.md` for the current
plugin support status.
