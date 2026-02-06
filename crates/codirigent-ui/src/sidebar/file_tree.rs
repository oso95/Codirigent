//! Sidebar file tree panel.
//!
//! Provides a file tree view for the sidebar, displaying the project's
//! file structure with expand/collapse functionality.

use super::types::Color;
use std::path::{Path, PathBuf};

/// Events emitted by the file tree panel.
#[derive(Debug, Clone, PartialEq)]
pub enum FileTreeEvent {
    /// File was selected (single click).
    FileSelected(PathBuf),
    /// File was activated (double click or enter).
    FileActivated(PathBuf),
    /// Directory was toggled (expand/collapse).
    DirectoryToggled(PathBuf),
    /// Path was dragged to a terminal.
    PathDraggedToTerminal {
        /// The path that was dragged.
        path: PathBuf,
        /// The session ID to insert the path into.
        session_id: u64,
    },
}

/// A flattened file tree item for rendering.
#[derive(Debug, Clone)]
pub struct FileTreeRenderItem {
    /// File path.
    pub path: PathBuf,
    /// File name.
    pub name: String,
    /// Whether this is a directory.
    pub is_dir: bool,
    /// Whether this directory is expanded.
    pub expanded: bool,
    /// Icon identifier for this file type.
    pub icon: FileTreeIcon,
    /// Indentation depth (0 = root level).
    pub depth: usize,
    /// Whether this item is selected.
    pub is_selected: bool,
    /// Whether this item is being hovered.
    pub is_hovered: bool,
}

/// Icon types for file tree items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileTreeIcon {
    /// Closed folder.
    Folder,
    /// Open folder.
    FolderOpen,
    /// Generic file.
    File,
    /// Rust source file.
    Rust,
    /// Markdown document.
    Markdown,
    /// JSON file.
    Json,
    /// TOML file.
    Toml,
    /// YAML file.
    Yaml,
    /// TypeScript file.
    TypeScript,
    /// JavaScript file.
    JavaScript,
    /// Python file.
    Python,
    /// Shell script.
    Shell,
    /// Git-related file.
    Git,
}

impl FileTreeIcon {
    /// Get a simple text representation of the icon.
    pub fn text(&self) -> &'static str {
        match self {
            Self::Folder => "📁",
            Self::FolderOpen => "📂",
            Self::File => "📄",
            Self::Rust => "🦀",
            Self::Markdown => "📝",
            Self::Json => "⚙️",
            Self::Toml => "⚙️",
            Self::Yaml => "⚙️",
            Self::TypeScript => "📜",
            Self::JavaScript => "📜",
            Self::Python => "🐍",
            Self::Shell => "💻",
            Self::Git => "🔧",
        }
    }

    /// Get the icon color.
    pub fn color(&self) -> Color {
        match self {
            Self::Folder | Self::FolderOpen => Color::from_hex("#F59E0B"), // Orange
            Self::File => Color::from_hex("#888888"),                       // Gray
            Self::Rust => Color::from_hex("#FF6B6B"),                      // Rust orange-red
            Self::Markdown => Color::from_hex("#5B8DEF"),                  // Blue
            Self::Json | Self::Toml | Self::Yaml => Color::from_hex("#4ECDC4"), // Teal
            Self::TypeScript => Color::from_hex("#3178C6"),                // TypeScript blue
            Self::JavaScript => Color::from_hex("#F7DF1E"),                // JavaScript yellow
            Self::Python => Color::from_hex("#3776AB"),                    // Python blue
            Self::Shell => Color::from_hex("#4ECDC4"),                     // Teal
            Self::Git => Color::from_hex("#F05032"),                       // Git orange
        }
    }

    /// Get the Lucide icon string for GPUI rendering.
    pub fn lucide_icon(&self) -> String {
        crate::icons::icon_str(match self {
            Self::Folder => lucide_icons::Icon::Folder,
            Self::FolderOpen => lucide_icons::Icon::FolderOpen,
            Self::Rust | Self::Python | Self::TypeScript | Self::JavaScript | Self::Shell => {
                lucide_icons::Icon::FileCode
            }
            Self::Markdown => lucide_icons::Icon::FileText,
            Self::Json | Self::Toml | Self::Yaml => lucide_icons::Icon::FileCog,
            Self::Git => lucide_icons::Icon::GitBranch,
            Self::File => lucide_icons::Icon::File,
        })
    }

    /// Get icon from file extension.
    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "rs" => Self::Rust,
            "md" | "markdown" => Self::Markdown,
            "json" => Self::Json,
            "toml" => Self::Toml,
            "yaml" | "yml" => Self::Yaml,
            "ts" | "tsx" => Self::TypeScript,
            "js" | "jsx" | "mjs" | "cjs" => Self::JavaScript,
            "py" | "pyi" => Self::Python,
            "sh" | "bash" | "zsh" => Self::Shell,
            "gitignore" | "gitattributes" => Self::Git,
            _ => Self::File,
        }
    }
}

/// File tree panel state for the sidebar.
#[derive(Debug)]
pub struct FileTreePanel {
    /// Root directory being displayed.
    root: Option<PathBuf>,
    /// Flattened visible entries for rendering.
    visible_items: Vec<FileTreeRenderItem>,
    /// Currently selected path.
    selected: Option<PathBuf>,
    /// Currently hovered path.
    hovered: Option<PathBuf>,
    /// Set of expanded directory paths.
    expanded_dirs: std::collections::HashSet<PathBuf>,
    /// Whether to show hidden files.
    show_hidden: bool,
    /// Pending events to process.
    pending_events: Vec<FileTreeEvent>,
    /// Panel height for rendering.
    height: f32,
}

impl Default for FileTreePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl FileTreePanel {
    /// Default panel height.
    pub const DEFAULT_HEIGHT: f32 = 200.0;
    /// Height of each tree item row.
    pub const ITEM_HEIGHT: f32 = 24.0;
    /// Indentation per level in pixels.
    pub const INDENT_SIZE: f32 = 16.0;

    /// Create a new file tree panel.
    pub fn new() -> Self {
        Self {
            root: None,
            visible_items: Vec::new(),
            selected: None,
            hovered: None,
            expanded_dirs: std::collections::HashSet::new(),
            show_hidden: false,
            pending_events: Vec::new(),
            height: Self::DEFAULT_HEIGHT,
        }
    }

    /// Set the root directory for the file tree.
    pub fn set_root(&mut self, path: PathBuf) {
        self.root = Some(path);
        self.expanded_dirs.clear();
        self.selected = None;
    }

    /// Get the root directory.
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Update visible items from a codirigent-filetree FileTree.
    ///
    /// This method takes the visible entries from the file tree crate
    /// and converts them to render items.
    pub fn update_from_entries(&mut self, entries: Vec<(usize, FileTreeEntryData)>) {
        let mut expanded_dirs = std::collections::HashSet::new();

        self.visible_items = entries
            .into_iter()
            .map(|(depth, entry)| {
                if entry.is_dir && entry.expanded {
                    expanded_dirs.insert(entry.path.clone());
                }

                let icon = if entry.is_dir {
                    if entry.expanded {
                        FileTreeIcon::FolderOpen
                    } else {
                        FileTreeIcon::Folder
                    }
                } else {
                    entry
                        .path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(FileTreeIcon::from_extension)
                        .unwrap_or(FileTreeIcon::File)
                };

                FileTreeRenderItem {
                    is_selected: self.selected.as_ref() == Some(&entry.path),
                    is_hovered: self.hovered.as_ref() == Some(&entry.path),
                    expanded: entry.expanded,
                    path: entry.path,
                    name: entry.name,
                    is_dir: entry.is_dir,
                    icon,
                    depth,
                }
            })
            .collect();

        self.expanded_dirs = expanded_dirs;
    }

    /// Get the visible items for rendering.
    pub fn visible_items(&self) -> &[FileTreeRenderItem] {
        &self.visible_items
    }

    /// Get the number of visible items.
    pub fn visible_count(&self) -> usize {
        self.visible_items.len()
    }

    /// Select a path.
    pub fn select(&mut self, path: &Path) {
        self.selected = Some(path.to_path_buf());
        self.pending_events
            .push(FileTreeEvent::FileSelected(path.to_path_buf()));
    }

    /// Activate a path (double-click or enter).
    pub fn activate(&mut self, path: &Path) {
        self.pending_events
            .push(FileTreeEvent::FileActivated(path.to_path_buf()));
    }

    /// Toggle a directory's expanded state.
    pub fn toggle_directory(&mut self, path: &Path) {
        if self.expanded_dirs.contains(path) {
            self.expanded_dirs.remove(path);
        } else {
            self.expanded_dirs.insert(path.to_path_buf());
        }
        self.pending_events
            .push(FileTreeEvent::DirectoryToggled(path.to_path_buf()));
    }

    /// Check if a directory is expanded.
    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded_dirs.contains(path)
    }

    /// Set hover state for a path.
    pub fn set_hovered(&mut self, path: Option<&Path>) {
        self.hovered = path.map(|p| p.to_path_buf());
    }

    /// Get the selected path.
    pub fn selected(&self) -> Option<&Path> {
        self.selected.as_deref()
    }

    /// Set whether to show hidden files.
    pub fn set_show_hidden(&mut self, show: bool) {
        self.show_hidden = show;
    }

    /// Get whether hidden files are shown.
    pub fn show_hidden(&self) -> bool {
        self.show_hidden
    }

    /// Set the panel height.
    pub fn set_height(&mut self, height: f32) {
        self.height = height.max(Self::ITEM_HEIGHT * 3.0);
    }

    /// Get the panel height.
    pub fn height(&self) -> f32 {
        self.height
    }

    /// Calculate the content height based on visible items.
    pub fn content_height(&self) -> f32 {
        self.visible_items.len() as f32 * Self::ITEM_HEIGHT
    }

    /// Take all pending events.
    pub fn take_events(&mut self) -> Vec<FileTreeEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Request a path to be dragged to a terminal session.
    pub fn drag_path_to_terminal(&mut self, path: &Path, session_id: u64) {
        self.pending_events
            .push(FileTreeEvent::PathDraggedToTerminal {
                path: path.to_path_buf(),
                session_id,
            });
    }
}

/// Simplified entry data for the file tree panel.
///
/// This avoids coupling directly to codirigent-filetree types.
#[derive(Debug, Clone)]
pub struct FileTreeEntryData {
    /// File path.
    pub path: PathBuf,
    /// File name.
    pub name: String,
    /// Whether this is a directory.
    pub is_dir: bool,
    /// Whether this directory is expanded.
    pub expanded: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_tree_panel_new() {
        let panel = FileTreePanel::new();
        assert!(panel.root().is_none());
        assert!(panel.visible_items().is_empty());
        assert!(panel.selected().is_none());
    }

    #[test]
    fn test_file_tree_panel_default() {
        let panel = FileTreePanel::default();
        assert!(panel.root().is_none());
    }

    #[test]
    fn test_set_root() {
        let mut panel = FileTreePanel::new();
        panel.set_root(PathBuf::from("/tmp/project"));
        assert_eq!(panel.root(), Some(Path::new("/tmp/project")));
    }

    #[test]
    fn test_toggle_directory() {
        let mut panel = FileTreePanel::new();
        let path = PathBuf::from("/tmp/project/src");

        assert!(!panel.is_expanded(&path));
        panel.toggle_directory(&path);
        assert!(panel.is_expanded(&path));
        panel.toggle_directory(&path);
        assert!(!panel.is_expanded(&path));
    }

    #[test]
    fn test_select() {
        let mut panel = FileTreePanel::new();
        let path = PathBuf::from("/tmp/project/main.rs");

        panel.select(&path);
        assert_eq!(panel.selected(), Some(path.as_path()));

        let events = panel.take_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], FileTreeEvent::FileSelected(p) if p == &path));
    }

    #[test]
    fn test_activate() {
        let mut panel = FileTreePanel::new();
        let path = PathBuf::from("/tmp/project/main.rs");

        panel.activate(&path);
        let events = panel.take_events();
        assert!(matches!(&events[0], FileTreeEvent::FileActivated(p) if p == &path));
    }

    #[test]
    fn test_show_hidden() {
        let mut panel = FileTreePanel::new();
        assert!(!panel.show_hidden());
        panel.set_show_hidden(true);
        assert!(panel.show_hidden());
    }

    #[test]
    fn test_height() {
        let mut panel = FileTreePanel::new();
        assert_eq!(panel.height(), FileTreePanel::DEFAULT_HEIGHT);
        panel.set_height(300.0);
        assert_eq!(panel.height(), 300.0);
    }

    #[test]
    fn test_height_minimum() {
        let mut panel = FileTreePanel::new();
        panel.set_height(10.0); // Too small
        assert!(panel.height() >= FileTreePanel::ITEM_HEIGHT * 3.0);
    }

    #[test]
    fn test_update_from_entries() {
        let mut panel = FileTreePanel::new();
        let entries = vec![
            (
                0,
                FileTreeEntryData {
                    path: PathBuf::from("/tmp/project"),
                    name: "project".to_string(),
                    is_dir: true,
                    expanded: true,
                },
            ),
            (
                1,
                FileTreeEntryData {
                    path: PathBuf::from("/tmp/project/main.rs"),
                    name: "main.rs".to_string(),
                    is_dir: false,
                    expanded: false,
                },
            ),
        ];

        panel.update_from_entries(entries);
        assert_eq!(panel.visible_count(), 2);

        let items = panel.visible_items();
        assert_eq!(items[0].name, "project");
        assert!(items[0].is_dir);
        assert_eq!(items[1].name, "main.rs");
        assert!(!items[1].is_dir);
    }

    #[test]
    fn test_file_tree_icon_from_extension() {
        assert_eq!(FileTreeIcon::from_extension("rs"), FileTreeIcon::Rust);
        assert_eq!(FileTreeIcon::from_extension("py"), FileTreeIcon::Python);
        assert_eq!(FileTreeIcon::from_extension("js"), FileTreeIcon::JavaScript);
        assert_eq!(FileTreeIcon::from_extension("unknown"), FileTreeIcon::File);
    }

    #[test]
    fn test_file_tree_icon_text() {
        assert_eq!(FileTreeIcon::Folder.text(), "📁");
        assert_eq!(FileTreeIcon::FolderOpen.text(), "📂");
        assert_eq!(FileTreeIcon::Rust.text(), "🦀");
    }

    #[test]
    fn test_file_tree_icon_color() {
        let folder_color = FileTreeIcon::Folder.color();
        let file_color = FileTreeIcon::File.color();
        // Different icons should have different colors
        assert!(
            folder_color.r != file_color.r
                || folder_color.g != file_color.g
                || folder_color.b != file_color.b
        );
    }

    #[test]
    fn test_drag_path_to_terminal() {
        let mut panel = FileTreePanel::new();
        let path = PathBuf::from("/tmp/project/file.txt");

        panel.drag_path_to_terminal(&path, 42);
        let events = panel.take_events();
        assert!(
            matches!(&events[0], FileTreeEvent::PathDraggedToTerminal { path: p, session_id } if p == &path && *session_id == 42)
        );
    }

    #[test]
    fn test_file_tree_icon_lucide_icon() {
        let variants = [
            FileTreeIcon::Folder,
            FileTreeIcon::FolderOpen,
            FileTreeIcon::File,
            FileTreeIcon::Rust,
            FileTreeIcon::Markdown,
            FileTreeIcon::Json,
            FileTreeIcon::Toml,
            FileTreeIcon::Yaml,
            FileTreeIcon::TypeScript,
            FileTreeIcon::JavaScript,
            FileTreeIcon::Python,
            FileTreeIcon::Shell,
            FileTreeIcon::Git,
        ];
        for variant in &variants {
            let icon = variant.lucide_icon();
            assert!(!icon.is_empty(), "lucide_icon() empty for {:?}", variant);
        }
    }

    #[test]
    fn test_set_hovered() {
        let mut panel = FileTreePanel::new();
        let path = PathBuf::from("/tmp/file.txt");

        panel.set_hovered(Some(&path));
        // Hover state is reflected in render items when updated
    }

    #[test]
    fn test_content_height() {
        let mut panel = FileTreePanel::new();
        let entries = vec![
            (
                0,
                FileTreeEntryData {
                    path: PathBuf::from("/tmp/a"),
                    name: "a".to_string(),
                    is_dir: false,
                    expanded: false,
                },
            ),
            (
                0,
                FileTreeEntryData {
                    path: PathBuf::from("/tmp/b"),
                    name: "b".to_string(),
                    is_dir: false,
                    expanded: false,
                },
            ),
        ];
        panel.update_from_entries(entries);

        let expected = 2.0 * FileTreePanel::ITEM_HEIGHT;
        assert_eq!(panel.content_height(), expected);
    }
}
