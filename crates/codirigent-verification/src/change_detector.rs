//! Git-based change detection.
//!
//! This module provides change detection by parsing git diff output
//! to determine which files have been modified, added, or deleted.
//!
//! ## Overview
//!
//! The [`GitChangeDetector`] uses git commands to:
//! - Detect file changes since a specific commit
//! - Parse diff statistics for line counts
//! - Generate complete change summaries with risk assessment
//!
//! ## Example
//!
//! ```no_run
//! use codirigent_verification::GitChangeDetector;
//! use codirigent_core::{ChangeDetector, SessionId, TaskId};
//! use std::path::Path;
//!
//! let detector = GitChangeDetector::new();
//!
//! // Detect changes since last commit
//! let changes = detector.detect_changes(Path::new("/path/to/repo"), None).unwrap();
//! println!("Found {} changed files", changes.len());
//!
//! // Generate full summary
//! let summary = detector.generate_summary(
//!     TaskId("task-001".to_string()),
//!     SessionId(1),
//!     Path::new("/path/to/repo"),
//!     Some("HEAD~5"),
//! ).unwrap();
//! println!("Overall risk: {:?}", summary.risk_assessment.overall_risk);
//! ```

use crate::risk_assessor::RuleBasedRiskAssessor;
use anyhow::{Context, Result};
use codirigent_core::{
    ChangeDetector, ChangeSummary, ChangeType, FileChange, RiskAssessor, SessionId, TaskId,
};
use std::path::Path;
use std::process::Command;
use tracing::{debug, warn};

/// Git-based change detector.
///
/// Detects file changes by parsing git diff output and uses a
/// [`RuleBasedRiskAssessor`] for risk evaluation.
///
/// # Example
///
/// ```
/// use codirigent_verification::GitChangeDetector;
///
/// let detector = GitChangeDetector::new();
/// // Use with a git repository...
/// ```
#[derive(Debug, Clone)]
pub struct GitChangeDetector {
    risk_assessor: RuleBasedRiskAssessor,
}

impl Default for GitChangeDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl GitChangeDetector {
    /// Create a new git change detector.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::GitChangeDetector;
    ///
    /// let detector = GitChangeDetector::new();
    /// ```
    pub fn new() -> Self {
        Self {
            risk_assessor: RuleBasedRiskAssessor::new(),
        }
    }

    /// Create a new git change detector with a custom risk assessor.
    ///
    /// # Arguments
    ///
    /// * `risk_assessor` - Custom risk assessor to use
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::{GitChangeDetector, RuleBasedRiskAssessor};
    ///
    /// let mut assessor = RuleBasedRiskAssessor::new();
    /// assessor.add_high_risk_pattern("billing".to_string());
    /// let detector = GitChangeDetector::with_assessor(assessor);
    /// ```
    pub fn with_assessor(risk_assessor: RuleBasedRiskAssessor) -> Self {
        Self { risk_assessor }
    }

    /// Get a reference to the risk assessor.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::GitChangeDetector;
    /// use codirigent_core::{RiskAssessor, ChangeType, RiskLevel};
    /// use std::path::Path;
    ///
    /// let detector = GitChangeDetector::new();
    /// let assessor = detector.risk_assessor();
    /// let risk = assessor.assess_file(Path::new("test.rs"), ChangeType::Modified);
    /// ```
    pub fn risk_assessor(&self) -> &RuleBasedRiskAssessor {
        &self.risk_assessor
    }

    /// Get a mutable reference to the risk assessor.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::GitChangeDetector;
    ///
    /// let mut detector = GitChangeDetector::new();
    /// detector.risk_assessor_mut().add_high_risk_pattern("payment".to_string());
    /// ```
    pub fn risk_assessor_mut(&mut self) -> &mut RuleBasedRiskAssessor {
        &mut self.risk_assessor
    }

    /// Parse git diff --stat output to extract file changes with line counts.
    ///
    /// # Arguments
    ///
    /// * `output` - The raw output from `git diff --stat`
    ///
    /// # Returns
    ///
    /// A vector of tuples: (file_path, lines_added, lines_removed)
    ///
    /// # Format
    ///
    /// Git diff --stat outputs lines like:
    /// ```text
    /// src/auth.ts      | 10 ++++------
    /// src/utils.ts     | 5 +++--
    /// ```
    pub fn parse_diff_stat(&self, output: &str) -> Vec<(String, u32, u32)> {
        let mut results = Vec::new();

        for line in output.lines() {
            // Skip empty lines and summary lines
            if line.is_empty() || line.contains("files changed") || line.contains("file changed") {
                continue;
            }

            // Format: " file.ts | 10 ++----"
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() != 2 {
                continue;
            }

            let file = parts[0].trim().to_string();
            if file.is_empty() {
                continue;
            }

            let stats = parts[1].trim();

            // Count + and - characters for approximate line changes
            let added = stats.matches('+').count() as u32;
            let removed = stats.matches('-').count() as u32;

            results.push((file, added, removed));
        }

        results
    }

    /// Parse git diff --numstat output for accurate line counts.
    ///
    /// # Arguments
    ///
    /// * `output` - The raw output from `git diff --numstat`
    ///
    /// # Returns
    ///
    /// A vector of tuples: (file_path, lines_added, lines_removed)
    ///
    /// # Format
    ///
    /// Git diff --numstat outputs lines like:
    /// ```text
    /// 10    5    src/auth.ts
    /// 20    3    src/utils.ts
    /// ```
    pub fn parse_numstat(&self, output: &str) -> Vec<(String, u32, u32)> {
        let mut results = Vec::new();

        for line in output.lines() {
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }

            // Handle binary files (shown as "-")
            let added = parts[0].parse::<u32>().unwrap_or(0);
            let removed = parts[1].parse::<u32>().unwrap_or(0);
            let file = parts[2..].join(" ");

            if !file.is_empty() {
                results.push((file, added, removed));
            }
        }

        results
    }

    /// Get change type from git status code.
    ///
    /// # Arguments
    ///
    /// * `status` - Git status code (A, D, M, R, etc.)
    ///
    /// # Returns
    ///
    /// The corresponding [`ChangeType`].
    pub fn get_change_type(&self, status: &str) -> ChangeType {
        match status.chars().next() {
            Some('A') => ChangeType::Created,
            Some('D') => ChangeType::Deleted,
            Some('R') => ChangeType::Renamed,
            Some('M') => ChangeType::Modified,
            _ => ChangeType::Modified,
        }
    }

    /// Parse git status --porcelain output for unstaged changes.
    ///
    /// # Arguments
    ///
    /// * `output` - The raw output from `git status --porcelain`
    ///
    /// # Returns
    ///
    /// A vector of file changes (without accurate line counts).
    pub fn parse_porcelain_status(&self, output: &[u8]) -> Result<Vec<FileChange>> {
        let status_str = String::from_utf8_lossy(output);
        let mut changes = Vec::new();

        for line in status_str.lines() {
            if line.len() < 3 {
                continue;
            }

            let status = &line[0..2];
            let file_path = line[3..].trim();

            if file_path.is_empty() {
                continue;
            }

            let change_type = match status.trim() {
                "??" | "A" | "AM" => ChangeType::Created,
                "D" | " D" => ChangeType::Deleted,
                "R" | "RM" => ChangeType::Renamed,
                _ => ChangeType::Modified,
            };

            let path = std::path::PathBuf::from(file_path);
            let risk_level = self.risk_assessor.assess_file(&path, change_type);
            let categories = self.risk_assessor.categorize_file(&path);

            changes.push(FileChange {
                path,
                change_type,
                lines_added: 0, // Can't get accurate counts from porcelain status
                lines_removed: 0,
                risk_level,
                categories,
            });
        }

        Ok(changes)
    }

    /// Check if a directory is a git repository.
    ///
    /// # Arguments
    ///
    /// * `dir` - Directory to check
    ///
    /// # Returns
    ///
    /// `true` if the directory is in a git repository.
    pub fn is_git_repo(&self, dir: &Path) -> bool {
        Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .current_dir(dir)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

impl ChangeDetector for GitChangeDetector {
    /// Detect changes in a working directory since a commit.
    ///
    /// # Arguments
    ///
    /// * `working_dir` - Directory to scan for changes
    /// * `since_commit` - Optional commit hash to compare against (defaults to HEAD~1)
    ///
    /// # Returns
    ///
    /// A list of file changes detected in the directory.
    ///
    /// # Errors
    ///
    /// Returns an error if git commands fail or the directory is not a git repository.
    fn detect_changes(
        &self,
        working_dir: &Path,
        since_commit: Option<&str>,
    ) -> Result<Vec<FileChange>> {
        let base = since_commit.unwrap_or("HEAD~1");
        debug!(?working_dir, base, "Detecting changes since commit");

        // Check if it's a git repo
        if !self.is_git_repo(working_dir) {
            warn!(
                ?working_dir,
                "Not a git repository, returning empty changes"
            );
            return Ok(Vec::new());
        }

        // Get file status (change type per file)
        let status_output = Command::new("git")
            .args(["diff", "--name-status", base])
            .current_dir(working_dir)
            .output()
            .context("Failed to run git diff --name-status")?;

        if !status_output.status.success() {
            let stderr = String::from_utf8_lossy(&status_output.stderr);
            warn!(
                ?working_dir,
                %stderr,
                "git diff failed, falling back to status --porcelain"
            );

            // Fall back to checking uncommitted changes
            let status_output = Command::new("git")
                .args(["status", "--porcelain"])
                .current_dir(working_dir)
                .output()
                .context("Failed to run git status --porcelain")?;

            return self.parse_porcelain_status(&status_output.stdout);
        }

        // Get numeric line counts
        let numstat_output = Command::new("git")
            .args(["diff", "--numstat", base])
            .current_dir(working_dir)
            .output()
            .context("Failed to run git diff --numstat")?;

        let status_str = String::from_utf8_lossy(&status_output.stdout);
        let numstat_str = String::from_utf8_lossy(&numstat_output.stdout);

        let line_stats = self.parse_numstat(&numstat_str);
        let mut changes = Vec::new();

        for line in status_str.lines() {
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            let status = parts[0];
            let file_path = parts[1];

            let (lines_added, lines_removed) = line_stats
                .iter()
                .find(|(f, _, _)| f == file_path)
                .map(|(_, a, r)| (*a, *r))
                .unwrap_or((0, 0));

            let path = std::path::PathBuf::from(file_path);
            let change_type = self.get_change_type(status);
            let risk_level = self.risk_assessor.assess_file(&path, change_type);
            let categories = self.risk_assessor.categorize_file(&path);

            changes.push(FileChange {
                path,
                change_type,
                lines_added,
                lines_removed,
                risk_level,
                categories,
            });
        }

        debug!("Detected {} file changes", changes.len());
        Ok(changes)
    }

    /// Generate a full summary of changes.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task these changes are for
    /// * `session_id` - The session that made the changes
    /// * `working_dir` - Directory to scan for changes
    /// * `since_commit` - Optional commit hash to compare against
    ///
    /// # Returns
    ///
    /// A complete change summary with risk assessment.
    fn generate_summary(
        &self,
        task_id: TaskId,
        session_id: SessionId,
        working_dir: &Path,
        since_commit: Option<&str>,
    ) -> Result<ChangeSummary> {
        let changes = self.detect_changes(working_dir, since_commit)?;
        let risk_assessment = self.risk_assessor.assess_changeset(&changes);

        Ok(ChangeSummary {
            task_id,
            session_id,
            changes,
            risk_assessment,
            generated_at: chrono::Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // Constructor tests

    #[test]
    fn test_new_detector() {
        let detector = GitChangeDetector::new();
        // Verify the detector is usable by assessing a file
        let risk = detector
            .risk_assessor()
            .assess_file(Path::new("src/lib.rs"), ChangeType::Modified);
        assert_eq!(risk, codirigent_core::RiskLevel::Medium);
    }

    #[test]
    fn test_default_detector() {
        let detector = GitChangeDetector::default();
        // Verify the detector works
        let risk = detector
            .risk_assessor()
            .assess_file(Path::new("src/lib.rs"), ChangeType::Modified);
        assert_eq!(risk, codirigent_core::RiskLevel::Medium);
    }

    #[test]
    fn test_with_assessor() {
        let mut assessor = RuleBasedRiskAssessor::new();
        assessor.add_high_risk_pattern("custom".to_string());

        let detector = GitChangeDetector::with_assessor(assessor);
        // Verify custom pattern is used
        let risk = detector
            .risk_assessor()
            .assess_file(Path::new("src/custom/file.ts"), ChangeType::Modified);
        assert_eq!(risk, codirigent_core::RiskLevel::High);
    }

    #[test]
    fn test_clone_detector() {
        let detector = GitChangeDetector::new();
        let cloned = detector.clone();
        // Verify the cloned detector works
        let risk = cloned
            .risk_assessor()
            .assess_file(Path::new("src/lib.rs"), ChangeType::Modified);
        assert_eq!(risk, codirigent_core::RiskLevel::Medium);
    }

    #[test]
    fn test_risk_assessor_mut() {
        let mut detector = GitChangeDetector::new();
        detector
            .risk_assessor_mut()
            .add_high_risk_pattern("payment".to_string());
        // Verify the pattern was added
        let risk = detector
            .risk_assessor()
            .assess_file(Path::new("src/payment/stripe.ts"), ChangeType::Modified);
        assert_eq!(risk, codirigent_core::RiskLevel::High);
    }

    // Diff stat parsing tests

    #[test]
    fn test_parse_diff_stat_basic() {
        let detector = GitChangeDetector::new();
        let output = r#"
 src/auth.ts      | 10 ++++------
 src/utils.ts     | 5 +++--
"#;

        let results = detector.parse_diff_stat(output);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "src/auth.ts");
        assert_eq!(results[0].1, 4); // 4 plus signs
        assert_eq!(results[0].2, 6); // 6 minus signs
    }

    #[test]
    fn test_parse_diff_stat_empty() {
        let detector = GitChangeDetector::new();
        let output = "";
        let results = detector.parse_diff_stat(output);
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_diff_stat_with_summary() {
        let detector = GitChangeDetector::new();
        let output = r#"
 src/auth.ts | 10 ++++------
 2 files changed, 15 insertions(+), 10 deletions(-)
"#;

        let results = detector.parse_diff_stat(output);
        assert_eq!(results.len(), 1); // Summary line should be skipped
    }

    #[test]
    fn test_parse_diff_stat_no_changes() {
        let detector = GitChangeDetector::new();
        let output = " src/auth.ts | 0";
        let results = detector.parse_diff_stat(output);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, 0);
        assert_eq!(results[0].2, 0);
    }

    // Numstat parsing tests

    #[test]
    fn test_parse_numstat_basic() {
        let detector = GitChangeDetector::new();
        let output = "10\t5\tsrc/auth.ts\n20\t3\tsrc/utils.ts";

        let results = detector.parse_numstat(output);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], ("src/auth.ts".to_string(), 10, 5));
        assert_eq!(results[1], ("src/utils.ts".to_string(), 20, 3));
    }

    #[test]
    fn test_parse_numstat_empty() {
        let detector = GitChangeDetector::new();
        let output = "";
        let results = detector.parse_numstat(output);
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_numstat_binary_file() {
        let detector = GitChangeDetector::new();
        let output = "-\t-\timage.png";
        let results = detector.parse_numstat(output);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], ("image.png".to_string(), 0, 0));
    }

    #[test]
    fn test_parse_numstat_with_spaces_in_path() {
        let detector = GitChangeDetector::new();
        let output = "10\t5\tpath with spaces/file.ts";
        let results = detector.parse_numstat(output);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "path with spaces/file.ts");
    }

    // Change type tests

    #[test]
    fn test_get_change_type_added() {
        let detector = GitChangeDetector::new();
        assert_eq!(detector.get_change_type("A"), ChangeType::Created);
    }

    #[test]
    fn test_get_change_type_deleted() {
        let detector = GitChangeDetector::new();
        assert_eq!(detector.get_change_type("D"), ChangeType::Deleted);
    }

    #[test]
    fn test_get_change_type_modified() {
        let detector = GitChangeDetector::new();
        assert_eq!(detector.get_change_type("M"), ChangeType::Modified);
    }

    #[test]
    fn test_get_change_type_renamed() {
        let detector = GitChangeDetector::new();
        assert_eq!(detector.get_change_type("R"), ChangeType::Renamed);
    }

    #[test]
    fn test_get_change_type_unknown() {
        let detector = GitChangeDetector::new();
        assert_eq!(detector.get_change_type("X"), ChangeType::Modified);
    }

    #[test]
    fn test_get_change_type_empty() {
        let detector = GitChangeDetector::new();
        assert_eq!(detector.get_change_type(""), ChangeType::Modified);
    }

    // Porcelain status parsing tests

    #[test]
    fn test_parse_porcelain_status_new_file() {
        let detector = GitChangeDetector::new();
        let output = b"?? src/new.ts";
        let changes = detector.parse_porcelain_status(output).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, PathBuf::from("src/new.ts"));
        assert_eq!(changes[0].change_type, ChangeType::Created);
    }

    #[test]
    fn test_parse_porcelain_status_modified() {
        let detector = GitChangeDetector::new();
        let output = b" M src/utils.ts";
        let changes = detector.parse_porcelain_status(output).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].change_type, ChangeType::Modified);
    }

    #[test]
    fn test_parse_porcelain_status_deleted() {
        let detector = GitChangeDetector::new();
        let output = b" D src/old.ts";
        let changes = detector.parse_porcelain_status(output).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].change_type, ChangeType::Deleted);
    }

    #[test]
    fn test_parse_porcelain_status_multiple() {
        let detector = GitChangeDetector::new();
        let output = b"?? new.ts\n M modified.ts\n D deleted.ts";
        let changes = detector.parse_porcelain_status(output).unwrap();
        assert_eq!(changes.len(), 3);
    }

    #[test]
    fn test_parse_porcelain_status_empty() {
        let detector = GitChangeDetector::new();
        let output = b"";
        let changes = detector.parse_porcelain_status(output).unwrap();
        assert!(changes.is_empty());
    }

    #[test]
    fn test_parse_porcelain_status_with_risk_assessment() {
        let detector = GitChangeDetector::new();
        let output = b"?? src/auth/login.ts";
        let changes = detector.parse_porcelain_status(output).unwrap();
        assert_eq!(changes.len(), 1);
        // Auth files should be high risk
        assert_eq!(changes[0].risk_level, codirigent_core::RiskLevel::High);
    }

    // Git repo detection tests

    #[test]
    fn test_is_git_repo_non_existent() {
        let detector = GitChangeDetector::new();
        assert!(!detector.is_git_repo(Path::new("/non/existent/path")));
    }

    #[test]
    fn test_is_git_repo_tmp() {
        let detector = GitChangeDetector::new();
        // /tmp is typically not a git repo
        let is_repo = detector.is_git_repo(Path::new("/tmp"));
        // This test just verifies the function runs; result depends on system
        let _ = is_repo;
    }

    // Change detection tests

    #[test]
    fn test_detect_changes_non_git_dir() {
        let detector = GitChangeDetector::new();
        let result = detector.detect_changes(Path::new("/tmp"), None);
        assert!(result.is_ok());
        // Non-git directory returns empty changes
        assert!(result.unwrap().is_empty());
    }

    // Summary generation tests

    #[test]
    fn test_generate_summary_non_git_dir() {
        let detector = GitChangeDetector::new();
        let result = detector.generate_summary(
            TaskId("task-001".to_string()),
            SessionId(1),
            Path::new("/tmp"),
            None,
        );
        assert!(result.is_ok());
        let summary = result.unwrap();
        assert_eq!(summary.task_id, TaskId("task-001".to_string()));
        assert_eq!(summary.session_id, SessionId(1));
        assert!(summary.changes.is_empty());
    }

    // Debug trait test

    #[test]
    fn test_debug_trait() {
        let detector = GitChangeDetector::new();
        let debug_str = format!("{:?}", detector);
        assert!(debug_str.contains("GitChangeDetector"));
    }
}
