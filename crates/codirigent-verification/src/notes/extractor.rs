//! Learnings extraction from session output.
//!
//! Provides the [`PatternLearningsExtractor`] implementation that uses
//! regular expressions to identify learnings in session output.

use codirigent_core::session_notes::{Learning, LearningCategory, LearningsExtractor};
use regex::Regex;
use tracing::debug;

/// Pattern-based learnings extractor.
///
/// Analyzes session output text to identify patterns that indicate
/// learnings (preferences, gotchas, conventions, etc.) and extracts
/// them for potential inclusion in CLAUDE.md.
///
/// # Recognized Patterns
///
/// - Preferences: "prefer X over Y", "use X instead of Y", "recommend X over Y"
/// - Gotchas: "gotcha:", "note:", "warning:", "careful:", "beware:", "watch out:"
/// - Conventions: "convention:", "always:", "never:", "rule:"
///
/// # Example
///
/// ```
/// use codirigent_verification::notes::PatternLearningsExtractor;
/// use codirigent_core::{LearningsExtractor, LearningCategory};
///
/// let extractor = PatternLearningsExtractor::new();
///
/// let output = "I recommend using jose instead of jsonwebtoken for ESM compatibility";
/// let learnings = extractor.extract(output);
///
/// assert!(!learnings.is_empty());
/// assert_eq!(learnings[0].category, LearningCategory::Preference);
/// ```
#[derive(Debug)]
pub struct PatternLearningsExtractor {
    preference_pattern: Regex,
    gotcha_pattern: Regex,
    convention_pattern: Regex,
}

impl Default for PatternLearningsExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl PatternLearningsExtractor {
    /// Create a new learnings extractor.
    ///
    /// Initializes the regular expression patterns for each learning category.
    pub fn new() -> Self {
        Self {
            // Matches: "prefer X over Y" or "use X instead of Y" or "recommend using X instead of Y"
            preference_pattern: Regex::new(
                r"(?i)(?:prefer|use|recommend(?:\s+using)?)\s+(\w+)\s+(?:over|instead of|rather than)\s+(\w+)"
            ).expect("Invalid preference pattern regex"),
            // Matches: "gotcha:" or "note:" or "warning:" followed by content
            gotcha_pattern: Regex::new(
                r"(?i)(?:gotcha|note|warning|careful|beware|watch out):\s*(.+)"
            ).expect("Invalid gotcha pattern regex"),
            // Matches: "convention:" or "always:" or "never:" followed by content
            convention_pattern: Regex::new(
                r"(?i)(?:convention|always|never|rule):\s*(.+)"
            ).expect("Invalid convention pattern regex"),
        }
    }

    /// Extract preferences (X over Y patterns).
    ///
    /// Looks for patterns like "prefer jose over jsonwebtoken" and converts
    /// them to structured learnings.
    ///
    /// # Arguments
    ///
    /// * `output` - The text to search
    ///
    /// # Returns
    ///
    /// A vector of preference learnings.
    fn extract_preferences(&self, output: &str) -> Vec<Learning> {
        let mut learnings = Vec::new();

        for caps in self.preference_pattern.captures_iter(output) {
            if let (Some(preferred), Some(avoided)) = (caps.get(1), caps.get(2)) {
                learnings.push(Learning {
                    category: LearningCategory::Preference,
                    content: format!("Use {} instead of {}", preferred.as_str(), avoided.as_str()),
                    suggested_for_claude_md: true,
                });
            }
        }

        learnings
    }

    /// Extract gotchas and warnings.
    ///
    /// Looks for patterns like "Warning: API returns null for empty arrays"
    /// and converts them to structured learnings.
    ///
    /// # Arguments
    ///
    /// * `output` - The text to search
    ///
    /// # Returns
    ///
    /// A vector of gotcha learnings.
    fn extract_gotchas(&self, output: &str) -> Vec<Learning> {
        let mut learnings = Vec::new();

        for caps in self.gotcha_pattern.captures_iter(output) {
            if let Some(content) = caps.get(1) {
                learnings.push(Learning {
                    category: LearningCategory::Gotcha,
                    content: content.as_str().trim().to_string(),
                    suggested_for_claude_md: true,
                });
            }
        }

        learnings
    }

    /// Extract conventions.
    ///
    /// Looks for patterns like "Convention: All API routes should start with /api/v1"
    /// and converts them to structured learnings.
    ///
    /// # Arguments
    ///
    /// * `output` - The text to search
    ///
    /// # Returns
    ///
    /// A vector of convention learnings.
    fn extract_conventions(&self, output: &str) -> Vec<Learning> {
        let mut learnings = Vec::new();

        for caps in self.convention_pattern.captures_iter(output) {
            if let Some(content) = caps.get(1) {
                learnings.push(Learning {
                    category: LearningCategory::Convention,
                    content: content.as_str().trim().to_string(),
                    suggested_for_claude_md: true,
                });
            }
        }

        learnings
    }
}

impl LearningsExtractor for PatternLearningsExtractor {
    fn extract(&self, output: &str) -> Vec<Learning> {
        debug!("Extracting learnings from output ({} chars)", output.len());

        let mut learnings = Vec::new();
        learnings.extend(self.extract_preferences(output));
        learnings.extend(self.extract_gotchas(output));
        learnings.extend(self.extract_conventions(output));

        debug!(count = learnings.len(), "Extracted learnings");
        learnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extractor_new() {
        let extractor = PatternLearningsExtractor::new();
        assert!(format!("{:?}", extractor).contains("PatternLearningsExtractor"));
    }

    #[test]
    fn test_extractor_default() {
        let extractor = PatternLearningsExtractor::default();
        assert!(format!("{:?}", extractor).contains("PatternLearningsExtractor"));
    }

    #[test]
    fn test_extract_preferences_prefer() {
        let extractor = PatternLearningsExtractor::new();
        let output = "I prefer jose over jsonwebtoken for ESM compatibility";

        let learnings = extractor.extract(output);
        assert!(!learnings.is_empty());
        assert_eq!(learnings[0].category, LearningCategory::Preference);
        assert!(learnings[0].content.contains("jose"));
        assert!(learnings[0].content.contains("jsonwebtoken"));
    }

    #[test]
    fn test_extract_preferences_use() {
        let extractor = PatternLearningsExtractor::new();
        let output = "You should use async over callbacks";

        let learnings = extractor.extract(output);
        assert!(!learnings.is_empty());
        assert_eq!(learnings[0].category, LearningCategory::Preference);
        assert!(learnings[0].content.contains("async"));
        assert!(learnings[0].content.contains("callbacks"));
    }

    #[test]
    fn test_extract_preferences_recommend() {
        let extractor = PatternLearningsExtractor::new();
        let output = "I recommend using jose instead of jsonwebtoken for ESM compatibility";

        let learnings = extractor.extract(output);
        assert!(!learnings.is_empty());
        assert_eq!(learnings[0].category, LearningCategory::Preference);
        assert!(learnings[0].content.contains("jose"));
    }

    #[test]
    fn test_extract_preferences_rather_than() {
        let extractor = PatternLearningsExtractor::new();
        let output = "Prefer axios rather than fetch for better error handling";

        let learnings = extractor.extract(output);
        assert!(!learnings.is_empty());
        assert_eq!(learnings[0].category, LearningCategory::Preference);
        assert!(learnings[0].content.contains("axios"));
        assert!(learnings[0].content.contains("fetch"));
    }

    #[test]
    fn test_extract_preferences_case_insensitive() {
        let extractor = PatternLearningsExtractor::new();
        let output = "PREFER typescript OVER javascript";

        let learnings = extractor.extract(output);
        assert!(!learnings.is_empty());
        assert_eq!(learnings[0].category, LearningCategory::Preference);
    }

    #[test]
    fn test_extract_gotchas_warning() {
        let extractor = PatternLearningsExtractor::new();
        let output = "Warning: The API returns null for empty arrays";

        let learnings = extractor.extract(output);
        assert!(!learnings.is_empty());
        assert_eq!(learnings[0].category, LearningCategory::Gotcha);
        assert!(learnings[0].content.contains("API returns null"));
    }

    #[test]
    fn test_extract_gotchas_gotcha() {
        let extractor = PatternLearningsExtractor::new();
        let output = "Gotcha: The timeout is in seconds, not milliseconds";

        let learnings = extractor.extract(output);
        assert!(!learnings.is_empty());
        assert_eq!(learnings[0].category, LearningCategory::Gotcha);
        assert!(learnings[0].content.contains("timeout"));
    }

    #[test]
    fn test_extract_gotchas_note() {
        let extractor = PatternLearningsExtractor::new();
        let output = "Note: This function is deprecated in v2.0";

        let learnings = extractor.extract(output);
        assert!(!learnings.is_empty());
        assert_eq!(learnings[0].category, LearningCategory::Gotcha);
        assert!(learnings[0].content.contains("deprecated"));
    }

    #[test]
    fn test_extract_gotchas_careful() {
        let extractor = PatternLearningsExtractor::new();
        let output = "Careful: This operation is not atomic";

        let learnings = extractor.extract(output);
        assert!(!learnings.is_empty());
        assert_eq!(learnings[0].category, LearningCategory::Gotcha);
    }

    #[test]
    fn test_extract_gotchas_beware() {
        let extractor = PatternLearningsExtractor::new();
        let output = "Beware: Memory leaks if you don't clean up";

        let learnings = extractor.extract(output);
        assert!(!learnings.is_empty());
        assert_eq!(learnings[0].category, LearningCategory::Gotcha);
    }

    #[test]
    fn test_extract_gotchas_watch_out() {
        let extractor = PatternLearningsExtractor::new();
        let output = "Watch out: Race conditions possible here";

        let learnings = extractor.extract(output);
        assert!(!learnings.is_empty());
        assert_eq!(learnings[0].category, LearningCategory::Gotcha);
    }

    #[test]
    fn test_extract_conventions_convention() {
        let extractor = PatternLearningsExtractor::new();
        let output = "Convention: All API routes should start with /api/v1";

        let learnings = extractor.extract(output);
        assert!(!learnings.is_empty());
        assert_eq!(learnings[0].category, LearningCategory::Convention);
        assert!(learnings[0].content.contains("API routes"));
    }

    #[test]
    fn test_extract_conventions_always() {
        let extractor = PatternLearningsExtractor::new();
        let output = "Always: Use snake_case for database column names";

        let learnings = extractor.extract(output);
        assert!(!learnings.is_empty());
        assert_eq!(learnings[0].category, LearningCategory::Convention);
        assert!(learnings[0].content.contains("snake_case"));
    }

    #[test]
    fn test_extract_conventions_never() {
        let extractor = PatternLearningsExtractor::new();
        let output = "Never: Commit .env files to version control";

        let learnings = extractor.extract(output);
        assert!(!learnings.is_empty());
        assert_eq!(learnings[0].category, LearningCategory::Convention);
        assert!(learnings[0].content.contains(".env"));
    }

    #[test]
    fn test_extract_conventions_rule() {
        let extractor = PatternLearningsExtractor::new();
        let output = "Rule: All tests must have descriptive names";

        let learnings = extractor.extract(output);
        assert!(!learnings.is_empty());
        assert_eq!(learnings[0].category, LearningCategory::Convention);
    }

    #[test]
    fn test_extract_empty() {
        let extractor = PatternLearningsExtractor::new();
        let output = "This is just normal output without any learnings";

        let learnings = extractor.extract(output);
        assert!(learnings.is_empty());
    }

    #[test]
    fn test_extract_multiple() {
        let extractor = PatternLearningsExtractor::new();
        let output = r#"
            I recommend using jose instead of jsonwebtoken.
            Warning: The API returns null for empty arrays.
            Convention: All tests must pass before merging.
        "#;

        let learnings = extractor.extract(output);
        assert_eq!(learnings.len(), 3);

        let categories: Vec<_> = learnings.iter().map(|l| l.category).collect();
        assert!(categories.contains(&LearningCategory::Preference));
        assert!(categories.contains(&LearningCategory::Gotcha));
        assert!(categories.contains(&LearningCategory::Convention));
    }

    #[test]
    fn test_suggested_for_claude_md() {
        let extractor = PatternLearningsExtractor::new();
        let output = "Prefer async over callbacks for readability";

        let learnings = extractor.extract(output);
        assert!(!learnings.is_empty());
        assert!(learnings[0].suggested_for_claude_md);
    }

    #[test]
    fn test_extract_whitespace_handling() {
        let extractor = PatternLearningsExtractor::new();
        let output = "Warning:   Extra whitespace should be trimmed   ";

        let learnings = extractor.extract(output);
        assert!(!learnings.is_empty());
        assert!(!learnings[0].content.ends_with(' '));
        assert!(!learnings[0].content.starts_with(' '));
    }

    #[test]
    fn test_extract_multiline() {
        let extractor = PatternLearningsExtractor::new();
        let output = "Line 1\nWarning: Something important\nLine 3";

        let learnings = extractor.extract(output);
        assert!(!learnings.is_empty());
        assert_eq!(learnings[0].category, LearningCategory::Gotcha);
    }

    #[test]
    fn test_extract_same_pattern_multiple_times() {
        let extractor = PatternLearningsExtractor::new();
        let output = r#"
            Warning: First warning
            Warning: Second warning
            Warning: Third warning
        "#;

        let learnings = extractor.extract(output);
        assert_eq!(learnings.len(), 3);
        assert!(learnings
            .iter()
            .all(|l| l.category == LearningCategory::Gotcha));
    }

    #[test]
    fn test_extract_case_insensitive() {
        let extractor = PatternLearningsExtractor::new();

        let output1 = "WARNING: uppercase";
        let output2 = "warning: lowercase";
        let output3 = "Warning: mixed case";

        assert!(!extractor.extract(output1).is_empty());
        assert!(!extractor.extract(output2).is_empty());
        assert!(!extractor.extract(output3).is_empty());
    }

    #[test]
    fn test_learning_content_preserved() {
        let extractor = PatternLearningsExtractor::new();
        let output = "Convention: Use PascalCase for component names and camelCase for variables";

        let learnings = extractor.extract(output);
        assert!(!learnings.is_empty());
        assert!(learnings[0].content.contains("PascalCase"));
        assert!(learnings[0].content.contains("camelCase"));
    }

    #[test]
    fn test_extract_large_text() {
        let extractor = PatternLearningsExtractor::new();
        let mut output = String::new();
        for i in 0..1000 {
            output.push_str(&format!("Line {} of output\n", i));
        }
        output.push_str("Warning: Important gotcha here\n");
        for i in 1000..2000 {
            output.push_str(&format!("Line {} of output\n", i));
        }

        let learnings = extractor.extract(&output);
        assert_eq!(learnings.len(), 1);
        assert!(learnings[0].content.contains("Important gotcha"));
    }
}
