//! cc@zang aka Gaurav-x111

use crate::app::*;
use crate::editor::{ChangeKind, EditMode, SymbolKind};
use crate::syntax;
use crate::terminal::map_color;
use crate::theme::*;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    canvas::{Canvas, Line as CLine, Rectangle},
    Block, BorderType, Borders, Cell, Clear, List, Paragraph, Row, Table, Tabs, Wrap,
};
use ratatui::Frame;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Terminal display width of a string (2 for emoji, 1 for ASCII).
fn dw(s: &str) -> u16 {
    s.width() as u16
}

pub const ACTIVITY_W: u16 = 4;
#[allow(dead_code)]
pub const SIDEBAR_W: u16 = 28;
pub const RIGHT_ITEMS_W: u16 = 42;

// ── Refined semantic tokens (laid over the shared theme.rs palette) ──
// The shared palette in theme.rs treats INFO == DIM (both #6272a4), so an
// "info" signal reads as merely muted. Give it a real blue here, plus a few
// surfaces the theme doesn't name, so the UI speaks in semantic tokens.
#[allow(dead_code)]
const INFO_BLUE: Color = Color::Rgb(98, 158, 235); // #629eeb — real info blue
#[allow(dead_code)]
const CURRENT_LINE: Color = Color::Rgb(36, 38, 54); // subtle current-line tint
#[allow(dead_code)]
const GUTTER_BG: Color = Color::Rgb(26, 26, 40); // editor line-number gutter
#[allow(dead_code)]
const MATCH_HI: Color = Color::Rgb(80, 78, 110); // find-match highlight
#[allow(dead_code)]
const BAR_TRACK: Color = Color::Rgb(45, 47, 64); // status meter background
#[allow(dead_code)]
const BAR_OK: Color = Color::Rgb(80, 250, 123); // green meter fill
#[allow(dead_code)]
const BAR_WARN: Color = Color::Rgb(241, 196, 80); // amber meter fill
#[allow(dead_code)]
const BAR_ERR: Color = Color::Rgb(255, 110, 110); // red meter fill

const MIN_W: u16 = 70;
const MIN_H: u16 = 20;

/// Build a two-tone `█`/`░` mini-meter of `width` cells, coloured green→amber→
/// red by the percentage. Returns the two spans (fill, track) so callers slot
/// them into one larger `Line`. Monochrome-safe: the fill cells and the track
/// read differently even with colour stripped.
#[allow(dead_code)]
fn meter(pct: f32, width: u16) -> (Span<'static>, Span<'static>) {
    let w = (width as usize).max(4);
    let cells = ((pct.clamp(0.0, 100.0) / 100.0) * w as f32).round() as usize;
    let cells = cells.min(w);
    let color = if pct >= 90.0 {
        BAR_ERR
    } else if pct >= 70.0 {
        BAR_WARN
    } else {
        BAR_OK
    };
    (
        Span::styled(
            "█".repeat(cells),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled("░".repeat(w - cells), Style::default().fg(BAR_TRACK)),
    )
}

// ── Top-level draw ──────────────────────────────────────────────────

pub fn draw(frame: &mut Frame, app: &mut App) {
    // Rebuild the unified hit-test map from scratch each frame.
    app.hit_regions.clear();

    let area = frame.area();

    // Overlays take precedence regardless of size.
    if app.help_visible {
        draw_help_overlay(frame, area, app);
        return;
    }
    if app.confirm.is_some() {
        draw_confirm_overlay(frame, area, app);
        return;
    }

    // Floor guard — below the minimum size the multi-pane layout would
    // collide into garbage. Render a single clean message instead.
    if area.width < MIN_W || area.height < MIN_H {
        draw_too_small(frame, area, app);
        if app.palette_open {
            draw_command_palette(frame, area, app);
        }
        if app.ctx_menu.is_some() {
            draw_ctx_menu(frame, area, app);
        }
        if app.prompt.is_some() {
            draw_prompt(frame, area, app);
        }
        return;
    }

    let terminal_h = if app.terminal_visible {
        app.terminal_height
    } else {
        0
    };

    let vert = Layout::vertical([
        Constraint::Length(1), // title
        Constraint::Length(1), // tabs
        Constraint::Min(1),    // body
        Constraint::Length(terminal_h),
        Constraint::Length(1), // status
        Constraint::Length(1), // keybind hints
    ]);
    let [title, tabs, body, term, status, keybind] = vert.areas(area);

    draw_title_bar(frame, title, app);
    draw_tab_bar(frame, tabs, app);
    draw_body(frame, body, app);
    if app.terminal_visible {
        draw_terminal_panel(frame, term, app);
    }
    draw_status_bar(frame, status, app);
    draw_keybind_bar(frame, keybind, app);

    if app.palette_open {
        draw_command_palette(frame, area, app);
    }
    if app.ctx_menu.is_some() {
        draw_ctx_menu(frame, area, app);
    }
    if app.prompt.is_some() {
        draw_prompt(frame, area, app);
    }
    if app.model_picker_open {
        draw_model_picker(frame, area, app);
    }
}

fn draw_command_palette(frame: &mut Frame, area: Rect, app: &App) {
    let max_w = area.width.saturating_sub(8).min(70);
    let max_h = area.height.saturating_sub(6).min(20);
    if max_w < 30 || max_h < 5 {
        return;
    }
    let w = max_w;
    let box_h = max_h;
    let x = (area.width.saturating_sub(w)) / 2;
    let y = area.height.saturating_sub(box_h) / 3; // sit in the upper third
    let box_rect = Rect::new(x, y, w, box_h);

    // Dim the background.
    frame.render_widget(ratatui::widgets::Clear, Rect::new(x, y, w, box_h));

    let title = match app.palette_mode {
        PaletteMode::Command => "Command Palette",
        PaletteMode::File => "Go to File",
        PaletteMode::Launch => "Launch File",
        PaletteMode::Bag => "Bag Play",
    };
    let block = modal_block(title, ACCENT);
    let inner = block.inner(box_rect);
    frame.render_widget(block, box_rect);
    if inner.height == 0 || inner.width == 0 {
        return;
    }

    // Input line (row 0 of inner).
    let input_row = Rect::new(inner.x, inner.y, inner.width, 1);
    let input_line = Line::from(vec![
        Span::styled(
            " › ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(&app.palette_query, Style::default().fg(FG)),
        Span::styled("▌", Style::default().fg(ACCENT)),
    ]);
    frame.render_widget(
        Paragraph::new(input_line).style(Style::default().bg(BG)),
        input_row,
    );

    // Separator
    let sep = Rect::new(inner.x, inner.y + 1, inner.width, 1);
    frame.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::default().fg(BORDER)),
        sep,
    );

    // Results list.
    let list_area = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(2),
    );
    let is_file = !matches!(app.palette_mode, PaletteMode::Command);
    let (results, _): (Vec<(String, String)>, bool) = if is_file {
        let cands = crate::palette::file_candidates(&app.palette_files());
        let mut filtered = crate::palette::filter_files(&cands, &app.palette_query);
        match app.palette_mode {
            PaletteMode::Launch => filtered.retain(|(_, (_, p))| crate::palette::is_launch_file(p)),
            PaletteMode::Bag => filtered.retain(|(_, (_, p))| crate::palette::is_bag_file(p)),
            _ => {}
        }
        (
            filtered
                .into_iter()
                .take(list_area.height as usize)
                .map(|(_, (name, path))| {
                    let dir = path
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();
                    (name, dir)
                })
                .collect(),
            true,
        )
    } else {
        let filtered = crate::palette::filter_commands(&app.palette_query);
        (
            filtered
                .into_iter()
                .take(list_area.height as usize)
                .map(|(_, item)| (item.title, item.category.to_string()))
                .collect(),
            false,
        )
    };

    let mut spans: Vec<Line> = Vec::new();
    for (i, (title, sub)) in results.iter().enumerate() {
        let active = i == app.palette_sel;
        let base = if active {
            Style::default()
                .fg(BG)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(FG).bg(BG)
        };
        let label = if is_file {
            format!(" {:<28} {}", title, sub)
        } else {
            format!(" {:<32} {}", title, sub)
        };
        spans.push(Line::from(Span::styled(label, base)));
    }
    frame.render_widget(
        Paragraph::new(spans).style(Style::default().bg(BG)),
        list_area,
    );
}

// ── Title bar ──────────────────────────────────────────────────────

fn draw_title_bar(frame: &mut Frame, area: Rect, app: &App) {
    let ws = app
        .workspace
        .as_ref()
        .and_then(|w| w.workspaces.first().cloned())
        .unwrap_or_else(|| "/".to_string());

    // Determine ROS2 connection status and display values
    let (ros2_status, distro, dds, status_color) = match &app.ros2 {
        Some(r) if r.error.is_some() => ("Error", r.distro.as_str(), r.dds.as_str(), ERROR),
        Some(r) if !r.distro.is_empty() => ("Connected", r.distro.as_str(), r.dds.as_str(), OK),
        Some(_) => ("Connecting...", "", "", WARN),
        None => ("No data", "", "", DIM),
    };

    let distro_display = if distro.is_empty() { "-" } else { distro };
    let dds_display = if dds.is_empty() { "-" } else { dds };

    let battery = app
        .system
        .as_ref()
        .and_then(|s| s.battery_percent)
        .map(|b| format!("{}%", b as i32))
        .unwrap_or_else(|| "100%".to_string());
    let clock = chrono::Local::now().format("%H:%M");
    let clock = clock.to_string();

    frame.render_widget(
        Paragraph::new(Line::from("")).style(Style::default().bg(HEADER_BG)),
        area,
    );

    // Priority when width is tight: keep the sigil + wordmark (identity),
    // then the ROS status (connection), then clock; workspace + Mode are
    // the variable left-side content and drop first. Right shrinks its
    // fields battery → DDS → distro; left drops Mode: before the WS path.
    let chrome: Vec<Span> = vec![
        Span::styled(
            format!(" {} ", glyph("#", "\u{26A1}", "\u{f0e7}")),
            Style::default().fg(WARN).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "ROS2_INFO",
            Style::default().fg(FG).add_modifier(Modifier::BOLD),
        ),
    ];
    let chrome_w: u16 = chrome.iter().map(|s| s.width()).sum::<usize>() as u16;
    let mode_label = app.sandbox_label().to_string();
    let ws_lbl = Span::styled("   Workspace: ", Style::default().fg(DIM));
    let mode_lbl = Span::styled("   Mode: ", Style::default().fg(DIM));
    let mode_span = Span::styled(
        mode_label.clone(),
        Style::default()
            .fg(app.mode_color())
            .add_modifier(Modifier::BOLD),
    );
    let tail_pad = Span::styled(" ", Style::default().fg(FG));
    // The fixed tail = "   Workspace: " placeholder-none + "   Mode: SANDBOX "
    // — but workspace is variable; tail-fixed = label chrome only (ws_lbl+mode_lbl+mode+pad).
    let tail_fixed_w =
        (ws_lbl.width() + mode_lbl.width() + mode_span.width() + tail_pad.width()) as u16;

    let right_natural =
        title_right_full_w(ros2_status, distro_display, dds_display, &battery, &clock);
    // Reserve up to half the row for the right block but never starve the
    // app identity (sigil + wordmark + a 3-cell "… " workspace stub).
    let left_min = chrome_w + 6;
    let rw = right_natural
        .min(area.width.saturating_sub(left_min))
        .min(area.width);
    let lw = area.width.saturating_sub(rw);

    // Left gets `lw`. If it fits chrome + fixed-tail + ≥1 ws cell, render
    // the full tail (ws truncated to fill). Else drop Mode (show ws only).
    // Else drop ws (chrome only).
    let avail_after_chrome = lw.saturating_sub(chrome_w);
    let show_full_tail = avail_after_chrome > tail_fixed_w;
    let show_ws_only = !show_full_tail && avail_after_chrome >= 4;
    let ws_budget = if show_full_tail {
        avail_after_chrome.saturating_sub(tail_fixed_w)
    } else if show_ws_only {
        avail_after_chrome.saturating_sub(2) // " <ws> "
    } else {
        0
    };
    let ws_shown = if ws_budget == 0 {
        String::new()
    } else if dw(&ws) <= ws_budget {
        ws.clone()
    } else if ws_budget >= 2 {
        truncate_label(&ws, ws_budget)
    } else {
        String::new()
    };

    let mut left_spans = chrome;
    if show_full_tail {
        left_spans.push(ws_lbl);
        left_spans.push(Span::styled(ws_shown.clone(), Style::default().fg(FG)));
        left_spans.push(mode_lbl);
        left_spans.push(mode_span);
        left_spans.push(tail_pad);
    } else if show_ws_only && !ws_shown.is_empty() {
        left_spans.push(Span::styled(
            format!(" {} ", ws_shown),
            Style::default().fg(FG),
        ));
    }

    let half = Layout::horizontal([Constraint::Length(lw), Constraint::Length(rw)]);
    let [l, r] = half.areas(area);
    frame.render_widget(
        Paragraph::new(Line::from(left_spans)).style(Style::default().bg(HEADER_BG)),
        l,
    );

    // Right — drop the lowest-priority fields (battery → DDS → distro)
    // until the remaining block fits the slot.
    let right_line = fit_title_right(
        rw,
        ros2_status,
        status_color,
        distro_display,
        dds_display,
        &battery,
        &clock,
    );
    frame.render_widget(
        Paragraph::new(right_line)
            .style(Style::default().bg(HEADER_BG))
            .alignment(Alignment::Right),
        r,
    );
}

/// Natural display width of the full right-side status block (no dropping).
fn title_right_full_w(ros: &str, distro: &str, dds: &str, battery: &str, clock: &str) -> u16 {
    dw(&format!(
        "ROS: {ros}  Distro: {distro}  DDS: {dds}  {battery}  {clock}"
    ))
}

/// Build the right-side status line, shedding fields until it fits `max_w`.
/// Drop order (least → most important): battery → DDS → distro → all but ROS.
fn fit_title_right(
    max_w: u16,
    ros2_status: &str,
    sc: Color,
    distro: &str,
    dds: &str,
    battery: &str,
    clock: &str,
) -> Line<'static> {
    let line_w = |s: &[Span]| s.iter().map(|x| x.width()).sum::<usize>() as u16;
    let mk = Line::from;
    let full: Vec<Span<'static>> = vec![
        Span::styled("ROS: ".to_string(), Style::default().fg(DIM)),
        Span::styled(
            format!("{ros2_status}  ").to_string(),
            Style::default().fg(sc).add_modifier(Modifier::BOLD),
        ),
        Span::styled("Distro: ".to_string(), Style::default().fg(DIM)),
        Span::styled(
            format!("{distro}  ").to_string(),
            Style::default().fg(ACCENT),
        ),
        Span::styled("DDS: ".to_string(), Style::default().fg(DIM)),
        Span::styled(format!("{dds}  ").to_string(), Style::default().fg(ACCENT)),
        Span::styled(format!("{battery}  ").to_string(), Style::default().fg(DIM)),
        Span::styled(clock.to_string(), Style::default().fg(DIM)),
    ];
    if line_w(&full) <= max_w {
        return mk(full);
    }
    let no_batt: Vec<Span<'static>> = full
        .iter()
        .take(6)
        .cloned()
        .chain([Span::styled(clock.to_string(), Style::default().fg(DIM))])
        .collect();
    if line_w(&no_batt) <= max_w {
        return mk(no_batt);
    }
    let no_dds: Vec<Span<'static>> = full
        .iter()
        .take(4)
        .cloned()
        .chain([Span::styled(clock.to_string(), Style::default().fg(DIM))])
        .collect();
    if line_w(&no_dds) <= max_w {
        return mk(no_dds);
    }
    let bare: Vec<Span<'static>> = vec![
        Span::styled("ROS: ".to_string(), Style::default().fg(DIM)),
        Span::styled(
            format!("{ros2_status}  ").to_string(),
            Style::default().fg(sc).add_modifier(Modifier::BOLD),
        ),
        Span::styled(clock.to_string(), Style::default().fg(DIM)),
    ];
    if line_w(&bare) <= max_w {
        return mk(bare);
    }
    mk(vec![Span::styled(
        ros2_status.to_string(),
        Style::default().fg(sc),
    )])
}

// ── Tab bar ────────────────────────────────────────────────────────

/// Display width of one tab label " ICON LABEL " — measured, not assumed.
/// Padding = leading space + space between icon/label + trailing space = 3.
fn tab_display_width(idx: usize) -> u16 {
    let t = Tab::from_index(idx);
    dw(t.icon()) + dw(t.label()) + 3
}

fn draw_tab_bar(frame: &mut Frame, area: Rect, app: &mut App) {
    let tabs_area_w = area.width.saturating_sub(RIGHT_ITEMS_W);
    let tabs_area = Rect::new(area.x, area.y, tabs_area_w, area.height);
    let right_area = Rect::new(
        area.x + tabs_area_w,
        area.y,
        RIGHT_ITEMS_W.min(area.width),
        area.height,
    );

    frame.render_widget(
        Paragraph::new("").style(Style::default().bg(HEADER_BG)),
        area,
    );

    // Measure each tab first; sum tells us whether the row fits or must shrink.
    let widths: Vec<u16> = (0..Tab::COUNT).map(tab_display_width).collect();
    let total: u64 = widths.iter().map(|w| *w as u64).sum();
    let avail = tabs_area_w as u64;

    // Per-tab cells. If it fits, give each tab its measured width. If not,
    // squeeze every tab to an equal share and let the label truncate inside —
    // no tab renders outside its slice, so no overlap.
    let rects: Vec<Rect> = if total <= avail {
        let constraints: Vec<Constraint> = widths.iter().map(|w| Constraint::Length(*w)).collect();
        Layout::horizontal(&constraints).split(tabs_area).to_vec()
    } else {
        // Proportional shrink: each tab gets floor(avail / COUNT).
        let each = (avail / Tab::COUNT as u64).max(4) as u16;
        Layout::horizontal([Constraint::Length(each); Tab::COUNT])
            .split(tabs_area)
            .to_vec()
    };

    for (i, r) in rects.iter().enumerate() {
        let t = Tab::from_index(i);
        let active = i == app.current_tab;
        let style = if active {
            Style::default()
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD)
                .bg(SURFACE_HI)
        } else {
            Style::default().fg(DIM).bg(HEADER_BG)
        };
        let label = format!(" {} {} ", t.icon(), t.label());
        // Truncate to the cell width; ellipsis if it doesn't fit.
        let shown = truncate_label(&label, r.width);
        frame.render_widget(
            Paragraph::new(Span::styled(shown, style)).style(Style::default().bg(HEADER_BG)),
            *r,
        );
        app.hit_regions.push((*r, HitTarget::TopTab(i)));
        // Active tab is highlighted via the raised SURFACE_HI background above
        // (no underline), so it reads clearly as "selected".
    }

    // Right items: Ros Graph toggle, Live dot, gear.
    let live_color = live_color(app);
    let graph_label = if app.ros_graph_full {
        "Graph"
    } else {
        "ROS Graph"
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{} {}", glyph("+", "\u{26F6}", "\u{f78e}"), graph_label),
                Style::default().fg(if app.ros_graph_full { ACCENT } else { FG }),
            ),
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("{} ", glyph("*", "\u{25CF}", "\u{f111}")),
                Style::default().fg(live_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled("Live", Style::default().fg(live_color)),
            Span::styled("  ", Style::default()),
            Span::styled(
                glyph("o", "\u{2699}\u{FE0F}", "\u{f013}"),
                Style::default().fg(DIM),
            ),
        ]))
        .style(Style::default().bg(HEADER_BG))
        .alignment(Alignment::Right),
        right_area,
    );
    app.hit_regions.push((right_area, HitTarget::RightItem(0)));
}

/// Truncate `s` to fit `max` display cells, ending in '…' if it doesn't.
/// Uses unicode-width so emoji/CJK are measured correctly.
fn truncate_label(s: &str, max: u16) -> String {
    let max = max as usize;
    let w = s.width();
    if w <= max {
        return s.to_string();
    }
    if max <= 1 {
        return "…".to_string();
    }
    // Walk chars, accumulate display width, stop before the ellipsis would overflow.
    let mut out = String::new();
    let mut used = 0usize;
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if used + cw + 1 > max {
            break;
        }
        out.push(c);
        used += cw;
    }
    out.push('…');
    out
}

#[allow(dead_code)]
pub enum RightItem {
    RosGraphToggle,
    LiveDot,
    Gear,
}

/// Compute the rendered display-width for an editor tab at index `idx`.
/// Must stay in lock-step with `draw_editor_tabs` so mouse hit-testing lines
/// up exactly with what is painted: a leading gap space for every tab after
/// the first, then `icon + ' '`, the name, the dirty/spacer pair, and " ✕ ".
pub fn editor_tab_dw(idx: usize, app: &App) -> u16 {
    let f = &app.editor.files[idx];
    let (icon, _) = file_icon(&f.path);
    let lead = if idx > 0 { 1 } else { 0 };
    lead + dw(icon) + 1 + dw(&f.name()) + 2 + 3
}

/// Column (relative to the editor tab strip) where the trailing "+" new-tab
/// button begins, plus its clickable width.
pub fn editor_new_tab_range(app: &App) -> (u16, u16) {
    let total: u16 = (0..app.editor.files.len())
        .map(|i| editor_tab_dw(i, app))
        .sum();
    // Reserve 4 cells for the "+" button: " + " with padding on each side.
    (total, 4)
}

/// Given a column offset (from editor tabs area x), returns which tab the
/// cursor is on and whether the ✕ close button was hit.
pub fn editor_tab_hit(col: u16, app: &App) -> Option<(usize, bool)> {
    let mut cx = 0u16;
    for i in 0..app.editor.files.len() {
        let w = editor_tab_dw(i, app);
        if col >= cx && col < cx + w {
            // The close button is the trailing " ✕ " (3 cells) of the tab.
            return Some((i, col >= cx + w.saturating_sub(3)));
        }
        cx += w;
    }
    None
}

pub fn right_items_click(col: u16, area_width: u16) -> Option<RightItem> {
    let tabs_w = area_width.saturating_sub(RIGHT_ITEMS_W);
    if col < tabs_w {
        return None;
    }
    let rel = col - tabs_w;
    if rel < 14 {
        Some(RightItem::RosGraphToggle)
    } else {
        Some(RightItem::Gear)
    }
}

pub fn live_color(app: &App) -> Color {
    let age = app.last_heartbeat.elapsed().as_secs();
    if age > 10 {
        ERROR
    } else if age > 3 {
        WARN
    } else {
        OK
    }
}

// ── Body ───────────────────────────────────────────────────────────

fn draw_body(frame: &mut Frame, area: Rect, app: &mut App) {
    let mut constraints = vec![Constraint::Length(ACTIVITY_W)];
    if app.sidebar_visible {
        constraints.push(Constraint::Length(app.sidebar_width));
    }
    constraints.push(Constraint::Min(1));
    if app.right_visible {
        constraints.push(Constraint::Length(app.right_panel_width));
    }

    let chunks = Layout::horizontal(&constraints).split(area);
    let activity_area = chunks[0];
    let sidebar_area = if app.sidebar_visible {
        Some(chunks[1])
    } else {
        None
    };
    let center_area = if app.sidebar_visible {
        chunks[2]
    } else {
        chunks[1]
    };
    let right_area = if app.right_visible {
        if app.sidebar_visible {
            Some(chunks[3])
        } else {
            Some(chunks[2])
        }
    } else {
        None
    };

    draw_activity_bar(frame, activity_area, app);

    if let Some(sb) = sidebar_area {
        draw_sidebar(frame, sb, app);
    }

    draw_center(frame, center_area, app);

    if let Some(rp) = right_area {
        draw_right_panel_stack(frame, rp, app);
    }
}

fn draw_center(frame: &mut Frame, area: Rect, app: &mut App) {
    if app.ros_graph_full {
        // Full-pane ROS graph view.
        draw_ros2_graph_canvas(frame, area, app, true);
        return;
    }
    match app.tab() {
        Tab::Workspace => draw_editor(frame, area, app),
        _ => draw_dashboard(frame, area, app),
    }
}

// ── Activity bar ───────────────────────────────────────────────────

fn draw_activity_bar(frame: &mut Frame, area: Rect, app: &mut App) {
    let focus = app.focus == Focus::ActivityBar;

    // Render top icons (rows 0..TOP)
    for i in 0..Activity::TOP {
        let act = Activity::from_top_index(i);
        let active = app.active_activity == act;
        let style = if active {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else if focus {
            Style::default().fg(FG)
        } else {
            Style::default().fg(DIM)
        };
        let row = area.y + i as u16;
        if row < area.bottom() {
            let cell = Rect::new(area.x, row, area.width, 1);
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(format!(" {} ", act.icon()), style)))
                    .style(Style::default().bg(if active { SURFACE_HI } else { HEADER_BG })),
                cell,
            );
            app.hit_regions.push((cell, HitTarget::Activity(i)));
        }
    }

    // Render pinned items at the very bottom of the area
    let pinned = Activity::pinned();
    for (pi, act) in pinned.iter().enumerate() {
        if let Some(row) = area.bottom().checked_sub(pinned.len() as u16 - pi as u16) {
            if row >= area.y {
                let active = app.active_activity == *act;
                let style = if active {
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
                } else if focus {
                    Style::default().fg(FG)
                } else {
                    Style::default().fg(DIM)
                };
                let cell = Rect::new(area.x, row, area.width, 1);
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(format!(" {} ", act.icon()), style)))
                        .style(Style::default().bg(if active { SURFACE_HI } else { HEADER_BG })),
                    cell,
                );
                let target = if *act == Activity::Settings {
                    HitTarget::ActivitySettings
                } else {
                    HitTarget::ActivityHelp
                };
                app.hit_regions.push((cell, target));
            }
        }
    }

    // Left accent bar on active icon
    let active_row = if app.active_activity == Activity::Settings {
        area.bottom().checked_sub(2)
    } else if app.active_activity == Activity::Help {
        area.bottom().checked_sub(1)
    } else {
        (0..Activity::TOP)
            .position(|i| Activity::from_top_index(i) == app.active_activity)
            .map(|i| area.y + i as u16)
    };
    if let Some(r) = active_row {
        if r >= area.y && r < area.bottom() {
            let bar = Rect::new(area.x, r, 1, 1);
            frame.render_widget(Paragraph::new("│").style(Style::default().fg(ACCENT)), bar);
        }
    }
}

// ── Sidebar ────────────────────────────────────────────────────────

fn file_icon(path: &std::path::Path) -> (&'static str, Color) {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if name.ends_with(".launch.py") {
        return (glyph(">>", "\u{1F680}", "\u{f135}"), ACCENT); // rocket
    }
    match ext.as_str() {
        "py" => (glyph("py", "\u{1F40D}", "\u{e73c}"), OK), // snake
        "launch" | "xml" | "urdf" | "xacro" => (glyph("<>", "\u{1F4CB}", "\u{f15c}"), ACCENT_WARM),
        "msg" | "srv" | "action" => (glyph("m+", "\u{1F4AC}", "\u{f27a}"), MAGENTA),
        "yaml" | "yml" => (glyph("y", "\u{2699}\u{FE0F}", "\u{f013}"), WARN),
        "md" => (glyph("M", "\u{1F4DD}", "\u{f15c}"), INFO),
        "rs" => (glyph("Rs", "\u{1F980}", "\u{e7a8}"), ERROR),
        "cpp" | "cc" | "cxx" | "h" | "hpp" | "c" => (glyph("C", "\u{1F4C4}", "\u{f15c}"), FG),
        _ => (glyph("-", "\u{1F4C4}", "\u{f15c}"), DIM),
    }
}

fn draw_sidebar(frame: &mut Frame, area: Rect, app: &mut App) {
    let focus = app.focus == Focus::Sidebar;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(focus_style(focus))
        .border_type(BorderType::Plain);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Register the whole sidebar content area as one clickable region; the
    // per-row fine-grained logic (file-tree rows, + button, search input)
    // lives in `handle_click` and runs once this region is hit.
    if inner.height > 0 {
        app.hit_regions.push((inner, HitTarget::Sidebar));
    }

    if inner.height == 0 {
        return;
    }

    match app.active_activity {
        Activity::Explorer => draw_explorer_sidebar(frame, inner, app),
        Activity::Search => draw_search_sidebar(frame, inner, app),
        Activity::RosGraph => draw_rosgraph_sidebar(frame, inner, app),
        Activity::Diagnostics => draw_diagnostics_sidebar(frame, inner, app),
        Activity::Sandbox => draw_sandbox_sidebar(frame, inner, app),
        Activity::Git => draw_git_sidebar(frame, inner, app),
        Activity::Plugins => draw_plugins_sidebar(frame, inner, app),
        Activity::Settings => draw_settings_sidebar(frame, inner, app),
        Activity::Help => draw_explorer_sidebar(frame, inner, app),
    }
}

#[allow(dead_code)]
fn draw_placeholder_sidebar(frame: &mut Frame, area: Rect, title: &str, msg: &str) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER))
        .title(format!(" {title} "));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(msg).style(Style::default().fg(DIM).bg(BG)),
        inner,
    );
}

fn draw_search_sidebar(frame: &mut Frame, area: Rect, app: &mut App) {
    let header = Line::from(vec![Span::styled(
        " SEARCH",
        Style::default().fg(FG).add_modifier(Modifier::BOLD),
    )]);
    let ha = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(
        Paragraph::new(header).style(Style::default().bg(SURFACE)),
        ha,
    );

    let rest = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(1),
    );
    if rest.height == 0 {
        return;
    }

    // Search input box
    let input_h = 1u16;
    let input_area = Rect::new(rest.x, rest.y, rest.width, input_h);
    let input_style = if app.search_input_active {
        Style::default()
            .fg(ACCENT)
            .bg(SELECT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DIM).bg(SURFACE)
    };
    let query_display = if app.search_input_active || !app.search_query.is_empty() {
        format!(" 🔍 {}▌", app.search_query)
    } else {
        " 🔍 (type to search)".to_string()
    };
    frame.render_widget(Paragraph::new(query_display).style(input_style), input_area);
    app.hit_regions
        .push((input_area, HitTarget::SidebarSearchInput));

    let results_area = Rect::new(
        rest.x,
        rest.y + input_h + 1,
        rest.width,
        rest.height.saturating_sub(input_h + 1),
    );

    if app.search_query.is_empty() {
        frame.render_widget(
            Paragraph::new("Type to search file contents\nacross the workspace.\n\nCtrl+F to focus search from\nanywhere in the TUI.")
                .style(Style::default().fg(DIM).bg(BG)),
            results_area,
        );
        return;
    }

    if app.search_results.is_empty() {
        frame.render_widget(
            Paragraph::new(format!("No matches for \"{}\"", app.search_query))
                .style(Style::default().fg(DIM).bg(BG)),
            results_area,
        );
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        format!(" {} matches", app.search_results.len()),
        Style::default().fg(DIM),
    )));
    let maxw = (results_area.width as usize).saturating_sub(3);
    for (r, hit) in app
        .search_results
        .iter()
        .take(results_area.height as usize - 1)
        .enumerate()
    {
        let name = hit
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let loc = format!("{}:{}", name, hit.line);
        let mut text = hit.text.trim().to_string();
        if text.chars().count() > maxw {
            text = text
                .chars()
                .take(maxw.saturating_sub(1))
                .collect::<String>()
                + "…";
        }
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                loc,
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ", Style::default()),
            Span::styled(text, Style::default().fg(FG)),
        ]));
        // Result `r` is at list row `r+1` (after the "N matches" header).
        let row = results_area.y + 1 + r as u16;
        app.hit_regions.push((
            Rect::new(results_area.x, row, results_area.width, 1),
            HitTarget::SidebarSearchResult(r),
        ));
    }
    frame.render_widget(
        List::new(lines).style(Style::default().bg(BG)),
        results_area,
    );
}

fn draw_rosgraph_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let header = Line::from(vec![Span::styled(
        " ROS GRAPH",
        Style::default().fg(FG).add_modifier(Modifier::BOLD),
    )]);
    let ha = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(
        Paragraph::new(header).style(Style::default().bg(SURFACE)),
        ha,
    );

    let rest = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(1),
    );
    if rest.height == 0 {
        return;
    }

    let Some(graph) = &app.graph else {
        frame.render_widget(
            Paragraph::new("  Collecting graph data...\n\n  The interactive graph is\n  shown in the right panel.\n\n  Click a node to select it.").style(Style::default().fg(DIM).bg(BG)),
            rest,
        );
        return;
    };

    let mut lines: Vec<Line> = Vec::new();
    let mut nodes: Vec<&String> = graph.nodes.keys().collect();
    nodes.sort();
    lines.push(Line::from(Span::styled(
        format!(" {} nodes", nodes.len()),
        Style::default().fg(DIM),
    )));
    for n in &nodes {
        let info = &graph.nodes[*n];
        let pubsub = format!("↑{} ↓{}", info.pubs.len(), info.subs.len());
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(*n, Style::default().fg(ACCENT)),
            Span::styled(format!(" {pubsub}"), Style::default().fg(DIM)),
        ]));
    }
    frame.render_widget(List::new(lines).style(Style::default().bg(BG)), rest);
}

fn draw_diagnostics_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let header = Line::from(vec![Span::styled(
        " DIAGNOSTICS",
        Style::default().fg(FG).add_modifier(Modifier::BOLD),
    )]);
    let ha = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(
        Paragraph::new(header).style(Style::default().bg(SURFACE)),
        ha,
    );

    let rest = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(1),
    );
    if rest.height == 0 {
        return;
    }

    let Some(diag) = &app.diagnostics else {
        frame.render_widget(
            Paragraph::new("  No diagnostics data.\n\n  Issues will appear here\n  from the build system\n  and ROS 2 runtime.").style(Style::default().fg(DIM).bg(BG)),
            rest,
        );
        return;
    };

    if diag.issues.is_empty() {
        frame.render_widget(
            Paragraph::new("  ✓ No issues found.\n\n  All systems healthy.")
                .style(Style::default().fg(OK).bg(BG)),
            rest,
        );
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    let errors = diag.issues.iter().filter(|i| i.severity == "error").count();
    let warns = diag
        .issues
        .iter()
        .filter(|i| i.severity == "warn" || i.severity == "warning")
        .count();
    lines.push(Line::from(Span::styled(
        format!(" {} errors, {} warnings", errors, warns),
        Style::default().fg(if errors > 0 { ERROR } else { OK }),
    )));
    for issue in diag.issues.iter().take(rest.height as usize - 2) {
        let (badge, color) = match issue.severity.as_str() {
            "error" => ("E", ERROR),
            "warn" | "warning" => ("W", WARN),
            _ => ("I", DIM),
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!(" [{badge}] "),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                issue.message.chars().take(36).collect::<String>(),
                Style::default().fg(color),
            ),
        ]));
    }
    frame.render_widget(List::new(lines).style(Style::default().bg(BG)), rest);
}

fn draw_sandbox_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let header = Line::from(vec![Span::styled(
        " SANDBOX",
        Style::default().fg(FG).add_modifier(Modifier::BOLD),
    )]);
    let ha = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(
        Paragraph::new(header).style(Style::default().bg(SURFACE)),
        ha,
    );

    let rest = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(1),
    );
    if rest.height == 0 {
        return;
    }

    let mode = app.sandbox_label();
    let mode_color = app.mode_color();
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Mode: ", Style::default().fg(DIM)),
            Span::styled(
                mode,
                Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Sandbox mode isolates",
            Style::default().fg(DIM),
        )),
        Line::from(Span::styled(
            "  ROS 2 nodes into a",
            Style::default().fg(DIM),
        )),
        Line::from(Span::styled(
            "  separate namespace.",
            Style::default().fg(DIM),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Press F6 to toggle",
            Style::default().fg(DIM),
        )),
        Line::from(Span::styled(
            "  between Sandbox and",
            Style::default().fg(DIM),
        )),
        Line::from(Span::styled("  Global mode.", Style::default().fg(DIM))),
        Line::from(""),
        Line::from(Span::styled(
            "  Global mode runs",
            Style::default().fg(WARN),
        )),
        Line::from(Span::styled(
            "  commands on your real",
            Style::default().fg(WARN),
        )),
        Line::from(Span::styled(
            "  ROS 2 environment.",
            Style::default().fg(WARN),
        )),
    ];
    frame.render_widget(
        Paragraph::new(Text::from(lines)).style(Style::default().bg(BG)),
        rest,
    );
}

fn draw_plugins_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let header = Line::from(vec![
        Span::styled(
            " PLUGINS",
            Style::default().fg(FG).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" ({})", app.plugin_manager.count()),
            Style::default().fg(DIM),
        ),
    ]);
    let ha = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(
        Paragraph::new(header).style(Style::default().bg(SURFACE)),
        ha,
    );

    let rest = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(1),
    );
    if rest.height == 0 {
        return;
    }

    let plugins = app.plugin_manager.list();

    if plugins.is_empty() {
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No plugins installed.",
                Style::default().fg(DIM),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Plugins extend TUI",
                Style::default().fg(DIM),
            )),
            Line::from(Span::styled(
                "  functionality. See docs/plugins.md",
                Style::default().fg(DIM),
            )),
            Line::from(Span::styled(
                "  and add a crate under",
                Style::default().fg(DIM),
            )),
            Line::from(Span::styled(
                "  src/tui/src/plugins/.",
                Style::default().fg(DIM),
            )),
        ];
        frame.render_widget(
            Paragraph::new(Text::from(lines)).style(Style::default().bg(BG)),
            rest,
        );
        return;
    }

    // List registered plugins (name + version), then render the first
    // contributed dashboard tab beneath them.
    let mut lines: Vec<Line> = vec![Line::from("")];
    for (name, version) in &plugins {
        lines.push(Line::from(vec![
            Span::styled("  ◆ ", Style::default().fg(ACCENT)),
            Span::styled(
                name.clone(),
                Style::default().fg(FG).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" v{}", version), Style::default().fg(DIM)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Add a plugin: see docs/plugins.md",
        Style::default().fg(DIM),
    )));

    // Reserve a few lines for the list, render the first plugin tab below it.
    let list_h = lines.len().min(rest.height as usize) as u16;
    let list_area = Rect::new(rest.x, rest.y, rest.width, list_h);
    let tab_area = Rect::new(
        rest.x,
        rest.y + list_h,
        rest.width,
        rest.height.saturating_sub(list_h),
    );

    frame.render_widget(
        Paragraph::new(Text::from(lines)).style(Style::default().bg(BG)),
        list_area,
    );

    let tabs = app.plugin_manager.all_tabs();
    if let Some(first) = tabs.first() {
        if tab_area.height > 1 {
            frame.render_widget(
                Paragraph::new(Text::from(vec![Line::from(Span::styled(
                    format!("  ── tab: {} ──", first.title()),
                    Style::default().fg(ACCENT),
                ))])),
                Rect::new(tab_area.x, tab_area.y, tab_area.width, 1),
            );
            let body = Rect::new(
                tab_area.x,
                tab_area.y + 1,
                tab_area.width,
                tab_area.height.saturating_sub(1),
            );
            first.render(frame, body);
        }
    }
}

fn draw_settings_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let header = Line::from(vec![Span::styled(
        " SETTINGS",
        Style::default().fg(FG).add_modifier(Modifier::BOLD),
    )]);
    let ha = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(
        Paragraph::new(header).style(Style::default().bg(SURFACE)),
        ha,
    );

    let rest = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(1),
    );
    if rest.height == 0 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Key Bindings",
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled("    Scheme: ", Style::default().fg(DIM)),
        Span::styled(
            app.keybind_mode.label(),
            Style::default().fg(if app.keybind_mode == crate::app::KeybindMode::Neovim {
                ACCENT
            } else {
                OK
            }),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        "    Press K to toggle (Normal ⇄ Neovim)",
        Style::default().fg(DIM),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Appearance",
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled("    Theme: ", Style::default().fg(DIM)),
        Span::styled("Dracula Dark+", Style::default().fg(FG)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Sidebar: ", Style::default().fg(DIM)),
        Span::styled(
            if app.sidebar_visible {
                "Visible"
            } else {
                "Hidden"
            },
            Style::default().fg(if app.sidebar_visible { OK } else { DIM }),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Right Panel: ", Style::default().fg(DIM)),
        Span::styled(
            if app.right_visible {
                "Visible"
            } else {
                "Hidden"
            },
            Style::default().fg(if app.right_visible { OK } else { DIM }),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Terminal: ", Style::default().fg(DIM)),
        Span::styled(
            if app.terminal_visible {
                "Visible"
            } else {
                "Hidden"
            },
            Style::default().fg(if app.terminal_visible { OK } else { DIM }),
        ),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Session",
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled("    Mode: ", Style::default().fg(DIM)),
        Span::styled(
            app.sandbox_label(),
            Style::default()
                .fg(app.mode_color())
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Workspace: ", Style::default().fg(DIM)),
        Span::styled(
            app.workspace
                .as_ref()
                .and_then(|w| w.workspaces.first().cloned())
                .unwrap_or_else(|| "/".into()),
            Style::default().fg(FG),
        ),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Editor",
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled("    Open files: ", Style::default().fg(DIM)),
        Span::styled(app.editor.files.len().to_string(), Style::default().fg(FG)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Mode: ", Style::default().fg(DIM)),
        Span::styled(
            match app.editor.mode {
                EditMode::Preview => "Preview",
                EditMode::Edit => "Edit",
            },
            Style::default().fg(ACCENT),
        ),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Terminal",
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled("    Sessions: ", Style::default().fg(DIM)),
        Span::styled(
            app.terminal_mgr.sessions.len().to_string(),
            Style::default().fg(FG),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("    Height: ", Style::default().fg(DIM)),
        Span::styled(
            format!("{} rows", app.terminal_height),
            Style::default().fg(FG),
        ),
    ]));
    frame.render_widget(List::new(lines).style(Style::default().bg(BG)), rest);
}

fn draw_git_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let header = Line::from(vec![Span::styled(
        " GIT",
        Style::default().fg(FG).add_modifier(Modifier::BOLD),
    )]);
    let header_area = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(
        Paragraph::new(header).style(Style::default().bg(SURFACE)),
        header_area,
    );

    let rest = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(1),
    );
    if rest.height == 0 {
        return;
    }

    let gs = &app.git;
    let mut lines: Vec<Line> = Vec::new();

    // Branch info
    if let Some(ref status) = gs.status {
        let branch_color = if status.ahead > 0 {
            ACCENT
        } else if status.behind > 0 {
            WARN
        } else {
            OK
        };
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("🔀 ", Style::default().fg(branch_color)),
            Span::styled(
                &status.branch,
                Style::default().fg(FG).add_modifier(Modifier::BOLD),
            ),
        ]));
        if status.ahead > 0 || status.behind > 0 {
            let mut detail_spans = vec![Span::styled("    ", Style::default())];
            if status.ahead > 0 {
                detail_spans.push(Span::styled(
                    format!("↑{} ", status.ahead),
                    Style::default().fg(OK),
                ));
            }
            if status.behind > 0 {
                detail_spans.push(Span::styled(
                    format!("↓{}", status.behind),
                    Style::default().fg(ERROR),
                ));
            }
            lines.push(Line::from(detail_spans));
        }
        lines.push(Line::from(""));

        // Staged files
        if !status.staged.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("  Staged ({})", status.staged.len()),
                Style::default().fg(OK).add_modifier(Modifier::BOLD),
            )));
            for f in &status.staged {
                let status_color = match f.status {
                    'M' => WARN_AMBER,
                    'A' => OK,
                    'D' => ERROR,
                    'R' => ACCENT,
                    _ => DIM,
                };
                lines.push(Line::from(vec![
                    Span::styled("    ", Style::default()),
                    Span::styled(format!("{} ", f.status), Style::default().fg(status_color)),
                    Span::styled(&f.path, Style::default().fg(FG)),
                ]));
            }
            lines.push(Line::from(""));
        }

        // Unstaged files
        if !status.unstaged.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("  Modified ({}):", status.unstaged.len()),
                Style::default().fg(WARN_AMBER).add_modifier(Modifier::BOLD),
            )));
            for f in &status.unstaged {
                lines.push(Line::from(vec![
                    Span::styled("    ", Style::default()),
                    Span::styled("M ", Style::default().fg(WARN_AMBER)),
                    Span::styled(&f.path, Style::default().fg(FG)),
                ]));
            }
        }

        if !status.untracked.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("  Untracked (", Style::default().fg(DIM)),
                Span::styled(status.untracked.len().to_string(), Style::default().fg(OK)),
                Span::styled("):", Style::default().fg(DIM)),
            ]));
            for f in status.untracked.iter().take(10) {
                lines.push(Line::from(vec![
                    Span::styled("    ", Style::default()),
                    Span::styled("? ", Style::default().fg(OK)),
                    Span::styled(&f.path, Style::default().fg(FG)),
                ]));
            }
        }

        if status.staged.is_empty() && status.unstaged.is_empty() && status.untracked.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("✓ Working tree clean", Style::default().fg(OK)),
            ]));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "  Loading git status...",
            Style::default().fg(DIM),
        )));
    }

    // Recent commits
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Recent Commits",
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    )));
    for commit in gs.log.iter().take(5) {
        let short_hash = &commit.hash[..commit.hash.len().min(7)];
        lines.push(Line::from(vec![
            Span::styled(format!("  {} ", short_hash), Style::default().fg(MAGENTA)),
            Span::styled(&commit.message, Style::default().fg(FG)),
        ]));
    }

    // GitHub section
    if !gs.issues.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                format!("  GitHub Issues ({})", gs.issues.len()),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  c: new",
                Style::default()
                    .fg(DIM)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        for issue in gs.issues.iter().take(5) {
            let (sym, sc) = if issue.state == "open" {
                ("○", OK)
            } else {
                ("✓", DIM)
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {sym} #{:<6}", issue.number),
                    Style::default().fg(sc).add_modifier(Modifier::BOLD),
                ),
                Span::styled(&issue.title, Style::default().fg(DIM)),
            ]));
        }
    } else {
        // When there are no (loaded) issues, still surface the create-issue
        // action so users discover the GitHub integration.
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                "  GitHub Issues",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  c: new",
                Style::default()
                    .fg(DIM)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    if !gs.prs.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  Pull Requests ({})", gs.prs.len()),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )));
        for pr in gs.prs.iter().take(5) {
            let (sym, sc) = if pr.state == "open" {
                ("○", OK)
            } else {
                ("✓", DIM)
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {sym} #{:<6}", pr.number),
                    Style::default().fg(sc).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default()),
                Span::styled(&pr.title, Style::default().fg(DIM)),
            ]));
        }
    }

    let list = List::new(lines).style(Style::default().bg(BG));
    frame.render_widget(list, rest);

    // "Create Issue" inline prompt (captured by `app.issue_input`).
    if let Some(ref text) = app.issue_input {
        let (ix, iy, iw, ih) = (area.x, area.y + area.height.saturating_sub(1), area.width, 1);
        if ih > 0 {
            let line = Line::from(vec![
                Span::styled(
                    " New Issue: ",
                    Style::default()
                        .fg(WARN)
                        .add_modifier(Modifier::BOLD)
                        .bg(BG),
                ),
                Span::styled(text.as_str(), Style::default().fg(FG).bg(BG)),
                Span::styled("█", Style::default().fg(ACCENT).bg(BG)),
                Span::styled(
                    "  Enter: create  Esc: cancel",
                    Style::default().fg(DIM).bg(BG),
                ),
            ]);
            frame.render_widget(
                Paragraph::new(line).style(Style::default().bg(BG)),
                Rect::new(ix, iy, iw, ih),
            );
        }
    }
}

fn draw_explorer_sidebar(frame: &mut Frame, area: Rect, app: &mut App) {
    let header = Line::from(vec![
        Span::styled(
            " EXPLORER",
            Style::default().fg(FG).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" +", Style::default().fg(DIM)),
    ]);
    let header_area = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(
        Paragraph::new(header).style(Style::default().bg(SURFACE)),
        header_area,
    );
    // Register the trailing " +" as the new-file button.
    let plus_rect = Rect::new(area.x + area.width.saturating_sub(2), area.y, 2, 1);
    app.hit_regions.push((plus_rect, HitTarget::SidebarNew));

    let rest = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(1),
    );
    if rest.height == 0 {
        return;
    }

    // Layout: tree (flex) | OPEN EDITORS | OUTLINE
    let open_h = open_editors_h(app);
    let outline_h_val = outline_h(app);
    let tree_h = rest.height.saturating_sub(open_h + outline_h_val + 2);
    let chunks = Layout::vertical([
        Constraint::Length(tree_h.max(3)),
        Constraint::Length(open_h + 1),
        Constraint::Length(outline_h_val + 1),
    ])
    .split(rest);

    draw_tree(frame, chunks[0], app);
    draw_open_editors(frame, chunks[1], app);
    draw_outline(frame, chunks[2], &*app);
}

fn open_editors_h(app: &App) -> u16 {
    (app.editor.files.len() as u16 + 1).clamp(2, 6)
}

fn outline_h(app: &App) -> u16 {
    (app.editor.outline().len() as u16 + 1).clamp(2, 8)
}

fn draw_tree(frame: &mut Frame, area: Rect, app: &mut App) {
    let Some(tree) = &app.file_tree else { return };
    let inner = area;
    let h = inner.height as usize;

    // Keep the scroll offset valid and auto-scroll the selected item into view.
    let max_start = tree.items.len().saturating_sub(h);
    let mut so = tree.scroll_offset.min(max_start);
    if let Some(sel) = &tree.selected {
        if let Some(idx) = tree.items.iter().position(|i| &i.path == sel) {
            if idx < so {
                so = idx;
            } else if idx >= so + h {
                so = (idx - h + 1).min(max_start);
            }
        }
    }

    let mut lines: Vec<Line> = Vec::new();
    for (j, item) in tree.items.iter().skip(so).take(h).enumerate() {
        let idx = so + j;
        let row = inner.y + j as u16;
        let indent = "  ".repeat(item.depth);
        let (icon, color) = if item.is_dir {
            let expanded = tree.expanded.contains(&item.path);
            if expanded {
                ("▼ ", if item.depth == 0 { FG } else { DIM })
            } else {
                ("▶ ", if item.depth == 0 { FG } else { DIM })
            }
        } else {
            let (ico, c) = file_icon(&item.path);
            (ico, c)
        };
        let is_selected = tree.selected.as_ref() == Some(&item.path);
        let style = if is_selected {
            Style::default()
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD)
                .bg(SELECT)
        } else {
            Style::default().fg(color)
        };
        // status badge (git status or dirty)
        let badge = if let Some(status) = app
            .file_tree
            .as_ref()
            .and_then(|t| t.git_status.get(&item.path))
        {
            let (label, color) = match status {
                'U' => (" U", MAGENTA),
                'M' => (" M", WARN_AMBER),
                'A' => (" A", OK),
                'D' => (" D", ERROR),
                'R' => (" R", ACCENT),
                _ => ("", DIM),
            };
            Span::styled(
                label,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )
        } else if let Some(f) = app.editor.files.iter().find(|f| f.path == item.path) {
            if f.dirty {
                Span::styled(
                    " M",
                    Style::default().fg(WARN_AMBER).add_modifier(Modifier::BOLD),
                )
            } else {
                Span::raw("")
            }
        } else {
            Span::raw("")
        };
        lines.push(Line::from(vec![
            Span::raw(indent),
            Span::styled(format!("{icon} "), Style::default().fg(color)),
            Span::styled(item.name.clone(), style),
            badge,
        ]));
        // Register a per-row hit region so clicks resolve by absolute tree
        // index (accounting for scroll), not by fragile screen-row math.
        app.hit_regions.push((
            Rect::new(inner.x, row, inner.width, 1),
            HitTarget::SidebarFile(idx),
        ));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (empty)",
            Style::default().fg(DIM),
        )));
    }
    let list = List::new(lines).style(Style::default().bg(BG));
    frame.render_widget(list, inner);
}

fn draw_open_editors(frame: &mut Frame, area: Rect, app: &mut App) {
    let header = Line::from(Span::styled(
        " OPEN EDITORS",
        Style::default().fg(DIM).add_modifier(Modifier::BOLD),
    ));
    let ha = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(
        Paragraph::new(header).style(Style::default().bg(SURFACE)),
        ha,
    );
    let rest = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(1),
    );
    let mut lines = Vec::new();
    for (i, f) in app.editor.files.iter().enumerate() {
        let (icon, _c) = file_icon(&f.path);
        let style = if i == app.editor.active {
            Style::default().fg(ACCENT).bg(SELECT)
        } else {
            Style::default().fg(FG)
        };
        let dot = if f.dirty {
            Span::styled(" ●", Style::default().fg(WARN_AMBER))
        } else {
            Span::raw("")
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{icon} "), Style::default().fg(DIM)),
            Span::styled(f.name(), style),
            dot,
        ]));
        app.hit_regions.push((
            Rect::new(rest.x, rest.y + i as u16, rest.width, 1),
            HitTarget::SidebarOpenEditor(i),
        ));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (none)",
            Style::default().fg(DIM),
        )));
    }
    frame.render_widget(List::new(lines).style(Style::default().bg(BG)), rest);
}

fn draw_outline(frame: &mut Frame, area: Rect, app: &App) {
    let header = Line::from(Span::styled(
        " OUTLINE",
        Style::default().fg(DIM).add_modifier(Modifier::BOLD),
    ));
    let ha = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(
        Paragraph::new(header).style(Style::default().bg(SURFACE)),
        ha,
    );
    let rest = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(1),
    );
    let mut lines = Vec::new();
    for item in app.editor.outline() {
        let icon = match item.kind {
            SymbolKind::Function => "ƒ",
            SymbolKind::Class => "◉",
            SymbolKind::Variable => "◇",
        };
        lines.push(Line::from(Span::styled(
            format!("  {icon} {}()", item.name),
            Style::default().fg(ACCENT),
        )));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (open a file)",
            Style::default().fg(DIM),
        )));
    }
    frame.render_widget(List::new(lines).style(Style::default().bg(BG)), rest);
}

// ── Editor ──────────────────────────────────────────────────────────

fn draw_editor(frame: &mut Frame, area: Rect, app: &mut App) {
    if app.editor.is_empty() {
        // Welcome screen when no files are open — the hero first impression.
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(focus_style(app.focus == Focus::Editor))
            .title(" Welcome ");
        let inner = block.inner(area);
        frame.render_widget(block, area);
        app.hit_regions.push((inner, HitTarget::EditorBody));
        if inner.height < 6 || inner.width < 30 {
            return;
        }
        draw_welcome(frame, inner);
        return;
    }

    // Tab strip
    let tab_h = 1u16;
    let bread_h = 1u16;
    let status_h = 1u16;
    let input_h = if app.editor.goto_line_input.is_some()
        || app.editor.find_active
        || app.save_as_input.is_some()
    {
        1u16
    } else {
        0u16
    };
    let tab_area = Rect::new(area.x, area.y, area.width, tab_h);
    let bread_area = Rect::new(area.x, area.y + tab_h, area.width, bread_h);
    let code_area = Rect::new(
        area.x,
        area.y + tab_h + bread_h,
        area.width,
        area.height
            .saturating_sub(tab_h + bread_h + status_h + input_h),
    );
    let status_area = Rect::new(
        area.x,
        area.y + area.height.saturating_sub(status_h + input_h),
        area.width,
        status_h,
    );
    let input_area = if input_h > 0 {
        Some(Rect::new(
            area.x,
            area.y + area.height.saturating_sub(input_h),
            area.width,
            input_h,
        ))
    } else {
        None
    };

    draw_editor_tabs(frame, tab_area, app);
    draw_breadcrumb(frame, bread_area, app);
    app.editor_area = code_area;
    app.hit_regions.push((code_area, HitTarget::EditorBody));
    if app.editor.active_file().map(|f| f.welcome).unwrap_or(false) {
        // Welcome tab: rich hero instead of the ASCII welcome_text().
        draw_welcome(frame, code_area);
    } else {
        draw_code_pane(frame, code_area, app);
    }
    draw_editor_status(frame, status_area, app);

    // Draw input bars (goto-line or find)
    if let Some(ia) = input_area {
        if let Some(ref text) = app.editor.goto_line_input {
            let line = Line::from(vec![
                Span::styled(
                    " Go to Line: ",
                    Style::default()
                        .fg(WARN)
                        .add_modifier(Modifier::BOLD)
                        .bg(BG),
                ),
                Span::styled(text.as_str(), Style::default().fg(FG).bg(BG)),
                Span::styled("█", Style::default().fg(ACCENT).bg(BG)),
            ]);
            frame.render_widget(Paragraph::new(line).style(Style::default().bg(BG)), ia);
        } else if app.editor.find_active {
            let mode = if app.editor.find_replace_mode {
                "Replace"
            } else {
                "Find"
            };
            let query = app.editor.find_query.as_deref().unwrap_or("");
            let count = app.editor.find_match_count;
            let mut spans = vec![
                Span::styled(
                    format!(" {mode}: "),
                    Style::default()
                        .fg(ACCENT)
                        .add_modifier(Modifier::BOLD)
                        .bg(BG),
                ),
                Span::styled(query, Style::default().fg(FG).bg(BG)),
                Span::styled("█", Style::default().fg(ACCENT).bg(BG)),
            ];
            if app.editor.find_replace_mode {
                let r = app.editor.replace_query.as_deref().unwrap_or("");
                spans.push(Span::styled(
                    format!("  With: {r}"),
                    Style::default().fg(WARN_AMBER).bg(BG),
                ));
            }
            spans.push(Span::styled(
                format!("  ({count} matches)  Tab: mode  Alt+Enter: replace all  Esc: close"),
                Style::default().fg(DIM).bg(BG),
            ));
            frame.render_widget(
                Paragraph::new(Line::from(spans)).style(Style::default().bg(BG)),
                ia,
            );
        } else if app.save_as_input.is_some() {
            let text = app.save_as_input.as_deref().unwrap_or("");
            let line = Line::from(vec![
                Span::styled(
                    " Save As: ",
                    Style::default()
                        .fg(WARN)
                        .add_modifier(Modifier::BOLD)
                        .bg(BG),
                ),
                Span::styled(text, Style::default().fg(FG).bg(BG)),
                Span::styled("█", Style::default().fg(ACCENT).bg(BG)),
                Span::styled(
                    "  Enter: save  Esc: cancel",
                    Style::default().fg(DIM).bg(BG),
                ),
            ]);
            frame.render_widget(Paragraph::new(line).style(Style::default().bg(BG)), ia);
        }
    }
}

// ── Welcome hero (no files open) ───────────────────────────────────
// Centred layout: logo → tagline → two-column key grid → hint strip.
// No borders inside — the editor frame already carries the boundary
// (skill: don't nest borders; one border between edge and content).

fn draw_welcome(frame: &mut Frame, area: Rect) {
    // Vertical: logo → tagline → two-column key grid → hint strip.
    // The grid is a fixed 6 lines (header + rule + 4 key rows); the logo
    // gets 2 lines (wordmark + accent rule) rather than 6 of dead space.
    let v = Layout::vertical([
        Constraint::Length(2), // logo block: wordmark + accent rule
        Constraint::Length(1),
        Constraint::Length(1), // tagline
        Constraint::Length(1),
        Constraint::Length(6), // key grid: header + rule + 4 rows
        Constraint::Length(1),
        Constraint::Length(1), // hint strip
    ]);
    let [logo_a, _, tag_a, _, grid_a, _, hint_a] = v.areas(area);

    // Logo: small ◇ sigil + wordmark, centred in cyan/amber (ROS2 brand-ish).
    let logo = Line::from(vec![
        Span::styled(
            "  ◇  ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "ROS2_INFO",
            Style::default().fg(FG).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "  ◇",
            Style::default()
                .fg(ACCENT_WARM)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(logo)
            .alignment(Alignment::Center)
            .style(Style::default().bg(BG)),
        Rect::new(logo_a.x, logo_a.y, logo_a.width, 1),
    );
    // Thin accent rule under the wordmark — centred, ≤40 cells.
    let rule_w = (logo_a.width as usize).min(40);
    let rule = "─".repeat(rule_w);
    frame.render_widget(
        Paragraph::new(rule)
            .alignment(Alignment::Center)
            .style(Style::default().fg(BORDER)),
        Rect::new(logo_a.x, logo_a.y + 1, logo_a.width, 1),
    );

    // Tagline
    frame.render_widget(
        Paragraph::new(" a fastfetch-shaped lens on your ROS 2 workstation ")
            .alignment(Alignment::Center)
            .style(Style::default().fg(DIM).bg(BG)),
        tag_a,
    );

    // Two-column key grid. Left = "Start" core keys; right = "Power" keys.
    let left_keys = [
        ("Ctrl+P", "Go to file"),
        ("Ctrl+Shift+P", "Command palette"),
        ("Ctrl+`", "Toggle terminal"),
        ("Ctrl+B", "Toggle sidebar"),
    ];
    let right_keys = [
        ("Ctrl+G", "Go to line"),
        ("Ctrl+F", "Find in file"),
        ("i / Esc", "Edit / Preview"),
        ("? / F1", "All shortcuts"),
    ];
    let mut grid: Vec<Line> = Vec::new();
    grid.push(Line::from(vec![
        Span::styled(
            " START",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::raw(""),
        Span::styled(
            "        POWER",
            Style::default().fg(WARN).add_modifier(Modifier::BOLD),
        ),
    ]));
    grid.push(Line::from(Span::styled(
        " ──────────────────        ──────────────────",
        Style::default().fg(BORDER),
    )));
    let row_count = left_keys.len().max(right_keys.len());
    for i in 0..row_count {
        let l = left_keys.get(i).copied().unwrap_or(("", ""));
        let r = right_keys.get(i).copied().unwrap_or(("", ""));
        grid.push(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                format!("{:<14}", l.0),
                Style::default().fg(OK).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("{:<14}", l.1), Style::default().fg(DIM)),
            Span::raw(" "),
            Span::styled(
                format!("{:<14}", r.0),
                Style::default().fg(INFO_BLUE).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("{:<14}", r.1), Style::default().fg(DIM)),
        ]));
    }
    // Lay the grid in a centred 58-col strip.
    let grid_w = (grid_a.width as usize).min(58);
    let gx = grid_a.x + (grid_a.width as usize - grid_w) as u16 / 2;
    frame.render_widget(
        Paragraph::new(Text::from(grid)).style(Style::default().bg(BG)),
        Rect::new(gx, grid_a.y, grid_w as u16, grid_a.height),
    );

    // Hint strip — what to do next, centred.
    let hint = Line::from(vec![
        Span::styled(" press ", Style::default().fg(DIM)),
        Span::styled(
            "Ctrl+P",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" to open a file  ·  ", Style::default().fg(DIM)),
        Span::styled(
            "Ctrl+Shift+P",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" for ROS 2 tools", Style::default().fg(DIM)),
    ]);
    frame.render_widget(
        Paragraph::new(hint)
            .alignment(Alignment::Center)
            .style(Style::default().bg(BG)),
        hint_a,
    );

    // Watermark / credit — a faint signature pinned to the bottom of the pane,
    // the way editors tuck a build stamp into the corner. Only drawn when the
    // pane is tall enough that it won't collide with the hint strip above.
    if area.height > 9 {
        let credit = Line::from(vec![
            Span::styled("cc", Style::default().fg(BORDER)),
            Span::styled("@zang", Style::default().fg(DIM)),
            Span::styled(" aka ", Style::default().fg(BORDER)),
            Span::styled(
                "Gaurav-x111",
                Style::default().fg(DIM).add_modifier(Modifier::BOLD),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(credit)
                .alignment(Alignment::Center)
                .style(Style::default().bg(BG)),
            Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
        );
    }
}

fn draw_editor_tabs(frame: &mut Frame, area: Rect, app: &mut App) {
    let mut spans: Vec<Span> = Vec::new();
    let plus_w: u16 = 4; // " + " with padding — always reserve this
    let avail = area.width.saturating_sub(plus_w);
    let mut used_w: u16 = 0;
    for (i, f) in app.editor.files.iter().enumerate() {
        let (icon, _) = file_icon(&f.path);
        let active = i == app.editor.active;
        let base = if active {
            Style::default()
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD)
                .bg(SURFACE_HI)
        } else {
            Style::default().fg(DIM).bg(SURFACE)
        };
        let lead = if i > 0 { 1u16 } else { 0u16 };
        let tab_w = lead + dw(icon) + 1 + dw(&f.name()) + 2 + 3;
        if used_w + tab_w > avail && avail > 4 {
            // Truncate remaining tabs — show ellipsis to signal overflow.
            spans.push(Span::styled(" …", Style::default().fg(DIM).bg(SURFACE)));
            break;
        }
        used_w += tab_w;
        if i > 0 {
            spans.push(Span::styled(" ", Style::default().bg(SURFACE)));
        }
        spans.push(Span::styled(format!("{icon} "), base));
        spans.push(Span::styled(f.name(), base));
        if f.dirty {
            spans.push(Span::styled(
                " ●",
                Style::default().fg(WARN_AMBER).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled("  ", base));
        }
        // Close button: wider hit target (space + cross + space) so mouse is easy.
        spans.push(Span::styled(" ✕ ", Style::default().fg(DIM)));
    }
    // "+" new-tab button — boxed style so it's visually clickable.
    spans.push(Span::styled(
        " + ",
        Style::default()
            .fg(ACCENT)
            .bg(SURFACE_HI)
            .add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(SURFACE)),
        area,
    );
    // Coarse region; `handle_click` runs `editor_tab_hit` for fine-grained
    // switch / close / + handling.
    app.hit_regions.push((area, HitTarget::EditorTab(0)));
}

fn draw_breadcrumb(frame: &mut Frame, area: Rect, app: &mut App) {
    app.hit_regions.push((area, HitTarget::Breadcrumb));
    let Some(f) = app.editor.active_file() else {
        return;
    };
    let mut parts: Vec<String> = f
        .path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    parts.pop(); // filename handled separately
    let dir = parts.join(" > ");
    let (icon, _) = file_icon(&f.path);
    let line = Line::from(vec![
        Span::styled(format!("{dir} > "), Style::default().fg(DIM)),
        Span::styled(format!("{icon} "), Style::default().fg(DIM)),
        Span::styled(f.name(), Style::default().fg(FG)),
    ]);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(SURFACE)),
        area,
    );
}

fn draw_code_pane(frame: &mut Frame, area: Rect, app: &mut App) {
    let focus = app.focus == Focus::Editor;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(focus_style(focus))
        .border_type(BorderType::Plain);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }
    let code_rect = inner;

    // Keep the cursor logically within the viewport (auto-scroll).
    {
        if let Some(f) = app.editor.active_file_mut() {
            if f.scroll_row > f.lines.len() {
                f.scroll_row = f.lines.len().saturating_sub(1);
            }
            if f.cursor_row < f.scroll_row {
                f.scroll_row = f.cursor_row;
            } else if f.cursor_row >= f.scroll_row + inner.height as usize {
                f.scroll_row = f.cursor_row - inner.height as usize + 1;
            }
        }
    }

    let Some(f) = app.editor.active_file() else {
        return;
    };
    let lang = f.language();
    let highlighted = syntax::highlight_lines(&f.lines, lang);

    let gutter: u16 = 6; // change indicator (1) + line number (5)
    let text_w = code_rect.width.saturating_sub(gutter).max(1) as usize;
    let wrap = app.editor.word_wrap;
    let visual = f.visual_lines(text_w, wrap);
    let total_vis = visual.len();

    let scroll = f.scroll_row.min(f.lines.len().saturating_sub(1));
    let vis_start = visual
        .iter()
        .position(|&(r, _, _)| r >= scroll)
        .unwrap_or(0);

    let match_bg = MATCH_HI;

    // Find matches to highlight (only while the find bar is open).
    let matches: Vec<(usize, usize, usize)> = if app.editor.find_active {
        if let Some(q) = &app.editor.find_query {
            f.find_matches(q)
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let height = inner.height as usize;
    let mut lines: Vec<Line> = Vec::new();

    for &(logical, cstart, cend) in &visual[vis_start..(vis_start + height).min(total_vis)] {
        let is_current = logical == f.cursor_row;
        // The gutter renders on its own surface so the current-line tint doesn't
        // bleed into line numbers; the code area gets the active-line tint.
        let gutter_bg = GUTTER_BG;
        let line_bg = if is_current { CURRENT_LINE } else { BG };

        // Flatten the highlighted line into per-char styled cells.
        let mut all: Vec<(char, Style)> = Vec::new();
        if let Some(hl) = highlighted.get(logical) {
            for sp in &hl.spans {
                for ch in sp.content.chars() {
                    all.push((ch, sp.style));
                }
            }
        }

        let change = f.change_indicator(logical);
        let change_span = match change {
            ChangeKind::Added => Span::styled("▌", Style::default().fg(OK).bg(gutter_bg)),
            ChangeKind::Modified => {
                Span::styled("▌", Style::default().fg(WARN_AMBER).bg(gutter_bg))
            }
            ChangeKind::Deleted => Span::styled("▌", Style::default().fg(ERROR).bg(gutter_bg)),
            ChangeKind::None => Span::styled(" ", Style::default().bg(gutter_bg)),
        };
        let lineno = format!("{:>4} ", logical + 1);
        // Active line number reads accent + bold so the cursor row is findable
        // at a glance even with colour stripped (bold + the row-length hint).
        let lineno_style = if is_current {
            Style::default()
                .fg(ACCENT)
                .bg(gutter_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(DIM).bg(gutter_bg)
        };
        let mut spans = vec![change_span, Span::styled(lineno, lineno_style)];

        // Build the char cells for this visual chunk with overlays.
        let mut cells: Vec<(char, Style)> = Vec::new();
        #[allow(clippy::needless_range_loop)]
        for ci in cstart..cend.min(all.len()) {
            let (ch, st) = all[ci];
            let mut bg = line_bg;
            if f.char_selected(logical, ci) {
                bg = SELECT;
            } else if matches
                .iter()
                .any(|&(mr, mc, ml)| mr == logical && ci >= mc && ci < mc + ml)
            {
                bg = match_bg;
            }
            let fg = st.fg.unwrap_or(FG);
            cells.push((ch, Style::default().fg(fg).bg(bg)));
        }

        // Cursor placement within this chunk.
        let cursor_in_chunk = is_current && f.cursor_col >= cstart && f.cursor_col <= cend;
        let mut cur_idx: i32 = if cursor_in_chunk {
            (f.cursor_col - cstart) as i32
        } else {
            -1
        };
        if cur_idx as usize > cells.len() {
            cur_idx = -1;
        }

        for (i, (ch, st)) in cells.into_iter().enumerate() {
            if i as i32 == cur_idx {
                spans.push(cursor_cell(focus, ch, line_bg));
                cur_idx = -1;
            }
            spans.push(Span::styled(ch.to_string(), st));
        }
        // Trailing cursor (end of line / end of chunk).
        if cur_idx >= 0 {
            if focus {
                spans.push(Span::styled(
                    "\u{2588}",
                    Style::default()
                        .fg(BG)
                        .bg(WARN)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(" ", Style::default().bg(ACCENT)));
            }
        }

        lines.push(Line::from(spans));
    }

    frame.render_widget(
        Paragraph::new(Text::from(lines)).style(Style::default().bg(BG)),
        code_rect,
    );
}

/// Render the cursor cell. When focused, a solid block hides the char; when
/// unfocused, an inverted (highlighted) cell keeps the char visible so the
/// user can always see where the cursor is.
fn cursor_cell(focus: bool, ch: char, _line_bg: Color) -> Span<'static> {
    if focus {
        Span::styled(
            "\u{2588}",
            Style::default()
                .fg(BG)
                .bg(WARN)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            ch.to_string(),
            Style::default()
                .fg(BG)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD),
        )
    }
}

fn draw_editor_status(frame: &mut Frame, area: Rect, app: &App) {
    let Some(f) = app.editor.active_file() else {
        return;
    };
    let mode = if app.keybind_mode == crate::app::KeybindMode::Neovim {
        match app.editor.mode {
            EditMode::Edit => "INSERT",
            _ => "NORMAL",
        }
    } else {
        match app.editor.mode {
            EditMode::Edit => "INS",
            _ => "PREVIEW",
        }
    };
    let wrap = if app.editor.word_wrap { "Wrap " } else { "" };
    let line = Line::from(vec![
        Span::styled(
            format!(" Ln {}, Col {} ", f.cursor_row + 1, f.cursor_col + 1),
            Style::default().fg(FG),
        ),
        Span::styled("   Spaces: 4 ", Style::default().fg(DIM)),
        Span::styled("   UTF-8 ", Style::default().fg(DIM)),
        Span::styled("   LF ", Style::default().fg(DIM)),
        Span::styled(format!("   {} ", f.language()), Style::default().fg(DIM)),
        Span::styled(format!("   {wrap}"), Style::default().fg(WARN)),
        Span::styled(format!("   [{mode}]",), Style::default().fg(ACCENT)),
    ]);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(SURFACE)),
        area,
    );
}

// ── Right panel stack ──────────────────────────────────────────────

fn draw_right_panel_stack(frame: &mut Frame, area: Rect, app: &mut App) {
    let focus = app.focus == Focus::RightPanel;
    let mut constraints: Vec<Constraint> = Vec::new();
    for &exp in &app.right_expanded {
        if exp {
            constraints.push(Constraint::Min(5));
        } else {
            constraints.push(Constraint::Length(1));
        }
    }
    let chunks = Layout::vertical(&constraints).split(area);
    draw_right_panel(frame, chunks[0], app, 0, "ROS2 GRAPH", focus, |f, a, ap| {
        draw_ros2_graph_canvas(f, a, ap, false)
    });
    draw_right_panel(
        frame,
        chunks[1],
        app,
        1,
        "ROS2 ENTITIES",
        focus,
        draw_entities,
    );
    draw_right_panel(frame, chunks[2], app, 2, "TELEMETRY", focus, draw_telemetry);
    draw_right_panel(
        frame,
        chunks[3],
        app,
        3,
        "SANDBOX MANAGER",
        focus,
        draw_sandbox_manager,
    );
}

fn draw_right_panel<F>(
    frame: &mut Frame,
    area: Rect,
    app: &mut App,
    idx: usize,
    title: &str,
    focus: bool,
    content: F,
) where
    F: FnOnce(&mut Frame, Rect, &App),
{
    let expanded = app.right_expanded[idx];
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(if focus { ACCENT } else { BORDER })
        .border_type(BorderType::Plain);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let header = Line::from(vec![
        Span::styled(
            title,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{} ⟳ ⛶", if expanded { "  " } else { "  ▴" }),
            Style::default().fg(DIM).add_modifier(Modifier::BOLD),
        ),
    ]);
    let ha = Rect::new(inner.x, inner.y, inner.width, 1);
    frame.render_widget(
        Paragraph::new(header).style(Style::default().bg(SURFACE)),
        ha,
    );
    app.hit_regions.push((ha, HitTarget::RightPanelHeader(idx)));

    if expanded && inner.height > 1 {
        let content_area = Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1);
        // Only the Entities panel has clickable rows; register it so
        // `handle_click` can run its (coordinate-based) row logic.
        if idx == 1 {
            app.hit_regions
                .push((content_area, HitTarget::RightPanelEntity(0)));
        }
        content(frame, content_area, app);
    }
}

// ── ROS2 Graph (Canvas) ────────────────────────────────────────────

fn draw_ros2_graph_canvas(frame: &mut Frame, area: Rect, app: &App, full: bool) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width < 10 || inner.height < 5 {
        return;
    }

    let graph = match &app.graph {
        Some(g) => g,
        None => {
            frame.render_widget(
                Paragraph::new("Collecting graph...").style(Style::default().fg(DIM)),
                inner,
            );
            return;
        }
    };

    let mut all_nodes: Vec<String> = graph.nodes.keys().cloned().collect();
    all_nodes.sort();
    let mut topics: Vec<String> = Vec::new();
    for n in &all_nodes {
        for t in &graph.nodes[n].pubs {
            if !topics.contains(t) {
                topics.push(t.clone());
            }
        }
        for t in &graph.nodes[n].subs {
            if !topics.contains(t) {
                topics.push(t.clone());
            }
        }
    }
    topics.sort();

    if all_nodes.is_empty() {
        frame.render_widget(
            Paragraph::new("  No active nodes.").style(Style::default().fg(DIM)),
            inner,
        );
        return;
    }

    // 3-column layout: Publishers (left) | Topics (center) | Subscribers (right)
    let col_w = (inner.width as f64) / 3.0;
    let x_pub = 1.0;
    let x_topic = col_w + 1.0;
    let x_sub = col_w * 2.0 + 1.0;

    // Collect publisher and subscriber node names (deduplicated).
    let mut pub_nodes: Vec<String> = Vec::new();
    let mut sub_nodes: Vec<String> = Vec::new();
    for n in &all_nodes {
        if !graph.nodes[n].pubs.is_empty() && !pub_nodes.contains(n) {
            pub_nodes.push(n.clone());
        }
        if !graph.nodes[n].subs.is_empty() && !sub_nodes.contains(n) {
            sub_nodes.push(n.clone());
        }
    }

    let row_h = (inner.height as f64 - 2.0)
        / (pub_nodes.len().max(topics.len()).max(sub_nodes.len()) as f64).max(1.0);

    let pub_pos: Vec<(f64, f64)> = pub_nodes
        .iter()
        .enumerate()
        .map(|(i, _)| (x_pub, 1.0 + i as f64 * row_h + row_h / 2.0))
        .collect();
    let topic_pos: Vec<(f64, f64)> = topics
        .iter()
        .enumerate()
        .map(|(i, _)| (x_topic, 1.0 + i as f64 * row_h + row_h / 2.0))
        .collect();
    let sub_pos: Vec<(f64, f64)> = sub_nodes
        .iter()
        .enumerate()
        .map(|(i, _)| (x_sub, 1.0 + i as f64 * row_h + row_h / 2.0))
        .collect();

    let canvas = Canvas::default()
        .block(Block::default())
        .x_bounds([0.0, inner.width as f64])
        .y_bounds([0.0, inner.height as f64])
        .paint(|ctx| {
            // Draw column labels
            ctx.print(
                x_pub,
                0.0,
                Line::from(Span::styled("Publishers", Style::default().fg(DIM))),
            );
            ctx.print(
                x_topic,
                0.0,
                Line::from(Span::styled("Topics", Style::default().fg(DIM))),
            );
            ctx.print(
                x_sub,
                0.0,
                Line::from(Span::styled("Subscribers", Style::default().fg(DIM))),
            );

            // Edges: pub → topic
            for (pi, pn) in pub_nodes.iter().enumerate() {
                let (px, py) = pub_pos[pi];
                for t in &graph.nodes[pn].pubs {
                    if let Some(ti) = topics.iter().position(|x| x == t) {
                        let (tx, ty) = topic_pos[ti];
                        ctx.draw(&CLine {
                            x1: px + node_label_w(pn) as f64,
                            y1: py,
                            x2: tx,
                            y2: ty,
                            color: ACCENT,
                        });
                    }
                }
            }
            // Edges: topic → sub
            for (si, sn) in sub_nodes.iter().enumerate() {
                let (sx, sy) = sub_pos[si];
                for t in &graph.nodes[sn].subs {
                    if let Some(ti) = topics.iter().position(|x| x == t) {
                        let (tx, ty) = topic_pos[ti];
                        ctx.draw(&CLine {
                            x1: tx + topic_label_w(t) as f64,
                            y1: ty,
                            x2: sx,
                            y2: sy,
                            color: WARN,
                        });
                    }
                }
            }

            // Publisher node boxes
            for (pi, n) in pub_nodes.iter().enumerate() {
                let (x, y) = pub_pos[pi];
                let sel = app.selected_node.as_deref() == Some(n.as_str());
                let w = node_label_w(n) as f64;
                ctx.draw(&Rectangle {
                    x,
                    y: y - 0.5,
                    width: w,
                    height: 1.0,
                    color: if sel { ACCENT } else { OK },
                });
                ctx.print(
                    x,
                    y,
                    Line::from(Span::styled(n.clone(), Style::default().fg(FG))),
                );
            }
            // Topic boxes
            for (ti, t) in topics.iter().enumerate() {
                let (x, y) = topic_pos[ti];
                let sel = app.selected_node.as_deref() == Some(t.as_str());
                let w = topic_label_w(t) as f64;
                ctx.draw(&Rectangle {
                    x,
                    y: y - 0.5,
                    width: w,
                    height: 1.0,
                    color: if sel { ACCENT } else { MAGENTA },
                });
                ctx.print(
                    x,
                    y,
                    Line::from(Span::styled(t.clone(), Style::default().fg(FG))),
                );
            }
            // Subscriber node boxes
            for (si, n) in sub_nodes.iter().enumerate() {
                let (x, y) = sub_pos[si];
                let sel = app.selected_node.as_deref() == Some(n.as_str());
                let w = node_label_w(n) as f64;
                ctx.draw(&Rectangle {
                    x,
                    y: y - 0.5,
                    width: w,
                    height: 1.0,
                    color: if sel { ACCENT } else { OK },
                });
                ctx.print(
                    x,
                    y,
                    Line::from(Span::styled(n.clone(), Style::default().fg(FG))),
                );
            }
            let _ = full;
        });
    frame.render_widget(canvas, inner);
}

fn node_label_w(s: &str) -> u16 {
    (s.chars().count() as u16 + 2).min(20)
}
fn topic_label_w(s: &str) -> u16 {
    (s.chars().count() as u16 + 2).min(20)
}

// ── ROS2 Entities ──────────────────────────────────────────────────

fn draw_entities(frame: &mut Frame, area: Rect, app: &App) {
    let tabs = ["Nodes", "Topics", "Services", "Actions"];
    let titles: Vec<Line> = tabs
        .iter()
        .enumerate()
        .map(|(i, t)| {
            if i == app.entities_tab {
                Line::from(Span::styled(
                    format!(" {t} "),
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::styled(format!(" {t} "), Style::default().fg(DIM)))
            }
        })
        .collect();
    let tab_area = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(
        Tabs::new(titles).style(Style::default().bg(SURFACE)),
        tab_area,
    );

    let body = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(1),
    );
    let rows: Vec<Row> = match app.entities_tab {
        0 => app
            .ros2
            .as_ref()
            .map(|r| {
                r.nodes
                    .iter()
                    .map(|n| entity_row(n, "Node", "-", OK))
                    .collect()
            })
            .unwrap_or_default(),
        1 => app
            .ros2
            .as_ref()
            .map(|r| {
                r.topics
                    .iter()
                    .map(|(n, t)| entity_row(n, t, "10", OK))
                    .collect()
            })
            .unwrap_or_default(),
        2 => app
            .ros2
            .as_ref()
            .map(|r| {
                r.services
                    .iter()
                    .map(|n| entity_row(n, "Service", "-", WARN))
                    .collect()
            })
            .unwrap_or_default(),
        _ => app
            .ros2
            .as_ref()
            .map(|r| {
                r.actions
                    .iter()
                    .map(|n| entity_row(n, "Action", "-", MAGENTA))
                    .collect()
            })
            .unwrap_or_default(),
    };

    let widths = [
        Constraint::Min(10),
        Constraint::Min(8),
        Constraint::Length(5),
        Constraint::Length(3),
    ];
    let table = Table::new(rows, widths)
        .style(Style::default().bg(BG))
        .header(
            Row::new(vec!["Name", "Type", "Hz", ""])
                .style(Style::default().fg(DIM).add_modifier(Modifier::BOLD)),
        )
        .column_spacing(1);
    frame.render_widget(table, body);
}

fn entity_row<'a>(name: &'a str, r#type: &'a str, hz: &'a str, color: Color) -> Row<'a> {
    Row::new(vec![
        Cell::from(Span::styled(name.to_string(), Style::default().fg(ACCENT))),
        Cell::from(Span::styled(r#type.to_string(), Style::default().fg(DIM))),
        Cell::from(Span::styled(hz.to_string(), Style::default().fg(FG))),
        Cell::from(Span::styled(
            "●",
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )),
    ])
}

// ── Telemetry sparklines ───────────────────────────────────────────

fn draw_telemetry(frame: &mut Frame, area: Rect, app: &App) {
    // Render the collected ROS 2 node logs (the system CPU/MEM/NET/DISK
    // sparklines are shown on the Overview tab). The collected logs were
    // previously gathered but never displayed — `render_telemetry` does that.
    crate::telemetry::render_telemetry(frame, area, &app.telemetry);
}

// ── Sandbox manager ────────────────────────────────────────────────

fn draw_sandbox_manager(frame: &mut Frame, area: Rect, app: &App) {
    if area.height < 3 {
        return;
    }
    let active = app.sandbox_label();
    let top = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Active: ", Style::default().fg(DIM)),
            Span::styled(
                active.to_string(),
                Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD),
            ),
        ]))
        .style(Style::default().bg(BG)),
        top,
    );

    let table_h = area.height.saturating_sub(2 + 2); // minus active + button
    let tbody = Rect::new(area.x, area.y + 1, area.width, table_h);
    let rows = vec![Row::new(vec![
        Cell::from(Span::styled("sandbox_dev", Style::default().fg(FG))),
        Cell::from(Span::styled("●", Style::default().fg(OK))),
        Cell::from(Span::styled("2m ago", Style::default().fg(DIM))),
    ])];
    let widths = [
        Constraint::Min(10),
        Constraint::Length(3),
        Constraint::Min(6),
    ];
    frame.render_widget(
        Table::new(rows, widths)
            .header(Row::new(vec!["Name", "", "Modified"]).style(Style::default().fg(DIM)))
            .style(Style::default().bg(BG)),
        tbody,
    );

    let btn = Rect::new(
        area.x + 1,
        area.y + area.height.saturating_sub(2),
        area.width.saturating_sub(2),
        1,
    );
    frame.render_widget(
        Paragraph::new(" Export to Global Workspace ")
            .style(
                Style::default()
                    .bg(MAGENTA)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center),
        btn,
    );
}

// ── Terminal panel ─────────────────────────────────────────────────

fn draw_terminal_panel(frame: &mut Frame, area: Rect, app: &mut App) {
    let focus = app.focus == Focus::Terminal;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(focus_style(focus))
        .border_type(BorderType::Plain);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    // Terminal session tabs (like VS Code terminal tabs).
    // The whole strip is kept on ONE row: tabs are laid out left-to-right and
    // clipped once they would overrun the space reserved for `+`. Without this
    // the single `Line` of tab spans wraps onto the sub-tab row, breaking the
    // layout and making `+`/close clicks resolve to the wrong widget.
    let session_count = app.terminal_mgr.sessions.len();
    let plus_w = 2u16;
    let gap = 1u16;
    let max_tabs = inner.width.saturating_sub(plus_w + gap); // space for `+ _`
    let mut tab_spans: Vec<Span> = Vec::new();
    let mut cursor_x = 0u16;
    for (i, sess) in app.terminal_mgr.sessions.iter().enumerate() {
        let active = i == app.terminal_mgr.active;
        let style = if active {
            Style::default()
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD)
                .bg(SURFACE_HI)
        } else {
            Style::default().fg(DIM).bg(SURFACE)
        };
        let icon = if sess.is_ai { "🤖" } else { ">" };
        // Compact label (just the session number) so several terminals fit on
        // one row even in a narrow panel; the `×` closes, `+` adds.
        let num = sess.name.split(':').next().unwrap_or(&sess.name);
        let label = if sess.is_ai { "AI" } else { num };
        let text = format!(" {icon}{label} ");
        let tw = dw(&text);
        // Stop before the last tab would cross into the `+` reserved area.
        let need = tw + if i > 0 { dw("×") } else { 0u16 };
        if cursor_x + need > max_tabs {
            break;
        }
        // Register the hit rect for this tab in the unified hit-test map.
        let tab_rect = Rect::new(inner.x + cursor_x, inner.y, tw, 1);
        app.hit_regions
            .push((tab_rect, HitTarget::TerminalSession(i)));
        tab_spans.push(Span::styled(text, style));
        cursor_x += tw;
        if i > 0 {
            let sep = "×";
            let sep_w = dw(sep);
            let close_rect = Rect::new(inner.x + cursor_x, inner.y, sep_w, 1);
            app.hit_regions
                .push((close_rect, HitTarget::TerminalClose(i)));
            tab_spans.push(Span::styled(sep, Style::default().fg(DIM)));
            cursor_x += sep_w;
        }
        if i < session_count - 1 {
            let pipe = "│";
            let pipe_w = dw(pipe);
            tab_spans.push(Span::styled(pipe, Style::default().fg(BORDER)));
            cursor_x += pipe_w;
        }
    }
    // One-column gap so `+` is clearly distinct from the last close `×`.
    let plus_rect = Rect::new(inner.x + cursor_x + gap, inner.y, plus_w, 1);
    app.hit_regions.push((plus_rect, HitTarget::TerminalPlus));
    tab_spans.push(Span::styled(
        " ".repeat(gap as usize),
        Style::default().fg(BORDER),
    ));
    tab_spans.push(Span::styled(" +", Style::default().fg(DIM)));

    let tab_area = Rect::new(inner.x, inner.y, inner.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(tab_spans)).style(Style::default().bg(SURFACE)),
        tab_area,
    );

    // Sub-tabs: TERMINAL | PROBLEMS | OUTPUT | DEBUG CONSOLE
    let sub_tabs = ["TERMINAL", "PROBLEMS", "OUTPUT", "DEBUG CONSOLE"];
    let problems_count = app
        .diagnostics
        .as_ref()
        .map(|d| d.issues.len())
        .unwrap_or(0);
    let sub_labels: Vec<String> = sub_tabs
        .iter()
        .enumerate()
        .map(|(i, t)| {
            if i == 1 {
                format!(" {t} ({problems_count}) ")
            } else {
                format!(" {t} ")
            }
        })
        .collect();
    let sub_tab_area = Rect::new(inner.x, inner.y + 1, inner.width, 1);
    // Render sub-tabs manually so the active one gets a clear background
    // highlight (the `Tabs` widget has no selected index here and only tinted
    // text, which is easy to miss). Each label is a distinct clickable region.
    let sub_constraints: Vec<Constraint> = sub_labels
        .iter()
        .map(|l| Constraint::Length(dw(l)))
        .collect();
    let sub_rects: Vec<Rect> = Layout::horizontal(&sub_constraints)
        .split(sub_tab_area)
        .to_vec();
    let mut sub_spans: Vec<Span> = Vec::new();
    for (i, r) in sub_rects.iter().enumerate() {
        let active = i == app.terminal_tab;
        let style = if active {
            Style::default()
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD)
                .bg(SELECT)
        } else {
            Style::default().fg(DIM).bg(SURFACE)
        };
        sub_spans.push(Span::styled(sub_labels[i].clone(), style));
        app.hit_regions.push((*r, HitTarget::TerminalSubTab(i)));
    }
    frame.render_widget(
        Paragraph::new(Line::from(sub_spans)).style(Style::default().bg(SURFACE)),
        sub_tab_area,
    );

    let body = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(2),
    );
    app.hit_regions.push((body, HitTarget::TerminalBody));
    match app.terminal_tab {
        0 => draw_terminal_screen(frame, body, app),
        1 => draw_problems(frame, body, app),
        // OUTPUT: the ring-buffered ROS 2 telemetry log (distinct from the
        // live PTY screen, so it isn't a duplicate of the TERMINAL tab).
        2 => draw_output(frame, body, app),
        // DEBUG CONSOLE: AI session command history / events. Also distinct.
        _ => draw_debug_console(frame, body, app),
    }
}

fn draw_output(frame: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(SURFACE_HI))
        .title(" OUTPUT ");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if app.telemetry.entries.is_empty() {
        frame.render_widget(
            Paragraph::new("  No ROS 2 telemetry captured yet.").style(Style::default().fg(DIM)),
            inner,
        );
        return;
    }
    let msg_w = inner.width.saturating_sub(20);
    for (i, e) in app.telemetry.entries.iter().rev().enumerate() {
        if i as u16 >= inner.height {
            break;
        }
        let fg = e.level.color();
        let ts = e.timestamp.chars().take(12).collect::<String>();
        let y = inner.y + i as u16;
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(ts, Style::default().fg(DIM)),
                Span::raw(" "),
                Span::styled(e.level.label().to_string(), Style::default().fg(fg)),
                Span::raw(" "),
                Span::styled(truncate_label(&e.message, msg_w), Style::default().fg(FG)),
            ])),
            Rect::new(inner.x, y, inner.width, 1),
        );
    }
}

fn draw_debug_console(frame: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(SURFACE_HI))
        .title(" DEBUG CONSOLE ");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut y = inner.y;
    let intro = "No debug adapter attached. AI task sessions are shown below.";
    frame.render_widget(
        Paragraph::new(Span::styled(intro.to_string(), Style::default().fg(DIM))),
        Rect::new(inner.x, y, inner.width, 1),
    );
    y += 1;
    for s in app.terminal_mgr.sessions.iter() {
        if y >= inner.y + inner.height {
            break;
        }
        if let Some(last) = s.ai_history.last() {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(format!("[{}] ", s.name), Style::default().fg(ACCENT)),
                    Span::styled(
                        truncate_label(last, inner.width.saturating_sub(s.name.len() as u16 + 4)),
                        Style::default().fg(FG),
                    ),
                ])),
                Rect::new(inner.x, y, inner.width, 1),
            );
            y += 1;
        }
    }
}

fn draw_terminal_screen(frame: &mut Frame, area: Rect, app: &mut App) {
    // Only resize the PTY when the pane actually changes size, otherwise we
    // would recreate the vt100 parser every frame and wipe the screen.
    let (cur_rows, cur_cols) = {
        let s = app.terminal_mgr.active_session();
        (
            s.map(|x| x.rows).unwrap_or(0),
            s.map(|x| x.cols).unwrap_or(0),
        )
    };
    if cur_rows != area.height || cur_cols != area.width {
        app.terminal_mgr.resize_active(area.height, area.width);
    }
    app.terminal_mgr.pump();
    let Some(sess) = app.terminal_mgr.active_session_mut() else {
        return;
    };
    sess.parser.set_scrollback(sess.scrollback);
    let screen = sess.parser.screen();
    // Terminal defaults: light text on dark background (Dracula-inspired)
    let term_fg = FG;
    let term_bg = Color::Rgb(20, 20, 30);
    let (cur_row, cur_col) = screen.cursor_position();
    let cursor_visible = !screen.hide_cursor();
    let mut lines: Vec<Line> = Vec::new();
    for r in 0..area.height {
        let mut spans: Vec<Span> = Vec::new();
        for c in 0..area.width {
            let is_cursor = cursor_visible && r == cur_row && c == cur_col;
            if let Some(cell) = screen.cell(r, c) {
                let content = cell.contents();
                if content.is_empty() {
                    if is_cursor {
                        spans.push(Span::styled(
                            "_",
                            Style::default()
                                .fg(term_bg)
                                .bg(ACCENT)
                                .add_modifier(Modifier::BOLD),
                        ));
                    } else {
                        spans.push(Span::styled(" ", Style::default().fg(term_fg).bg(term_bg)));
                    }
                } else {
                    let fg = map_color(cell.fgcolor()).unwrap_or(term_fg);
                    let bg = map_color(cell.bgcolor()).unwrap_or(term_bg);
                    let mut style = if is_cursor {
                        Style::default()
                            .fg(term_bg)
                            .bg(ACCENT)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(fg).bg(bg)
                    };
                    if cell.bold() {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    if cell.underline() {
                        style = style.add_modifier(Modifier::UNDERLINED);
                    }
                    if cell.inverse() {
                        if is_cursor {
                            style = Style::default()
                                .fg(term_bg)
                                .bg(ACCENT)
                                .add_modifier(Modifier::BOLD);
                        } else {
                            style = Style::default().fg(bg).bg(fg);
                        }
                    }
                    spans.push(Span::styled(content, style));
                }
            } else {
                if is_cursor {
                    spans.push(Span::styled(
                        "_",
                        Style::default()
                            .fg(term_bg)
                            .bg(ACCENT)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else {
                    spans.push(Span::styled(" ", Style::default().fg(term_fg).bg(term_bg)));
                }
            }
        }
        lines.push(Line::from(spans));
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines)).style(Style::default().bg(term_bg)),
        area,
    );
}

fn draw_problems(frame: &mut Frame, area: Rect, app: &App) {
    let Some(diag) = &app.diagnostics else {
        frame.render_widget(
            Paragraph::new("No diagnostics.").style(Style::default().fg(DIM).bg(BG)),
            area,
        );
        return;
    };
    let mut lines: Vec<Line> = Vec::new();
    for issue in &diag.issues {
        let color = match issue.severity.as_str() {
            "error" => ERROR,
            "warn" | "warning" => WARN,
            _ => DIM,
        };
        let badge = match issue.severity.as_str() {
            "error" => "E",
            "warn" | "warning" => "W",
            _ => "I",
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("[{badge}] "),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(issue.message.clone(), Style::default().fg(color)),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  ✓ No problems found.",
            Style::default().fg(OK),
        )));
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines)).style(Style::default().bg(BG)),
        area,
    );
}

// ── Status bar ─────────────────────────────────────────────────────

fn draw_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let mode_color = app.mode_color();
    let cpu_pct = app.system.as_ref().map(|s| s.cpu_percent).unwrap_or(0.0);
    let mem_pct = app.system.as_ref().map(|s| s.mem_percent).unwrap_or(0.0);
    let mem_str = app
        .system
        .as_ref()
        .map(|s| format!("{:.1}/{:.1}G", s.mem_used_gb, s.mem_total_gb))
        .unwrap_or_default();
    let (ln, col) = app
        .editor
        .active_file()
        .map(|f| (f.cursor_row + 1, f.cursor_col + 1))
        .unwrap_or((0, 0));
    let mode_ind = match app.editor.mode {
        EditMode::Preview => "PREVIEW",
        EditMode::Edit => "INS",
    };

    let (cpu_fill, cpu_track) = meter(cpu_pct, 8);
    let (mem_fill, mem_track) = meter(mem_pct, 6);
    let sep = Span::styled(" │ ", Style::default().fg(BORDER));
    let label = |k: &'static str| Span::styled(k, Style::default().fg(DIM));

    let left = Line::from(vec![
        Span::styled(
            format!(" {} ", app.sandbox_label()),
            Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
        ),
        sep.clone(),
        Span::styled(
            "keep moving",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        sep.clone(),
        label("CPU "),
        cpu_fill,
        cpu_track,
        Span::styled(format!(" {:>3.0}%", cpu_pct), Style::default().fg(DIM)),
        sep.clone(),
        label("MEM "),
        mem_fill,
        mem_track,
        Span::styled(format!(" {}", mem_str), Style::default().fg(DIM)),
    ]);
    let right = Line::from(vec![
        Span::styled(format!("Ln {ln}, Col {col} "), Style::default().fg(FG)),
        Span::styled(format!("│ {mode_ind} "), Style::default().fg(ACCENT)),
        Span::styled("│ ", Style::default().fg(BORDER)),
        Span::styled(
            format!("✦ {}", app.unread_notifications),
            Style::default().fg(DIM),
        ),
    ]);

    frame.render_widget(
        Paragraph::new(Line::from("")).style(Style::default().bg(HEADER_BG)),
        area,
    );
    let half = Layout::horizontal([Constraint::Percentage(75), Constraint::Percentage(25)]);
    let [l, r] = half.areas(area);
    frame.render_widget(
        Paragraph::new(left).style(Style::default().bg(HEADER_BG)),
        l,
    );
    frame.render_widget(
        Paragraph::new(right)
            .style(Style::default().bg(HEADER_BG))
            .alignment(Alignment::Right),
        r,
    );
}

// ── Keybind hint bar ───────────────────────────────────────────────

fn draw_keybind_bar(frame: &mut Frame, area: Rect, app: &App) {
    if app.palette_open {
        let hints = if matches!(app.palette_mode, PaletteMode::Command) {
            "Type to filter commands │ ↑↓: Move │ Enter: Run │ Esc: Close"
        } else if matches!(app.palette_mode, PaletteMode::Launch) {
            "Type to filter launch files │ ↑↓: Move │ Enter: ros2 launch │ Esc: Close"
        } else if matches!(app.palette_mode, PaletteMode::Bag) {
            "Type to filter bags │ ↑↓: Move │ Enter: ros2 bag play │ Esc: Close"
        } else {
            "Type to filter files │ ↑↓: Move │ Enter: Open │ Esc: Close"
        };
        let line = Line::from(Span::styled(
            format!(" {hints}"),
            Style::default().fg(ACCENT),
        ));
        frame.render_widget(
            Paragraph::new(line).style(Style::default().bg(HEADER_BG)),
            area,
        );
        return;
    }
    let hints = match app.focus {
        Focus::Terminal => {
            if app.search_input_active {
                "Esc: Cancel │ Enter: Search │ Backspace: Delete"
            } else {
                "Ctrl+T: New Tab │ Ctrl+Shift+T: Close Tab │ Ctrl+C: Interrupt │ Ctrl+L: Clear │ ↑↓: Scroll │ Esc: Unfocus"
            }
        }
        Focus::Editor => {
            if app.editor.goto_line_input.is_some() {
                "Type line number │ Enter: Go │ Esc: Cancel"
            } else if app.editor.find_active {
                "Type search term │ Enter: Find Next │ Esc: Close"
            } else if app.editor.mode == EditMode::Edit {
                "Type: Insert │ Ctrl+S: Save │ Ctrl+Z: Undo │ Ctrl+Y: Redo │ Ctrl+G: Goto │ Ctrl+F: Find │ Alt+Z: Wrap │ Esc: Preview"
            } else {
                "i: Edit │ Ctrl+S: Save │ Ctrl+Z: Undo │ Ctrl+G: Goto │ Ctrl+F: Find │ Ctrl+A: End │ Alt+Z: Wrap │ Esc: Back"
            }
        }
        Focus::Sidebar => {
            if app.search_input_active {
                "Type: Search │ Enter: Results │ Esc: Cancel"
            } else {
                "↑↓: Navigate │ Enter/→: Open/Expand │ ←: Collapse │ Ctrl+B: Close Sidebar │ Ctrl+F: Search"
            }
        }
        Focus::RightPanel => "↑↓: Scroll │ Click: Select │ Click Header: Toggle",
        Focus::ActivityBar => "↑↓: Switch View │ Enter: Activate │ Click: Select",
        Focus::None => "Tab: Focus │ Ctrl+Shift+P: Palette │ Ctrl+P: Go to File │ Ctrl+B: Sidebar │ Ctrl+`: Terminal │ F6: Sandbox │ F1: Help │ Esc: Quit",
    };
    let line = Line::from(Span::styled(format!(" {hints}"), Style::default().fg(DIM)));
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(HEADER_BG)),
        area,
    );
}

// ── Dashboards (non-Workspace tabs) ────────────────────────────────

fn draw_dashboard(frame: &mut Frame, area: Rect, app: &App) {
    match app.tab() {
        Tab::Overview => draw_overview(frame, area, app),
        Tab::Ros2 => draw_ros2_dash(frame, area, app),
        Tab::Workspace => unreachable!(),
        Tab::Diagnostics => draw_diag_dash(frame, area, app),
        Tab::Trends => draw_trends_dash(frame, area, app),
        Tab::Fleet => draw_fleet_dash(frame, area, app),
    }
}

fn panel_block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER))
        .title(format!(" {title} "))
        .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
}

fn draw_overview(frame: &mut Frame, area: Rect, app: &App) {
    let chunks =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);
    draw_system_panel(frame, chunks[0], app);
    draw_ros2_summary(frame, chunks[1], app);
}

fn draw_system_panel(frame: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("System");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(sys) = &app.system else {
        frame.render_widget(
            Paragraph::new("Collecting...").style(Style::default().fg(DIM)),
            inner,
        );
        return;
    };
    let lines = vec![
        Line::from(Span::styled(
            format!(" {} | {} ", sys.hostname, sys.os_name),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!(" CPU: {:.1}%", sys.cpu_percent),
            Style::default().fg(OK),
        )),
        Line::from(Span::styled(
            format!(" MEM: {:.1}%", sys.mem_percent),
            Style::default().fg(WARN),
        )),
        Line::from(Span::styled(
            format!(" DISK: {:.1}%", sys.disk_percent),
            Style::default().fg(DIM),
        )),
        Line::from(Span::styled(
            format!(" NET ↑{:.1} ↓{:.1} MB", sys.net_sent_mb, sys.net_recv_mb),
            Style::default().fg(DIM),
        )),
        Line::from(Span::styled(
            format!(" Cores: {} @ {:.0}MHz", sys.cpu_cores, sys.cpu_freq),
            Style::default().fg(DIM),
        )),
    ];
    frame.render_widget(
        Paragraph::new(Text::from(lines)).style(Style::default().bg(BG)),
        inner,
    );
}

fn draw_ros2_summary(frame: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("ROS2");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(ros) = &app.ros2 else {
        frame.render_widget(
            Paragraph::new("Collecting...").style(Style::default().fg(DIM)),
            inner,
        );
        return;
    };
    let lines = vec![
        Line::from(Span::styled(
            format!(" Distro: {}", ros.distro),
            Style::default().fg(OK),
        )),
        Line::from(Span::styled(
            format!(" DDS: {}", ros.dds),
            Style::default().fg(DIM),
        )),
        Line::from(Span::styled(
            format!(" RMW: {}", ros.rmw),
            Style::default().fg(DIM),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!(" Nodes: {}", ros.nodes.len()),
            Style::default().fg(OK),
        )),
        Line::from(Span::styled(
            format!(" Topics: {}", ros.topics.len()),
            Style::default().fg(ACCENT),
        )),
        Line::from(Span::styled(
            format!(" Services: {}", ros.services.len()),
            Style::default().fg(WARN),
        )),
        Line::from(Span::styled(
            format!(" Actions: {}", ros.actions.len()),
            Style::default().fg(MAGENTA),
        )),
    ];
    frame.render_widget(
        Paragraph::new(Text::from(lines)).style(Style::default().bg(BG)),
        inner,
    );
}

fn draw_ros2_dash(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(area);
    let block = panel_block("ROS2 Live");
    let inner = block.inner(chunks[0]);
    frame.render_widget(block, chunks[0]);
    if let Some(ros) = &app.ros2 {
        let line = Line::from(Span::styled(
            format!(
                " Nodes: {} | Topics: {} | Services: {} | Mode: {} ",
                ros.nodes.len(),
                ros.topics.len(),
                ros.services.len(),
                app.sandbox_label()
            ),
            Style::default().fg(ACCENT),
        ));
        frame.render_widget(Paragraph::new(line).style(Style::default().bg(BG)), inner);
    }
    draw_entities(frame, chunks[1], app);
}

fn draw_diag_dash(frame: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("Diagnostics");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    draw_problems(frame, inner, app);
}

fn draw_trends_dash(frame: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("Trends");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(t) = &app.trends else {
        frame.render_widget(
            Paragraph::new("Collecting...").style(Style::default().fg(DIM)),
            inner,
        );
        return;
    };
    let mut lines: Vec<Line> = vec![Line::from(Span::styled(
        " Historical averages:",
        Style::default().fg(OK),
    ))];
    for (k, v) in &t.summary {
        lines.push(Line::from(Span::styled(
            format!("  {k}: {v:.1}"),
            Style::default().fg(DIM),
        )));
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines)).style(Style::default().bg(BG)),
        inner,
    );
}

fn draw_fleet_dash(frame: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("Fleet");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(f) = &app.fleet else {
        frame.render_widget(
            Paragraph::new("No fleet data.").style(Style::default().fg(DIM)),
            inner,
        );
        return;
    };
    let rows: Vec<Row> = f
        .hosts
        .iter()
        .map(|h| {
            Row::new(vec![
                Cell::from(Span::styled(
                    if h.reachable { "✓" } else { "✗" },
                    Style::default().fg(if h.reachable { OK } else { ERROR }),
                )),
                Cell::from(Span::styled(
                    h.hostname.clone(),
                    Style::default().fg(ACCENT),
                )),
                Cell::from(Span::styled(
                    h.ros_distro.clone().unwrap_or_default(),
                    Style::default().fg(DIM),
                )),
            ])
        })
        .collect();
    let widths = [
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Min(8),
    ];
    frame.render_widget(
        Table::new(rows, widths).style(Style::default().bg(BG)),
        inner,
    );
}

// ── Overlays ───────────────────────────────────────────────────────

/// A rounded, floating modal block — the modern TUI aesthetic for surfaces
/// that float over the app (palette, confirm, prompt, help, context menu).
/// Main panels below stay single-line `Plain`; reserving round for modals
/// keeps the chrome-vs-data hierarchy readable.
fn modal_block<'a>(title: &str, accent: Color) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .title(format!(" {title} "))
        .title_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(SURFACE))
}

/// Floor guard content — what the user sees when the terminal is too small
/// for the multi-pane layout. Tells them exactly what they need, no mangled
/// half-panels, no crash.
fn draw_too_small(frame: &mut Frame, area: Rect, app: &App) {
    frame.render_widget(Clear, area);
    let need = format!("{MIN_W} × {MIN_H}");
    let have = format!("{} × {}", area.width, area.height);
    let block = modal_block("Terminal too small", WARN);
    let inner = block.inner(area);
    // If even the message block won't fit, fall back to a bare one-liner.
    if inner.width < 8 || inner.height < 4 {
        frame.render_widget(
            Paragraph::new(format!("Terminal too small (need {need})"))
                .style(Style::default().fg(WARN).bg(BG)),
            area,
        );
        return;
    }
    frame.render_widget(block, area);
    let mut lines = vec![
        Line::from(Span::styled(
            " ⚠  Terminal too small",
            Style::default().fg(WARN).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  need   ", Style::default().fg(DIM)),
            Span::styled(
                need,
                Style::default().fg(ERROR).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  have   ", Style::default().fg(DIM)),
            Span::styled(have, Style::default().fg(FG)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ROS2_INFO needs ", Style::default().fg(DIM)),
            Span::styled(
                format!("{MIN_W}×{MIN_H}"),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" for its multi-pane layout.", Style::default().fg(DIM)),
        ]),
        Line::from(Span::styled(
            "  Widen the terminal, split a smaller tmux pane, or zoom the pane (Ctrl+Z).",
            Style::default().fg(DIM),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  TUI still responds — type  Esc  to quit, or widen to resume.",
            Style::default().fg(DIM),
        )),
    ];
    let _ = app; // shape kept for future context (battery, ROS status) in the floor message
    if inner.height < lines.len() as u16 {
        lines.truncate(inner.height as usize);
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines)).style(Style::default().bg(SURFACE)),
        inner,
    );
}

fn draw_help_overlay(frame: &mut Frame, area: Rect, _app: &App) {
    let outer = Layout::vertical([
        Constraint::Percentage(5),
        Constraint::Percentage(90),
        Constraint::Percentage(5),
    ]);
    let [_t, mid, _b] = outer.areas(area);
    let inner = Layout::horizontal([
        Constraint::Percentage(10),
        Constraint::Percentage(80),
        Constraint::Percentage(10),
    ]);
    let [_l, help_area, _r] = inner.areas(mid);

    let col_w = help_area.width / 3;

    let col1 = vec![
        Line::from(Span::styled(
            "  NAVIGATION",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Tab / Shift+Tab   Focus next/prev panel",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Esc               Back / Unfocus / Quit",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Alt+1-6           Switch tab directly",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  1-6 (no editor)   Switch tab directly",
            Style::default().fg(FG),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  SIDEBAR",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Ctrl+B            Toggle sidebar",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Ctrl+E            Focus file explorer",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  ↑↓                Navigate tree",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Enter / →         Expand dir / Open file",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  ←                 Collapse directory",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  n / N             New file / New folder",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  F2                Rename item",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Delete            Delete item",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  F5                Refresh tree",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  m / Right-click   Context menu",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  d                 Duplicate item",
            Style::default().fg(FG),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  EDITOR",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  i                 Enter Edit mode",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Ctrl+S            Save file",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Ctrl+X / Ctrl+C   Cut / Copy selection",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Ctrl+V            Paste",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Ctrl+W            Close current tab",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Ctrl+Shift+W      Close all tabs",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Ctrl+F            Search files",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  f / h             Find / Find & Replace",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Ctrl+A            Select all",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Alt+Z             Toggle word wrap",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  PgUp / PgDn       Scroll code",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Shift+Arrows      Select text",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Click line        Place cursor",
            Style::default().fg(FG),
        )),
    ];

    let col2 = vec![
        Line::from(Span::styled(
            "  TERMINAL",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Ctrl+`            Toggle terminal",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Ctrl+T            New terminal tab",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Ctrl+Shift+T      Close terminal tab",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Ctrl+C            Interrupt process",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Ctrl+L            Clear terminal",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  ↑↓ PgUp/PgDn      Scroll terminal",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Click tab          Switch session",
            Style::default().fg(FG),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  SANDBOX",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  F6                Toggle Sandbox/Global",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  [Y] in prompt     Confirm mode switch",
            Style::default().fg(FG),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  AI ASSISTANT",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  ai help           Show AI commands",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  ai scan           Scan build errors",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  ai auto           Autonomous fix mode",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  ai fix <file>     Fix specific file",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  ai explain <err>  Explain an error",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  ai chat <msg>     Chat about code",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  ai web [port]     Start web chat UI",
            Style::default().fg(FG),
        )),
    ];

    let col3 = vec![
        Line::from(Span::styled(
            "  RIGHT PANELS",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Click header       Expand/Collapse",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Click entity       Select node",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Click sub-tab      Switch Nodes/Topics",
            Style::default().fg(FG),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  RESIZE PANELS",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Drag sidebar edge  Resize sidebar",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Drag right edge    Resize right panel",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Drag terminal edge Resize terminal",
            Style::default().fg(FG),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  MOUSE",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Click              Select / Activate",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Scroll wheel       Scroll focused pane",
            Style::default().fg(FG),
        )),
        Line::from(Span::styled(
            "  Drag panel edges   Resize panels",
            Style::default().fg(FG),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  ABOUT",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  ROS2 Info TUI v0.1",
            Style::default().fg(DIM),
        )),
        Line::from(Span::styled(
            "  Built with Rust + Ratatui",
            Style::default().fg(DIM),
        )),
        Line::from(Span::styled(
            "  VS Code Dark+ theme",
            Style::default().fg(DIM),
        )),
    ];

    let block = modal_block("ROS2 Info — Guide", ACCENT);

    let inner_area = block.inner(help_area);
    frame.render_widget(block, help_area);

    if inner_area.width < 30 || inner_area.height < 5 {
        return;
    }

    // Render 3 columns
    let col1_area = Rect::new(inner_area.x, inner_area.y, col_w, inner_area.height);
    let col2_area = Rect::new(inner_area.x + col_w, inner_area.y, col_w, inner_area.height);
    let col3_area = Rect::new(
        inner_area.x + col_w * 2,
        inner_area.y,
        inner_area.width.saturating_sub(col_w * 2),
        inner_area.height,
    );

    frame.render_widget(
        Paragraph::new(Text::from(col1)).style(Style::default().bg(Color::Rgb(20, 20, 40))),
        col1_area,
    );
    frame.render_widget(
        Paragraph::new(Text::from(col2)).style(Style::default().bg(Color::Rgb(20, 20, 40))),
        col2_area,
    );
    frame.render_widget(
        Paragraph::new(Text::from(col3)).style(Style::default().bg(Color::Rgb(20, 20, 40))),
        col3_area,
    );

    // Footer hint
    let footer = Rect::new(
        inner_area.x,
        inner_area.y + inner_area.height.saturating_sub(1),
        inner_area.width,
        1,
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " Press ? or Esc to close",
            Style::default().fg(DIM),
        )))
        .style(Style::default().bg(Color::Rgb(20, 20, 40))),
        footer,
    );
}

fn draw_confirm_overlay(frame: &mut Frame, area: Rect, app: &App) {
    let outer = Layout::vertical([
        Constraint::Percentage(35),
        Constraint::Percentage(30),
        Constraint::Percentage(35),
    ]);
    let [_t, mid, _b] = outer.areas(area);
    let inner = Layout::horizontal([
        Constraint::Percentage(25),
        Constraint::Percentage(50),
        Constraint::Percentage(25),
    ]);
    let [_l, confirm_area, _r] = inner.areas(mid);

    let message = app
        .confirm
        .as_ref()
        .map(|c| c.message.as_str())
        .unwrap_or("Confirm?");
    let text = Text::from(vec![
        Line::from(Span::styled(
            "  Confirm Action",
            Style::default().fg(WARN).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {message}"),
            Style::default().fg(DIM),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  [Y] Yes  [N] No  [Esc] Cancel",
            Style::default().fg(ACCENT),
        )),
    ]);
    let block = modal_block("⚠", WARN);
    frame.render_widget(Paragraph::new(text).block(block), confirm_area);
}

fn draw_ctx_menu(frame: &mut Frame, area: Rect, app: &App) {
    let menu = match &app.ctx_menu {
        Some(m) => m,
        None => return,
    };
    if menu.items.is_empty() {
        return;
    }
    let w = 26u16;
    let h = menu.items.len() as u16 + 2;
    let x = menu.x.min(area.width.saturating_sub(w));
    let y = menu.y.min(area.height.saturating_sub(h));
    let rect = Rect::new(x, y, w, h);
    let block = modal_block("Actions", ACCENT);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let mut lines: Vec<Line> = Vec::new();
    for (i, action) in menu.items.iter().enumerate() {
        let label = crate::App::ctx_action_label(*action);
        let style = if i == app.ctx_menu_sel {
            Style::default()
                .bg(ACCENT)
                .fg(BG)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(FG)
        };
        lines.push(Line::from(Span::styled(format!(" {label}"), style)));
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines)).style(Style::default().bg(SURFACE)),
        inner,
    );
}

fn draw_prompt(frame: &mut Frame, area: Rect, app: &App) {
    let p = match &app.prompt {
        Some(p) => p,
        None => return,
    };
    let w = 50u16.min(area.width.saturating_sub(4));
    let h = 7u16;
    let x = area.width.saturating_sub(w) / 2;
    let y = area.height.saturating_sub(h) / 2;
    let rect = Rect::new(x, y, w, h);
    let block = modal_block(&p.label, WARN);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let mut lines = vec![
        Line::from(Span::styled(
            format!("  {}", p.target.display()),
            Style::default().fg(DIM),
        )),
        Line::from(""),
    ];
    let cursor_marker = "█";
    let input = format!("  {}{}", p.value, cursor_marker);
    lines.push(Line::from(Span::styled(input, Style::default().fg(FG))));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  [Enter] Confirm   [Esc] Cancel",
        Style::default().fg(ACCENT),
    )));
    frame.render_widget(
        Paragraph::new(Text::from(lines)).style(Style::default().bg(SURFACE)),
        inner,
    );
}

/// Interactive Ollama model picker (opened from the command palette as
/// "AI: Choose Model"). Lists locally installed models; the active model is
/// marked and the highlighted row is committed with Enter.
fn draw_model_picker(frame: &mut Frame, area: Rect, app: &App) {
    let n = app.available_models.len();
    // Reserve 2 lines per entry + header + hints; cap to the screen.
    let per = 2u16;
    let list_h = if n == 0 { 3 } else { (n as u16) * per };
    let h = (list_h + 5).min(area.height.saturating_sub(4)).max(6);
    let w = 60u16.min(area.width.saturating_sub(6));
    let x = area.width.saturating_sub(w) / 2;
    let y = area.height.saturating_sub(h) / 2;
    let rect = Rect::new(x, y, w, h);
    let block = modal_block("AI: Choose Model", ACCENT);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        format!("Active: {}", app.ai_model),
        Style::default().fg(DIM),
    )));
    lines.push(Line::from(""));

    if n == 0 {
        lines.push(Line::from(Span::styled(
            "No Ollama models found on localhost:11434.",
            Style::default().fg(WARN),
        )));
        lines.push(Line::from(Span::styled(
            "Install one: ollama pull qwen2.5-coder:7b",
            Style::default().fg(DIM),
        )));
    } else {
        for (i, m) in app.available_models.iter().enumerate() {
            let selected = i == app.model_picker_index;
            let marker = if m == &app.ai_model {
                " ← active"
            } else {
                ""
            };
            let text = format!("{}{}{}", if selected { "› " } else { "  " }, m, marker);
            let style = if selected {
                Style::default().fg(ACCENT).bg(SELECT)
            } else {
                Style::default().fg(FG)
            };
            lines.push(Line::from(Span::styled(text, style)));
            if per > 1 {
                lines.push(Line::from(""));
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "↑/↓ navigate   [Enter] select   [Esc] cancel",
        Style::default().fg(ACCENT),
    )));

    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .style(Style::default().bg(SURFACE))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn focus_style(focus: bool) -> Style {
    if focus {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(BORDER)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, DataEvent};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::sync::mpsc;

    // Regression test: rendering must never panic for any combination of
    // sidebar / terminal / right-panel visibility (this caught a real crash
    // where hiding the sidebar indexed past the layout chunks).
    #[test]
    fn draw_does_not_panic_for_any_panel_combination() {
        let combos = [
            (true, true, true),
            (false, true, true),
            (true, false, true),
            (true, true, false),
            (false, false, true),
            (false, true, false),
            (true, false, false),
            (false, false, false),
        ];
        for (sb, tv, rv) in combos {
            let (_, rx) = mpsc::channel::<DataEvent>();
            let mut app = App::new(rx);
            app.sidebar_visible = sb;
            app.terminal_visible = tv;
            app.right_visible = rv;
            let backend = TestBackend::new(120, 40);
            let mut term = Terminal::new(backend).unwrap();
            term.draw(|f| draw(f, &mut app)).unwrap();
        }
    }

    // Floor: tiny terminals must hit the too-small guard, not panic.
    #[test]
    fn draw_at_floor_does_not_panic() {
        let (_, rx) = mpsc::channel::<DataEvent>();
        let mut app = App::new(rx);
        for &(w, h) in &[(60u16, 15u16), (68, 24), (70, 19), (70, 20), (80, 24)] {
            let backend = TestBackend::new(w, h);
            let mut term = Terminal::new(backend).unwrap();
            term.draw(|f| draw(f, &mut app)).unwrap();
        }
    }
}
