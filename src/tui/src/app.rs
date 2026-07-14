#![allow(dead_code)]
//! cc@zang aka Gaurav-x111

use crate::ai;
use crate::editor::Editor;
use crate::file_tree::FileTree;
use crate::plugin::PluginManager;
use crate::telemetry::TelemetryLog;
use crate::terminal::TerminalManager;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::Instant;

/// Icon rendering tier. Terminals without an emoji-capable font render the
/// emoji/box-emoji glyphs as empty boxes ("tofu"). We default to ASCII which
/// every monospace font ships, and let the user opt into richer glyphs via
/// `ROS2_INFO_ICONS=emoji|nerd`. `auto` picks emoji only when COLORTERM is
/// set (a decent proxy for a modern, font-complete terminal).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IconTier {
    Ascii,
    Emoji,
    Nerd,
}

pub fn icon_tier() -> IconTier {
    // Resolved once; cheap to recompute but stable across a run.
    match std::env::var("ROS2_INFO_ICONS").ok().as_deref() {
        Some("nerd") => IconTier::Nerd,
        Some("emoji") => IconTier::Emoji,
        Some("ascii") => IconTier::Ascii,
        _ => {
            // auto: emoji if a modern colour terminal is detected, else ascii.
            if std::env::var("COLORTERM").is_ok() {
                IconTier::Emoji
            } else {
                IconTier::Ascii
            }
        }
    }
}

/// Pick a glyph for the configured tier. `ascii` must always render on
/// every font (plain ASCII). `emoji` is a U+1F*** pictograph; `nerd` is a
/// Nerd Font private-use codepoint — both can be tofu on minimal fonts.
pub fn glyph(ascii: &'static str, emoji: &'static str, nerd: &'static str) -> &'static str {
    match icon_tier() {
        IconTier::Ascii => ascii,
        IconTier::Emoji => emoji,
        IconTier::Nerd => nerd,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdge {
    SidebarRight,
    RightPanelLeft,
    TerminalTop,
}

/// What kind of entries the command palette is currently listing.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum PaletteMode {
    #[default]
    Command,
    /// Go to File (all workspace files).
    File,
    /// Launch-file picker (only `*.launch.py` / `*.launch.xml` / `*.launch`).
    Launch,
    /// Bag picker (only `*.db3` / `*.mcap` / `*.bag`).
    Bag,
}

#[derive(Clone, Default)]
pub struct SystemData {
    pub hostname: String,
    pub os_name: String,
    pub kernel: String,
    pub uptime_secs: u64,
    pub cpu_percent: f32,
    pub cpu_cores: usize,
    pub cpu_freq: f64,
    pub mem_percent: f32,
    pub mem_used_gb: f64,
    pub mem_total_gb: f64,
    pub disk_percent: f64,
    pub net_sent_mb: f64,
    pub net_recv_mb: f64,
    pub gpu_info: String,
    pub battery_percent: Option<f32>,
    pub temperatures: Vec<(String, f32)>,
}

#[derive(Clone, Default)]
pub struct Ros2Data {
    pub distro: String,
    pub domain_id: String,
    pub dds: String,
    pub rmw: String,
    pub nodes: Vec<String>,
    pub topics: Vec<(String, String)>,
    pub services: Vec<String>,
    pub actions: Vec<String>,
    pub error: Option<String>,
}

#[derive(Clone, Default)]
pub struct WorkspaceData {
    pub workspaces: Vec<String>,
    pub packages: usize,
    pub built_packages: usize,
    pub modified_packages: Vec<String>,
    pub launch_count: usize,
}

#[derive(Clone, Default)]
pub struct DiagnosticsData {
    pub issues: Vec<DiagnosticIssue>,
}

#[derive(Clone)]
pub struct DiagnosticIssue {
    pub severity: String,
    pub message: String,
    pub details: HashMap<String, String>,
}

#[derive(Clone, Default)]
pub struct TrendsData {
    pub summary: HashMap<String, f64>,
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct TrendSnapshot {
    pub timestamp: f64,
    pub cpu_percent: f64,
    pub memory_percent: f64,
    pub disk_percent: f64,
    pub node_count: usize,
    pub topic_count: usize,
}

#[derive(Clone, Default)]
pub struct FleetData {
    pub hosts: Vec<FleetHost>,
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct FleetHost {
    pub hostname: String,
    pub ip: String,
    pub reachable: bool,
    pub uptime: Option<String>,
    pub memory: Option<String>,
    pub disk: Option<String>,
    pub ros2_nodes: Option<usize>,
    pub ros_distro: Option<String>,
}

#[derive(Clone, Default)]
pub struct GraphData {
    pub nodes: HashMap<String, GraphNode>,
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct GraphNode {
    pub pubs: Vec<String>,
    pub subs: Vec<String>,
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct TopicConns {
    pub publishers: Vec<String>,
    pub subscribers: Vec<String>,
}

pub enum Tab {
    Overview,
    Ros2,
    Workspace,
    Diagnostics,
    Trends,
    Fleet,
}

impl Tab {
    pub const COUNT: usize = 6;

    pub fn label(&self) -> &str {
        match self {
            Tab::Overview => "Overview",
            Tab::Ros2 => "ROS2",
            Tab::Workspace => "Workspace",
            Tab::Diagnostics => "Diagnostics",
            Tab::Trends => "Trends",
            Tab::Fleet => "Fleet",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Tab::Overview => glyph("=", "📊", "\u{f1e6}"), // dashboard
            Tab::Ros2 => glyph("*", "🤖", "\u{f2c7}"),     // robot
            Tab::Workspace => glyph(">", "📁", "\u{f07b}"), // folder
            Tab::Diagnostics => glyph("!", "🩺", "\u{f457}"), // stethoscope
            Tab::Trends => glyph("^", "📈", "\u{f201}"),   // chart
            Tab::Fleet => glyph("#", "🏢", "\u{f1ad}"),    // building
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i % Self::COUNT {
            0 => Tab::Overview,
            1 => Tab::Ros2,
            2 => Tab::Workspace,
            3 => Tab::Diagnostics,
            4 => Tab::Trends,
            _ => Tab::Fleet,
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq)]
pub enum SandboxMode {
    #[default]
    Global,
    Sandbox,
}

/// Editor key-binding scheme. `Normal` is the default: typing inserts text
/// immediately (non-modal). `Neovim` is modal — `Preview` acts as vim's NORMAL
/// mode (letters are commands; press `i`/`a`/`o` to enter INSERT).
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum KeybindMode {
    #[default]
    Normal,
    Neovim,
}

impl KeybindMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            KeybindMode::Normal => "normal",
            KeybindMode::Neovim => "neovim",
        }
    }

    pub fn from_str(s: &str) -> Self {
        if s.trim().eq_ignore_ascii_case("neovim") {
            KeybindMode::Neovim
        } else {
            KeybindMode::Normal
        }
    }

    pub fn toggle(&self) -> Self {
        match self {
            KeybindMode::Normal => KeybindMode::Neovim,
            KeybindMode::Neovim => KeybindMode::Normal,
        }
    }

    /// Human label for the status/settings panels.
    pub fn label(&self) -> &'static str {
        match self {
            KeybindMode::Normal => "Normal",
            KeybindMode::Neovim => "Neovim",
        }
    }
}

#[allow(dead_code)]
pub enum DataEvent {
    System(SystemData),
    Ros2(Ros2Data),
    Workspace(WorkspaceData),
    Diagnostics(DiagnosticsData),
    Trends(TrendsData),
    Fleet(FleetData),
    Graph(GraphData),
    Telemetry(Vec<crate::telemetry::LogEntry>),
    Git(crate::git::GitState),
    Error(String),
}

/// Which region currently holds focus. Used for the visible focus ring and
/// for deciding whether a keystroke is consumed by a panel vs. a global
/// shortcut.
#[derive(Default, Clone, Copy, PartialEq)]
/// The currently focused widget. `handle_key` routes key events to the focused
/// widget *first* (terminal/editor/sidebar/right-panel claim their keys) and
/// only falls through to the global keymap when the focused widget doesn't
/// consume the event. This is the "single focused widget" half of the unified
/// input contract; the mouse half is `App::hit_regions`.
pub enum Focus {
    #[default]
    None,
    ActivityBar,
    Sidebar,
    Editor,
    RightPanel,
    Terminal,
}

/// Activity bar views. The first 7 are top-anchored; Settings and Help are
/// bottom-pinned.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Activity {
    Explorer,
    Search,
    RosGraph,
    Diagnostics,
    Sandbox,
    Git,
    Plugins,
    Settings,
    Help,
}

/// A single clickable widget region, identified by a stable `HitTarget`.
///
/// Every render pass the UI registers each interactive widget's screen `Rect`
/// into `App::hit_regions`. Mouse clicks are then resolved by a single
/// registry lookup instead of ad-hoc coordinate math scattered through
/// `handle_click`. This is the unified input contract: one hit-test map,
/// rebuilt per frame, one state mutation per gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitTarget {
    /// Activity-bar icon (by index into `Activity::` order).
    Activity(usize),
    /// The Settings icon (second-from-bottom of the activity bar body).
    ActivitySettings,
    /// The Help icon (bottom of the activity bar body).
    ActivityHelp,
    /// Top dashboard tab (by index).
    TopTab(usize),
    /// Right-side title-bar item (by index).
    RightItem(usize),
    /// Terminal session tab (by session index).
    TerminalSession(usize),
    /// Terminal session close button `×` (by session index).
    TerminalClose(usize),
    /// Terminal "+" new-session button.
    TerminalPlus,
    /// Terminal sub-tab (TERMINAL/PROBLEMS/OUTPUT/DEBUG) by index.
    TerminalSubTab(usize),
    /// The terminal body (PTY surface).
    TerminalBody,
    /// Sidebar body (file tree / search results).
    Sidebar,
    /// A file-tree row, identified by its absolute index into `file_tree.items`.
    /// Clicking toggles a directory or opens a file.
    SidebarFile(usize),
    /// An "OPEN EDITORS" row, identified by open-file index.
    SidebarOpenEditor(usize),
    /// Sidebar "new file" button in the Explorer header.
    SidebarNew,
    /// Sidebar search input field.
    SidebarSearchInput,
    /// A sidebar search-result row, by index into `app.search_results`.
    SidebarSearchResult(usize),
    /// Right-panel collapsible header (by index).
    RightPanelHeader(usize),
    /// Right-panel entity row (by index) within the active sub-tab.
    RightPanelEntity(usize),
    /// Editor document tab (by index).
    EditorTab(usize),
    /// Editor document tab close button `✕` (by index).
    EditorTabClose(usize),
    /// Editor "+" new-buffer button.
    EditorNewTab,
    /// Breadcrumb row (parent-dir navigation).
    Breadcrumb,
    /// Editor text body.
    EditorBody,
}

impl Activity {
    pub const COUNT: usize = 9;
    pub const TOP: usize = 7;

    pub fn icon(&self) -> &'static str {
        match self {
            Activity::Explorer => glyph("E", "\u{1F4C1}", "\u{f07b}"), // folder
            Activity::Search => glyph("S", "\u{1F50D}", "\u{f002}"),   // magnifier
            Activity::RosGraph => glyph("G", "\u{1F4CA}", "\u{f080}"), // bars
            Activity::Diagnostics => glyph("D", "\u{1F9EA}", "\u{f6ff}"), // heart-pulse stand-in
            Activity::Sandbox => glyph("B", "\u{1F4E6}", "\u{f4a6}"),  // box
            Activity::Git => glyph("T", "\u{1F500}", "\u{f126}"),      // branch
            Activity::Plugins => glyph("P", "\u{1F9F0}", "\u{f46d}"),  // toolbox
            Activity::Settings => glyph("X", "\u{2699}\u{FE0F}", "\u{f013}"), // gear
            Activity::Help => glyph("?", "\u{2753}", "\u{f059}"),      // question
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Activity::Explorer => "Explorer",
            Activity::Search => "Search",
            Activity::RosGraph => "ROS Graph",
            Activity::Diagnostics => "Diagnostics",
            Activity::Sandbox => "Sandbox",
            Activity::Git => "Git",
            Activity::Plugins => "Plugins",
            Activity::Settings => "Settings",
            Activity::Help => "Help",
        }
    }

    pub fn from_top_index(i: usize) -> Self {
        match i {
            0 => Activity::Explorer,
            1 => Activity::Search,
            2 => Activity::RosGraph,
            3 => Activity::Diagnostics,
            4 => Activity::Sandbox,
            5 => Activity::Git,
            _ => Activity::Plugins,
        }
    }

    pub fn pinned() -> [Activity; 2] {
        [Activity::Settings, Activity::Help]
    }
}

pub enum ConfirmAction {
    EnterGlobal,
    Delete(PathBuf),
}

pub struct ConfirmPrompt {
    pub message: String,
    pub action: ConfirmAction,
}

/// What a generic text prompt is asking for (file-tree operations).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    NewFile,
    NewFolder,
    Rename,
}

/// A small text-input overlay used for creating/renaming files & folders.
pub struct TextPrompt {
    pub label: String,
    pub value: String,
    pub kind: PromptKind,
    /// The directory (NewFile/NewFolder) or file/dir being renamed.
    pub target: PathBuf,
}

/// Actions available from the file-tree context menu.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CtxAction {
    NewFile,
    NewFolder,
    Rename,
    Delete,
    CopyPath,
    Duplicate,
    Reveal,
    OpenTerminal,
    Refresh,
}

/// A popup context menu anchored in the sidebar at (x, y).
pub struct CtxMenu {
    pub x: u16,
    pub y: u16,
    pub target: PathBuf,
    pub items: Vec<CtxAction>,
}

/// A single content-search match: the file, the 1-based line number, and the
/// matched line text (for inline preview in the Search sidebar).
#[derive(Clone)]
pub struct SearchHit {
    pub path: PathBuf,
    pub line: usize,
    pub text: String,
}

pub struct App {
    pub current_tab: usize,
    pub help_visible: bool,
    pub quit: bool,
    pub data_rx: mpsc::Receiver<DataEvent>,

    pub system: Option<SystemData>,
    pub ros2: Option<Ros2Data>,
    pub workspace: Option<WorkspaceData>,
    pub diagnostics: Option<DiagnosticsData>,
    pub trends: Option<TrendsData>,
    pub fleet: Option<FleetData>,
    pub graph: Option<GraphData>,

    pub status_message: Option<String>,
    pub status_expiry: Option<Instant>,

    pub file_tree: Option<FileTree>,
    pub telemetry: TelemetryLog,

    pub sidebar_visible: bool,
    pub right_visible: bool,
    pub terminal_visible: bool,
    pub ros_graph_full: bool,

    pub active_activity: Activity,
    pub focus: Focus,
    pub confirm: Option<ConfirmPrompt>,

    pub sandbox_mode: SandboxMode,

    pub editor: Editor,
    pub terminal_mgr: TerminalManager,

    pub right_expanded: [bool; 4],
    pub entities_tab: usize,
    pub terminal_tab: usize,
    pub selected_node: Option<String>,

    // Panel resize state
    pub sidebar_width: u16,
    pub right_panel_width: u16,
    pub terminal_height: u16,
    pub resizing: Option<ResizeEdge>,

    /// Unified hit-test map: every interactive widget registers its screen
    /// `Rect` here each frame. `handle_click` resolves a click with a single
    /// lookup against this list (topmost wins).
    pub hit_regions: Vec<(ratatui::layout::Rect, HitTarget)>,

    /// Set when a system-clipboard read (OSC 52) has been requested via Ctrl+V;
    /// cleared once the reply arrives or the short timeout elapses (falling
    /// back to the internal clipboard).
    pub clipboard_read_pending: Option<std::time::Instant>,

    /// Screen rect of the editor code area, captured each frame in
    /// `draw_editor`. `handle_editor_body_click` uses it to map a click to the
    /// correct line, instead of a hardcoded row offset.
    pub editor_area: ratatui::layout::Rect,

    /// Debounce for activity-bar icon toggles: `(icon_id, time)` of the last
    /// toggle. A terminal double-click sends two `Down` events, which would
    /// otherwise toggle the sidebar twice (hide→show). We collapse a rapid
    /// second click on the same icon into a single toggle (VS Code behavior).
    pub last_activity_click: Option<(u8, std::time::Instant)>,

    // AI terminal command buffer
    pub terminal_input_buffer: String,
    pub ai_terminal_idx: Option<usize>,
    pub ai_rx: Option<mpsc::Receiver<ai::AiEvent>>,
    pub ai_model: String,

    /// Interactive model-selection overlay (opened from the command palette).
    pub model_picker_open: bool,
    pub model_picker_index: usize,
    pub available_models: Vec<String>,

    // Search
    pub search_query: String,
    pub search_results: Vec<SearchHit>,
    pub search_input_active: bool,

    pub cpu_hist: Vec<f64>,
    pub mem_hist: Vec<f64>,
    pub net_up_hist: Vec<f64>,
    pub net_down_hist: Vec<f64>,
    pub disk_hist: Vec<f64>,

    pub last_heartbeat: Instant,
    pub unread_notifications: usize,

    pub web_chat_running: bool,
    pub web_chat_port: u16,
    pub web_chat_stop: Option<Arc<AtomicBool>>,
    pub web_chat_handle: Option<JoinHandle<()>>,

    pub git: crate::git::GitState,

    pub save_as_input: Option<String>,

    /// Editor key-binding scheme (Normal vs Neovim). Persisted to disk so the
    /// user's choice survives restarts.
    pub keybind_mode: KeybindMode,

    /// File-tree context menu (right-click / `m` in the sidebar).
    pub ctx_menu: Option<CtxMenu>,
    /// Selected index within the open context menu.
    pub ctx_menu_sel: usize,
    /// Generic text-input prompt for create/rename operations.
    pub prompt: Option<TextPrompt>,

    pub palette_open: bool,
    pub palette_query: String,
    pub palette_sel: usize,
    pub palette_mode: PaletteMode,

    /// Registered plugins (built-ins + future loaders).
    pub plugin_manager: PluginManager,
}

impl App {
    pub fn new(rx: mpsc::Receiver<DataEvent>) -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            current_tab: 2, // Workspace tab: opens on the Welcome editor (VS Code-like)
            help_visible: false,
            quit: false,
            data_rx: rx,
            system: None,
            ros2: None,
            workspace: None,
            diagnostics: None,
            trends: None,
            fleet: None,
            graph: None,
            status_message: None,
            status_expiry: None,
            file_tree: Some(FileTree::new(root)),
            telemetry: TelemetryLog::new(500),
            sidebar_visible: true,
            right_visible: false,
            terminal_visible: true,
            ros_graph_full: false,
            active_activity: Activity::Explorer,
            focus: Focus::None,
            confirm: None,
            sandbox_mode: SandboxMode::Sandbox,
            editor: {
                let mut e = Editor::default();
                // ponytail: open the Welcome buffer as the default landing tab
                // so a fresh launch looks like an editor, not a blank pane.
                e.open_welcome();
                e.mode = crate::editor::EditMode::Preview;
                e
            },
            terminal_mgr: TerminalManager::new(24, 80),
            right_expanded: [true, true, true, true],
            entities_tab: 0,
            terminal_tab: 0,
            selected_node: None,
            sidebar_width: 30,
            right_panel_width: 35,
            terminal_height: 12,
            resizing: None,
            terminal_input_buffer: String::new(),
            ai_terminal_idx: None,
            ai_rx: None,
            ai_model: ai::DEFAULT_MODEL.to_string(),
            model_picker_open: false,
            model_picker_index: 0,
            available_models: Vec::new(),
            search_query: String::new(),
            search_results: Vec::new(),
            search_input_active: false,
            cpu_hist: Vec::new(),
            mem_hist: Vec::new(),
            net_up_hist: Vec::new(),
            net_down_hist: Vec::new(),
            disk_hist: Vec::new(),
            last_heartbeat: Instant::now(),
            unread_notifications: 0,
            web_chat_running: false,
            web_chat_port: 8899,
            web_chat_stop: None,
            web_chat_handle: None,

            git: crate::git::GitState::new(),
            save_as_input: None,
            keybind_mode: Self::load_keybind_mode(),
            ctx_menu: None,
            ctx_menu_sel: 0,
            prompt: None,
            palette_open: false,
            palette_query: String::new(),
            palette_sel: 0,
            palette_mode: PaletteMode::Command,
            plugin_manager: PluginManager::new(),
            hit_regions: Vec::new(),
            clipboard_read_pending: None,
            editor_area: ratatui::layout::Rect::default(),
            last_activity_click: None,
        }
    }

    pub fn set_status(&mut self, msg: String, ttl_secs: f32) {
        self.status_message = Some(msg);
        self.status_expiry = Some(Instant::now() + std::time::Duration::from_secs_f32(ttl_secs));
    }

    /// Path to the on-disk TUI config file (`~/.ros2_info/tui_config`).
    fn config_path() -> Option<PathBuf> {
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join(".ros2_info").join("tui_config"))
    }

    /// Load the persisted key-binding mode (defaults to `Normal`).
    fn load_keybind_mode() -> KeybindMode {
        let Some(p) = Self::config_path() else {
            return KeybindMode::default();
        };
        std::fs::read_to_string(&p)
            .map(|s| KeybindMode::from_str(&s))
            .unwrap_or_default()
    }

    /// Persist the key-binding mode and apply it.
    pub fn set_keybind_mode(&mut self, mode: KeybindMode) {
        self.keybind_mode = mode;
        if let Some(p) = Self::config_path() {
            if let Some(parent) = p.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&p, mode.as_str());
        }
    }

    /// Flip between Normal and Neovim key bindings, persisting the choice.
    pub fn toggle_keybind_mode(&mut self) {
        let next = self.keybind_mode.toggle();
        self.set_keybind_mode(next);
        self.set_status(
            format!("Key bindings: {} (restart not required)", next.label()),
            3.0,
        );
    }

    pub fn tick_status(&mut self) {
        if let Some(expiry) = self.status_expiry {
            if Instant::now() >= expiry {
                self.status_message = None;
                self.status_expiry = None;
            }
        }
    }

    /// Returns `true` (and does NOT update state) if a click on `icon_id`
    /// arrived within the debounce window of the previous one — used to collapse
    /// a terminal double-click (two `Down` events) into a single activity-bar
    /// toggle. Otherwise records this click and returns `false`.
    pub fn activity_click_debounced(&mut self, icon_id: u8) -> bool {
        const DEBOUNCE_MS: u64 = 280;
        let now = std::time::Instant::now();
        if let Some((last_id, t)) = self.last_activity_click {
            if last_id == icon_id && now.duration_since(t).as_millis() as u64 <= DEBOUNCE_MS {
                return true;
            }
        }
        self.last_activity_click = Some((icon_id, now));
        false
    }

    pub fn process_events(&mut self) {
        while let Ok(event) = self.data_rx.try_recv() {
            self.last_heartbeat = Instant::now();
            match event {
                DataEvent::System(d) => {
                    App::push_hist(&mut self.cpu_hist, d.cpu_percent as f64);
                    App::push_hist(&mut self.mem_hist, d.mem_percent as f64);
                    App::push_hist(&mut self.net_up_hist, d.net_sent_mb);
                    App::push_hist(&mut self.net_down_hist, d.net_recv_mb);
                    App::push_hist(&mut self.disk_hist, d.disk_percent);
                    self.system = Some(d);
                }
                DataEvent::Ros2(d) => {
                    self.ros2 = Some(d);
                }
                DataEvent::Workspace(d) => {
                    self.workspace = Some(d);
                }
                DataEvent::Diagnostics(d) => {
                    self.unread_notifications = d
                        .issues
                        .iter()
                        .filter(|i| i.severity == "error" || i.severity == "warn")
                        .count();
                    self.diagnostics = Some(d);
                }
                DataEvent::Trends(d) => {
                    self.trends = Some(d);
                }
                DataEvent::Fleet(d) => {
                    self.fleet = Some(d);
                }
                DataEvent::Graph(d) => {
                    self.graph = Some(d);
                }
                DataEvent::Telemetry(entries) => {
                    for entry in entries {
                        self.telemetry.add(entry);
                    }
                }
                DataEvent::Error(e) => {
                    self.set_status(e, 5.0);
                }
                DataEvent::Git(g) => {
                    // Preserve the user's current selection / open sub-tab so a
                    // periodic refresh doesn't reset their place in the panel.
                    let tab = self.git.tab;
                    let sel = self.git.selected_file;
                    self.git = g;
                    self.git.tab = tab;
                    self.git.selected_file = sel;
                }
            }
        }
        // Pump the active terminal session for fresh output.
        self.terminal_mgr.pump();
    }

    /// Best-effort derivation of the `owner/repo` slug for the current git
    /// checkout's `origin` remote, so we can populate the GitHub Issues/PRs
    /// panel. Returns `None` when it can't be determined.
    pub(crate) fn current_github_repo() -> Option<String> {
        let out = std::process::Command::new("git")
            .args(["remote", "get-url", "origin"])
            .output()
            .ok()?;
        let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let s = url.trim_end_matches(".git");
        let idx = s.find("github.com")?;
        let after = &s[idx + "github.com".len()..];
        let after = after.trim_start_matches('/').trim_start_matches(':');
        if after.is_empty() {
            None
        } else {
            Some(after.to_string())
        }
    }

    fn push_hist(v: &mut Vec<f64>, val: f64) {
        v.push(val);
        if v.len() > 60 {
            v.remove(0);
        }
    }

    pub fn tab(&self) -> Tab {
        Tab::from_index(self.current_tab)
    }

    pub fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
    }

    pub fn toggle_right_panel(&mut self) {
        self.right_visible = !self.right_visible;
    }

    pub fn toggle_sandbox(&mut self) {
        match self.sandbox_mode {
            SandboxMode::Sandbox => {
                self.sandbox_mode = SandboxMode::Global;
                std::env::remove_var("ROS2_INFO_SANDBOX");
                self.terminal_mgr.set_sandbox_context("global".to_string());
                self.set_status(
                    "Global mode — real system, commands execute directly".into(),
                    3.0,
                );
            }
            SandboxMode::Global => {
                self.sandbox_mode = SandboxMode::Sandbox;
                std::env::set_var("ROS2_INFO_SANDBOX", "1");
                self.terminal_mgr.set_sandbox_context("sandbox".to_string());
                self.set_status(
                    "Sandbox mode — nodes run in /sandbox namespace, read-only introspection"
                        .into(),
                    3.0,
                );
            }
        }
    }

    pub fn request_enter_global(&mut self) {
        self.confirm = Some(ConfirmPrompt {
            message: "Enter Global mode? This will execute commands on your real ROS2 environment."
                .into(),
            action: ConfirmAction::EnterGlobal,
        });
    }

    /// Ask for confirmation before deleting a file or directory.
    pub fn request_delete(&mut self, path: PathBuf) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        let kind = if path.is_dir() { "folder" } else { "file" };
        self.confirm = Some(ConfirmPrompt {
            message: format!("Delete {kind} \"{name}\"? This cannot be undone."),
            action: ConfirmAction::Delete(path),
        });
    }

    pub fn confirm_action(&mut self) {
        if let Some(prompt) = self.confirm.take() {
            match prompt.action {
                ConfirmAction::EnterGlobal => {
                    if matches!(self.sandbox_mode, SandboxMode::Sandbox) {
                        self.sandbox_mode = SandboxMode::Global;
                        std::env::remove_var("ROS2_INFO_SANDBOX");
                        self.terminal_mgr.set_sandbox_context("global".to_string());
                        self.set_status(
                            "Global mode — real system, commands execute directly".into(),
                            3.0,
                        );
                    }
                }
                ConfirmAction::Delete(path) => {
                    self.delete_path(&path);
                }
            }
        }
    }

    pub fn dismiss_confirm(&mut self) {
        self.confirm = None;
    }

    pub fn sandbox_label(&self) -> &'static str {
        match self.sandbox_mode {
            SandboxMode::Global => "GLOBAL",
            SandboxMode::Sandbox => "SANDBOX",
        }
    }

    pub fn mode_color(&self) -> ratatui::style::Color {
        match self.sandbox_mode {
            SandboxMode::Global => crate::theme::GLOBAL,
            SandboxMode::Sandbox => crate::theme::SANDBOX,
        }
    }

    pub fn open_file_in_editor(&mut self, path: PathBuf) {
        if path.is_file() {
            self.editor.open(path);
            self.focus = Focus::Editor;
        }
    }

    pub fn open_palette(&mut self, mode: PaletteMode) {
        self.palette_open = true;
        self.palette_mode = mode;
        self.palette_query.clear();
        self.palette_sel = 0;
        self.focus = Focus::None;
    }

    pub fn close_palette(&mut self) {
        self.palette_open = false;
        self.palette_query.clear();
        self.palette_sel = 0;
    }

    /// Open the interactive Ollama model picker, populated from the local
    /// `ollama` API. Selection is committed via `select_model`.
    pub fn open_model_picker(&mut self) {
        let (models, _) = ai::list_ollama_models();
        self.available_models = models;
        self.model_picker_index = self
            .available_models
            .iter()
            .position(|m| m == &self.ai_model)
            .unwrap_or(0);
        self.model_picker_open = true;
        self.focus = Focus::None;
    }

    /// Commit the currently highlighted model as the active AI model.
    pub fn select_model(&mut self) {
        if let Some(m) = self.available_models.get(self.model_picker_index).cloned() {
            self.ai_model = m.clone();
            self.status_message = Some(format!("✅ AI model set to: {}", m));
            self.status_expiry = Some(Instant::now() + std::time::Duration::from_secs(3));
        }
        self.model_picker_open = false;
        self.available_models.clear();
    }

    pub fn close_model_picker(&mut self) {
        self.model_picker_open = false;
        self.available_models.clear();
    }

    /// File paths from the workspace tree (for "Go to File" mode).
    pub fn palette_files(&self) -> Vec<PathBuf> {
        self.file_tree
            .as_ref()
            .map(|t| {
                t.items
                    .iter()
                    .filter(|i| !i.is_dir)
                    .map(|i| i.path.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Search file *contents* (grep) for `search_query` across the workspace.
    /// Results are clickable and open the file at the matching line.
    pub fn search_files(&mut self) {
        self.search_results.clear();
        let query = self.search_query.trim().to_lowercase();
        if query.is_empty() {
            return;
        }
        let root = self
            .file_tree
            .as_ref()
            .map(|t| t.root.clone())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        self.search_recursive(&root, &query, 0);
        self.search_results
            .sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
        if self.search_results.len() > 300 {
            self.search_results.truncate(300);
        }
    }

    fn search_recursive(&mut self, dir: &std::path::Path, query: &str, depth: usize) {
        if depth > 12 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.starts_with('.')
                || name == "target"
                || name == "__pycache__"
                || name == "node_modules"
                || name == "build"
                || name == "install"
            {
                continue;
            }
            if path.is_dir() {
                self.search_recursive(&path, query, depth + 1);
            } else if let Ok(text) = std::fs::read_to_string(&path) {
                // Skip files that are implausibly large for an interactive grep.
                if text.len() > 2_000_000 {
                    continue;
                }
                for (i, l) in text.lines().enumerate() {
                    if l.to_lowercase().contains(query) {
                        self.search_results.push(SearchHit {
                            path: path.clone(),
                            line: i + 1,
                            text: l.trim().to_string(),
                        });
                        if self.search_results.len() >= 300 {
                            return;
                        }
                    }
                }
            }
            if self.search_results.len() >= 300 {
                return;
            }
        }
    }

    /// Open a file and jump the cursor straight to a given 1-based line.
    pub fn open_file_at_line(&mut self, path: PathBuf, line: usize) {
        if path.is_file() {
            self.editor.open(path);
            if let Some(f) = self.editor.active_file_mut() {
                f.goto_line(line);
            }
            self.focus = Focus::Editor;
        }
    }

    pub fn open_next_unopened_file(&mut self) {
        let Some(tree) = &self.file_tree else { return };
        for item in &tree.items {
            if !item.is_dir && !self.editor.files.iter().any(|f| f.path == item.path) {
                self.open_file_in_editor(item.path.clone());
                return;
            }
        }
        // All files open — create untitled buffer
        self.editor.new_untitled();
        self.editor.mode = crate::editor::EditMode::Edit;
        self.focus = Focus::Editor;
    }

    // ── File-tree context menu ──────────────────────────────────────

    pub fn ctx_menu_items() -> Vec<CtxAction> {
        vec![
            CtxAction::NewFile,
            CtxAction::NewFolder,
            CtxAction::Rename,
            CtxAction::Duplicate,
            CtxAction::Delete,
            CtxAction::CopyPath,
            CtxAction::Reveal,
            CtxAction::OpenTerminal,
            CtxAction::Refresh,
        ]
    }

    /// Open the context menu for `target`, anchored at (x, y).
    pub fn open_ctx_menu(&mut self, x: u16, y: u16, target: PathBuf) {
        self.ctx_menu_sel = 0;
        self.ctx_menu = Some(CtxMenu {
            x,
            y,
            target,
            items: Self::ctx_menu_items(),
        });
    }

    /// Human-readable label for a context-menu action.
    pub fn ctx_action_label(a: CtxAction) -> &'static str {
        match a {
            CtxAction::NewFile => "New File",
            CtxAction::NewFolder => "New Folder",
            CtxAction::Rename => "Rename",
            CtxAction::Duplicate => "Duplicate",
            CtxAction::Delete => "Delete",
            CtxAction::CopyPath => "Copy Path",
            CtxAction::Reveal => "Reveal in File Manager",
            CtxAction::OpenTerminal => "Open in Terminal",
            CtxAction::Refresh => "Refresh",
        }
    }

    fn ctx_parent(target: &Path) -> PathBuf {
        if target.is_dir() {
            target.to_path_buf()
        } else {
            target
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."))
        }
    }

    /// Dispatch a context-menu action.
    pub fn run_ctx_action(&mut self, action: CtxAction) {
        let target = match self.ctx_menu.take() {
            Some(m) => m.target,
            None => return,
        };
        self.dispatch_ctx(action, target);
    }

    /// Run a context action against an explicit target (used by sidebar keys).
    pub fn run_ctx_action_target(&mut self, action: CtxAction, target: PathBuf) {
        self.dispatch_ctx(action, target);
    }

    fn dispatch_ctx(&mut self, action: CtxAction, target: PathBuf) {
        match action {
            CtxAction::NewFile => {
                let parent = Self::ctx_parent(&target);
                self.prompt = Some(TextPrompt {
                    label: "New File".into(),
                    value: String::new(),
                    kind: PromptKind::NewFile,
                    target: parent,
                });
            }
            CtxAction::NewFolder => {
                let parent = Self::ctx_parent(&target);
                self.prompt = Some(TextPrompt {
                    label: "New Folder".into(),
                    value: String::new(),
                    kind: PromptKind::NewFolder,
                    target: parent,
                });
            }
            CtxAction::Rename => {
                let name = target
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                self.prompt = Some(TextPrompt {
                    label: "Rename".into(),
                    value: name,
                    kind: PromptKind::Rename,
                    target,
                });
            }
            CtxAction::Delete => self.request_delete(target),
            CtxAction::CopyPath => {
                let p = target.to_string_lossy().to_string();
                self.set_status(format!("Path copied: {}", p), 3.0);
            }
            CtxAction::Duplicate => self.duplicate_path(&target),
            CtxAction::Reveal => {
                let dir = if target.is_dir() {
                    target.clone()
                } else {
                    target
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or(target.clone())
                };
                let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
                self.set_status(format!("Revealing: {}", dir.display()), 3.0);
            }
            CtxAction::OpenTerminal => {
                let dir = if target.is_dir() {
                    target.clone()
                } else {
                    target
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or(target.clone())
                };
                let cd = format!("cd \"{}\"\n", dir.display());
                self.terminal_mgr.write_input(cd.as_bytes());
                self.terminal_visible = true;
                self.focus = Focus::Terminal;
                self.set_status(format!("Terminal → {}", dir.display()), 2.5);
            }
            CtxAction::Refresh => {
                if let Some(tree) = &mut self.file_tree {
                    tree.refresh();
                }
            }
        }
    }

    /// Delete a file or (recursively) a directory, closing any open editors.
    pub fn delete_path(&mut self, path: &PathBuf) {
        if !path.exists() {
            return;
        }
        let is_dir = path.is_dir();
        let res = if is_dir {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        };
        match res {
            Ok(()) => {
                // Close any editor tabs pointing at the removed path or below it.
                self.editor
                    .files
                    .retain(|f| f.path != *path && !f.path.starts_with(path));
                if self.editor.active >= self.editor.files.len() && self.editor.active > 0 {
                    self.editor.active -= 1;
                }
                if let Some(tree) = &mut self.file_tree {
                    tree.selected = None;
                    tree.refresh();
                }
                let what = if is_dir { "Folder" } else { "File" };
                self.set_status(format!("{} deleted.", what), 2.5);
            }
            Err(e) => {
                self.set_status(format!("Delete failed: {}", e), 4.0);
            }
        }
    }

    /// Duplicate a file or directory (recursively) next to the original,
    /// choosing a non-colliding "copy" name.
    pub fn duplicate_path(&mut self, path: &PathBuf) {
        if !path.exists() {
            return;
        }
        let parent = path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let stem = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "item".to_string());

        let dest = self.unique_copy_name(&parent, &stem);
        let res = if path.is_dir() {
            copy_dir_recursive(path, &dest)
        } else {
            std::fs::copy(path, &dest).map(|_| ())
        };
        match res {
            Ok(_) => {
                if let Some(tree) = &mut self.file_tree {
                    tree.selected = Some(dest.clone());
                    tree.refresh();
                }
                self.set_status(format!("Duplicated to {}", dest.display()), 2.5);
            }
            Err(e) => self.set_status(format!("Duplicate failed: {}", e), 4.0),
        }
    }

    /// Pick a sibling name of the form `<stem> copy` / `<stem> copy 2` ...
    fn unique_copy_name(&self, parent: &Path, stem: &str) -> PathBuf {
        let base = format!("{} copy", stem);
        let mut candidate = parent.join(&base);
        let mut n = 2;
        while candidate.exists() {
            candidate = parent.join(format!("{} copy {}", stem, n));
            n += 1;
        }
        candidate
    }

    /// Commit the current text prompt (create file/folder or rename).
    pub fn submit_prompt(&mut self) {
        let prompt = match self.prompt.take() {
            Some(p) => p,
            None => return,
        };
        let name = prompt.value.trim().to_string();
        if name.is_empty() {
            return;
        }
        match prompt.kind {
            PromptKind::NewFile => {
                let path = prompt.target.join(&name);
                match std::fs::write(&path, "") {
                    Ok(()) => {
                        if let Some(tree) = &mut self.file_tree {
                            tree.refresh();
                        }
                        self.open_file_in_editor(path);
                    }
                    Err(e) => self.set_status(format!("Create failed: {}", e), 4.0),
                }
            }
            PromptKind::NewFolder => {
                let path = prompt.target.join(&name);
                match std::fs::create_dir_all(&path) {
                    Ok(()) => {
                        if let Some(tree) = &mut self.file_tree {
                            tree.refresh();
                        }
                        self.set_status(format!("Folder created: {}", name), 2.5);
                    }
                    Err(e) => self.set_status(format!("Create failed: {}", e), 4.0),
                }
            }
            PromptKind::Rename => {
                let new_path = if let Some(parent) = prompt.target.parent() {
                    parent.join(&name)
                } else {
                    PathBuf::from(&name)
                };
                match std::fs::rename(&prompt.target, &new_path) {
                    Ok(()) => {
                        // Update any open editor tab whose path matches.
                        for f in self.editor.files.iter_mut() {
                            if f.path == prompt.target {
                                f.set_path(new_path.clone());
                            }
                        }
                        if let Some(tree) = &mut self.file_tree {
                            tree.selected = Some(new_path.clone());
                            tree.refresh();
                        }
                        self.set_status(format!("Renamed to {}", name), 2.5);
                    }
                    Err(e) => self.set_status(format!("Rename failed: {}", e), 4.0),
                }
            }
        }
    }
}

/// Recursively copy a directory tree (`from` → `to`).
fn copy_dir_recursive(from: &PathBuf, to: &PathBuf) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_dir_recursive(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}
