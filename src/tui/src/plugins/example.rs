//! Reference plugin for `ros2_info`.
//!
//! Demonstrates every extension point of the plugin API: a dashboard tab, a
//! terminal command, and an event hook. Copy this crate to author your own
//! (see `docs/plugins.md`).

use crate::plugin::{AppEvent, DashboardTab, Plugin, PluginAction, TerminalCommand};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::path::PathBuf;

/// The reference plugin. Name is shown in the Plugins sidebar.
pub struct ExamplePlugin;

impl Plugin for ExamplePlugin {
    fn name(&self) -> &str {
        "example"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn on_event(&self, event: &AppEvent) -> Option<PluginAction> {
        match event {
            AppEvent::FileSaved(path) => Some(PluginAction::Notify(format!(
                "example: saved {}",
                path.display()
            ))),
            AppEvent::BuildFailed(errors) => Some(PluginAction::Notify(format!(
                "example: build failed ({} errors)",
                errors.len()
            ))),
            _ => None,
        }
    }

    fn dashboard_tabs(&self) -> Vec<Box<dyn DashboardTab>> {
        vec![Box::new(ExampleTab)]
    }

    fn terminal_commands(&self) -> Vec<Box<dyn TerminalCommand>> {
        vec![Box::new(BatteryCommand)]
    }
}

/// A custom dashboard tab rendered in the Plugins sidebar.
pub struct ExampleTab;

impl DashboardTab for ExampleTab {
    fn title(&self) -> &str {
        "Example"
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let lines = vec![
            Line::from(Span::styled(
                " Example plugin tab",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(" This tab is contributed by a plugin."),
            Line::from(" Render anything with ratatui here:"),
            Line::from("  • sensor monitors"),
            Line::from("  • battery / GPU dashboards"),
            Line::from("  • custom ROS 2 introspection"),
        ];
        frame.render_widget(Paragraph::new(lines), area);
    }
}

/// A terminal command: `ai battery`.
pub struct BatteryCommand;

impl TerminalCommand for BatteryCommand {
    fn name(&self) -> &str {
        "battery"
    }

    fn run(&self, args: &str) -> String {
        let path = PathBuf::from("/sys/class/power_supply/BAT0/capacity");
        match std::fs::read_to_string(&path) {
            Ok(cap) => format!("Battery: {}% ({})", cap.trim(), args.trim()),
            Err(_) => "Battery: no power supply found".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_metadata_is_stable() {
        let p = ExamplePlugin;
        assert_eq!(p.name(), "example");
        assert!(!p.version().is_empty());
        assert_eq!(p.dashboard_tabs().len(), 1);
        assert_eq!(p.terminal_commands().len(), 1);
        assert_eq!(p.terminal_commands()[0].name(), "battery");
    }
}
