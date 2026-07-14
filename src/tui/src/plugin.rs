//! Plugin system for the `ros2_info` TUI.
//!
//! Plugins extend the TUI without forking the core crate. They can add custom
//! dashboard tabs, register new terminal / AI commands, react to lifecycle
//! events, and surface extra data in the dashboard.
//!
//! This is an in-process plugin API: a plugin is a Rust type that implements
//! [`Plugin`] and is registered into the [`PluginManager`] at startup. See
//! `docs/plugins.md` for the full "how to write a plugin" guide and
//! `src/tui/src/plugins/example.rs` for a working reference plugin.

use std::path::PathBuf;

use ratatui::layout::Rect;
use ratatui::Frame;

/// Lifecycle events delivered to every registered plugin.
#[derive(Debug, Clone)]
#[allow(dead_code)] // some variants are emitted by future host integrations
pub enum AppEvent {
    /// An `ai auto` / `cargo build` failed. Contains the error lines.
    BuildFailed(Vec<String>),
    /// The editor saved a file.
    FileSaved(PathBuf),
    /// A push was issued from the Git sidebar.
    GitPush,
    /// A new topic message arrived in the telemetry stream.
    TopicReceived(String),
}

/// What a plugin may ask the host to do in response to an event.
#[derive(Debug, Clone)]
#[allow(dead_code)] // some variants are consumed by future host integrations
pub enum PluginAction {
    /// Surface a toast / status message.
    Notify(String),
    /// Focus / switch to a dashboard tab by title.
    OpenTab(String),
    /// Enqueue a terminal command (e.g. `ros2 topic echo /scan`).
    RunCommand(String),
}

/// A custom dashboard tab. Rendered with `ratatui` exactly like the core tabs.
pub trait DashboardTab: Send + Sync {
    /// Title shown in the tab strip / plugins sidebar.
    fn title(&self) -> &str;
    /// Draw the tab content into `area`.
    fn render(&self, frame: &mut Frame, area: Rect);
}

/// A terminal / AI command contributed by a plugin (e.g. `ai mytool ...`).
pub trait TerminalCommand: Send + Sync {
    /// Command name (without a prefix). Invoked as `ai <name> [args]`.
    fn name(&self) -> &str;
    /// Execute the command. The returned string is printed to the terminal.
    fn run(&self, args: &str) -> String;
}

/// The trait every plugin implements.
pub trait Plugin: Send + Sync {
    /// Unique plugin name (kebab-case).
    fn name(&self) -> &str;
    /// Semver-ish version string.
    fn version(&self) -> &str;

    /// React to a lifecycle event. Return `None` to do nothing.
    fn on_event(&self, _event: &AppEvent) -> Option<PluginAction> {
        None
    }

    /// Dashboard tabs contributed by this plugin.
    fn dashboard_tabs(&self) -> Vec<Box<dyn DashboardTab>> {
        Vec::new()
    }

    /// Terminal / AI commands contributed by this plugin.
    fn terminal_commands(&self) -> Vec<Box<dyn TerminalCommand>> {
        Vec::new()
    }
}

/// Owns every registered plugin and fans events / lookups out to them.
pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginManager {
    /// Build the manager and register all built-in plugins.
    pub fn new() -> Self {
        let mut mgr = Self {
            plugins: Vec::new(),
        };
        crate::plugins::register_builtins(&mut mgr);
        mgr
    }

    /// Register an additional plugin (used by built-ins and future loaders).
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    /// `(name, version)` for every registered plugin.
    pub fn list(&self) -> Vec<(String, String)> {
        self.plugins
            .iter()
            .map(|p| (p.name().to_string(), p.version().to_string()))
            .collect()
    }

    /// Number of registered plugins.
    pub fn count(&self) -> usize {
        self.plugins.len()
    }

    /// Fan an event out to every plugin, collecting requested actions.
    pub fn dispatch(&self, event: &AppEvent) -> Vec<PluginAction> {
        let mut actions = Vec::new();
        for p in &self.plugins {
            if let Some(a) = p.on_event(event) {
                actions.push(a);
            }
        }
        actions
    }

    /// Every dashboard tab contributed by all plugins.
    pub fn all_tabs(&self) -> Vec<Box<dyn DashboardTab>> {
        let mut tabs = Vec::new();
        for p in &self.plugins {
            tabs.extend(p.dashboard_tabs());
        }
        tabs
    }

    /// Look up and run a contributed terminal command by name.
    pub fn run_command(&self, name: &str, args: &str) -> Option<String> {
        for p in &self.plugins {
            for cmd in p.terminal_commands() {
                if cmd.name() == name {
                    return Some(cmd.run(args));
                }
            }
        }
        None
    }
}
