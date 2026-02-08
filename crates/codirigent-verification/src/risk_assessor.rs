//! Rule-based risk assessment for file changes.
//!
//! This module provides the default risk assessment implementation that
//! categorizes files and determines risk levels based on file paths and
//! naming patterns.
//!
//! ## Overview
//!
//! The risk assessor uses pattern matching to:
//! - Assign risk levels (Low/Medium/High) to file changes
//! - Categorize files by type (Test, Security, API, etc.)
//! - Generate aggregate risk assessments for changesets
//!
//! ## Example
//!
//! ```
//! use codirigent_verification::RuleBasedRiskAssessor;
//! use codirigent_core::{RiskAssessor, ChangeType, RiskLevel, FileCategory};
//! use std::path::Path;
//!
//! let assessor = RuleBasedRiskAssessor::new();
//!
//! // Auth files are high risk
//! let risk = assessor.assess_file(Path::new("src/auth/login.ts"), ChangeType::Modified);
//! assert_eq!(risk, RiskLevel::High);
//!
//! // Test files are low risk
//! let risk = assessor.assess_file(Path::new("tests/auth.test.ts"), ChangeType::Modified);
//! assert_eq!(risk, RiskLevel::Low);
//!
//! // Categorize files
//! let categories = assessor.categorize_file(Path::new("tests/unit.test.ts"));
//! assert!(categories.contains(&FileCategory::Test));
//! ```

use codirigent_core::{
    ChangeType, FileCategory, FileChange, RiskAssessment, RiskAssessor, RiskLevel,
};
use std::path::Path;

/// Default rule-based risk assessor.
///
/// Uses configurable patterns to assess file risk and categorization.
/// Patterns are matched case-insensitively against file paths.
///
/// # Default Patterns
///
/// ## High Risk (security-sensitive)
/// - `auth`, `security`, `password`, `secret`, `token`
/// - `migration`, `schema`, `.env`, `config`
/// - `credential`, `key`, `cert`, `pem`
/// - All deleted files
///
/// ## Low Risk (safe to review quickly)
/// - `test`, `spec`, `.test.`, `.spec.`
/// - `readme`, `.md`, `doc`, `license`
/// - `changelog`, `.txt`
///
/// ## Medium Risk (default)
/// - Everything else
///
/// # Example
///
/// ```
/// use codirigent_verification::RuleBasedRiskAssessor;
/// use codirigent_core::{RiskAssessor, ChangeType, RiskLevel};
/// use std::path::Path;
///
/// let mut assessor = RuleBasedRiskAssessor::new();
///
/// // Add custom high-risk pattern
/// assessor.add_high_risk_pattern("payment".to_string());
///
/// let risk = assessor.assess_file(Path::new("src/payment/stripe.ts"), ChangeType::Modified);
/// assert_eq!(risk, RiskLevel::High);
/// ```
#[derive(Debug, Clone)]
pub struct RuleBasedRiskAssessor {
    /// Custom high-risk patterns (glob-style matching).
    high_risk_patterns: Vec<String>,
    /// Custom low-risk patterns.
    low_risk_patterns: Vec<String>,
}

impl Default for RuleBasedRiskAssessor {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleBasedRiskAssessor {
    /// Create a new risk assessor with default patterns.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::RuleBasedRiskAssessor;
    ///
    /// let assessor = RuleBasedRiskAssessor::new();
    /// ```
    pub fn new() -> Self {
        Self {
            high_risk_patterns: Vec::new(),
            low_risk_patterns: Vec::new(),
        }
    }

    /// Add a custom high-risk pattern (case-insensitive substring match).
    ///
    /// # Arguments
    ///
    /// * `pattern` - Pattern to match against file paths
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::RuleBasedRiskAssessor;
    ///
    /// let mut assessor = RuleBasedRiskAssessor::new();
    /// assessor.add_high_risk_pattern("billing".to_string());
    /// ```
    pub fn add_high_risk_pattern(&mut self, pattern: String) {
        self.high_risk_patterns.push(pattern);
    }

    /// Add a custom low-risk pattern (case-insensitive substring match).
    ///
    /// # Arguments
    ///
    /// * `pattern` - Pattern to match against file paths
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::RuleBasedRiskAssessor;
    ///
    /// let mut assessor = RuleBasedRiskAssessor::new();
    /// assessor.add_low_risk_pattern("mock".to_string());
    /// ```
    pub fn add_low_risk_pattern(&mut self, pattern: String) {
        self.low_risk_patterns.push(pattern);
    }

    /// Check if a path matches any of the given patterns.
    fn matches_patterns(&self, path: &Path, patterns: &[&str]) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();
        patterns.iter().any(|p| path_str.contains(*p))
    }

    /// Check if a path matches any custom patterns.
    fn matches_custom_patterns(&self, path: &Path, patterns: &[String]) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();
        patterns
            .iter()
            .any(|p| path_str.contains(&p.to_lowercase()))
    }

    /// Get the default high-risk patterns.
    fn default_high_risk_patterns() -> &'static [&'static str] {
        &[
            "auth",
            "security",
            "password",
            "secret",
            "token",
            "migration",
            "schema",
            ".env",
            "config",
            "credential",
            "key",
            "cert",
            "pem",
            "private",
            "admin",
            "permission",
            "role",
            "acl",
        ]
    }

    /// Get the default low-risk patterns.
    fn default_low_risk_patterns() -> &'static [&'static str] {
        &[
            "test",
            "spec",
            ".test.",
            ".spec.",
            "readme",
            ".md",
            "doc",
            "license",
            "changelog",
            ".txt",
            "example",
            "sample",
            "mock",
            "fixture",
            "stub",
            "__tests__",
            "__mocks__",
        ]
    }
}

impl RiskAssessor for RuleBasedRiskAssessor {
    /// Assess risk level for a single file.
    ///
    /// # Risk Assessment Rules
    ///
    /// 1. Deleted files are always high risk
    /// 2. Test/spec files are always low risk (even if they contain high-risk keywords)
    /// 3. Files matching high-risk patterns (default or custom) are high risk
    /// 4. Files matching low-risk patterns (default or custom) are low risk
    /// 5. All other files are medium risk
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file being assessed
    /// * `change_type` - Type of change made to the file
    ///
    /// # Returns
    ///
    /// The assessed risk level for this file change.
    fn assess_file(&self, path: &Path, change_type: ChangeType) -> RiskLevel {
        // Deleted files are always higher risk
        if change_type == ChangeType::Deleted {
            return RiskLevel::High;
        }

        // Check low-risk patterns FIRST - tests/docs are low risk even if they
        // contain high-risk keywords (e.g., "auth.test.ts" should be low risk)
        if self.matches_patterns(path, Self::default_low_risk_patterns()) {
            return RiskLevel::Low;
        }

        // Check custom low-risk patterns
        if self.matches_custom_patterns(path, &self.low_risk_patterns) {
            return RiskLevel::Low;
        }

        // Check default high-risk patterns
        if self.matches_patterns(path, Self::default_high_risk_patterns()) {
            return RiskLevel::High;
        }

        // Check custom high-risk patterns
        if self.matches_custom_patterns(path, &self.high_risk_patterns) {
            return RiskLevel::High;
        }

        // Default to medium
        RiskLevel::Medium
    }

    /// Categorize a file based on its path.
    ///
    /// A file can belong to multiple categories (e.g., a test file for
    /// security code could be both Test and Security).
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file being categorized
    ///
    /// # Returns
    ///
    /// A list of categories the file belongs to.
    fn categorize_file(&self, path: &Path) -> Vec<FileCategory> {
        let path_str = path.to_string_lossy().to_lowercase();
        let mut categories = Vec::new();

        // Test files
        if path_str.contains("test")
            || path_str.contains("spec")
            || path_str.contains("__tests__")
            || path_str.contains("__mocks__")
        {
            categories.push(FileCategory::Test);
        }

        // Documentation
        if path_str.ends_with(".md")
            || path_str.contains("/doc/")
            || path_str.contains("/docs/")
            || path_str.contains("readme")
        {
            categories.push(FileCategory::Documentation);
        }

        // Config files
        if path_str.contains("config")
            || path_str.ends_with(".json")
            || path_str.ends_with(".yaml")
            || path_str.ends_with(".yml")
            || path_str.ends_with(".toml")
            || path_str.ends_with(".ini")
            || path_str.contains(".env")
        {
            categories.push(FileCategory::Config);
        }

        // Security
        if path_str.contains("auth")
            || path_str.contains("security")
            || path_str.contains("password")
            || path_str.contains("permission")
            || path_str.contains("role")
            || path_str.contains("acl")
        {
            categories.push(FileCategory::Security);
        }

        // Database
        if path_str.contains("migration")
            || path_str.contains("schema")
            || path_str.contains("database")
            || path_str.contains("/db/")
            || path_str.ends_with(".sql")
        {
            categories.push(FileCategory::Database);
        }

        // API
        if path_str.contains("api")
            || path_str.contains("route")
            || path_str.contains("endpoint")
            || path_str.contains("controller")
            || path_str.contains("handler")
        {
            categories.push(FileCategory::Api);
        }

        // UI
        if path_str.contains("component")
            || path_str.ends_with(".tsx")
            || path_str.ends_with(".jsx")
            || path_str.ends_with(".vue")
            || path_str.ends_with(".svelte")
            || path_str.ends_with(".css")
            || path_str.ends_with(".scss")
            || path_str.ends_with(".less")
            || path_str.contains("/ui/")
            || path_str.contains("/view/")
            || path_str.contains("/views/")
        {
            categories.push(FileCategory::Ui);
        }

        // Build
        if path_str.contains("build")
            || path_str.contains("webpack")
            || path_str.contains("vite")
            || path_str.contains("rollup")
            || path_str.contains("esbuild")
            || path_str.ends_with("package.json")
            || path_str.ends_with("cargo.toml")
            || path_str.ends_with("makefile")
            || path_str.ends_with("dockerfile")
            || path_str.ends_with(".github")
            || path_str.contains("ci/")
            || path_str.contains(".ci")
        {
            categories.push(FileCategory::Build);
        }

        // Core business logic (source files not in other categories)
        if categories.is_empty() {
            let is_source = path_str.ends_with(".rs")
                || path_str.ends_with(".ts")
                || path_str.ends_with(".js")
                || path_str.ends_with(".py")
                || path_str.ends_with(".go")
                || path_str.ends_with(".java")
                || path_str.ends_with(".kt")
                || path_str.ends_with(".swift")
                || path_str.ends_with(".c")
                || path_str.ends_with(".cpp")
                || path_str.ends_with(".h");

            if is_source {
                categories.push(FileCategory::Core);
            }
        }

        // Default to Other if no category matched
        if categories.is_empty() {
            categories.push(FileCategory::Other);
        }

        categories
    }

    /// Generate overall risk assessment for a changeset.
    ///
    /// Aggregates statistics and generates warnings for high-risk changes.
    ///
    /// # Arguments
    ///
    /// * `changes` - The list of file changes to assess
    ///
    /// # Returns
    ///
    /// An aggregate risk assessment for all changes.
    fn assess_changeset(&self, changes: &[FileChange]) -> RiskAssessment {
        let mut high_count = 0u32;
        let mut medium_count = 0u32;
        let mut low_count = 0u32;
        let mut total_added = 0u32;
        let mut total_removed = 0u32;
        let mut warnings = Vec::new();

        for change in changes {
            total_added += change.lines_added;
            total_removed += change.lines_removed;

            match change.risk_level {
                RiskLevel::High => {
                    high_count += 1;
                    // Generate warning for high-risk files
                    let category_str: String = change
                        .categories
                        .iter()
                        .map(|c| format!("{}", c))
                        .collect::<Vec<_>>()
                        .join(", ");
                    warnings.push(format!(
                        "{}: {} ({})",
                        change.change_type,
                        change.path.display(),
                        category_str
                    ));
                }
                RiskLevel::Medium => medium_count += 1,
                RiskLevel::Low => low_count += 1,
            }
        }

        let overall_risk = if high_count > 0 {
            RiskLevel::High
        } else if medium_count > 0 {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        };

        RiskAssessment {
            overall_risk,
            high_risk_count: high_count,
            medium_risk_count: medium_count,
            low_risk_count: low_count,
            total_files: changes.len() as u32,
            total_lines_added: total_added,
            total_lines_removed: total_removed,
            warnings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // Constructor tests

    #[test]
    fn test_new_assessor() {
        let assessor = RuleBasedRiskAssessor::new();
        assert!(assessor.high_risk_patterns.is_empty());
        assert!(assessor.low_risk_patterns.is_empty());
    }

    #[test]
    fn test_default_assessor() {
        let assessor = RuleBasedRiskAssessor::default();
        assert!(assessor.high_risk_patterns.is_empty());
        assert!(assessor.low_risk_patterns.is_empty());
    }

    #[test]
    fn test_clone_assessor() {
        let mut assessor = RuleBasedRiskAssessor::new();
        assessor.add_high_risk_pattern("billing".to_string());
        let cloned = assessor.clone();
        assert_eq!(cloned.high_risk_patterns.len(), 1);
    }

    // Custom pattern tests

    #[test]
    fn test_add_high_risk_pattern() {
        let mut assessor = RuleBasedRiskAssessor::new();
        assessor.add_high_risk_pattern("billing".to_string());
        let risk = assessor.assess_file(Path::new("src/billing/stripe.ts"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::High);
    }

    #[test]
    fn test_add_low_risk_pattern() {
        let mut assessor = RuleBasedRiskAssessor::new();
        assessor.add_low_risk_pattern("generated".to_string());
        let risk = assessor.assess_file(Path::new("src/generated/types.ts"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::Low);
    }

    #[test]
    fn test_custom_pattern_case_insensitive() {
        let mut assessor = RuleBasedRiskAssessor::new();
        assessor.add_high_risk_pattern("PayMent".to_string());
        let risk = assessor.assess_file(Path::new("src/payment/stripe.ts"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::High);
    }

    // Risk level tests - High risk

    #[test]
    fn test_assess_auth_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(Path::new("src/auth/middleware.ts"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::High);
    }

    #[test]
    fn test_assess_security_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(Path::new("lib/security/crypto.rs"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::High);
    }

    #[test]
    fn test_assess_password_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(Path::new("src/utils/password.ts"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::High);
    }

    #[test]
    fn test_assess_migration_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(
            Path::new("db/migrations/20240101_add_users.sql"),
            ChangeType::Created,
        );
        assert_eq!(risk, RiskLevel::High);
    }

    #[test]
    fn test_assess_env_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(Path::new(".env.production"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::High);
    }

    #[test]
    fn test_assess_config_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(Path::new("src/config/database.ts"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::High);
    }

    #[test]
    fn test_assess_deleted_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(Path::new("src/utils.ts"), ChangeType::Deleted);
        assert_eq!(risk, RiskLevel::High);
    }

    #[test]
    fn test_assess_key_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(Path::new("certs/server.key"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::High);
    }

    #[test]
    fn test_assess_cert_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(Path::new("certs/server.cert"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::High);
    }

    #[test]
    fn test_assess_pem_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(Path::new("ssl/private.pem"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::High);
    }

    #[test]
    fn test_assess_permission_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk =
            assessor.assess_file(Path::new("src/permissions/roles.ts"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::High);
    }

    // Risk level tests - Low risk

    #[test]
    fn test_assess_test_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(Path::new("src/auth.test.ts"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::Low);
    }

    #[test]
    fn test_assess_spec_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(Path::new("src/auth.spec.ts"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::Low);
    }

    #[test]
    fn test_assess_readme_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(Path::new("README.md"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::Low);
    }

    #[test]
    fn test_assess_markdown_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(Path::new("docs/api.md"), ChangeType::Created);
        assert_eq!(risk, RiskLevel::Low);
    }

    #[test]
    fn test_assess_license_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(Path::new("LICENSE"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::Low);
    }

    #[test]
    fn test_assess_changelog_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(Path::new("CHANGELOG.md"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::Low);
    }

    #[test]
    fn test_assess_fixture_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(Path::new("tests/fixtures/user.json"), ChangeType::Created);
        assert_eq!(risk, RiskLevel::Low);
    }

    #[test]
    fn test_assess_mock_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(Path::new("__mocks__/api.ts"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::Low);
    }

    // Risk level tests - Medium risk

    #[test]
    fn test_assess_regular_source_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(Path::new("src/utils.ts"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::Medium);
    }

    #[test]
    fn test_assess_model_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let risk = assessor.assess_file(Path::new("src/models/user.rs"), ChangeType::Modified);
        assert_eq!(risk, RiskLevel::Medium);
    }

    // File categorization tests

    #[test]
    fn test_categorize_test_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("src/auth.test.ts"));
        assert!(categories.contains(&FileCategory::Test));
    }

    #[test]
    fn test_categorize_tests_directory() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("__tests__/unit/auth.test.ts"));
        assert!(categories.contains(&FileCategory::Test));
    }

    #[test]
    fn test_categorize_documentation() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("docs/api.md"));
        assert!(categories.contains(&FileCategory::Documentation));
    }

    #[test]
    fn test_categorize_readme() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("README.md"));
        assert!(categories.contains(&FileCategory::Documentation));
    }

    #[test]
    fn test_categorize_config_json() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("tsconfig.json"));
        assert!(categories.contains(&FileCategory::Config));
    }

    #[test]
    fn test_categorize_config_yaml() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("docker-compose.yaml"));
        assert!(categories.contains(&FileCategory::Config));
    }

    #[test]
    fn test_categorize_config_toml() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("Cargo.toml"));
        assert!(categories.contains(&FileCategory::Config));
    }

    #[test]
    fn test_categorize_security() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("src/auth/login.ts"));
        assert!(categories.contains(&FileCategory::Security));
    }

    #[test]
    fn test_categorize_migration() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("db/migrations/002.sql"));
        assert!(categories.contains(&FileCategory::Database));
    }

    #[test]
    fn test_categorize_sql_file() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("queries/users.sql"));
        assert!(categories.contains(&FileCategory::Database));
    }

    #[test]
    fn test_categorize_api() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("src/api/users.ts"));
        assert!(categories.contains(&FileCategory::Api));
    }

    #[test]
    fn test_categorize_route() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("src/routes/index.ts"));
        assert!(categories.contains(&FileCategory::Api));
    }

    #[test]
    fn test_categorize_controller() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("src/controllers/user.ts"));
        assert!(categories.contains(&FileCategory::Api));
    }

    #[test]
    fn test_categorize_ui_tsx() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("src/components/Button.tsx"));
        assert!(categories.contains(&FileCategory::Ui));
    }

    #[test]
    fn test_categorize_ui_vue() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("src/components/Modal.vue"));
        assert!(categories.contains(&FileCategory::Ui));
    }

    #[test]
    fn test_categorize_ui_css() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("styles/main.css"));
        assert!(categories.contains(&FileCategory::Ui));
    }

    #[test]
    fn test_categorize_build_dockerfile() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("Dockerfile"));
        assert!(categories.contains(&FileCategory::Build));
    }

    #[test]
    fn test_categorize_build_makefile() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("Makefile"));
        assert!(categories.contains(&FileCategory::Build));
    }

    #[test]
    fn test_categorize_build_package_json() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("package.json"));
        assert!(categories.contains(&FileCategory::Build));
    }

    #[test]
    fn test_categorize_core_rust() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("src/lib.rs"));
        assert!(categories.contains(&FileCategory::Core));
    }

    #[test]
    fn test_categorize_core_typescript() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("src/utils.ts"));
        assert!(categories.contains(&FileCategory::Core));
    }

    #[test]
    fn test_categorize_other() {
        let assessor = RuleBasedRiskAssessor::new();
        let categories = assessor.categorize_file(Path::new("data/sample.csv"));
        assert!(categories.contains(&FileCategory::Other));
    }

    #[test]
    fn test_categorize_multiple_categories() {
        let assessor = RuleBasedRiskAssessor::new();
        // A test file for authentication would have both Test and Security categories
        let categories = assessor.categorize_file(Path::new("src/auth/login.test.ts"));
        assert!(categories.contains(&FileCategory::Test));
        assert!(categories.contains(&FileCategory::Security));
    }

    // Changeset assessment tests

    #[test]
    fn test_assess_empty_changeset() {
        let assessor = RuleBasedRiskAssessor::new();
        let assessment = assessor.assess_changeset(&[]);
        assert_eq!(assessment.overall_risk, RiskLevel::Low);
        assert_eq!(assessment.total_files, 0);
        assert_eq!(assessment.total_lines_added, 0);
        assert_eq!(assessment.total_lines_removed, 0);
        assert!(assessment.warnings.is_empty());
    }

    #[test]
    fn test_assess_changeset_single_low_risk() {
        let assessor = RuleBasedRiskAssessor::new();
        let changes = vec![FileChange {
            path: PathBuf::from("README.md"),
            change_type: ChangeType::Modified,
            lines_added: 10,
            lines_removed: 5,
            risk_level: RiskLevel::Low,
            categories: vec![FileCategory::Documentation],
        }];

        let assessment = assessor.assess_changeset(&changes);
        assert_eq!(assessment.overall_risk, RiskLevel::Low);
        assert_eq!(assessment.low_risk_count, 1);
        assert_eq!(assessment.total_files, 1);
        assert_eq!(assessment.total_lines_added, 10);
        assert_eq!(assessment.total_lines_removed, 5);
        assert!(assessment.warnings.is_empty());
    }

    #[test]
    fn test_assess_changeset_mixed() {
        let assessor = RuleBasedRiskAssessor::new();
        let changes = vec![
            FileChange {
                path: PathBuf::from("src/auth/middleware.ts"),
                change_type: ChangeType::Modified,
                lines_added: 45,
                lines_removed: 30,
                risk_level: RiskLevel::High,
                categories: vec![FileCategory::Security],
            },
            FileChange {
                path: PathBuf::from("src/auth.test.ts"),
                change_type: ChangeType::Modified,
                lines_added: 20,
                lines_removed: 0,
                risk_level: RiskLevel::Low,
                categories: vec![FileCategory::Test],
            },
            FileChange {
                path: PathBuf::from("src/utils.ts"),
                change_type: ChangeType::Modified,
                lines_added: 15,
                lines_removed: 10,
                risk_level: RiskLevel::Medium,
                categories: vec![FileCategory::Core],
            },
        ];

        let assessment = assessor.assess_changeset(&changes);
        assert_eq!(assessment.overall_risk, RiskLevel::High);
        assert_eq!(assessment.high_risk_count, 1);
        assert_eq!(assessment.medium_risk_count, 1);
        assert_eq!(assessment.low_risk_count, 1);
        assert_eq!(assessment.total_files, 3);
        assert_eq!(assessment.total_lines_added, 80);
        assert_eq!(assessment.total_lines_removed, 40);
        assert_eq!(assessment.warnings.len(), 1);
    }

    #[test]
    fn test_assess_changeset_all_medium() {
        let assessor = RuleBasedRiskAssessor::new();
        let changes = vec![
            FileChange {
                path: PathBuf::from("src/utils.ts"),
                change_type: ChangeType::Modified,
                lines_added: 10,
                lines_removed: 5,
                risk_level: RiskLevel::Medium,
                categories: vec![FileCategory::Core],
            },
            FileChange {
                path: PathBuf::from("src/helpers.ts"),
                change_type: ChangeType::Modified,
                lines_added: 20,
                lines_removed: 10,
                risk_level: RiskLevel::Medium,
                categories: vec![FileCategory::Core],
            },
        ];

        let assessment = assessor.assess_changeset(&changes);
        assert_eq!(assessment.overall_risk, RiskLevel::Medium);
        assert_eq!(assessment.medium_risk_count, 2);
        assert!(assessment.warnings.is_empty());
    }

    #[test]
    fn test_assess_changeset_warnings_format() {
        let assessor = RuleBasedRiskAssessor::new();
        let changes = vec![FileChange {
            path: PathBuf::from("src/auth/login.ts"),
            change_type: ChangeType::Modified,
            lines_added: 10,
            lines_removed: 5,
            risk_level: RiskLevel::High,
            categories: vec![FileCategory::Security, FileCategory::Api],
        }];

        let assessment = assessor.assess_changeset(&changes);
        assert_eq!(assessment.warnings.len(), 1);
        assert!(assessment.warnings[0].contains("Modified"));
        assert!(assessment.warnings[0].contains("auth/login.ts"));
        assert!(assessment.warnings[0].contains("Security"));
    }

    #[test]
    fn test_assess_changeset_multiple_high_risk() {
        let assessor = RuleBasedRiskAssessor::new();
        let changes = vec![
            FileChange {
                path: PathBuf::from("src/auth/login.ts"),
                change_type: ChangeType::Modified,
                lines_added: 10,
                lines_removed: 5,
                risk_level: RiskLevel::High,
                categories: vec![FileCategory::Security],
            },
            FileChange {
                path: PathBuf::from("db/migrations/001.sql"),
                change_type: ChangeType::Created,
                lines_added: 50,
                lines_removed: 0,
                risk_level: RiskLevel::High,
                categories: vec![FileCategory::Database],
            },
        ];

        let assessment = assessor.assess_changeset(&changes);
        assert_eq!(assessment.overall_risk, RiskLevel::High);
        assert_eq!(assessment.high_risk_count, 2);
        assert_eq!(assessment.warnings.len(), 2);
    }

    // Debug trait test
    #[test]
    fn test_debug_trait() {
        let assessor = RuleBasedRiskAssessor::new();
        let debug_str = format!("{:?}", assessor);
        assert!(debug_str.contains("RuleBasedRiskAssessor"));
    }
}
