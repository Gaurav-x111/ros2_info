#![allow(dead_code)]
//! Color palette + shared style helpers for the ros2_info TUI.
//!
//! Palette is VS Code Dark+/Dracula-adjacent, matching the reference image.

use ratatui::style::{Color, Modifier, Style};

// ── Surfaces ────────────────────────────────────────────────────────
pub const BG: Color = Color::Rgb(30, 30, 46); // #1e1e2e main background
pub const SURFACE: Color = Color::Rgb(40, 42, 54); // #282a36 panel surface
pub const SURFACE_HI: Color = Color::Rgb(52, 55, 70); // #343746 active/selected surface
pub const SELECT: Color = Color::Rgb(68, 71, 90); // #44475a selection
pub const BORDER: Color = Color::Rgb(68, 71, 90); // #44475a inactive border
pub const HEADER_BG: Color = Color::Rgb(25, 25, 40); // title/status background

// ── Foreground ──────────────────────────────────────────────────────
pub const FG: Color = Color::Rgb(248, 248, 242); // #f8f8f2 foreground
pub const DIM: Color = Color::Rgb(98, 114, 164); // #6272a4 dim
pub const ACCENT: Color = Color::Rgb(139, 233, 253); // #8be9fd cyan accent
pub const ACCENT_WARM: Color = Color::Rgb(255, 184, 108); // #ffb86c orange

// ── Semantic ────────────────────────────────────────────────────────
pub const OK: Color = Color::Rgb(80, 250, 123); // #50fa7b green
pub const WARN: Color = Color::Rgb(241, 250, 140); // #f1fa8c yellow
pub const WARN_AMBER: Color = Color::Rgb(240, 165, 0); // #f0a500 amber (modified)
pub const ERROR: Color = Color::Rgb(255, 85, 85); // #ff5555 red
pub const INFO: Color = Color::Rgb(98, 114, 164); // #6272a4 blue
pub const SANDBOX: Color = Color::Rgb(240, 165, 0); // #f0a500 amber
pub const GLOBAL: Color = Color::Rgb(231, 76, 60); // #e74c3c red-orange
pub const MAGENTA: Color = Color::Rgb(189, 147, 249); // #bd93f9 sandbox accent

pub fn surface_style() -> Style {
    Style::default().bg(BG).fg(FG)
}

pub fn dim(s: Style) -> Style {
    s.fg(DIM)
}

pub fn bold(s: Style) -> Style {
    s.add_modifier(Modifier::BOLD)
}
