//! Git worktree management.
//!
//! Provides functionality for managing git worktrees to enable
//! parallel development across multiple AI sessions. Each session
//! can be bound to its own worktree, allowing work on different
//! branches without conflicts.
//!
//! # Overview
//!
//! Git worktrees allow multiple working directories to be associated
//! with a single repository. This module provides:
//!
//! - Listing existing worktrees
//! - Creating new worktrees from branches
//! - Removing worktrees
//! - Binding sessions to worktrees
//! - Cleaning up merged worktrees
//!
//! # Example
//!
//! ```no_run
//! use codirigent_session::WorktreeManager;
//! use std::path::Path;
//!
//! let manager = WorktreeManager::new(Path::new("/path/to/repo")).unwrap();
//!
//! // List existing worktrees
//! for wt in manager.list() {
//!     println!("{}: {}", wt.branch, wt.path.display());
//! }
//! ```

use anyhow::{bail, Context, Result};
use codirigent_core::{SessionId, Worktree, WorktreeCreateOptions};
use git2::Repository;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Normalize a path by canonicalizing and stripping the `\\?\` prefix on Windows.
///
/// On Windows, `std::fs::canonicalize` returns UNC-style paths like `\\?\C:\...`
/// which break string comparisons with regular paths. This helper strips that prefix.
fn normalize_path(path: &Path) -> PathBuf {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    #[cfg(windows)]
    {
        let s = canonical.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
    }
    canonical
}

/// Git worktree manager.
///
/// Manages git worktrees for isolated parallel development across sessions.
/// This allows multiple AI sessions to work on different branches simultaneously
/// without conflicts.
///
/// # Example
///
/// ```no_run
/// use codirigent_session::WorktreeManager;
/// use std::path::Path;
///
/// let manager = WorktreeManager::new(Path::new("/path/to/repo")).unwrap();
/// println!("Found {} worktrees", manager.list().len());
/// ```
#[derive(Debug)]
pub struct WorktreeManager {
    /// Path to the main repository.
    repo_path: PathBuf,
    /// Cached worktree list.
    worktrees: Vec<Worktree>,
}

impl WorktreeManager {
    /// Create a new worktree manager for the given repository.
    ///
    /// This will verify that the path is a valid git repository and
    /// refresh the list of worktrees.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the git repository
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path cannot be canonicalized
    /// - The path is not a valid git repository
    ///
    /// # Example
    ///
    /// ```no_run
    /// use codirigent_session::WorktreeManager;
    /// use std::path::Path;
    ///
    /// let manager = WorktreeManager::new(Path::new("/path/to/repo")).unwrap();
    /// ```
    pub fn new(repo_path: &Path) -> Result<Self> {
        let repo_path = normalize_path(repo_path);

        // Verify it's a git repository
        Repository::open(&repo_path).context("Not a git repository")?;

        let mut manager = Self {
            repo_path,
            worktrees: Vec::new(),
        };
        manager.refresh()?;
        Ok(manager)
    }

    /// Get the repository path.
    ///
    /// Returns the canonical path to the main repository.
    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    /// List all worktrees.
    ///
    /// Returns a slice of all known worktrees, including the main worktree.
    /// Call `refresh()` to update this list from git.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use codirigent_session::WorktreeManager;
    /// use std::path::Path;
    ///
    /// let manager = WorktreeManager::new(Path::new("/path/to/repo")).unwrap();
    /// for wt in manager.list() {
    ///     println!("{}: {}", wt.branch, wt.path.display());
    /// }
    /// ```
    pub fn list(&self) -> &[Worktree] {
        &self.worktrees
    }

    /// Refresh the worktree list from git.
    ///
    /// This method reads the current worktree state from the git repository
    /// and updates the internal list. It includes the main worktree and all
    /// linked worktrees.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository cannot be opened or the worktree
    /// information cannot be read.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use codirigent_session::WorktreeManager;
    /// use std::path::Path;
    ///
    /// let mut manager = WorktreeManager::new(Path::new("/path/to/repo")).unwrap();
    /// manager.refresh().unwrap();
    /// ```
    pub fn refresh(&mut self) -> Result<()> {
        let repo = Repository::open(&self.repo_path)?;
        let mut worktrees = Vec::new();

        // Add main worktree
        let head = repo.head().ok();
        let main_branch = head
            .as_ref()
            .and_then(|h| h.shorthand())
            .unwrap_or("HEAD")
            .to_string();
        let head_sha = head
            .and_then(|h| h.target())
            .map(|oid| oid.to_string()[..8].to_string());

        let mut main_wt = Worktree::new(normalize_path(&self.repo_path), main_branch, true);
        if let Some(sha) = head_sha {
            main_wt = main_wt.with_head_sha(sha);
        }
        worktrees.push(main_wt);

        // List linked worktrees
        if let Ok(wt_names) = repo.worktrees() {
            for name in wt_names.iter().flatten() {
                if let Ok(wt) = repo.find_worktree(name) {
                    let path = normalize_path(wt.path());
                    // Try to get branch info from the worktree
                    if let Ok(wt_repo) = Repository::open(&path) {
                        let branch = wt_repo
                            .head()
                            .ok()
                            .and_then(|h| h.shorthand().map(String::from))
                            .unwrap_or_else(|| name.to_string());
                        let sha = wt_repo
                            .head()
                            .ok()
                            .and_then(|h| h.target())
                            .map(|oid| oid.to_string()[..8].to_string());

                        let mut linked_wt = Worktree::new(path, branch, false);
                        if let Some(sha) = sha {
                            linked_wt = linked_wt.with_head_sha(sha);
                        }
                        worktrees.push(linked_wt);
                    }
                }
            }
        }

        self.worktrees = worktrees;
        debug!(count = self.worktrees.len(), "Refreshed worktree list");
        Ok(())
    }

    /// Create a new worktree.
    ///
    /// Creates a new worktree with the specified options. If the branch doesn't
    /// exist, it will be created from the base branch (or HEAD if not specified).
    ///
    /// # Arguments
    ///
    /// * `options` - Configuration for the new worktree
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The repository cannot be opened
    /// - The branch cannot be created or found
    /// - The worktree directory already exists
    /// - Git worktree creation fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use codirigent_session::WorktreeManager;
    /// use codirigent_core::WorktreeCreateOptions;
    /// use std::path::Path;
    ///
    /// let mut manager = WorktreeManager::new(Path::new("/path/to/repo")).unwrap();
    /// let options = WorktreeCreateOptions::new("feature-branch".to_string())
    ///     .with_base_branch("main".to_string());
    /// let worktree = manager.create(options).unwrap();
    /// ```
    pub fn create(&mut self, options: WorktreeCreateOptions) -> Result<Worktree> {
        let repo = Repository::open(&self.repo_path)?;

        let worktree_path = options
            .path
            .unwrap_or_else(|| self.repo_path.join("worktrees").join(&options.branch));

        // Create parent directory if needed
        if let Some(parent) = worktree_path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create worktree parent directory")?;
        }

        info!(branch = %options.branch, path = ?worktree_path, "Creating worktree");

        // Check if branch exists, create if not
        let branch_exists = repo
            .find_branch(&options.branch, git2::BranchType::Local)
            .is_ok();

        if !branch_exists {
            // Create branch from base
            let base = options.base_branch.as_deref().unwrap_or("HEAD");
            let commit = repo
                .revparse_single(base)
                .context("Failed to find base branch")?
                .peel_to_commit()
                .context("Failed to peel to commit")?;
            repo.branch(&options.branch, &commit, false)
                .context("Failed to create branch")?;
        }

        // Get the reference for the branch
        let reference_name = format!("refs/heads/{}", options.branch);
        let reference = repo
            .find_reference(&reference_name)
            .context("Failed to find branch reference")?;

        // Create the worktree
        let mut add_options = git2::WorktreeAddOptions::new();
        add_options.reference(Some(&reference));
        repo.worktree(&options.branch, &worktree_path, Some(&add_options))
            .context("Failed to create worktree")?;

        // Refresh to pick up new worktree
        self.refresh()?;

        // Find and return the new worktree
        let normalized = normalize_path(&worktree_path);

        self.worktrees
            .iter()
            .find(|wt| wt.path == normalized || wt.path == worktree_path)
            .cloned()
            .context("Failed to find created worktree")
    }

    /// Remove a worktree.
    ///
    /// Removes the specified worktree from the repository. The main worktree
    /// cannot be removed.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the worktree to remove
    /// * `force` - If true, remove even if the worktree has uncommitted changes
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The worktree is not found
    /// - Attempting to remove the main worktree
    /// - Git worktree removal fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use codirigent_session::WorktreeManager;
    /// use std::path::Path;
    ///
    /// let mut manager = WorktreeManager::new(Path::new("/path/to/repo")).unwrap();
    /// manager.remove(Path::new("/path/to/repo/worktrees/feature"), false).unwrap();
    /// ```
    pub fn remove(&mut self, path: &Path, force: bool) -> Result<()> {
        let repo = Repository::open(&self.repo_path)?;
        let normalized = normalize_path(path);

        // Find worktree in our list
        let wt = self
            .worktrees
            .iter()
            .find(|w| w.path == normalized || w.path == path)
            .context("Worktree not found")?;

        if wt.is_main {
            bail!("Cannot remove main worktree");
        }

        info!(path = ?path, force, "Removing worktree");

        // Find and prune the worktree
        if let Ok(wt_names) = repo.worktrees() {
            for name in wt_names.iter().flatten() {
                if let Ok(git_wt) = repo.find_worktree(name) {
                    let git_wt_path = normalize_path(git_wt.path());
                    if git_wt_path == normalized {
                        if force {
                            // Remove directory first
                            std::fs::remove_dir_all(path).ok();
                        }
                        git_wt
                            .prune(Some(
                                git2::WorktreePruneOptions::new()
                                    .working_tree(true)
                                    .valid(force)
                                    .locked(force),
                            ))
                            .context("Failed to prune worktree")?;
                        break;
                    }
                }
            }
        }

        self.refresh()?;
        Ok(())
    }

    /// Bind a session to a worktree.
    ///
    /// Associates a session with a worktree. If the session is already bound
    /// to another worktree, it will be unbound first.
    ///
    /// # Arguments
    ///
    /// * `worktree_path` - Path to the worktree to bind to
    /// * `session_id` - ID of the session to bind
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The worktree is not found
    /// - The worktree is already bound to another session
    ///
    /// # Example
    ///
    /// ```no_run
    /// use codirigent_session::WorktreeManager;
    /// use codirigent_core::SessionId;
    /// use std::path::Path;
    ///
    /// let mut manager = WorktreeManager::new(Path::new("/path/to/repo")).unwrap();
    /// manager.bind_session(Path::new("/path/to/repo"), SessionId(1)).unwrap();
    /// ```
    pub fn bind_session(&mut self, worktree_path: &Path, session_id: SessionId) -> Result<()> {
        // Unbind from any existing worktree first
        self.unbind_session(session_id)?;

        let normalized = normalize_path(worktree_path);
        let wt = self
            .worktrees
            .iter_mut()
            .find(|w| w.path == normalized || w.path == worktree_path)
            .context("Worktree not found")?;

        if wt.bound_session.is_some() {
            bail!("Worktree already bound to another session");
        }

        info!(?session_id, path = ?worktree_path, "Binding session to worktree");
        wt.bound_session = Some(session_id);
        Ok(())
    }

    /// Unbind a session from its worktree.
    ///
    /// If the session is not bound to any worktree, this is a no-op.
    ///
    /// # Arguments
    ///
    /// * `session_id` - ID of the session to unbind
    ///
    /// # Errors
    ///
    /// This method always succeeds. If the session is not bound, it returns Ok.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use codirigent_session::WorktreeManager;
    /// use codirigent_core::SessionId;
    /// use std::path::Path;
    ///
    /// let mut manager = WorktreeManager::new(Path::new("/path/to/repo")).unwrap();
    /// manager.unbind_session(SessionId(1)).unwrap();
    /// ```
    pub fn unbind_session(&mut self, session_id: SessionId) -> Result<()> {
        for wt in &mut self.worktrees {
            if wt.bound_session == Some(session_id) {
                debug!(?session_id, path = ?wt.path, "Unbinding session from worktree");
                wt.bound_session = None;
                return Ok(());
            }
        }
        Ok(()) // Not bound to anything is fine
    }

    /// Get the worktree for a session.
    ///
    /// Returns the worktree that a session is bound to, if any.
    ///
    /// # Arguments
    ///
    /// * `session_id` - ID of the session to look up
    ///
    /// # Example
    ///
    /// ```no_run
    /// use codirigent_session::WorktreeManager;
    /// use codirigent_core::SessionId;
    /// use std::path::Path;
    ///
    /// let mut manager = WorktreeManager::new(Path::new("/path/to/repo")).unwrap();
    /// if let Some(wt) = manager.get_session_worktree(SessionId(1)) {
    ///     println!("Session 1 is on branch: {}", wt.branch);
    /// }
    /// ```
    pub fn get_session_worktree(&self, session_id: SessionId) -> Option<&Worktree> {
        self.worktrees
            .iter()
            .find(|wt| wt.bound_session == Some(session_id))
    }

    /// Clean up worktrees whose branches have been merged.
    ///
    /// Removes worktrees where the branch has been merged into the target branch
    /// and no session is bound. This helps keep the repository clean.
    ///
    /// # Arguments
    ///
    /// * `target_branch` - The branch to check merges against (e.g., "main")
    ///
    /// # Returns
    ///
    /// A list of paths that were removed.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The repository cannot be opened
    /// - The target branch cannot be found
    ///
    /// # Example
    ///
    /// ```no_run
    /// use codirigent_session::WorktreeManager;
    /// use std::path::Path;
    ///
    /// let mut manager = WorktreeManager::new(Path::new("/path/to/repo")).unwrap();
    /// let removed = manager.cleanup_merged("main").unwrap();
    /// println!("Removed {} merged worktrees", removed.len());
    /// ```
    pub fn cleanup_merged(&mut self, target_branch: &str) -> Result<Vec<PathBuf>> {
        let repo = Repository::open(&self.repo_path)?;
        let mut removed = Vec::new();

        // Get target branch commit
        let target = repo
            .find_branch(target_branch, git2::BranchType::Local)
            .context("Target branch not found")?;
        let target_commit = target
            .get()
            .peel_to_commit()
            .context("Failed to get target commit")?;

        // Collect paths to remove (non-main, unbound worktrees whose branches are merged)
        let paths_to_remove: Vec<_> = self
            .worktrees
            .iter()
            .filter(|wt| !wt.is_main && wt.bound_session.is_none())
            .filter_map(|wt| {
                // Check if branch is merged into target
                if let Ok(branch) = repo.find_branch(&wt.branch, git2::BranchType::Local) {
                    if let Ok(branch_commit) = branch.get().peel_to_commit() {
                        // A branch is "merged" if target contains the branch commit
                        if repo
                            .graph_descendant_of(target_commit.id(), branch_commit.id())
                            .unwrap_or(false)
                        {
                            return Some(wt.path.clone());
                        }
                    }
                }
                None
            })
            .collect();

        for path in paths_to_remove {
            if self.remove(&path, true).is_ok() {
                info!(path = ?path, "Cleaned up merged worktree");
                removed.push(path);
            }
        }

        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Repository;
    use tempfile::TempDir;

    /// Create a test repository with an initial commit.
    fn setup_test_repo() -> (TempDir, PathBuf) {
        let temp = TempDir::new().unwrap();
        let repo_path = temp.path().to_path_buf();
        let repo = Repository::init(&repo_path).unwrap();

        // Create an initial commit (required for worktree operations)
        let sig = repo.signature().unwrap();
        let tree_id = repo.index().unwrap().write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        (temp, repo_path)
    }

    /// Create a bare test repository (no initial commit).
    fn setup_bare_test_repo() -> (TempDir, PathBuf) {
        let temp = TempDir::new().unwrap();
        let repo_path = temp.path().to_path_buf();
        Repository::init(&repo_path).unwrap();
        (temp, repo_path)
    }

    #[test]
    fn test_worktree_manager_new() {
        let (_temp, path) = setup_test_repo();
        let manager = WorktreeManager::new(&path);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_worktree_manager_not_a_repo() {
        let temp = TempDir::new().unwrap();
        let result = WorktreeManager::new(temp.path());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Not a git repository"));
    }

    #[test]
    fn test_worktree_manager_invalid_path() {
        let result = WorktreeManager::new(Path::new("/nonexistent/path/to/repo"));
        assert!(result.is_err());
    }

    #[test]
    fn test_worktree_manager_repo_path() {
        let (_temp, path) = setup_test_repo();
        let manager = WorktreeManager::new(&path).unwrap();
        let expected = normalize_path(&path);
        assert_eq!(manager.repo_path(), expected);
    }

    #[test]
    fn test_worktree_manager_repo_path_is_canonical() {
        let (_temp, path) = setup_test_repo();
        // Add a .. segment to the path to test canonicalization
        let non_canonical = path.join("subdir").join("..");
        std::fs::create_dir(path.join("subdir")).unwrap();
        let manager = WorktreeManager::new(&non_canonical).unwrap();
        // The manager should store the canonical path (no .. segments)
        assert!(!manager.repo_path().to_string_lossy().contains(".."));
    }

    #[test]
    fn test_worktree_manager_list_initial() {
        let (_temp, path) = setup_test_repo();
        let manager = WorktreeManager::new(&path).unwrap();
        // Should have at least the main worktree
        assert!(!manager.list().is_empty());
        assert!(manager.list()[0].is_main);
    }

    #[test]
    fn test_worktree_manager_list_returns_main_worktree() {
        let (_temp, path) = setup_test_repo();
        let manager = WorktreeManager::new(&path).unwrap();

        let main_wt = manager.list().iter().find(|wt| wt.is_main);
        assert!(main_wt.is_some());

        let main_wt = main_wt.unwrap();
        let expected = normalize_path(&path);
        assert_eq!(main_wt.path, expected);
    }

    #[test]
    fn test_worktree_manager_refresh() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        // Refresh should succeed
        assert!(manager.refresh().is_ok());

        // Should still have the main worktree
        assert!(!manager.list().is_empty());
    }

    #[test]
    fn test_worktree_manager_refresh_updates_head() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        // Main worktree should have a head SHA
        let main_wt = manager.list().iter().find(|wt| wt.is_main).unwrap();
        assert!(main_wt.head_sha.is_some());

        // Refresh and check again
        manager.refresh().unwrap();
        let main_wt = manager.list().iter().find(|wt| wt.is_main).unwrap();
        assert!(main_wt.head_sha.is_some());
    }

    #[test]
    fn test_worktree_manager_list_empty_slice() {
        let (_temp, path) = setup_test_repo();
        let manager = WorktreeManager::new(&path).unwrap();

        // list() should return a slice that can be iterated
        let count = manager.list().iter().count();
        assert!(count >= 1);
    }

    #[test]
    fn test_worktree_manager_refresh_bare_repo() {
        let (_temp, path) = setup_bare_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        // Refresh should work even on a repo without commits
        // (though branch name will be HEAD)
        assert!(manager.refresh().is_ok());
    }

    #[test]
    fn test_worktree_manager_main_branch_name() {
        let (_temp, path) = setup_test_repo();
        let manager = WorktreeManager::new(&path).unwrap();

        let main_wt = manager.list().iter().find(|wt| wt.is_main).unwrap();
        // Default branch on new repos is usually 'master' or 'main'
        assert!(
            main_wt.branch == "master" || main_wt.branch == "main",
            "Expected 'master' or 'main', got '{}'",
            main_wt.branch
        );
    }

    // Create worktree tests
    #[test]
    fn test_create_worktree() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        let options = WorktreeCreateOptions::new("feature-test".to_string())
            .with_base_branch("HEAD".to_string());

        let result = manager.create(options);
        assert!(result.is_ok());

        let wt = result.unwrap();
        assert_eq!(wt.branch, "feature-test");
        assert!(!wt.is_main);
    }

    #[test]
    fn test_create_worktree_adds_to_list() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        let initial_count = manager.list().len();

        let options = WorktreeCreateOptions::new("feature-add".to_string());
        manager.create(options).unwrap();

        assert_eq!(manager.list().len(), initial_count + 1);
    }

    #[test]
    fn test_create_worktree_custom_path() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        let custom_path = path.join("custom-worktree");
        let options =
            WorktreeCreateOptions::new("feature-custom".to_string()).with_path(custom_path.clone());

        let wt = manager.create(options).unwrap();
        // Path might be canonicalized — use normalize_path for consistent comparison
        let expected = normalize_path(&custom_path);
        assert_eq!(wt.path, expected);
    }

    #[test]
    fn test_create_worktree_default_path() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        let options = WorktreeCreateOptions::new("feature-default".to_string());
        let wt = manager.create(options).unwrap();

        // Default path should be in worktrees/<branch>
        let expected_path = path.join("worktrees").join("feature-default");
        let expected = normalize_path(&expected_path);
        assert_eq!(wt.path, expected);
    }

    #[test]
    fn test_create_worktree_with_existing_branch() {
        let (_temp, path) = setup_test_repo();
        let repo = Repository::open(&path).unwrap();

        // Create a branch first
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("existing-branch", &head, false).unwrap();

        let mut manager = WorktreeManager::new(&path).unwrap();
        let options = WorktreeCreateOptions::new("existing-branch".to_string());
        let result = manager.create(options);

        assert!(result.is_ok());
    }

    #[test]
    fn test_create_worktree_creates_parent_dirs() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        let deep_path = path.join("deep").join("nested").join("path").join("wt");
        let options =
            WorktreeCreateOptions::new("feature-deep".to_string()).with_path(deep_path.clone());

        let result = manager.create(options);
        assert!(result.is_ok());
        assert!(deep_path.exists());
    }

    // Remove worktree tests
    #[test]
    fn test_remove_worktree() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        // Create a worktree first
        let options = WorktreeCreateOptions::new("to-remove".to_string());
        let wt = manager.create(options).unwrap();
        let wt_path = wt.path.clone();

        // To use non-force remove, the worktree must be "invalid" (dir removed)
        // So first remove the directory manually
        std::fs::remove_dir_all(&wt_path).unwrap();

        // Now remove without force should work
        let result = manager.remove(&wt_path, false);
        assert!(result.is_ok());

        // Should no longer be in list
        let found = manager.list().iter().any(|w| w.path == wt_path);
        assert!(!found);
    }

    #[test]
    fn test_remove_worktree_force() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        // Create a worktree
        let options = WorktreeCreateOptions::new("to-force-remove".to_string());
        let wt = manager.create(options).unwrap();
        let wt_path = wt.path.clone();

        // Remove with force
        let result = manager.remove(&wt_path, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_remove_main_worktree_fails() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        let canonical_path = path.canonicalize().unwrap();
        let result = manager.remove(&canonical_path, false);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("main worktree"));
    }

    #[test]
    fn test_remove_nonexistent_worktree_fails() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        let fake_path = path.join("nonexistent-worktree");
        let result = manager.remove(&fake_path, false);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_remove_worktree_updates_list() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        // Create a worktree
        let options = WorktreeCreateOptions::new("to-update-remove".to_string());
        let wt = manager.create(options).unwrap();
        let wt_path = wt.path.clone();

        let count_before = manager.list().len();

        // Remove it
        manager.remove(&wt_path, true).unwrap();

        assert_eq!(manager.list().len(), count_before - 1);
    }

    // Session binding tests
    #[test]
    fn test_bind_session() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        let session_id = SessionId(1);
        let main_path = manager.repo_path().to_path_buf();

        let result = manager.bind_session(&main_path, session_id);
        assert!(result.is_ok());

        let wt = manager.get_session_worktree(session_id);
        assert!(wt.is_some());
        assert!(wt.unwrap().is_main);
    }

    #[test]
    fn test_bind_session_nonexistent_worktree() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        let fake_path = path.join("nonexistent");
        let result = manager.bind_session(&fake_path, SessionId(1));

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_bind_session_already_bound() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        let main_path = manager.repo_path().to_path_buf();

        // Bind first session
        manager.bind_session(&main_path, SessionId(1)).unwrap();

        // Try to bind second session to same worktree
        let result = manager.bind_session(&main_path, SessionId(2));

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already bound"));
    }

    #[test]
    fn test_bind_session_rebinds_from_other_worktree() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        let main_path = manager.repo_path().to_path_buf();

        // Create a second worktree
        let options = WorktreeCreateOptions::new("feature-rebind".to_string());
        let wt2 = manager.create(options).unwrap();
        let wt2_path = wt2.path.clone();

        // Bind session to main
        manager.bind_session(&main_path, SessionId(1)).unwrap();

        // Now bind to second worktree - should unbind from main
        manager.bind_session(&wt2_path, SessionId(1)).unwrap();

        // Session should be on second worktree
        let wt = manager.get_session_worktree(SessionId(1)).unwrap();
        assert_eq!(wt.path, wt2_path);

        // Main worktree should not be bound
        let main_wt = manager.list().iter().find(|w| w.is_main).unwrap();
        assert!(main_wt.bound_session.is_none());
    }

    #[test]
    fn test_unbind_session() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        let session_id = SessionId(1);
        let main_path = manager.repo_path().to_path_buf();

        manager.bind_session(&main_path, session_id).unwrap();
        let result = manager.unbind_session(session_id);
        assert!(result.is_ok());

        assert!(manager.get_session_worktree(session_id).is_none());
    }

    #[test]
    fn test_unbind_session_not_bound() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        // Unbinding a session that's not bound should succeed
        let result = manager.unbind_session(SessionId(999));
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_session_worktree() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        let main_path = manager.repo_path().to_path_buf();
        manager.bind_session(&main_path, SessionId(1)).unwrap();

        let wt = manager.get_session_worktree(SessionId(1));
        assert!(wt.is_some());
        assert_eq!(wt.unwrap().path, main_path);
    }

    #[test]
    fn test_get_session_worktree_not_found() {
        let (_temp, path) = setup_test_repo();
        let manager = WorktreeManager::new(&path).unwrap();

        let wt = manager.get_session_worktree(SessionId(999));
        assert!(wt.is_none());
    }

    // Cleanup merged tests
    #[test]
    fn test_cleanup_merged_no_merged_worktrees() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        // Create a worktree (not merged)
        let options = WorktreeCreateOptions::new("feature-not-merged".to_string());
        manager.create(options).unwrap();

        // Get main branch name
        let main_branch = manager
            .list()
            .iter()
            .find(|w| w.is_main)
            .unwrap()
            .branch
            .clone();

        let removed = manager.cleanup_merged(&main_branch).unwrap();
        assert!(removed.is_empty());
    }

    #[test]
    fn test_cleanup_merged_target_branch_not_found() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        let result = manager.cleanup_merged("nonexistent-branch");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_cleanup_merged_skips_main_worktree() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        let main_branch = manager
            .list()
            .iter()
            .find(|w| w.is_main)
            .unwrap()
            .branch
            .clone();

        // This should not try to remove the main worktree
        let removed = manager.cleanup_merged(&main_branch).unwrap();
        assert!(removed.is_empty());

        // Main worktree should still exist
        assert!(manager.list().iter().any(|w| w.is_main));
    }

    #[test]
    fn test_cleanup_merged_skips_bound_worktrees() {
        let (_temp, path) = setup_test_repo();
        let mut manager = WorktreeManager::new(&path).unwrap();

        // Create and bind a worktree
        let options = WorktreeCreateOptions::new("feature-bound".to_string());
        let wt = manager.create(options).unwrap();
        manager.bind_session(&wt.path, SessionId(1)).unwrap();

        let main_branch = manager
            .list()
            .iter()
            .find(|w| w.is_main)
            .unwrap()
            .branch
            .clone();

        // Even if merged, bound worktrees should not be removed
        let removed = manager.cleanup_merged(&main_branch).unwrap();

        // The worktree should still exist because it's bound
        let wt_exists = manager.list().iter().any(|w| w.branch == "feature-bound");
        assert!(wt_exists);
        assert!(removed.is_empty());
    }
}
