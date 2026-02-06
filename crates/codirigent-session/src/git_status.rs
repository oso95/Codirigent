//! Git repository status detection service.
//!
//! Provides detection of git branch, dirty file count, staged status,
//! and HEAD SHA for session working directories. Results are cached
//! with a configurable TTL to avoid excessive git operations.

use codirigent_core::{GitChangeKind, GitChangedFile, GitRepoInfo};
use git2::{Repository, StatusOptions};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// Git status detection service with caching.
///
/// Detects git repository information for working directories
/// and caches results to avoid repeated filesystem operations.
///
/// # Example
///
/// ```no_run
/// use codirigent_session::git_status::GitStatusService;
/// use std::path::Path;
///
/// let mut service = GitStatusService::new();
/// if let Some(info) = service.detect(Path::new("/home/user/project")) {
///     println!("Branch: {}, Dirty: {}", info.branch, info.dirty_count);
/// }
/// ```
#[derive(Debug)]
pub struct GitStatusService {
    /// Cache of repo_root -> (timestamp, info).
    cache: HashMap<PathBuf, (Instant, GitRepoInfo)>,
}

impl GitStatusService {
    /// Create a new git status service.
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Detect git repository info for a working directory.
    ///
    /// Uses `Repository::discover()` to find the git repository
    /// from any subdirectory. Returns `None` if the directory
    /// is not inside a git repository.
    pub fn detect(&self, working_dir: &Path) -> Option<GitRepoInfo> {
        let repo = match Repository::discover(working_dir) {
            Ok(repo) => repo,
            Err(e) => {
                debug!(?working_dir, %e, "Not a git repository");
                return None;
            }
        };

        let repo_root = repo.workdir()?.to_path_buf();
        let branch = Self::get_branch_name(&repo);
        let head_sha = Self::get_head_sha(&repo);
        let file_status = Self::collect_file_statuses(&repo);

        Some(GitRepoInfo {
            repo_root,
            branch,
            dirty_count: file_status.dirty_count,
            has_staged: file_status.has_staged,
            head_sha,
            unstaged_files: file_status.unstaged,
            staged_files: file_status.staged,
        })
    }

    /// Detect with caching. Returns cached result if within TTL.
    ///
    /// # Arguments
    ///
    /// * `working_dir` - The working directory to detect git info for
    /// * `ttl` - How long to cache results before refreshing
    pub fn detect_cached(&mut self, working_dir: &Path, ttl: Duration) -> Option<GitRepoInfo> {
        // Check if we have a cached result for a repo containing this dir
        if let Some(cached) = self.find_cached(working_dir, ttl) {
            return Some(cached);
        }

        // No valid cache, detect fresh
        let info = self.detect(working_dir)?;
        self.cache
            .insert(info.repo_root.clone(), (Instant::now(), info.clone()));
        Some(info)
    }

    /// Invalidate the cache for a specific repo root.
    pub fn invalidate(&mut self, repo_root: &Path) {
        self.cache.remove(repo_root);
    }

    /// Find a cached entry for a working directory.
    fn find_cached(&self, working_dir: &Path, ttl: Duration) -> Option<GitRepoInfo> {
        for (root, (timestamp, info)) in &self.cache {
            if working_dir.starts_with(root) && timestamp.elapsed() < ttl {
                return Some(info.clone());
            }
        }
        None
    }

    /// Get the current branch name.
    fn get_branch_name(repo: &Repository) -> String {
        match repo.head() {
            Ok(head) => {
                if head.is_branch() {
                    head.shorthand()
                        .unwrap_or("unknown")
                        .to_string()
                } else {
                    "HEAD detached".to_string()
                }
            }
            Err(_) => "HEAD detached".to_string(),
        }
    }

    /// Get the short HEAD SHA (8 characters).
    fn get_head_sha(repo: &Repository) -> Option<String> {
        let head = repo.head().ok()?;
        let oid = head.target()?;
        Some(oid.to_string()[..8].to_string())
    }

    /// Collect file-level status information from the repository.
    fn collect_file_statuses(repo: &Repository) -> FileStatusResult {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(false);

        let statuses = match repo.statuses(Some(&mut opts)) {
            Ok(s) => s,
            Err(e) => {
                warn!(%e, "Failed to get git statuses");
                return FileStatusResult::default();
            }
        };

        let mut result = FileStatusResult::default();

        for entry in statuses.iter() {
            let status = entry.status();
            let path = entry
                .path()
                .unwrap_or("<invalid utf-8>")
                .to_string();

            // Unstaged (working tree) changes
            if status.intersects(git2::Status::WT_MODIFIED) {
                result.unstaged.push(GitChangedFile { path: path.clone(), change: GitChangeKind::Modified });
                result.dirty_count += 1;
            } else if status.intersects(git2::Status::WT_NEW) {
                result.unstaged.push(GitChangedFile { path: path.clone(), change: GitChangeKind::Added });
                result.dirty_count += 1;
            } else if status.intersects(git2::Status::WT_DELETED) {
                result.unstaged.push(GitChangedFile { path: path.clone(), change: GitChangeKind::Deleted });
                result.dirty_count += 1;
            } else if status.intersects(git2::Status::WT_RENAMED) {
                result.unstaged.push(GitChangedFile { path: path.clone(), change: GitChangeKind::Renamed });
                result.dirty_count += 1;
            }

            // Staged (index) changes
            if status.intersects(git2::Status::INDEX_MODIFIED) {
                result.staged.push(GitChangedFile { path: path.clone(), change: GitChangeKind::Modified });
                result.has_staged = true;
            } else if status.intersects(git2::Status::INDEX_NEW) {
                result.staged.push(GitChangedFile { path: path.clone(), change: GitChangeKind::Added });
                result.has_staged = true;
            } else if status.intersects(git2::Status::INDEX_DELETED) {
                result.staged.push(GitChangedFile { path: path.clone(), change: GitChangeKind::Deleted });
                result.has_staged = true;
            } else if status.intersects(git2::Status::INDEX_RENAMED) {
                result.staged.push(GitChangedFile { path: path.clone(), change: GitChangeKind::Renamed });
                result.has_staged = true;
            }
        }

        result
    }
}

/// Internal result from collecting file statuses.
#[derive(Default)]
struct FileStatusResult {
    dirty_count: usize,
    has_staged: bool,
    unstaged: Vec<GitChangedFile>,
    staged: Vec<GitChangedFile>,
}

impl Default for GitStatusService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    /// Create a temporary git repository for testing.
    fn create_test_repo() -> (tempfile::TempDir, Repository) {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        // Configure user for commits
        {
            let mut config = repo.config().unwrap();
            config.set_str("user.name", "Test User").unwrap();
            config.set_str("user.email", "test@example.com").unwrap();
        }

        // Create initial commit so HEAD exists
        {
            let sig = repo.signature().unwrap();
            let tree_id = {
                let mut index = repo.index().unwrap();
                index.write_tree().unwrap()
            };
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
                .unwrap();
        }

        (dir, repo)
    }

    #[test]
    fn test_detect_git_repo() {
        let (dir, _repo) = create_test_repo();
        let service = GitStatusService::new();

        let info = service.detect(dir.path()).unwrap();
        assert_eq!(info.repo_root, dir.path());
        assert!(!info.branch.is_empty());
        assert_eq!(info.dirty_count, 0);
        assert!(!info.has_staged);
        assert!(info.head_sha.is_some());
        assert_eq!(info.head_sha.as_ref().unwrap().len(), 8);
    }

    #[test]
    fn test_detect_non_git_directory() {
        let dir = tempfile::tempdir().unwrap();
        let service = GitStatusService::new();

        let info = service.detect(dir.path());
        assert!(info.is_none());
    }

    #[test]
    fn test_detect_branch_name() {
        let (dir, _repo) = create_test_repo();
        let service = GitStatusService::new();

        let info = service.detect(dir.path()).unwrap();
        // Default branch for git init is usually "master" or "main"
        assert!(
            info.branch == "master" || info.branch == "main",
            "Branch was: {}",
            info.branch
        );
    }

    #[test]
    fn test_detect_dirty_files() {
        let (dir, _repo) = create_test_repo();

        // Create an untracked file
        let file_path = dir.path().join("untracked.txt");
        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(b"hello").unwrap();

        let service = GitStatusService::new();
        let info = service.detect(dir.path()).unwrap();
        assert_eq!(info.dirty_count, 1);
        assert!(!info.has_staged);
    }

    #[test]
    fn test_detect_staged_files() {
        let (dir, repo) = create_test_repo();

        // Create and stage a file
        let file_path = dir.path().join("staged.txt");
        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(b"staged content").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("staged.txt")).unwrap();
        index.write().unwrap();

        let service = GitStatusService::new();
        let info = service.detect(dir.path()).unwrap();
        assert!(info.has_staged);
    }

    #[test]
    fn test_detect_subdirectory() {
        let (dir, _repo) = create_test_repo();

        // Create a subdirectory
        let subdir = dir.path().join("src");
        fs::create_dir_all(&subdir).unwrap();

        let service = GitStatusService::new();
        let info = service.detect(&subdir).unwrap();
        assert_eq!(info.repo_root, dir.path());
    }

    #[test]
    fn test_detect_cached() {
        let (dir, _repo) = create_test_repo();
        let mut service = GitStatusService::new();

        // First call should detect and cache
        let info1 = service
            .detect_cached(dir.path(), Duration::from_secs(60))
            .unwrap();

        // Second call should return cached result
        let info2 = service
            .detect_cached(dir.path(), Duration::from_secs(60))
            .unwrap();

        assert_eq!(info1, info2);
    }

    #[test]
    fn test_cache_invalidation() {
        let (dir, _repo) = create_test_repo();
        let mut service = GitStatusService::new();

        // Populate cache
        let info = service
            .detect_cached(dir.path(), Duration::from_secs(60))
            .unwrap();

        // Invalidate
        service.invalidate(&info.repo_root);

        // Cache should be empty now, verify by checking internal state
        assert!(!service.cache.contains_key(&info.repo_root));
    }

    #[test]
    fn test_cache_ttl_expiry() {
        let (dir, _repo) = create_test_repo();
        let mut service = GitStatusService::new();

        // Populate cache with zero TTL (immediately expired)
        service
            .detect_cached(dir.path(), Duration::from_secs(0))
            .unwrap();

        // Wait a tiny bit to ensure TTL expires
        std::thread::sleep(Duration::from_millis(1));

        // Should detect fresh (cache expired)
        // The find_cached should return None for expired entries
        let cached = service.find_cached(dir.path(), Duration::from_secs(0));
        assert!(cached.is_none());
    }

    #[test]
    fn test_detached_head() {
        let (dir, repo) = create_test_repo();

        // Detach HEAD by checking out a specific commit
        let head = repo.head().unwrap();
        let oid = head.target().unwrap();
        repo.set_head_detached(oid).unwrap();

        let service = GitStatusService::new();
        let info = service.detect(dir.path()).unwrap();
        assert_eq!(info.branch, "HEAD detached");
    }

    #[test]
    fn test_default_trait() {
        let service = GitStatusService::default();
        assert!(service.cache.is_empty());
    }
}
