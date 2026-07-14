//! Command Palette (Ctrl+Shift+P) and "Go to File" (Ctrl+P).
//!
//! Both share one overlay: command mode lists actions; file mode lists files
//! from the workspace tree. Filtering is a small fuzzy (subsequence) match
//! with a contiguous-substring bonus, like VS Code's Quick Open.

use std::path::PathBuf;

#[derive(Clone)]
pub struct PaletteItem {
    pub id: &'static str,
    pub title: String,
    pub category: &'static str,
    pub keywords: String,
}

fn cmd(
    id: &'static str,
    title: &'static str,
    category: &'static str,
    keywords: &'static str,
) -> PaletteItem {
    PaletteItem {
        id,
        title: title.to_string(),
        category,
        keywords: keywords.to_string(),
    }
}

/// All available commands. Order is the default display order.
pub fn commands() -> Vec<PaletteItem> {
    vec![
        cmd(
            "file.open",
            "Go to File...",
            "File",
            "open find goto file fuzzy",
        ),
        cmd(
            "file.new",
            "New Untitled File",
            "File",
            "new untitled create buffer text",
        ),
        cmd("file.save", "Save File", "File", "save write disk"),
        cmd(
            "file.saveAs",
            "Save As...",
            "File",
            "save as rename filename extension",
        ),
        cmd("file.close", "Close Editor", "File", "close tab"),
        cmd(
            "term.new",
            "Terminal: New",
            "Terminal",
            "terminal tab new bash shell",
        ),
        cmd(
            "term.toggle",
            "Terminal: Toggle Panel",
            "Terminal",
            "show hide terminal panel",
        ),
        cmd(
            "view.toggleSidebar",
            "View: Toggle Sidebar",
            "View",
            "explorer sidebar show hide",
        ),
        cmd(
            "view.toggleRight",
            "View: Toggle Right Panel",
            "View",
            "entities panel",
        ),
        cmd(
            "view.toggleWordWrap",
            "View: Toggle Word Wrap",
            "View",
            "wrap line",
        ),
        cmd(
            "view.zen",
            "View: Toggle Zen Mode",
            "View",
            "minimize hide all",
        ),
        cmd(
            "nav.gotoLine",
            "Go to Line...",
            "Navigation",
            "line number jump",
        ),
        cmd("nav.find", "Find...", "Navigation", "search find replace"),
        cmd(
            "nav.overview",
            "Go to Tab: Overview",
            "Navigation",
            "tab switch",
        ),
        cmd("nav.ros2", "Go to Tab: ROS2", "Navigation", "tab switch"),
        cmd(
            "nav.workspace",
            "Go to Tab: Workspace",
            "Navigation",
            "tab switch editor",
        ),
        cmd(
            "nav.diagnostics",
            "Go to Tab: Diagnostics",
            "Navigation",
            "tab switch",
        ),
        cmd(
            "nav.trends",
            "Go to Tab: Trends",
            "Navigation",
            "tab switch",
        ),
        cmd("nav.fleet", "Go to Tab: Fleet", "Navigation", "tab switch"),
        cmd(
            "sandbox.toggle",
            "Sandbox: Toggle Sandbox / Global",
            "Sandbox",
            "isolation namespace",
        ),
        cmd(
            "settings.keybinds",
            "Settings: Toggle Normal / Neovim Key Bindings",
            "Settings",
            "keybindings vim neovim modal editor",
        ),
        cmd(
            "help",
            "Help: Show Shortcuts",
            "Help",
            "keybindings shortcuts guide",
        ),
        cmd(
            "refresh",
            "Workspace: Refresh Data",
            "Workspace",
            "reload update collect",
        ),
        cmd("quit", "Quit ROS2_INFO", "App", "exit close leave"),
        // ── ROS 2 quick tools (run in the integrated terminal) ──────────────
        cmd(
            "ros.nodeList",
            "ROS2: Node List",
            "ROS2",
            "ros2 node list nodes running",
        ),
        cmd(
            "ros.topicList",
            "ROS2: Topic List",
            "ROS2",
            "ros2 topic list topics messages",
        ),
        cmd(
            "ros.serviceList",
            "ROS2: Service List",
            "ROS2",
            "ros2 service list services",
        ),
        cmd(
            "ros.actionList",
            "ROS2: Action List",
            "ROS2",
            "ros2 action list actions",
        ),
        cmd(
            "ros.paramList",
            "ROS2: Param List",
            "ROS2",
            "ros2 param list parameters",
        ),
        cmd(
            "ros.interfaceList",
            "ROS2: Interface List",
            "ROS2",
            "ros2 interface list msg srv action",
        ),
        cmd(
            "ros.topicEcho",
            "ROS2: Topic Echo (selected)",
            "ROS2",
            "ros2 topic echo messages watch stream",
        ),
        cmd(
            "ros.topicHz",
            "ROS2: Topic Rate (selected)",
            "ROS2",
            "ros2 topic hz rate frequency",
        ),
        cmd(
            "ros.topicInfo",
            "ROS2: Topic Info (selected)",
            "ROS2",
            "ros2 topic info type",
        ),
        cmd(
            "ros.nodeInfo",
            "ROS2: Node Info (selected)",
            "ROS2",
            "ros2 node info publishers subscribers",
        ),
        cmd(
            "ros.doctor",
            "ROS2: Doctor",
            "ROS2",
            "ros2 doctor diagnose check health",
        ),
        cmd(
            "ros.daemon",
            "ROS2: Daemon Status",
            "ROS2",
            "ros2 daemon status running",
        ),
        cmd(
            "ros.bagRecord",
            "ROS2: Bag Record All",
            "ROS2",
            "ros2 bag record -a mcap",
        ),
        cmd(
            "ros.bagPlay",
            "ROS2: Bag Play…",
            "ROS2",
            "ros2 bag play mcap db3 replay",
        ),
        cmd(
            "ros.launch",
            "ROS2: Launch…",
            "ROS2",
            "ros2 launch package file type",
        ),
        cmd(
            "ros.launchPicker",
            "ROS2: Launch File…",
            "ROS2",
            "ros2 launch file picker run launch.py",
        ),
        cmd(
            "ros.run",
            "ROS2: Run Node…",
            "ROS2",
            "ros2 run package executable",
        ),
        // ── AI assistant ─────────────────────────────────────────────────────
        cmd(
            "ai.chooseModel",
            "AI: Choose Model",
            "AI",
            "ollama model select llm choose active",
        ),
        cmd(
            "ai.auto",
            "AI: Solve (Autonomous Fix)",
            "AI",
            "auto solve fix build errors autonomous",
        ),
    ]
}

/// Score a haystack against a lowercase query.
/// Returns `None` if it doesn't match (subsequence), else a lower-is-better score.
fn score(hay: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    // Contiguous substring is best.
    if let Some(pos) = hay.find(needle) {
        return Some(pos);
    }
    // Fall back to subsequence match.
    let mut it = hay.chars();
    let mut penalty = 0usize;
    for q in needle.chars() {
        let mut found = false;
        for (skipped, c) in it.by_ref().enumerate() {
            if c == q {
                penalty += skipped;
                found = true;
                break;
            }
        }
        if !found {
            return None;
        }
    }
    Some(1000 + penalty)
}

/// Filter commands by query, returning (score, item) sorted best-first.
pub fn filter_commands(query: &str) -> Vec<(usize, PaletteItem)> {
    let q = query.trim().to_lowercase();
    let mut out: Vec<(usize, PaletteItem)> = commands()
        .into_iter()
        .filter_map(|item| {
            let hay = format!("{} {} {}", item.title, item.keywords, item.category).to_lowercase();
            score(&hay, &q).map(|s| (s, item))
        })
        .collect();
    out.sort_by_key(|(s, _)| *s);
    out
}

/// Candidate files (relative-ish display + path) from the workspace tree.
pub fn file_candidates(files: &[std::path::PathBuf]) -> Vec<(String, PathBuf)> {
    files
        .iter()
        .map(|p| {
            let disp = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.to_string_lossy().to_string());
            (disp, p.clone())
        })
        .collect()
}

/// Filter file candidates by query (matches against full path too).
pub fn filter_files(cands: &[(String, PathBuf)], query: &str) -> Vec<(usize, (String, PathBuf))> {
    let q = query.trim().to_lowercase();
    let mut out: Vec<(usize, (String, PathBuf))> = cands
        .iter()
        .filter_map(|(name, path)| {
            let full = path.to_string_lossy().to_lowercase();
            let s = score(&full, &q)
                .or_else(|| score(&name.to_lowercase(), &q))
                .or_else(|| score(&name.to_lowercase().replace(['_', '-', '.'], " "), &q));
            s.map(|s| (s, (name.clone(), path.clone())))
        })
        .collect();
    out.sort_by_key(|(s, _)| *s);
    out
}

/// True for files the launch-file picker should list.
pub fn is_launch_file(p: &std::path::Path) -> bool {
    let s = p.to_string_lossy().to_ascii_lowercase();
    s.ends_with(".launch.py")
        || s.ends_with(".launch.xml")
        || s.ends_with(".launch.yaml")
        || s.ends_with(".launch")
}

/// True for files the bag picker should list.
pub fn is_bag_file(p: &std::path::Path) -> bool {
    let s = p.to_string_lossy().to_ascii_lowercase();
    s.ends_with(".db3") || s.ends_with(".mcap") || s.ends_with(".bag")
}
