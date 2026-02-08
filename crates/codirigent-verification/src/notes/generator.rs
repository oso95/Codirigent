//! Session notes generation.
//!
//! Provides the [`DefaultNotesGenerator`] implementation that creates
//! markdown session notes from task completion data.

use anyhow::{Context, Result};
use codirigent_core::{
    change_summary::{ChangeSummary, ChangeType, RiskLevel},
    session_notes::{GenerateNoteParams, Learning, NotesGenerator, SessionNote},
    verification::{VerificationCheckType, VerificationStatus},
};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

/// Default session notes generator.
///
/// Implements the [`NotesGenerator`] trait to create, render, and save
/// session notes as markdown files.
///
/// # Example
///
/// ```
/// use codirigent_verification::notes::DefaultNotesGenerator;
/// use codirigent_core::session_notes::{NotesGenerator, CompletionStatus, GenerateNoteParams};
/// use codirigent_core::{SessionId, TaskId};
///
/// let generator = DefaultNotesGenerator::new();
/// let params = GenerateNoteParams {
///     task_id: TaskId("task-001".to_string()),
///     session_id: SessionId(1),
///     title: "Test Task".to_string(),
///     duration_minutes: 30,
///     completion_status: CompletionStatus::Completed,
///     change_summary: None,
///     verification: None,
/// };
/// let note = generator.generate(params).unwrap();
///
/// let markdown = generator.render_markdown(&note);
/// assert!(markdown.contains("Test Task"));
/// ```
#[derive(Debug, Default, Clone)]
pub struct DefaultNotesGenerator;

impl DefaultNotesGenerator {
    /// Create a new notes generator.
    pub fn new() -> Self {
        Self
    }

    /// Format verification results section for markdown.
    ///
    /// # Arguments
    ///
    /// * `verification` - The verification status to format
    ///
    /// # Returns
    ///
    /// A markdown-formatted string for the verification section.
    fn format_verification(&self, verification: &VerificationStatus) -> String {
        let mut output = String::new();
        output.push_str("## Verification Results\n\n");

        for result in &verification.results {
            let check_name = match result.check_type {
                VerificationCheckType::UnitTest => "Unit Tests",
                VerificationCheckType::IntegrationTest => "Integration Tests",
                VerificationCheckType::TypeCheck => "Type Check",
                VerificationCheckType::Lint => "Lint",
                VerificationCheckType::Format => "Format",
                VerificationCheckType::Custom => "Custom",
            };

            let status = if result.passed { "PASS" } else { "FAIL" };
            let count = match (result.passed_count, result.total_count) {
                (Some(p), Some(t)) => format!("{}/{}", p, t),
                _ => String::new(),
            };

            let suffix = if result.passed {
                ""
            } else {
                " (see failures below)"
            };

            output.push_str(&format!(
                "- {} {}: {}{}\n",
                status, check_name, count, suffix
            ));
        }

        output
    }

    /// Format file changes section for markdown.
    ///
    /// # Arguments
    ///
    /// * `summary` - The change summary to format
    ///
    /// # Returns
    ///
    /// A markdown-formatted string with a table of changes.
    fn format_changes(&self, summary: &ChangeSummary) -> String {
        let mut output = String::new();
        output.push_str("## Files Changed\n\n");
        output.push_str("| File | Action | Changes | Risk |\n");
        output.push_str("|------|--------|---------|------|\n");

        for change in &summary.changes {
            let action = match change.change_type {
                ChangeType::Created => "Created",
                ChangeType::Modified => "Modified",
                ChangeType::Deleted => "Deleted",
                ChangeType::Renamed => "Renamed",
            };

            let changes = format!("+{}, -{}", change.lines_added, change.lines_removed);

            let risk = match change.risk_level {
                RiskLevel::High => "High",
                RiskLevel::Medium => "Medium",
                RiskLevel::Low => "Low",
            };

            output.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                change.path.display(),
                action,
                changes,
                risk
            ));
        }

        let assessment = &summary.risk_assessment;
        output.push_str(&format!(
            "\n**Total:** {} files, +{} lines, -{} lines\n",
            assessment.total_files, assessment.total_lines_added, assessment.total_lines_removed
        ));

        output
    }

    /// Format learnings section for markdown.
    ///
    /// # Arguments
    ///
    /// * `learnings` - The learnings to format
    ///
    /// # Returns
    ///
    /// A markdown-formatted string listing learnings.
    fn format_learnings(&self, learnings: &[Learning]) -> String {
        let mut output = String::new();
        output.push_str("## Learnings\n\n");

        let suggested: Vec<_> = learnings
            .iter()
            .filter(|l| l.suggested_for_claude_md)
            .collect();
        let other: Vec<_> = learnings
            .iter()
            .filter(|l| !l.suggested_for_claude_md)
            .collect();

        if !suggested.is_empty() {
            output.push_str("_Suggested for CLAUDE.md:_\n\n");
            for learning in suggested {
                output.push_str(&format!(
                    "- **[{}]** {}\n",
                    learning.category, learning.content
                ));
            }
            output.push('\n');
        }

        if !other.is_empty() {
            output.push_str("_Other learnings:_\n\n");
            for learning in other {
                output.push_str(&format!("- [{}] {}\n", learning.category, learning.content));
            }
            output.push('\n');
        }

        output
    }

    /// Generate a safe filename from a title.
    ///
    /// Converts the title to lowercase, replaces spaces with dashes,
    /// and removes non-alphanumeric characters.
    ///
    /// # Arguments
    ///
    /// * `title` - The title to convert
    ///
    /// # Returns
    ///
    /// A filename-safe version of the title.
    fn safe_filename(&self, title: &str) -> String {
        title
            .to_lowercase()
            .replace(' ', "-")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect()
    }
}

impl NotesGenerator for DefaultNotesGenerator {
    fn generate(&self, params: GenerateNoteParams) -> Result<SessionNote> {
        info!(%params.task_id, %params.session_id, "Generating session note");

        Ok(SessionNote {
            task_id: params.task_id,
            title: params.title,
            session_id: params.session_id,
            duration_minutes: params.duration_minutes,
            completion_status: params.completion_status,
            change_summary: params.change_summary,
            verification: params.verification,
            summary: None, // Will be set by AI if auto mode
            learnings: vec![],
            generated_at: chrono::Utc::now(),
        })
    }

    fn render_markdown(&self, note: &SessionNote) -> String {
        let mut output = String::new();

        // Header
        output.push_str(&format!("# Session Notes: {}\n\n", note.title));
        output.push_str(&format!("**Task ID:** {}  \n", note.task_id));
        output.push_str(&format!("**Session:** {}  \n", note.session_id));
        output.push_str(&format!(
            "**Duration:** {} minutes  \n",
            note.duration_minutes
        ));
        output.push_str(&format!("**Status:** {}\n\n", note.completion_status));

        output.push_str("---\n\n");

        // File changes
        if let Some(ref summary) = note.change_summary {
            output.push_str(&self.format_changes(summary));
            output.push('\n');
        }

        // Verification results
        if let Some(ref verification) = note.verification {
            output.push_str(&self.format_verification(verification));
            output.push('\n');
        }

        // Summary
        output.push_str("## Summary\n\n");
        if let Some(ref summary) = note.summary {
            output.push_str(summary);
            output.push_str("\n\n");
        } else {
            output.push_str("_Click [Generate Summary] to create (requires tokens)_\n\n");
        }

        // Learnings
        if !note.learnings.is_empty() {
            output.push_str(&self.format_learnings(&note.learnings));
        }

        // Footer
        output.push_str("---\n\n");
        output.push_str(&format!(
            "*Generated by Codirigent at {}*\n",
            note.generated_at.format("%Y-%m-%d %H:%M")
        ));

        output
    }

    fn save(&self, note: &SessionNote, output_dir: &Path) -> Result<PathBuf> {
        // Create date-based directory
        let date = note.generated_at.format("%Y-%m-%d").to_string();
        let dir = output_dir.join(&date);
        fs::create_dir_all(&dir).context("Failed to create notes directory")?;

        // Generate filename
        let safe_title = self.safe_filename(&note.title);
        let filename = format!("{}-{}.md", note.task_id, safe_title);
        let path = dir.join(&filename);

        // Render and save
        let content = self.render_markdown(note);
        fs::write(&path, content).context("Failed to write session note")?;

        info!(?path, "Saved session note");
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codirigent_core::change_summary::{FileCategory, FileChange, RiskAssessment};
    use codirigent_core::session_notes::CompletionStatus;
    use codirigent_core::verification::{VerificationResult, VerificationState};
    use codirigent_core::{SessionId, TaskId};
    use tempfile::TempDir;

    #[test]
    fn test_generator_new() {
        let generator = DefaultNotesGenerator::new();
        assert!(format!("{:?}", generator).contains("DefaultNotesGenerator"));
    }

    #[test]
    fn test_generator_default() {
        let generator = DefaultNotesGenerator;
        assert!(format!("{:?}", generator).contains("DefaultNotesGenerator"));
    }

    #[test]
    fn test_generator_clone() {
        let generator = DefaultNotesGenerator::new();
        let _cloned = generator.clone();
    }

    #[test]
    fn test_generate_note() {
        let generator = DefaultNotesGenerator::new();
        let params = GenerateNoteParams {
            task_id: TaskId("task-001".to_string()),
            session_id: SessionId(1),
            title: "Test Task".to_string(),
            duration_minutes: 30,
            completion_status: CompletionStatus::Completed,
            change_summary: None,
            verification: None,
        };
        let note = generator.generate(params).unwrap();

        assert_eq!(note.task_id.0, "task-001");
        assert_eq!(note.duration_minutes, 30);
        assert!(note.summary.is_none());
        assert!(note.learnings.is_empty());
    }

    #[test]
    fn test_generate_note_with_change_summary() {
        let generator = DefaultNotesGenerator::new();
        let summary = ChangeSummary {
            task_id: TaskId("task-001".to_string()),
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

        let params = GenerateNoteParams {
            task_id: TaskId("task-001".to_string()),
            session_id: SessionId(1),
            title: "Test Task".to_string(),
            duration_minutes: 30,
            completion_status: CompletionStatus::Completed,
            change_summary: Some(summary),
            verification: None,
        };
        let note = generator.generate(params).unwrap();

        assert!(note.change_summary.is_some());
    }

    #[test]
    fn test_generate_note_with_verification() {
        let generator = DefaultNotesGenerator::new();
        let verification = VerificationStatus {
            task_id: TaskId("task-001".to_string()),
            session_id: SessionId(1),
            state: VerificationState::Passed,
            retry_count: 0,
            results: vec![VerificationResult::passed(
                VerificationCheckType::UnitTest,
                1000,
            )],
            started_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
        };

        let params = GenerateNoteParams {
            task_id: TaskId("task-001".to_string()),
            session_id: SessionId(1),
            title: "Test Task".to_string(),
            duration_minutes: 30,
            completion_status: CompletionStatus::Completed,
            change_summary: None,
            verification: Some(verification),
        };
        let note = generator.generate(params).unwrap();

        assert!(note.verification.is_some());
    }

    #[test]
    fn test_render_markdown_basic() {
        let generator = DefaultNotesGenerator::new();
        let note = SessionNote {
            task_id: TaskId("task-001".to_string()),
            title: "Refactor Auth".to_string(),
            session_id: SessionId(1),
            duration_minutes: 45,
            completion_status: CompletionStatus::Completed,
            change_summary: None,
            verification: None,
            summary: None,
            learnings: vec![],
            generated_at: chrono::Utc::now(),
        };

        let markdown = generator.render_markdown(&note);
        assert!(markdown.contains("# Session Notes: Refactor Auth"));
        assert!(markdown.contains("**Task ID:** task-001"));
        assert!(markdown.contains("**Duration:** 45 minutes"));
        assert!(markdown.contains("**Status:** Completed"));
        assert!(markdown.contains("Generated by Codirigent"));
    }

    #[test]
    fn test_render_markdown_with_summary() {
        let generator = DefaultNotesGenerator::new();
        let note = SessionNote {
            task_id: TaskId("task-001".to_string()),
            title: "Test".to_string(),
            session_id: SessionId(1),
            duration_minutes: 30,
            completion_status: CompletionStatus::Completed,
            change_summary: None,
            verification: None,
            summary: Some("This task refactored the auth module.".to_string()),
            learnings: vec![],
            generated_at: chrono::Utc::now(),
        };

        let markdown = generator.render_markdown(&note);
        assert!(markdown.contains("This task refactored the auth module."));
    }

    #[test]
    fn test_render_markdown_no_summary_placeholder() {
        let generator = DefaultNotesGenerator::new();
        let note = SessionNote {
            task_id: TaskId("task-001".to_string()),
            title: "Test".to_string(),
            session_id: SessionId(1),
            duration_minutes: 30,
            completion_status: CompletionStatus::Completed,
            change_summary: None,
            verification: None,
            summary: None,
            learnings: vec![],
            generated_at: chrono::Utc::now(),
        };

        let markdown = generator.render_markdown(&note);
        assert!(markdown.contains("Click [Generate Summary]"));
    }

    #[test]
    fn test_render_markdown_with_changes() {
        let generator = DefaultNotesGenerator::new();
        let summary = ChangeSummary {
            task_id: TaskId("task-001".to_string()),
            session_id: SessionId(1),
            changes: vec![
                FileChange {
                    path: PathBuf::from("src/auth.rs"),
                    change_type: ChangeType::Modified,
                    lines_added: 50,
                    lines_removed: 20,
                    risk_level: RiskLevel::High,
                    categories: vec![FileCategory::Security],
                },
                FileChange {
                    path: PathBuf::from("tests/auth_test.rs"),
                    change_type: ChangeType::Created,
                    lines_added: 100,
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
                total_lines_added: 150,
                total_lines_removed: 20,
                warnings: vec![],
            },
            generated_at: chrono::Utc::now(),
        };

        let note = SessionNote {
            task_id: TaskId("task-001".to_string()),
            title: "Auth Refactor".to_string(),
            session_id: SessionId(1),
            duration_minutes: 60,
            completion_status: CompletionStatus::Completed,
            change_summary: Some(summary),
            verification: None,
            summary: None,
            learnings: vec![],
            generated_at: chrono::Utc::now(),
        };

        let markdown = generator.render_markdown(&note);
        assert!(markdown.contains("## Files Changed"));
        assert!(markdown.contains("src/auth.rs"));
        assert!(markdown.contains("Modified"));
        assert!(markdown.contains("+50, -20"));
        assert!(markdown.contains("High"));
        assert!(markdown.contains("**Total:** 2 files"));
    }

    #[test]
    fn test_render_markdown_with_verification() {
        let generator = DefaultNotesGenerator::new();
        let verification = VerificationStatus {
            task_id: TaskId("task-001".to_string()),
            session_id: SessionId(1),
            state: VerificationState::Passed,
            retry_count: 0,
            results: vec![
                VerificationResult::passed(VerificationCheckType::UnitTest, 1000)
                    .with_counts(20, 20),
                VerificationResult::passed(VerificationCheckType::Lint, 500),
            ],
            started_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
        };

        let note = SessionNote {
            task_id: TaskId("task-001".to_string()),
            title: "Test".to_string(),
            session_id: SessionId(1),
            duration_minutes: 30,
            completion_status: CompletionStatus::Completed,
            change_summary: None,
            verification: Some(verification),
            summary: None,
            learnings: vec![],
            generated_at: chrono::Utc::now(),
        };

        let markdown = generator.render_markdown(&note);
        assert!(markdown.contains("## Verification Results"));
        assert!(markdown.contains("PASS Unit Tests"));
        assert!(markdown.contains("PASS Lint"));
        assert!(markdown.contains("20/20"));
    }

    #[test]
    fn test_render_markdown_with_failed_verification() {
        let generator = DefaultNotesGenerator::new();
        let verification = VerificationStatus {
            task_id: TaskId("task-001".to_string()),
            session_id: SessionId(1),
            state: VerificationState::Failed,
            retry_count: 1,
            results: vec![VerificationResult::failed(
                VerificationCheckType::UnitTest,
                vec![],
                1000,
            )
            .with_counts(18, 20)],
            started_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
        };

        let note = SessionNote {
            task_id: TaskId("task-001".to_string()),
            title: "Test".to_string(),
            session_id: SessionId(1),
            duration_minutes: 30,
            completion_status: CompletionStatus::Failed,
            change_summary: None,
            verification: Some(verification),
            summary: None,
            learnings: vec![],
            generated_at: chrono::Utc::now(),
        };

        let markdown = generator.render_markdown(&note);
        assert!(markdown.contains("FAIL Unit Tests"));
        assert!(markdown.contains("see failures below"));
    }

    #[test]
    fn test_render_markdown_with_learnings() {
        let generator = DefaultNotesGenerator::new();
        let note = SessionNote {
            task_id: TaskId("task-001".to_string()),
            title: "Test".to_string(),
            session_id: SessionId(1),
            duration_minutes: 30,
            completion_status: CompletionStatus::Completed,
            change_summary: None,
            verification: None,
            summary: None,
            learnings: vec![
                Learning::new(
                    codirigent_core::session_notes::LearningCategory::Preference,
                    "Use jose instead of jsonwebtoken",
                    true,
                ),
                Learning::new(
                    codirigent_core::session_notes::LearningCategory::Gotcha,
                    "API returns null for empty arrays",
                    false,
                ),
            ],
            generated_at: chrono::Utc::now(),
        };

        let markdown = generator.render_markdown(&note);
        assert!(markdown.contains("## Learnings"));
        assert!(markdown.contains("Suggested for CLAUDE.md"));
        assert!(markdown.contains("Use jose instead of jsonwebtoken"));
        assert!(markdown.contains("Other learnings"));
        assert!(markdown.contains("API returns null for empty arrays"));
    }

    #[test]
    fn test_render_markdown_all_statuses() {
        let generator = DefaultNotesGenerator::new();
        let statuses = [
            CompletionStatus::Completed,
            CompletionStatus::Failed,
            CompletionStatus::Blocked,
            CompletionStatus::Stopped,
        ];

        for status in statuses {
            let note = SessionNote {
                task_id: TaskId("task-001".to_string()),
                title: "Test".to_string(),
                session_id: SessionId(1),
                duration_minutes: 30,
                completion_status: status,
                change_summary: None,
                verification: None,
                summary: None,
                learnings: vec![],
                generated_at: chrono::Utc::now(),
            };

            let markdown = generator.render_markdown(&note);
            assert!(markdown.contains(&format!("**Status:** {}", status)));
        }
    }

    #[test]
    fn test_save_note() {
        let temp = TempDir::new().unwrap();
        let generator = DefaultNotesGenerator::new();
        let note = SessionNote {
            task_id: TaskId("task-001".to_string()),
            title: "Test Task".to_string(),
            session_id: SessionId(1),
            duration_minutes: 30,
            completion_status: CompletionStatus::Completed,
            change_summary: None,
            verification: None,
            summary: None,
            learnings: vec![],
            generated_at: chrono::Utc::now(),
        };

        let path = generator.save(&note, temp.path()).unwrap();
        assert!(path.exists());
        assert!(path.to_string_lossy().contains("task-001"));
        assert!(path.to_string_lossy().contains("test-task"));
        assert!(path.extension().unwrap() == "md");
    }

    #[test]
    fn test_save_note_content() {
        let temp = TempDir::new().unwrap();
        let generator = DefaultNotesGenerator::new();
        let note = SessionNote {
            task_id: TaskId("task-001".to_string()),
            title: "Test Task".to_string(),
            session_id: SessionId(1),
            duration_minutes: 30,
            completion_status: CompletionStatus::Completed,
            change_summary: None,
            verification: None,
            summary: Some("Test summary".to_string()),
            learnings: vec![],
            generated_at: chrono::Utc::now(),
        };

        let path = generator.save(&note, temp.path()).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Session Notes: Test Task"));
        assert!(content.contains("Test summary"));
    }

    #[test]
    fn test_save_note_creates_date_directory() {
        let temp = TempDir::new().unwrap();
        let generator = DefaultNotesGenerator::new();
        let note = SessionNote {
            task_id: TaskId("task-001".to_string()),
            title: "Test".to_string(),
            session_id: SessionId(1),
            duration_minutes: 30,
            completion_status: CompletionStatus::Completed,
            change_summary: None,
            verification: None,
            summary: None,
            learnings: vec![],
            generated_at: chrono::Utc::now(),
        };

        let path = generator.save(&note, temp.path()).unwrap();
        let date_dir = path.parent().unwrap();
        assert!(date_dir.exists());
        // Directory name should be a date like 2026-02-01
        let dir_name = date_dir.file_name().unwrap().to_string_lossy();
        assert!(dir_name.len() == 10); // YYYY-MM-DD
    }

    #[test]
    fn test_safe_filename() {
        let generator = DefaultNotesGenerator::new();

        assert_eq!(generator.safe_filename("Test Task"), "test-task");
        assert_eq!(
            generator.safe_filename("Refactor Auth Module"),
            "refactor-auth-module"
        );
        assert_eq!(generator.safe_filename("Fix bug #123!"), "fix-bug-123");
        assert_eq!(
            generator.safe_filename("Update: API endpoints"),
            "update-api-endpoints"
        );
    }

    #[test]
    fn test_safe_filename_edge_cases() {
        let generator = DefaultNotesGenerator::new();

        assert_eq!(generator.safe_filename(""), "");
        assert_eq!(generator.safe_filename("   "), "---");
        assert_eq!(generator.safe_filename("!!!"), "");
        assert_eq!(generator.safe_filename("a"), "a");
    }

    #[test]
    fn test_format_changes_all_types() {
        let generator = DefaultNotesGenerator::new();
        let summary = ChangeSummary {
            task_id: TaskId("task-001".to_string()),
            session_id: SessionId(1),
            changes: vec![
                FileChange {
                    path: PathBuf::from("new.rs"),
                    change_type: ChangeType::Created,
                    lines_added: 100,
                    lines_removed: 0,
                    risk_level: RiskLevel::Low,
                    categories: vec![],
                },
                FileChange {
                    path: PathBuf::from("modified.rs"),
                    change_type: ChangeType::Modified,
                    lines_added: 10,
                    lines_removed: 5,
                    risk_level: RiskLevel::Medium,
                    categories: vec![],
                },
                FileChange {
                    path: PathBuf::from("deleted.rs"),
                    change_type: ChangeType::Deleted,
                    lines_added: 0,
                    lines_removed: 50,
                    risk_level: RiskLevel::Low,
                    categories: vec![],
                },
                FileChange {
                    path: PathBuf::from("renamed.rs"),
                    change_type: ChangeType::Renamed,
                    lines_added: 0,
                    lines_removed: 0,
                    risk_level: RiskLevel::Low,
                    categories: vec![],
                },
            ],
            risk_assessment: RiskAssessment::default(),
            generated_at: chrono::Utc::now(),
        };

        let output = generator.format_changes(&summary);
        assert!(output.contains("Created"));
        assert!(output.contains("Modified"));
        assert!(output.contains("Deleted"));
        assert!(output.contains("Renamed"));
    }

    #[test]
    fn test_format_verification_all_types() {
        let generator = DefaultNotesGenerator::new();
        let verification = VerificationStatus {
            task_id: TaskId("task-001".to_string()),
            session_id: SessionId(1),
            state: VerificationState::Passed,
            retry_count: 0,
            results: vec![
                VerificationResult::passed(VerificationCheckType::UnitTest, 1000),
                VerificationResult::passed(VerificationCheckType::IntegrationTest, 2000),
                VerificationResult::passed(VerificationCheckType::TypeCheck, 500),
                VerificationResult::passed(VerificationCheckType::Lint, 300),
                VerificationResult::passed(VerificationCheckType::Format, 100),
                VerificationResult::passed(VerificationCheckType::Custom, 1000),
            ],
            started_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
        };

        let output = generator.format_verification(&verification);
        assert!(output.contains("Unit Tests"));
        assert!(output.contains("Integration Tests"));
        assert!(output.contains("Type Check"));
        assert!(output.contains("Lint"));
        assert!(output.contains("Format"));
        assert!(output.contains("Custom"));
    }
}
