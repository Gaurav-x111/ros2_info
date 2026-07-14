#![allow(dead_code)]
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::Frame;
use std::collections::VecDeque;

const ACCENT: Color = Color::Rgb(100, 180, 255);
const DIM: Color = Color::Rgb(120, 120, 140);
const OK: Color = Color::Rgb(80, 220, 100);
const WARN: Color = Color::Rgb(255, 200, 50);
const ERROR: Color = Color::Rgb(255, 80, 80);
const SURFACE: Color = Color::Rgb(35, 35, 55);

#[derive(Clone, PartialEq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl LogLevel {
    pub(crate) fn color(&self) -> Color {
        match self {
            LogLevel::Debug => DIM,
            LogLevel::Info => OK,
            LogLevel::Warn => WARN,
            LogLevel::Error => ERROR,
            LogLevel::Fatal => ERROR,
        }
    }

    pub(crate) fn label(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO ",
            LogLevel::Warn => "WARN ",
            LogLevel::Error => "ERROR",
            LogLevel::Fatal => "FATAL",
        }
    }
}

#[derive(Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub node: String,
    pub level: LogLevel,
    pub message: String,
}

/// ROS 2 telemetry log buffer
pub struct TelemetryLog {
    /// Ring buffer of log entries
    pub entries: VecDeque<LogEntry>,
    /// Maximum buffer size
    pub max_size: usize,
    /// Severity filter (None = show all)
    pub filter: Option<LogLevel>,
    /// Scroll offset
    pub scroll_offset: usize,
}

impl TelemetryLog {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_size),
            max_size,
            filter: None,
            scroll_offset: 0,
        }
    }

    pub fn add(&mut self, entry: LogEntry) {
        if self.entries.len() >= self.max_size {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    pub fn set_filter(&mut self, level: Option<LogLevel>) {
        self.filter = level;
        self.scroll_offset = 0;
    }

    fn filtered_entries(&self) -> Vec<&LogEntry> {
        match self.filter {
            None => self.entries.iter().collect(),
            Some(ref filter_level) => {
                // Show only entries at or above the filter level
                let threshold = match filter_level {
                    LogLevel::Debug => 0,
                    LogLevel::Info => 1,
                    LogLevel::Warn => 2,
                    LogLevel::Error => 3,
                    LogLevel::Fatal => 4,
                };
                self.entries
                    .iter()
                    .filter(|e| {
                        let lvl = match e.level {
                            LogLevel::Debug => 0,
                            LogLevel::Info => 1,
                            LogLevel::Warn => 2,
                            LogLevel::Error => 3,
                            LogLevel::Fatal => 4,
                        };
                        lvl >= threshold
                    })
                    .collect()
            }
        }
    }
}

/// Render the telemetry log panel
pub fn render_telemetry(frame: &mut Frame, area: Rect, log: &TelemetryLog) {
    let title = match &log.filter {
        Some(level) => format!(" ROS2 Telemetry [{}] ", level.label()),
        None => " ROS2 Telemetry [ALL] ".to_string(),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(DIM))
        .title(title)
        .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let entries = log.filtered_entries();

    if entries.is_empty() {
        let para = ratatui::widgets::Paragraph::new(
            "  No log entries. ROS2 nodes will appear here when running.",
        )
        .style(Style::default().fg(DIM).bg(SURFACE));
        frame.render_widget(para, inner);
        return;
    }

    let items: Vec<ListItem> = entries
        .iter()
        .rev()
        .skip(log.scroll_offset)
        .take(inner.height as usize)
        .map(|entry| {
            ListItem::new(vec![Line::from(vec![
                Span::styled(format!(" {} ", entry.timestamp), Style::default().fg(DIM)),
                Span::styled(
                    format!("[{}] ", entry.level.label()),
                    Style::default()
                        .fg(entry.level.color())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{:<20} ", entry.node), Style::default().fg(ACCENT)),
                Span::styled(&entry.message, Style::default().fg(entry.level.color())),
            ])])
        })
        .collect();

    let list = List::new(items).highlight_style(Style::default().bg(SURFACE));

    frame.render_widget(list, inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_buffer_cap() {
        let mut log = TelemetryLog::new(3);
        log.add(LogEntry {
            timestamp: "t".into(),
            node: "n".into(),
            level: LogLevel::Info,
            message: "1".into(),
        });
        log.add(LogEntry {
            timestamp: "t".into(),
            node: "n".into(),
            level: LogLevel::Info,
            message: "2".into(),
        });
        log.add(LogEntry {
            timestamp: "t".into(),
            node: "n".into(),
            level: LogLevel::Info,
            message: "3".into(),
        });
        log.add(LogEntry {
            timestamp: "t".into(),
            node: "n".into(),
            level: LogLevel::Info,
            message: "4".into(),
        });
        assert_eq!(log.entries.len(), 3);
        assert_eq!(log.entries.front().unwrap().message, "2");
    }

    #[test]
    fn test_log_filter() {
        let mut log = TelemetryLog::new(10);
        log.add(LogEntry {
            timestamp: "t".into(),
            node: "n".into(),
            level: LogLevel::Debug,
            message: "dbg".into(),
        });
        log.add(LogEntry {
            timestamp: "t".into(),
            node: "n".into(),
            level: LogLevel::Error,
            message: "err".into(),
        });
        log.set_filter(Some(LogLevel::Warn));
        let filtered = log.filtered_entries();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message, "err");
    }
}
