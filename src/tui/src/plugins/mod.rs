//! Built-in plugins shipped with the TUI.
//!
//! To register a new built-in plugin, add a `pub mod` below and push an
//! instance of it from [`register_builtins`]. External / third-party plugins
//! are documented in `docs/plugins.md` and can be wired in here once a loader
//! lands.

pub mod example;

use crate::plugin::PluginManager;

/// Register every built-in plugin into `mgr`.
pub fn register_builtins(mgr: &mut PluginManager) {
    mgr.register(Box::new(example::ExamplePlugin));
}
