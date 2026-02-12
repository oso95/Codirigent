//! Change summary types for tracking file changes during tasks.
//!
//! This module provides types for detecting, categorizing, and summarizing
//! file changes made during a task session. Changes are assessed for risk
//! level based on file paths and patterns.
//!
//! ## Overview
//!
//! The change summary system tracks:
//! - Which files were created, modified, deleted, or renamed
//! - Lines added and removed for each file
//! - Risk level based on file type and location
//! - File categories for grouping and filtering
//!
//! ## Example
//!
//! ```
//! use codirigent_core::change_summary::{
//!     FileChange, ChangeType, RiskLevel, FileCategory, RiskAssessment,
//! };
//! use std::path::PathBuf;
//!
//! let change = FileChange {
//!     path: PathBuf::from("src/auth/middleware.ts"),
//!     change_type: ChangeType::Modified,
//!     lines_added: 45,
//!     lines_removed: 30,
//!     risk_level: RiskLevel::High,
//!     categories: vec![FileCategory::Security],
//! };
//! assert_eq!(change.risk_level, RiskLevel::High);
//! ```

use crate::types::{SessionId, TaskId};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Summary of all changes made during a task.
///
/// Contains the complete record of file changes, risk assessment,
/// and metadata about when the summary was generated.
///
/// # Example
///
/// ```
/// use codirigent_core::change_summary::{
///     ChangeSummary, FileChange, ChangeType, RiskLevel, FileCategory, RiskAssessment,
/// };
/// use codirigent_core::{SessionId, TaskId};
/// use std::path::PathBuf;
///
/// let summary = ChangeSummary {
///     task_id: TaskId::from("task-001"),
///     session_id: SessionId(1),
///     changes: vec![
///         FileChange {
///             path: PathBuf::from("src/lib.rs"),
///             change_type: ChangeType::Modified,
///             lines_added: 10,
///             lines_removed: 5,
///             risk_level: RiskLevel::Medium,
///             categories: vec![FileCategory::Core],
///         },
///     ],
///     risk_assessment: RiskAssessment {
///         overall_risk: RiskLevel::Medium,
///         high_risk_count: 0,
///         medium_risk_count: 1,
///         low_risk_count: 0,
///         total_files: 1,
///         total_lines_added: 10,
///         total_lines_removed: 5,
///         warnings: vec![],
///     },
///     generated_at: chrono::Utc::now(),
/// };
/// assert_eq!(summary.changes.len(), 1);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSummary {
    /// Task these changes are for.
    pub task_id: TaskId,
    /// Session that made the changes.
    pub session_id: SessionId,
    /// All file changes.
    pub changes: Vec<FileChange>,
    /// Risk assessment.
    pub risk_assessment: RiskAssessment,
    /// When the summary was generated.
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

/// A single file change.
///
/// Represents one file that was modified during the task, including
/// the type of change, line counts, risk level, and categories.
///
/// # Example
///
/// ```
/// use codirigent_core::change_summary::{FileChange, ChangeType, RiskLevel, FileCategory};
/// use std::path::PathBuf;
///
/// let change = FileChange {
///     path: PathBuf::from("tests/auth.test.ts"),
///     change_type: ChangeType::Created,
///     lines_added: 50,
///     lines_removed: 0,
///     risk_level: RiskLevel::Low,
///     categories: vec![FileCategory::Test],
/// };
/// assert!(change.lines_added > 0);
/// assert_eq!(change.lines_removed, 0);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileChange {
    /// Path to the file (relative to working directory).
    pub path: PathBuf,
    /// Type of change.
    pub change_type: ChangeType,
    /// Lines added.
    pub lines_added: u32,
    /// Lines removed.
    pub lines_removed: u32,
    /// Detected risk level for this file.
    pub risk_level: RiskLevel,
    /// Categories this file belongs to.
    pub categories: Vec<FileCategory>,
}

/// Type of file change.
///
/// Represents the kind of modification made to a file in version control.
///
/// # Example
///
/// ```
/// use codirigent_core::change_summary::ChangeType;
///
/// let change_type = ChangeType::Modified;
/// assert!(matches!(change_type, ChangeType::Modified));
///
/// let json = serde_json::to_string(&change_type).unwrap();
/// assert_eq!(json, "\"Modified\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    /// New file created.
    Created,
    /// Existing file modified.
    Modified,
    /// File deleted.
    Deleted,
    /// File renamed.
    Renamed,
}

impl std::fmt::Display for ChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangeType::Created => write!(f, "Created"),
            ChangeType::Modified => write!(f, "Modified"),
            ChangeType::Deleted => write!(f, "Deleted"),
            ChangeType::Renamed => write!(f, "Renamed"),
        }
    }
}

/// Risk level for a change.
///
/// Changes are classified by risk level to help reviewers prioritize
/// their attention. Higher risk changes typically involve security,
/// authentication, configuration, or database code.
///
/// # Ordering
///
/// Risk levels are ordered from Low to High:
/// - `Low` < `Medium` < `High`
///
/// # Example
///
/// ```
/// use codirigent_core::change_summary::RiskLevel;
///
/// assert!(RiskLevel::Low < RiskLevel::Medium);
/// assert!(RiskLevel::Medium < RiskLevel::High);
/// assert_eq!(RiskLevel::default(), RiskLevel::Medium);
/// ```
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum RiskLevel {
    /// Low risk - tests, docs, UI-only changes.
    Low,
    /// Medium risk - new features, business logic.
    #[default]
    Medium,
    /// High risk - auth, security, database, config.
    High,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "Low"),
            RiskLevel::Medium => write!(f, "Medium"),
            RiskLevel::High => write!(f, "High"),
        }
    }
}

/// Category of a file for grouping.
///
/// Files are categorized based on their path and extension to help
/// organize change reports and direct reviewer attention.
///
/// # Example
///
/// ```
/// use codirigent_core::change_summary::FileCategory;
///
/// let category = FileCategory::Security;
/// let json = serde_json::to_string(&category).unwrap();
/// assert_eq!(json, "\"Security\"");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FileCategory {
    /// Test files.
    Test,
    /// Documentation.
    Documentation,
    /// Configuration files.
    Config,
    /// Security/auth related.
    Security,
    /// Database/migration.
    Database,
    /// API endpoints.
    Api,
    /// User interface.
    Ui,
    /// Core business logic.
    Core,
    /// Build/tooling.
    Build,
    /// Other/unknown.
    Other,
}

impl std::fmt::Display for FileCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileCategory::Test => write!(f, "Test"),
            FileCategory::Documentation => write!(f, "Documentation"),
            FileCategory::Config => write!(f, "Config"),
            FileCategory::Security => write!(f, "Security"),
            FileCategory::Database => write!(f, "Database"),
            FileCategory::Api => write!(f, "Api"),
            FileCategory::Ui => write!(f, "Ui"),
            FileCategory::Core => write!(f, "Core"),
            FileCategory::Build => write!(f, "Build"),
            FileCategory::Other => write!(f, "Other"),
        }
    }
}

/// Overall risk assessment for a change set.
///
/// Aggregates statistics and warnings for all changes in a summary,
/// providing a quick overview for reviewers.
///
/// # Example
///
/// ```
/// use codirigent_core::change_summary::{RiskAssessment, RiskLevel};
///
/// let assessment = RiskAssessment {
///     overall_risk: RiskLevel::High,
///     high_risk_count: 2,
///     medium_risk_count: 3,
///     low_risk_count: 5,
///     total_files: 10,
///     total_lines_added: 342,
///     total_lines_removed: 89,
///     warnings: vec!["Modified auth configuration".to_string()],
/// };
/// assert_eq!(assessment.total_files, 10);
/// assert!(!assessment.warnings.is_empty());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskAssessment {
    /// Highest risk level in the changeset.
    pub overall_risk: RiskLevel,
    /// Number of high-risk files.
    pub high_risk_count: u32,
    /// Number of medium-risk files.
    pub medium_risk_count: u32,
    /// Number of low-risk files.
    pub low_risk_count: u32,
    /// Total files changed.
    pub total_files: u32,
    /// Total lines added.
    pub total_lines_added: u32,
    /// Total lines removed.
    pub total_lines_removed: u32,
    /// Specific warnings.
    pub warnings: Vec<String>,
}

impl Default for RiskAssessment {
    fn default() -> Self {
        Self {
            overall_risk: RiskLevel::Low,
            high_risk_count: 0,
            medium_risk_count: 0,
            low_risk_count: 0,
            total_files: 0,
            total_lines_added: 0,
            total_lines_removed: 0,
            warnings: Vec::new(),
        }
    }
}

/// Trait for generating change summaries.
///
/// Implementors detect file changes in a working directory and generate
/// structured summaries suitable for review.
///
/// # Example
///
/// ```
/// use codirigent_core::change_summary::{ChangeDetector, FileChange, ChangeSummary};
/// use codirigent_core::{SessionId, TaskId};
/// use std::path::Path;
///
/// // Trait is typically implemented by dirigent-verification crate
/// fn example_usage<T: ChangeDetector>(detector: &T, dir: &Path) {
///     let changes = detector.detect_changes(dir, None).unwrap();
///     println!("Found {} changes", changes.len());
/// }
/// ```
pub trait ChangeDetector: Send + Sync {
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
    fn detect_changes(
        &self,
        working_dir: &Path,
        since_commit: Option<&str>,
    ) -> anyhow::Result<Vec<FileChange>>;

    /// Generate a full summary.
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
    ) -> anyhow::Result<ChangeSummary>;
}

/// Trait for assessing risk of changes.
///
/// Implementors categorize files and assess risk levels based on
/// file paths, extensions, and change types.
///
/// # Example
///
/// ```
/// use codirigent_core::change_summary::{RiskAssessor, ChangeType, RiskLevel, FileCategory};
/// use std::path::Path;
///
/// // Trait is typically implemented by dirigent-verification crate
/// fn example_usage<T: RiskAssessor>(assessor: &T) {
///     let risk = assessor.assess_file(Path::new("src/auth.rs"), ChangeType::Modified);
///     println!("Risk level: {:?}", risk);
///
///     let categories = assessor.categorize_file(Path::new("tests/unit.rs"));
///     println!("Categories: {:?}", categories);
/// }
/// ```
pub trait RiskAssessor: Send + Sync {
    /// Assess risk level for a single file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file being assessed
    /// * `change_type` - Type of change made to the file
    ///
    /// # Returns
    ///
    /// The assessed risk level for this file change.
    fn assess_file(&self, path: &Path, change_type: ChangeType) -> RiskLevel;

    /// Categorize a file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file being categorized
    ///
    /// # Returns
    ///
    /// A list of categories the file belongs to.
    fn categorize_file(&self, path: &Path) -> Vec<FileCategory>;

    /// Generate overall risk assessment.
    ///
    /// # Arguments
    ///
    /// * `changes` - The list of file changes to assess
    ///
    /// # Returns
    ///
    /// An aggregate risk assessment for all changes.
    fn assess_changeset(&self, changes: &[FileChange]) -> RiskAssessment;
}

#[cfg(test)]
mod tests {
    use super::*;

    // FileChange tests
    #[test]
    fn test_file_change_creation() {
        let change = FileChange {
            path: PathBuf::from("src/auth/middleware.ts"),
            change_type: ChangeType::Modified,
            lines_added: 45,
            lines_removed: 30,
            risk_level: RiskLevel::High,
            categories: vec![FileCategory::Security],
        };
        assert_eq!(change.lines_added, 45);
        assert_eq!(change.lines_removed, 30);
        assert_eq!(change.risk_level, RiskLevel::High);
        assert_eq!(change.categories, vec![FileCategory::Security]);
    }

    #[test]
    fn test_file_change_equality() {
        let change1 = FileChange {
            path: PathBuf::from("src/lib.rs"),
            change_type: ChangeType::Modified,
            lines_added: 10,
            lines_removed: 5,
            risk_level: RiskLevel::Medium,
            categories: vec![FileCategory::Core],
        };
        let change2 = FileChange {
            path: PathBuf::from("src/lib.rs"),
            change_type: ChangeType::Modified,
            lines_added: 10,
            lines_removed: 5,
            risk_level: RiskLevel::Medium,
            categories: vec![FileCategory::Core],
        };
        assert_eq!(change1, change2);
    }

    #[test]
    fn test_file_change_inequality() {
        let change1 = FileChange {
            path: PathBuf::from("src/lib.rs"),
            change_type: ChangeType::Modified,
            lines_added: 10,
            lines_removed: 5,
            risk_level: RiskLevel::Medium,
            categories: vec![FileCategory::Core],
        };
        let change2 = FileChange {
            path: PathBuf::from("src/main.rs"),
            change_type: ChangeType::Modified,
            lines_added: 10,
            lines_removed: 5,
            risk_level: RiskLevel::Medium,
            categories: vec![FileCategory::Core],
        };
        assert_ne!(change1, change2);
    }

    #[test]
    fn test_file_change_serialization() {
        let change = FileChange {
            path: PathBuf::from("src/test.rs"),
            change_type: ChangeType::Created,
            lines_added: 100,
            lines_removed: 0,
            risk_level: RiskLevel::Low,
            categories: vec![FileCategory::Test],
        };
        let json = serde_json::to_string(&change).unwrap();
        let parsed: FileChange = serde_json::from_str(&json).unwrap();
        assert_eq!(change, parsed);
    }

    #[test]
    fn test_file_change_clone() {
        let change = FileChange {
            path: PathBuf::from("src/lib.rs"),
            change_type: ChangeType::Modified,
            lines_added: 10,
            lines_removed: 5,
            risk_level: RiskLevel::Medium,
            categories: vec![FileCategory::Core],
        };
        let cloned = change.clone();
        assert_eq!(change, cloned);
    }

    // ChangeType tests
    #[test]
    fn test_change_type_variants() {
        assert!(matches!(ChangeType::Created, ChangeType::Created));
        assert!(matches!(ChangeType::Modified, ChangeType::Modified));
        assert!(matches!(ChangeType::Deleted, ChangeType::Deleted));
        assert!(matches!(ChangeType::Renamed, ChangeType::Renamed));
    }

    #[test]
    fn test_change_type_serialization() {
        let change_type = ChangeType::Modified;
        let json = serde_json::to_string(&change_type).unwrap();
        assert_eq!(json, "\"Modified\"");

        let parsed: ChangeType = serde_json::from_str(&json).unwrap();
        assert_eq!(change_type, parsed);
    }

    #[test]
    fn test_change_type_all_variants_serialization() {
        let variants = [
            ChangeType::Created,
            ChangeType::Modified,
            ChangeType::Deleted,
            ChangeType::Renamed,
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: ChangeType = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_change_type_display() {
        assert_eq!(format!("{}", ChangeType::Created), "Created");
        assert_eq!(format!("{}", ChangeType::Modified), "Modified");
        assert_eq!(format!("{}", ChangeType::Deleted), "Deleted");
        assert_eq!(format!("{}", ChangeType::Renamed), "Renamed");
    }

    #[test]
    fn test_change_type_equality() {
        assert_eq!(ChangeType::Created, ChangeType::Created);
        assert_ne!(ChangeType::Created, ChangeType::Modified);
    }

    #[test]
    fn test_change_type_clone_copy() {
        let ct = ChangeType::Modified;
        let cloned = ct;
        assert_eq!(ct, cloned);
    }

    #[test]
    fn test_change_type_debug() {
        let ct = ChangeType::Renamed;
        let debug_str = format!("{:?}", ct);
        assert!(debug_str.contains("Renamed"));
    }

    // RiskLevel tests
    #[test]
    fn test_risk_level_ordering() {
        assert!(RiskLevel::Low < RiskLevel::Medium);
        assert!(RiskLevel::Medium < RiskLevel::High);
        assert!(RiskLevel::Low < RiskLevel::High);
    }

    #[test]
    fn test_risk_level_default() {
        assert_eq!(RiskLevel::default(), RiskLevel::Medium);
    }

    #[test]
    fn test_risk_level_serialization() {
        let risk = RiskLevel::High;
        let json = serde_json::to_string(&risk).unwrap();
        assert_eq!(json, "\"High\"");

        let parsed: RiskLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(risk, parsed);
    }

    #[test]
    fn test_risk_level_all_variants_serialization() {
        let variants = [RiskLevel::Low, RiskLevel::Medium, RiskLevel::High];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: RiskLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_risk_level_display() {
        assert_eq!(format!("{}", RiskLevel::Low), "Low");
        assert_eq!(format!("{}", RiskLevel::Medium), "Medium");
        assert_eq!(format!("{}", RiskLevel::High), "High");
    }

    #[test]
    fn test_risk_level_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(RiskLevel::High);
        assert!(set.contains(&RiskLevel::High));
        assert!(!set.contains(&RiskLevel::Low));
    }

    #[test]
    fn test_risk_level_clone_copy() {
        let risk = RiskLevel::Medium;
        let cloned = risk;
        assert_eq!(risk, cloned);
    }

    // FileCategory tests
    #[test]
    fn test_file_category_serialization() {
        let category = FileCategory::Security;
        let json = serde_json::to_string(&category).unwrap();
        assert_eq!(json, "\"Security\"");

        let parsed: FileCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(category, parsed);
    }

    #[test]
    fn test_file_category_all_variants_serialization() {
        let variants = [
            FileCategory::Test,
            FileCategory::Documentation,
            FileCategory::Config,
            FileCategory::Security,
            FileCategory::Database,
            FileCategory::Api,
            FileCategory::Ui,
            FileCategory::Core,
            FileCategory::Build,
            FileCategory::Other,
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: FileCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_file_category_display() {
        assert_eq!(format!("{}", FileCategory::Test), "Test");
        assert_eq!(format!("{}", FileCategory::Documentation), "Documentation");
        assert_eq!(format!("{}", FileCategory::Config), "Config");
        assert_eq!(format!("{}", FileCategory::Security), "Security");
        assert_eq!(format!("{}", FileCategory::Database), "Database");
        assert_eq!(format!("{}", FileCategory::Api), "Api");
        assert_eq!(format!("{}", FileCategory::Ui), "Ui");
        assert_eq!(format!("{}", FileCategory::Core), "Core");
        assert_eq!(format!("{}", FileCategory::Build), "Build");
        assert_eq!(format!("{}", FileCategory::Other), "Other");
    }

    #[test]
    fn test_file_category_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(FileCategory::Test);
        set.insert(FileCategory::Security);
        assert!(set.contains(&FileCategory::Test));
        assert!(set.contains(&FileCategory::Security));
        assert!(!set.contains(&FileCategory::Api));
    }

    #[test]
    fn test_file_category_equality() {
        assert_eq!(FileCategory::Test, FileCategory::Test);
        assert_ne!(FileCategory::Test, FileCategory::Api);
    }

    #[test]
    fn test_file_category_clone() {
        let category = FileCategory::Security;
        let cloned = category.clone();
        assert_eq!(category, cloned);
    }

    // RiskAssessment tests
    #[test]
    fn test_risk_assessment_creation() {
        let assessment = RiskAssessment {
            overall_risk: RiskLevel::High,
            high_risk_count: 2,
            medium_risk_count: 3,
            low_risk_count: 5,
            total_files: 10,
            total_lines_added: 342,
            total_lines_removed: 89,
            warnings: vec!["Modified auth configuration".to_string()],
        };
        assert_eq!(assessment.total_files, 10);
        assert_eq!(assessment.overall_risk, RiskLevel::High);
        assert!(!assessment.warnings.is_empty());
    }

    #[test]
    fn test_risk_assessment_default() {
        let assessment = RiskAssessment::default();
        assert_eq!(assessment.overall_risk, RiskLevel::Low);
        assert_eq!(assessment.high_risk_count, 0);
        assert_eq!(assessment.medium_risk_count, 0);
        assert_eq!(assessment.low_risk_count, 0);
        assert_eq!(assessment.total_files, 0);
        assert_eq!(assessment.total_lines_added, 0);
        assert_eq!(assessment.total_lines_removed, 0);
        assert!(assessment.warnings.is_empty());
    }

    #[test]
    fn test_risk_assessment_serialization() {
        let assessment = RiskAssessment {
            overall_risk: RiskLevel::Medium,
            high_risk_count: 1,
            medium_risk_count: 2,
            low_risk_count: 3,
            total_files: 6,
            total_lines_added: 100,
            total_lines_removed: 50,
            warnings: vec!["Warning 1".to_string(), "Warning 2".to_string()],
        };
        let json = serde_json::to_string(&assessment).unwrap();
        let parsed: RiskAssessment = serde_json::from_str(&json).unwrap();
        assert_eq!(assessment, parsed);
    }

    #[test]
    fn test_risk_assessment_equality() {
        let a1 = RiskAssessment::default();
        let a2 = RiskAssessment::default();
        assert_eq!(a1, a2);

        let a3 = RiskAssessment {
            total_files: 1,
            ..Default::default()
        };
        assert_ne!(a1, a3);
    }

    #[test]
    fn test_risk_assessment_clone() {
        let assessment = RiskAssessment {
            overall_risk: RiskLevel::High,
            warnings: vec!["Test".to_string()],
            ..Default::default()
        };
        let cloned = assessment.clone();
        assert_eq!(assessment, cloned);
    }

    // ChangeSummary tests
    #[test]
    fn test_change_summary_creation() {
        let summary = ChangeSummary {
            task_id: TaskId::from("task-001"),
            session_id: SessionId(1),
            changes: vec![FileChange {
                path: PathBuf::from("src/lib.rs"),
                change_type: ChangeType::Modified,
                lines_added: 10,
                lines_removed: 5,
                risk_level: RiskLevel::Medium,
                categories: vec![FileCategory::Core],
            }],
            risk_assessment: RiskAssessment::default(),
            generated_at: chrono::Utc::now(),
        };
        assert_eq!(summary.task_id, TaskId::from("task-001"));
        assert_eq!(summary.session_id, SessionId(1));
        assert_eq!(summary.changes.len(), 1);
    }

    #[test]
    fn test_change_summary_serialization() {
        let summary = ChangeSummary {
            task_id: TaskId::from("task-002"),
            session_id: SessionId(42),
            changes: vec![
                FileChange {
                    path: PathBuf::from("src/auth.rs"),
                    change_type: ChangeType::Modified,
                    lines_added: 20,
                    lines_removed: 10,
                    risk_level: RiskLevel::High,
                    categories: vec![FileCategory::Security],
                },
                FileChange {
                    path: PathBuf::from("tests/auth_test.rs"),
                    change_type: ChangeType::Created,
                    lines_added: 50,
                    lines_removed: 0,
                    risk_level: RiskLevel::Low,
                    categories: vec![FileCategory::Test],
                },
            ],
            risk_assessment: RiskAssessment {
                overall_risk: RiskLevel::High,
                high_risk_count: 1,
                medium_risk_count: 0,
                low_risk_count: 1,
                total_files: 2,
                total_lines_added: 70,
                total_lines_removed: 10,
                warnings: vec!["Modified security file".to_string()],
            },
            generated_at: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&summary).unwrap();
        let parsed: ChangeSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(summary.task_id, parsed.task_id);
        assert_eq!(summary.session_id, parsed.session_id);
        assert_eq!(summary.changes.len(), parsed.changes.len());
    }

    #[test]
    fn test_change_summary_empty_changes() {
        let summary = ChangeSummary {
            task_id: TaskId::from("task-empty"),
            session_id: SessionId(1),
            changes: vec![],
            risk_assessment: RiskAssessment::default(),
            generated_at: chrono::Utc::now(),
        };
        assert!(summary.changes.is_empty());
        assert_eq!(summary.risk_assessment.total_files, 0);
    }

    #[test]
    fn test_change_summary_clone() {
        let summary = ChangeSummary {
            task_id: TaskId::from("task-clone"),
            session_id: SessionId(1),
            changes: vec![],
            risk_assessment: RiskAssessment::default(),
            generated_at: chrono::Utc::now(),
        };
        let cloned = summary.clone();
        assert_eq!(summary.task_id, cloned.task_id);
        assert_eq!(summary.session_id, cloned.session_id);
    }

    #[test]
    fn test_change_summary_debug() {
        let summary = ChangeSummary {
            task_id: TaskId::from("task-debug"),
            session_id: SessionId(1),
            changes: vec![],
            risk_assessment: RiskAssessment::default(),
            generated_at: chrono::Utc::now(),
        };
        let debug_str = format!("{:?}", summary);
        assert!(debug_str.contains("ChangeSummary"));
        assert!(debug_str.contains("task-debug"));
    }
}
