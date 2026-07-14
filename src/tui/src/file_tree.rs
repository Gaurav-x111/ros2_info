#![allow(dead_code)]
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::Frame;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

const DIR_ICON: &str = "📁";
const FILE_ICON: &str = "📄";

const ACCENT: Color = Color::Rgb(100, 180, 255);
const DIM: Color = Color::Rgb(120, 120, 140);
const SURFACE: Color = Color::Rgb(35, 35, 55);
const OK: Color = Color::Rgb(80, 220, 100);

/// File tree state and navigation
pub struct FileTree {
    /// Root directory to display
    pub root: PathBuf,
    /// Currently expanded directories
    pub expanded: HashSet<PathBuf>,
    /// Currently selected file/dir
    pub selected: Option<PathBuf>,
    /// Scroll offset for large trees
    pub scroll_offset: usize,
    /// All visible items (flattened)
    pub items: Vec<FileTreeItem>,
    /// Git status: path → status char ('U' = untracked, 'M' = modified, etc.)
    pub git_status: HashMap<PathBuf, char>,
}

#[derive(Clone)]
pub struct FileTreeItem {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
    pub icon: &'static str,
}

impl FileTree {
    pub fn new(root: PathBuf) -> Self {
        let mut tree = Self {
            root: root.clone(),
            expanded: HashSet::new(),
            selected: None,
            scroll_offset: 0,
            items: Vec::new(),
            git_status: HashMap::new(),
        };
        // Expand root and first-level directories by default
        tree.expanded.insert(root.clone());
        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !name.starts_with('.') && name != "target" && name != "__pycache__" {
                        tree.expanded.insert(path);
                    }
                }
            }
        }
        tree.refresh();
        tree
    }

    /// Refresh the file tree from disk and update git status
    pub fn refresh(&mut self) {
        self.items.clear();
        let root = self.root.clone();
        let expanded = self.expanded.clone();
        self.collect_items(&root, 0, &expanded);
        self.refresh_git_status();
    }

    fn refresh_git_status(&mut self) {
        self.git_status.clear();
        let output = StdCommand::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&self.root)
            .output();
        if let Ok(out) = output {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                if line.len() < 4 {
                    continue;
                }
                let xy = &line[..2];
                let path_str = line[3..].trim();
                if path_str.is_empty() {
                    continue;
                }
                let full = self.root.join(path_str);
                let status = match xy {
                    "??" => 'U',
                    " M" | "MM" | "M " => 'M',
                    "A " => 'A',
                    "D " => 'D',
                    _ => continue,
                };
                self.git_status.insert(full, status);
            }
        }
    }

    fn collect_items(&mut self, dir: &Path, depth: usize, expanded: &HashSet<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };

        let mut dirs: Vec<_> = Vec::new();
        let mut files: Vec<_> = Vec::new();

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden and target directories
            if name.starts_with('.') || name == "target" || name == "__pycache__" {
                continue;
            }

            if path.is_dir() {
                dirs.push((name, path));
            } else {
                files.push((name, path));
            }
        }

        // Sort directories first, then files alphabetically
        dirs.sort_by(|a, b| a.0.cmp(&b.0));
        files.sort_by(|a, b| a.0.cmp(&b.0));

        // Add directories
        for (name, path) in dirs {
            let is_expanded = expanded.contains(&path);
            self.items.push(FileTreeItem {
                icon: DIR_ICON,
                path: path.clone(),
                name,
                is_dir: true,
                depth,
            });
            if is_expanded {
                self.collect_items(&path, depth + 1, expanded);
            }
        }

        // Add files
        for (name, path) in files {
            let icon = FILE_ICON;
            self.items.push(FileTreeItem {
                icon,
                path,
                name,
                is_dir: false,
                depth,
            });
        }
    }

    /// Toggle expansion of a directory
    pub fn toggle_expand(&mut self, path: &Path) {
        if self.expanded.contains(path) {
            self.expanded.remove(path);
        } else {
            self.expanded.insert(path.to_path_buf());
        }
        self.refresh();
    }

    /// Select the next item
    pub fn select_next(&mut self) {
        if let Some(selected) = &self.selected.clone() {
            if let Some(idx) = self.items.iter().position(|i| &i.path == selected) {
                if idx + 1 < self.items.len() {
                    self.selected = Some(self.items[idx + 1].path.clone());
                }
            }
        } else if !self.items.is_empty() {
            self.selected = Some(self.items[0].path.clone());
        }
    }

    /// Select the previous item
    pub fn select_prev(&mut self) {
        if let Some(selected) = &self.selected.clone() {
            if let Some(idx) = self.items.iter().position(|i| &i.path == selected) {
                if idx > 0 {
                    self.selected = Some(self.items[idx - 1].path.clone());
                }
            }
        }
    }

    /// Try to expand selected directory
    pub fn expand_selected(&mut self) {
        if let Some(selected) = &self.selected.clone() {
            if selected.is_dir() {
                self.toggle_expand(selected);
            }
        }
    }

    /// Get the selected file content
    pub fn get_selected_content(&self) -> Option<String> {
        if let Some(path) = &self.selected {
            if path.is_file() {
                return fs::read_to_string(path).ok();
            }
        }
        None
    }
}

/// Render the file tree panel
pub fn render_file_tree(frame: &mut Frame, area: Rect, tree: &FileTree) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" File Explorer ")
        .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
        .border_style(Style::default().fg(DIM));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if tree.items.is_empty() {
        return;
    }

    let items: Vec<ListItem> = tree
        .items
        .iter()
        .enumerate()
        .skip(tree.scroll_offset)
        .take(inner.height as usize)
        .map(|(_idx, item)| {
            let indent = "  ".repeat(item.depth);
            let is_selected = tree.selected.as_ref() == Some(&item.path);

            let style = if is_selected {
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
            } else if item.is_dir {
                Style::default().fg(OK)
            } else {
                Style::default().fg(DIM)
            };

            let content = format!("{}{} {}", item.icon, indent, item.name);
            ListItem::new(Line::from(Span::styled(content, style)))
        })
        .collect();

    let list = List::new(items).highlight_style(Style::default().bg(SURFACE));

    frame.render_widget(list, inner);
}
