//! File tree state and operations.
//!
//! This module provides the `FileTree` struct which manages a hierarchical
//! view of the filesystem with expand/collapse support.

use crate::entry::FileEntry;
use crate::error::{FileTreeError, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::debug;

/// File tree state managing a hierarchical view of the filesystem.
///
/// The file tree maintains a root directory and tracks which directories
/// are expanded. It provides methods for navigating, expanding, and
/// collapsing the tree.
///
/// # Example
///
/// ```no_run
/// use codirigent_filetree::FileTree;
/// use std::path::PathBuf;
///
/// let mut tree = FileTree::new(PathBuf::from("/home/user/project")).unwrap();
///
/// // Expand a directory
/// tree.expand(std::path::Path::new("/home/user/project/src")).unwrap();
///
/// // Get visible entries for rendering
/// for (depth, entry) in tree.visible_entries() {
///     println!("{}{}", "  ".repeat(depth), entry.name);
/// }
/// ```
pub struct FileTree {
    /// Root directory of the tree.
    root: PathBuf,

    /// Entries at the root level.
    entries: Vec<FileEntry>,

    /// Currently selected path.
    selected: Option<PathBuf>,

    /// Whether to show hidden files.
    show_hidden: bool,
}

impl FileTree {
    /// Create a new file tree for a directory.
    ///
    /// # Arguments
    ///
    /// * `root` - The root directory to display
    ///
    /// # Errors
    ///
    /// Returns an error if the path doesn't exist or isn't a directory.
    pub fn new(root: PathBuf) -> Result<Self> {
        if !root.exists() {
            return Err(FileTreeError::PathNotFound(root));
        }
        if !root.is_dir() {
            return Err(FileTreeError::NotADirectory(root));
        }

        let mut tree = Self {
            root: root.clone(),
            entries: Vec::new(),
            selected: None,
            show_hidden: false,
        };

        tree.load_entries(&root)?;
        Ok(tree)
    }

    /// Set whether to show hidden files.
    pub fn set_show_hidden(&mut self, show: bool) {
        self.show_hidden = show;
    }

    /// Get whether hidden files are shown.
    pub fn show_hidden(&self) -> bool {
        self.show_hidden
    }

    /// Get the root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Get all root-level entries.
    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    /// Get mutable access to entries.
    pub fn entries_mut(&mut self) -> &mut Vec<FileEntry> {
        &mut self.entries
    }

    /// Load entries for a directory.
    fn load_entries(&mut self, path: &Path) -> Result<()> {
        debug!(?path, "Loading file tree entries");

        let entries = read_directory(path, self.show_hidden)?;
        self.entries = entries;
        self.sort_entries();

        Ok(())
    }

    /// Sort entries (directories first, then alphabetically).
    fn sort_entries(&mut self) {
        sort_entries_recursive(&mut self.entries);
    }

    /// Refresh the entire tree.
    ///
    /// Re-reads the root directory and updates all entries.
    /// Preserves expansion state where possible.
    pub fn refresh(&mut self) -> Result<()> {
        let expanded_paths = self.collect_expanded_paths();
        let root = self.root.clone();
        self.load_entries(&root)?;
        self.restore_expanded_paths(&expanded_paths);
        Ok(())
    }

    /// Collect all expanded directory paths.
    fn collect_expanded_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        collect_expanded_paths_recursive(&self.entries, &mut paths);
        paths
    }

    /// Restore expansion state for paths.
    fn restore_expanded_paths(&mut self, paths: &[PathBuf]) {
        for path in paths {
            let _ = self.expand(path);
        }
    }

    /// Expand a directory.
    ///
    /// Loads the directory's children if not already loaded.
    ///
    /// # Errors
    ///
    /// Returns an error if the path is not found or cannot be read.
    pub fn expand(&mut self, path: &Path) -> Result<()> {
        if let Some(entry) = find_entry_mut(&mut self.entries, path) {
            if entry.is_dir && !entry.expanded {
                entry.children = Some(read_directory(&entry.path, self.show_hidden)?);
                if let Some(ref mut children) = entry.children {
                    sort_entries_recursive(children);
                }
                entry.expanded = true;
                debug!(?path, "Expanded directory");
            }
            Ok(())
        } else {
            Err(FileTreeError::EntryNotFound(path.to_path_buf()))
        }
    }

    /// Collapse a directory.
    ///
    /// Sets the expanded flag to false but keeps children loaded.
    pub fn collapse(&mut self, path: &Path) {
        if let Some(entry) = find_entry_mut(&mut self.entries, path) {
            entry.expanded = false;
            debug!(?path, "Collapsed directory");
        }
    }

    /// Toggle expand/collapse state.
    ///
    /// If expanded, collapses. If collapsed, expands.
    ///
    /// # Errors
    ///
    /// Returns an error if expansion fails.
    pub fn toggle(&mut self, path: &Path) -> Result<()> {
        if let Some(entry) = find_entry_mut(&mut self.entries, path) {
            if entry.expanded {
                entry.expanded = false;
            } else if entry.is_dir {
                entry.children = Some(read_directory(&entry.path, self.show_hidden)?);
                if let Some(ref mut children) = entry.children {
                    sort_entries_recursive(children);
                }
                entry.expanded = true;
            }
            Ok(())
        } else {
            Err(FileTreeError::EntryNotFound(path.to_path_buf()))
        }
    }

    /// Expand all directories recursively.
    ///
    /// Warning: This can be slow for large directory trees.
    pub fn expand_all(&mut self) -> Result<()> {
        expand_all_recursive(&mut self.entries, self.show_hidden)
    }

    /// Collapse all directories.
    pub fn collapse_all(&mut self) {
        collapse_all_recursive(&mut self.entries);
    }

    /// Select a path.
    pub fn select(&mut self, path: &Path) {
        self.selected = Some(path.to_path_buf());
    }

    /// Clear selection.
    pub fn clear_selection(&mut self) {
        self.selected = None;
    }

    /// Get the selected path.
    pub fn selected_path(&self) -> Option<&Path> {
        self.selected.as_deref()
    }

    /// Check if a path is selected.
    pub fn is_selected(&self, path: &Path) -> bool {
        self.selected.as_deref() == Some(path)
    }

    /// Find an entry by path.
    pub fn find_entry(&self, path: &Path) -> Option<&FileEntry> {
        find_entry(&self.entries, path)
    }

    /// Get a flat list of visible entries with their depth.
    ///
    /// Returns tuples of (depth, entry) for rendering. Only includes
    /// entries that are currently visible (i.e., in expanded directories).
    pub fn visible_entries(&self) -> Vec<(usize, &FileEntry)> {
        let mut result = Vec::new();
        collect_visible_entries(&self.entries, 0, &mut result);
        result
    }

    /// Get the total count of visible entries.
    pub fn visible_count(&self) -> usize {
        count_visible_entries(&self.entries)
    }

    /// Navigate to a specific path, expanding parents as needed.
    ///
    /// Returns true if the path was found and selected.
    pub fn navigate_to(&mut self, path: &Path) -> Result<bool> {
        // Collect ancestor paths
        let mut ancestors = Vec::new();
        let mut current = path.parent();
        while let Some(p) = current {
            if p.starts_with(&self.root) && p != self.root {
                ancestors.push(p.to_path_buf());
            }
            current = p.parent();
        }

        // Expand from root towards target
        ancestors.reverse();
        for ancestor in ancestors {
            self.expand(&ancestor)?;
        }

        // Select the target path
        if self.find_entry(path).is_some() {
            self.select(path);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get the path as a string suitable for terminal insertion.
    ///
    /// Properly escapes special characters for shell usage.
    pub fn path_for_terminal(&self, path: &Path) -> String {
        let path_str = path.to_string_lossy();
        // Escape spaces and special characters
        if path_str.contains(|c: char| c.is_whitespace() || "\"'`$\\!".contains(c)) {
            format!("'{}'", path_str.replace('\'', "'\\''"))
        } else {
            path_str.to_string()
        }
    }
}

/// Read directory entries.
fn read_directory(path: &Path, show_hidden: bool) -> Result<Vec<FileEntry>> {
    let mut entries = Vec::new();

    let read_result = fs::read_dir(path);
    let dir_iter = match read_result {
        Ok(iter) => iter,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            return Err(FileTreeError::PermissionDenied(path.to_path_buf()));
        }
        Err(e) => return Err(FileTreeError::Io(e)),
    };

    for entry in dir_iter {
        let entry = entry?;
        let file_entry = FileEntry::new(entry.path());

        if !show_hidden && file_entry.is_hidden {
            continue;
        }

        entries.push(file_entry);
    }

    Ok(entries)
}

/// Sort entries recursively (directories first, then alphabetically).
fn sort_entries_recursive(entries: &mut [FileEntry]) {
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    for entry in entries.iter_mut() {
        if let Some(ref mut children) = entry.children {
            sort_entries_recursive(children);
        }
    }
}

/// Find entry by path (immutable).
fn find_entry<'a>(entries: &'a [FileEntry], path: &Path) -> Option<&'a FileEntry> {
    for entry in entries {
        if entry.path == path {
            return Some(entry);
        }
        if let Some(ref children) = entry.children {
            if let Some(found) = find_entry(children, path) {
                return Some(found);
            }
        }
    }
    None
}

/// Find entry by path (mutable).
fn find_entry_mut<'a>(entries: &'a mut [FileEntry], path: &Path) -> Option<&'a mut FileEntry> {
    for entry in entries.iter_mut() {
        if entry.path == path {
            return Some(entry);
        }
        if let Some(ref mut children) = entry.children {
            if let Some(found) = find_entry_mut(children, path) {
                return Some(found);
            }
        }
    }
    None
}

/// Collect all expanded paths.
fn collect_expanded_paths_recursive(entries: &[FileEntry], paths: &mut Vec<PathBuf>) {
    for entry in entries {
        if entry.expanded {
            paths.push(entry.path.clone());
            if let Some(ref children) = entry.children {
                collect_expanded_paths_recursive(children, paths);
            }
        }
    }
}

/// Expand all entries recursively.
fn expand_all_recursive(entries: &mut [FileEntry], show_hidden: bool) -> Result<()> {
    for entry in entries.iter_mut() {
        if entry.is_dir && !entry.expanded {
            entry.children = Some(read_directory(&entry.path, show_hidden)?);
            entry.expanded = true;
        }
        if let Some(ref mut children) = entry.children {
            sort_entries_recursive(children);
            expand_all_recursive(children, show_hidden)?;
        }
    }
    Ok(())
}

/// Collapse all entries recursively.
fn collapse_all_recursive(entries: &mut [FileEntry]) {
    for entry in entries.iter_mut() {
        entry.expanded = false;
        if let Some(ref mut children) = entry.children {
            collapse_all_recursive(children);
        }
    }
}

/// Collect visible entries with depth.
fn collect_visible_entries<'a>(
    entries: &'a [FileEntry],
    depth: usize,
    result: &mut Vec<(usize, &'a FileEntry)>,
) {
    for entry in entries {
        result.push((depth, entry));
        if entry.expanded {
            if let Some(ref children) = entry.children {
                collect_visible_entries(children, depth + 1, result);
            }
        }
    }
}

/// Count visible entries.
fn count_visible_entries(entries: &[FileEntry]) -> usize {
    let mut count = entries.len();
    for entry in entries {
        if entry.expanded {
            if let Some(ref children) = entry.children {
                count += count_visible_entries(children);
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_structure(temp: &TempDir) {
        std::fs::write(temp.path().join("file1.txt"), "content").unwrap();
        std::fs::write(temp.path().join("file2.rs"), "code").unwrap();

        let subdir = temp.path().join("subdir");
        std::fs::create_dir(&subdir).unwrap();
        std::fs::write(subdir.join("nested.txt"), "nested").unwrap();

        let deep = subdir.join("deep");
        std::fs::create_dir(&deep).unwrap();
        std::fs::write(deep.join("deepfile.md"), "deep content").unwrap();
    }

    #[test]
    fn test_file_tree_new() {
        let temp = TempDir::new().unwrap();
        create_test_structure(&temp);

        let tree = FileTree::new(temp.path().to_path_buf()).unwrap();
        assert!(!tree.entries().is_empty());
        assert_eq!(tree.root(), temp.path());
    }

    #[test]
    fn test_file_tree_nonexistent() {
        let result = FileTree::new(PathBuf::from("/nonexistent/path/that/does/not/exist"));
        assert!(matches!(result, Err(FileTreeError::PathNotFound(_))));
    }

    #[test]
    fn test_file_tree_not_directory() {
        let temp = TempDir::new().unwrap();
        let file = temp.path().join("file.txt");
        std::fs::write(&file, "content").unwrap();

        let result = FileTree::new(file);
        assert!(matches!(result, Err(FileTreeError::NotADirectory(_))));
    }

    #[test]
    fn test_expand_collapse() {
        let temp = TempDir::new().unwrap();
        create_test_structure(&temp);

        let mut tree = FileTree::new(temp.path().to_path_buf()).unwrap();
        let subdir = temp.path().join("subdir");

        // Initially collapsed
        let entry = tree.find_entry(&subdir).unwrap();
        assert!(!entry.expanded);

        // Expand
        tree.expand(&subdir).unwrap();
        let entry = tree.find_entry(&subdir).unwrap();
        assert!(entry.expanded);
        assert!(entry.has_children());

        // Collapse
        tree.collapse(&subdir);
        let entry = tree.find_entry(&subdir).unwrap();
        assert!(!entry.expanded);
    }

    #[test]
    fn test_toggle() {
        let temp = TempDir::new().unwrap();
        create_test_structure(&temp);

        let mut tree = FileTree::new(temp.path().to_path_buf()).unwrap();
        let subdir = temp.path().join("subdir");

        // Toggle on
        tree.toggle(&subdir).unwrap();
        assert!(tree.find_entry(&subdir).unwrap().expanded);

        // Toggle off
        tree.toggle(&subdir).unwrap();
        assert!(!tree.find_entry(&subdir).unwrap().expanded);
    }

    #[test]
    fn test_visible_entries() {
        let temp = TempDir::new().unwrap();
        create_test_structure(&temp);

        let mut tree = FileTree::new(temp.path().to_path_buf()).unwrap();

        // Initially only root entries visible
        let initial_count = tree.visible_count();

        // Expand subdir
        let subdir = temp.path().join("subdir");
        tree.expand(&subdir).unwrap();

        // More entries should be visible now
        assert!(tree.visible_count() > initial_count);

        // Check depths
        let visible = tree.visible_entries();
        for (depth, entry) in &visible {
            if entry.path.starts_with(&subdir) && entry.path != subdir {
                assert!(*depth >= 1);
            }
        }
    }

    #[test]
    fn test_hidden_files() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join(".hidden"), "hidden").unwrap();
        std::fs::write(temp.path().join("visible.txt"), "visible").unwrap();

        // Without hidden files
        let tree = FileTree::new(temp.path().to_path_buf()).unwrap();
        assert_eq!(tree.entries().len(), 1);
        assert_eq!(tree.entries()[0].name, "visible.txt");

        // With hidden files
        let mut tree = FileTree::new(temp.path().to_path_buf()).unwrap();
        tree.set_show_hidden(true);
        tree.refresh().unwrap();
        assert_eq!(tree.entries().len(), 2);
    }

    #[test]
    fn test_select() {
        let temp = TempDir::new().unwrap();
        create_test_structure(&temp);

        let mut tree = FileTree::new(temp.path().to_path_buf()).unwrap();
        let file = temp.path().join("file1.txt");

        assert!(tree.selected_path().is_none());

        tree.select(&file);
        assert_eq!(tree.selected_path(), Some(file.as_path()));
        assert!(tree.is_selected(&file));

        tree.clear_selection();
        assert!(tree.selected_path().is_none());
    }

    #[test]
    fn test_refresh_preserves_expansion() {
        let temp = TempDir::new().unwrap();
        create_test_structure(&temp);

        let mut tree = FileTree::new(temp.path().to_path_buf()).unwrap();
        let subdir = temp.path().join("subdir");

        tree.expand(&subdir).unwrap();
        assert!(tree.find_entry(&subdir).unwrap().expanded);

        tree.refresh().unwrap();
        assert!(tree.find_entry(&subdir).unwrap().expanded);
    }

    #[test]
    fn test_collapse_all() {
        let temp = TempDir::new().unwrap();
        create_test_structure(&temp);

        let mut tree = FileTree::new(temp.path().to_path_buf()).unwrap();
        let subdir = temp.path().join("subdir");
        let deep = subdir.join("deep");

        tree.expand(&subdir).unwrap();
        tree.expand(&deep).unwrap();

        tree.collapse_all();

        assert!(!tree.find_entry(&subdir).unwrap().expanded);
    }

    #[test]
    fn test_sort_order() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("zebra.txt"), "z").unwrap();
        std::fs::write(temp.path().join("alpha.txt"), "a").unwrap();
        std::fs::create_dir(temp.path().join("middle")).unwrap();

        let tree = FileTree::new(temp.path().to_path_buf()).unwrap();
        let entries = tree.entries();

        // Directory should come first
        assert!(entries[0].is_dir);
        assert_eq!(entries[0].name, "middle");

        // Files should be alphabetically sorted
        assert_eq!(entries[1].name, "alpha.txt");
        assert_eq!(entries[2].name, "zebra.txt");
    }

    #[test]
    fn test_navigate_to() {
        let temp = TempDir::new().unwrap();
        create_test_structure(&temp);

        let mut tree = FileTree::new(temp.path().to_path_buf()).unwrap();
        let deep_file = temp.path().join("subdir").join("deep").join("deepfile.md");

        // Navigate should expand parents and select target
        let found = tree.navigate_to(&deep_file).unwrap();
        assert!(found);
        assert!(tree.is_selected(&deep_file));

        // Parents should be expanded
        assert!(
            tree.find_entry(&temp.path().join("subdir"))
                .unwrap()
                .expanded
        );
    }

    #[test]
    fn test_path_for_terminal_simple() {
        let temp = TempDir::new().unwrap();
        let tree = FileTree::new(temp.path().to_path_buf()).unwrap();

        let simple_path = PathBuf::from("/home/user/file.txt");
        assert_eq!(tree.path_for_terminal(&simple_path), "/home/user/file.txt");
    }

    #[test]
    fn test_path_for_terminal_with_spaces() {
        let temp = TempDir::new().unwrap();
        let tree = FileTree::new(temp.path().to_path_buf()).unwrap();

        let path_with_spaces = PathBuf::from("/home/user/my file.txt");
        assert_eq!(
            tree.path_for_terminal(&path_with_spaces),
            "'/home/user/my file.txt'"
        );
    }

    #[test]
    fn test_path_for_terminal_with_quote() {
        let temp = TempDir::new().unwrap();
        let tree = FileTree::new(temp.path().to_path_buf()).unwrap();

        let path_with_quote = PathBuf::from("/home/user/it's.txt");
        assert_eq!(
            tree.path_for_terminal(&path_with_quote),
            "'/home/user/it'\\''s.txt'"
        );
    }

    #[test]
    fn test_entries_mut() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("file.txt"), "content").unwrap();

        let mut tree = FileTree::new(temp.path().to_path_buf()).unwrap();
        let entries = tree.entries_mut();

        assert!(!entries.is_empty());
        // Modify an entry
        entries[0].name = "modified".to_string();

        assert_eq!(tree.entries()[0].name, "modified");
    }

    #[test]
    fn test_show_hidden_getter() {
        let temp = TempDir::new().unwrap();
        let mut tree = FileTree::new(temp.path().to_path_buf()).unwrap();

        assert!(!tree.show_hidden());
        tree.set_show_hidden(true);
        assert!(tree.show_hidden());
    }

    #[test]
    fn test_expand_nonexistent_entry() {
        let temp = TempDir::new().unwrap();
        let mut tree = FileTree::new(temp.path().to_path_buf()).unwrap();

        let result = tree.expand(Path::new("/nonexistent/path"));
        assert!(matches!(result, Err(FileTreeError::EntryNotFound(_))));
    }

    #[test]
    fn test_toggle_nonexistent_entry() {
        let temp = TempDir::new().unwrap();
        let mut tree = FileTree::new(temp.path().to_path_buf()).unwrap();

        let result = tree.toggle(Path::new("/nonexistent/path"));
        assert!(matches!(result, Err(FileTreeError::EntryNotFound(_))));
    }
}
