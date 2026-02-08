//! Project type and verification command detection.
//!
//! This module provides auto-detection of project types based on configuration files
//! and generates appropriate verification commands for each project type.
//!
//! ## Supported Project Types
//!
//! - **Node.js**: Detected by `package.json`
//! - **Rust**: Detected by `Cargo.toml`
//! - **Python**: Detected by `pyproject.toml` or `setup.py`
//! - **Go**: Detected by `go.mod`
//! - **JVM**: Detected by `pom.xml`, `build.gradle`, or `build.gradle.kts`
//! - **Make**: Detected by `Makefile`
//!
//! ## Example
//!
//! ```
//! use codirigent_verification::DefaultDetector;
//! use codirigent_core::VerificationDetector;
//! use std::path::Path;
//!
//! let detector = DefaultDetector::new();
//! // Detection returns None for non-project directories
//! let project_type = detector.detect_project_type(Path::new("/tmp"));
//! ```

use codirigent_core::verification::VerificationCommands;
use codirigent_core::{ProjectType, VerificationDetector};
use std::path::Path;
use tracing::debug;

/// Default implementation of project type and command detection.
///
/// Analyzes a project directory to determine the project type and
/// generates appropriate verification commands for running tests,
/// linting, type checking, and other checks.
#[derive(Debug, Default, Clone)]
pub struct DefaultDetector;

impl DefaultDetector {
    /// Create a new detector.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_verification::DefaultDetector;
    ///
    /// let detector = DefaultDetector::new();
    /// ```
    pub fn new() -> Self {
        Self
    }

    /// Generate verification commands for Node.js projects.
    fn nodejs_commands(&self, project_dir: &Path) -> VerificationCommands {
        let mut commands = VerificationCommands {
            unit: Some("npm test".to_string()),
            ..Default::default()
        };

        // Try to detect from package.json scripts
        if let Ok(content) = std::fs::read_to_string(project_dir.join("package.json")) {
            if content.contains("\"lint\"") {
                commands.lint = Some("npm run lint".to_string());
            }
            if content.contains("\"typecheck\"") || content.contains("\"type-check\"") {
                commands.type_check = Some("npm run typecheck".to_string());
            } else if project_dir.join("tsconfig.json").exists() {
                commands.type_check = Some("npx tsc --noEmit".to_string());
            }
        }

        commands
    }

    /// Generate verification commands for Rust projects.
    fn rust_commands(&self) -> VerificationCommands {
        VerificationCommands {
            unit: Some("cargo test".to_string()),
            lint: Some("cargo clippy -- -D warnings".to_string()),
            format: Some("cargo fmt -- --check".to_string()),
            ..Default::default()
        }
    }

    /// Generate verification commands for Python projects.
    fn python_commands(&self, project_dir: &Path) -> VerificationCommands {
        let unit = if project_dir.join("pytest.ini").exists()
            || project_dir.join("pyproject.toml").exists()
        {
            Some("pytest".to_string())
        } else {
            Some("python -m unittest discover".to_string())
        };

        VerificationCommands {
            unit,
            lint: Some("ruff check .".to_string()),
            type_check: Some("mypy .".to_string()),
            ..Default::default()
        }
    }

    /// Generate verification commands for Go projects.
    fn go_commands(&self) -> VerificationCommands {
        VerificationCommands {
            unit: Some("go test ./...".to_string()),
            lint: Some("golangci-lint run".to_string()),
            format: Some("gofmt -l .".to_string()),
            ..Default::default()
        }
    }

    /// Generate verification commands for JVM projects (Java/Kotlin).
    fn jvm_commands(&self, project_dir: &Path) -> VerificationCommands {
        let unit = if project_dir.join("gradlew").exists() {
            Some("./gradlew test".to_string())
        } else if project_dir.join("mvnw").exists() {
            Some("./mvnw test".to_string())
        } else if project_dir.join("build.gradle").exists()
            || project_dir.join("build.gradle.kts").exists()
        {
            Some("gradle test".to_string())
        } else {
            Some("mvn test".to_string())
        };

        VerificationCommands {
            unit,
            ..Default::default()
        }
    }

    /// Generate verification commands for projects with Makefile.
    fn make_commands(&self, project_dir: &Path) -> VerificationCommands {
        let mut commands = VerificationCommands::default();

        if let Ok(content) = std::fs::read_to_string(project_dir.join("Makefile")) {
            if content.contains("test:") {
                commands.unit = Some("make test".to_string());
            }
            if content.contains("lint:") {
                commands.lint = Some("make lint".to_string());
            }
        }

        commands
    }
}

impl VerificationDetector for DefaultDetector {
    fn detect(&self, project_dir: &Path) -> VerificationCommands {
        let project_type = self.detect_project_type(project_dir);
        debug!(?project_type, ?project_dir, "Detected project type");

        match project_type {
            Some(ProjectType::NodeJs) => self.nodejs_commands(project_dir),
            Some(ProjectType::Rust) => self.rust_commands(),
            Some(ProjectType::Python) => self.python_commands(project_dir),
            Some(ProjectType::Go) => self.go_commands(),
            Some(ProjectType::Jvm) => self.jvm_commands(project_dir),
            Some(ProjectType::Make) => self.make_commands(project_dir),
            None => VerificationCommands::default(),
        }
    }

    fn detect_project_type(&self, project_dir: &Path) -> Option<ProjectType> {
        // Check in order of priority (more specific project types first)
        if project_dir.join("package.json").exists() {
            Some(ProjectType::NodeJs)
        } else if project_dir.join("Cargo.toml").exists() {
            Some(ProjectType::Rust)
        } else if project_dir.join("pyproject.toml").exists()
            || project_dir.join("setup.py").exists()
        {
            Some(ProjectType::Python)
        } else if project_dir.join("go.mod").exists() {
            Some(ProjectType::Go)
        } else if project_dir.join("pom.xml").exists()
            || project_dir.join("build.gradle").exists()
            || project_dir.join("build.gradle.kts").exists()
        {
            Some(ProjectType::Jvm)
        } else if project_dir.join("Makefile").exists() {
            Some(ProjectType::Make)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // DefaultDetector::new tests

    #[test]
    fn test_default_detector_new() {
        let detector = DefaultDetector::new();
        // Just verify it can be created
        assert!(std::mem::size_of_val(&detector) == 0);
    }

    #[test]
    fn test_default_detector_default() {
        let detector = DefaultDetector;
        // Default should be the same as new
        assert!(std::mem::size_of_val(&detector) == 0);
    }

    #[test]
    fn test_default_detector_clone() {
        let detector = DefaultDetector::new();
        let cloned = detector.clone();
        assert!(std::mem::size_of_val(&cloned) == 0);
    }

    // detect_project_type tests

    #[test]
    fn test_detect_nodejs_project() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("package.json"), "{}").unwrap();

        let detector = DefaultDetector::new();
        assert_eq!(
            detector.detect_project_type(temp.path()),
            Some(ProjectType::NodeJs)
        );
    }

    #[test]
    fn test_detect_rust_project() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();

        let detector = DefaultDetector::new();
        assert_eq!(
            detector.detect_project_type(temp.path()),
            Some(ProjectType::Rust)
        );
    }

    #[test]
    fn test_detect_python_pyproject() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("pyproject.toml"), "").unwrap();

        let detector = DefaultDetector::new();
        assert_eq!(
            detector.detect_project_type(temp.path()),
            Some(ProjectType::Python)
        );
    }

    #[test]
    fn test_detect_python_setup_py() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("setup.py"), "").unwrap();

        let detector = DefaultDetector::new();
        assert_eq!(
            detector.detect_project_type(temp.path()),
            Some(ProjectType::Python)
        );
    }

    #[test]
    fn test_detect_go_project() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("go.mod"), "module test").unwrap();

        let detector = DefaultDetector::new();
        assert_eq!(
            detector.detect_project_type(temp.path()),
            Some(ProjectType::Go)
        );
    }

    #[test]
    fn test_detect_jvm_maven() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("pom.xml"), "<project>").unwrap();

        let detector = DefaultDetector::new();
        assert_eq!(
            detector.detect_project_type(temp.path()),
            Some(ProjectType::Jvm)
        );
    }

    #[test]
    fn test_detect_jvm_gradle() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("build.gradle"), "").unwrap();

        let detector = DefaultDetector::new();
        assert_eq!(
            detector.detect_project_type(temp.path()),
            Some(ProjectType::Jvm)
        );
    }

    #[test]
    fn test_detect_jvm_gradle_kotlin() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("build.gradle.kts"), "").unwrap();

        let detector = DefaultDetector::new();
        assert_eq!(
            detector.detect_project_type(temp.path()),
            Some(ProjectType::Jvm)
        );
    }

    #[test]
    fn test_detect_make_project() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("Makefile"), "").unwrap();

        let detector = DefaultDetector::new();
        assert_eq!(
            detector.detect_project_type(temp.path()),
            Some(ProjectType::Make)
        );
    }

    #[test]
    fn test_detect_no_project() {
        let temp = TempDir::new().unwrap();
        let detector = DefaultDetector::new();
        assert_eq!(detector.detect_project_type(temp.path()), None);
    }

    #[test]
    fn test_detect_priority_nodejs_over_make() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("package.json"), "{}").unwrap();
        fs::write(temp.path().join("Makefile"), "").unwrap();

        let detector = DefaultDetector::new();
        // Node.js should take priority over Makefile
        assert_eq!(
            detector.detect_project_type(temp.path()),
            Some(ProjectType::NodeJs)
        );
    }

    // detect (commands) tests

    #[test]
    fn test_rust_commands() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();

        let detector = DefaultDetector::new();
        let commands = detector.detect(temp.path());

        assert_eq!(commands.unit, Some("cargo test".to_string()));
        assert_eq!(
            commands.lint,
            Some("cargo clippy -- -D warnings".to_string())
        );
        assert_eq!(commands.format, Some("cargo fmt -- --check".to_string()));
    }

    #[test]
    fn test_nodejs_commands_basic() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("package.json"), "{}").unwrap();

        let detector = DefaultDetector::new();
        let commands = detector.detect(temp.path());

        assert_eq!(commands.unit, Some("npm test".to_string()));
    }

    #[test]
    fn test_nodejs_commands_with_lint() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("package.json"),
            r#"{"scripts": {"lint": "eslint ."}}"#,
        )
        .unwrap();

        let detector = DefaultDetector::new();
        let commands = detector.detect(temp.path());

        assert_eq!(commands.unit, Some("npm test".to_string()));
        assert_eq!(commands.lint, Some("npm run lint".to_string()));
    }

    #[test]
    fn test_nodejs_commands_with_typecheck() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("package.json"),
            r#"{"scripts": {"typecheck": "tsc --noEmit"}}"#,
        )
        .unwrap();

        let detector = DefaultDetector::new();
        let commands = detector.detect(temp.path());

        assert_eq!(commands.type_check, Some("npm run typecheck".to_string()));
    }

    #[test]
    fn test_nodejs_commands_with_tsconfig() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("package.json"), "{}").unwrap();
        fs::write(temp.path().join("tsconfig.json"), "{}").unwrap();

        let detector = DefaultDetector::new();
        let commands = detector.detect(temp.path());

        assert_eq!(commands.type_check, Some("npx tsc --noEmit".to_string()));
    }

    #[test]
    fn test_python_commands_with_pytest() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("pyproject.toml"), "").unwrap();

        let detector = DefaultDetector::new();
        let commands = detector.detect(temp.path());

        assert_eq!(commands.unit, Some("pytest".to_string()));
        assert_eq!(commands.lint, Some("ruff check .".to_string()));
        assert_eq!(commands.type_check, Some("mypy .".to_string()));
    }

    #[test]
    fn test_python_commands_with_pytest_ini() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("setup.py"), "").unwrap();
        fs::write(temp.path().join("pytest.ini"), "").unwrap();

        let detector = DefaultDetector::new();
        let commands = detector.detect(temp.path());

        assert_eq!(commands.unit, Some("pytest".to_string()));
    }

    #[test]
    fn test_python_commands_without_pytest() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("setup.py"), "").unwrap();

        let detector = DefaultDetector::new();
        let commands = detector.detect(temp.path());

        assert_eq!(
            commands.unit,
            Some("python -m unittest discover".to_string())
        );
    }

    #[test]
    fn test_go_commands() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("go.mod"), "module test").unwrap();

        let detector = DefaultDetector::new();
        let commands = detector.detect(temp.path());

        assert_eq!(commands.unit, Some("go test ./...".to_string()));
        assert_eq!(commands.lint, Some("golangci-lint run".to_string()));
        assert_eq!(commands.format, Some("gofmt -l .".to_string()));
    }

    #[test]
    fn test_jvm_commands_maven() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("pom.xml"), "<project>").unwrap();

        let detector = DefaultDetector::new();
        let commands = detector.detect(temp.path());

        assert_eq!(commands.unit, Some("mvn test".to_string()));
    }

    #[test]
    fn test_jvm_commands_maven_wrapper() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("pom.xml"), "<project>").unwrap();
        fs::write(temp.path().join("mvnw"), "#!/bin/bash").unwrap();

        let detector = DefaultDetector::new();
        let commands = detector.detect(temp.path());

        assert_eq!(commands.unit, Some("./mvnw test".to_string()));
    }

    #[test]
    fn test_jvm_commands_gradle() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("build.gradle"), "").unwrap();

        let detector = DefaultDetector::new();
        let commands = detector.detect(temp.path());

        assert_eq!(commands.unit, Some("gradle test".to_string()));
    }

    #[test]
    fn test_jvm_commands_gradle_wrapper() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("build.gradle"), "").unwrap();
        fs::write(temp.path().join("gradlew"), "#!/bin/bash").unwrap();

        let detector = DefaultDetector::new();
        let commands = detector.detect(temp.path());

        assert_eq!(commands.unit, Some("./gradlew test".to_string()));
    }

    #[test]
    fn test_make_commands_with_test() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("Makefile"),
            "test:\n\techo test\nlint:\n\techo lint",
        )
        .unwrap();

        let detector = DefaultDetector::new();
        let commands = detector.detect(temp.path());

        assert_eq!(commands.unit, Some("make test".to_string()));
        assert_eq!(commands.lint, Some("make lint".to_string()));
    }

    #[test]
    fn test_make_commands_without_targets() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("Makefile"), "build:\n\techo build").unwrap();

        let detector = DefaultDetector::new();
        let commands = detector.detect(temp.path());

        assert!(commands.unit.is_none());
        assert!(commands.lint.is_none());
    }

    #[test]
    fn test_no_project_empty_commands() {
        let temp = TempDir::new().unwrap();

        let detector = DefaultDetector::new();
        let commands = detector.detect(temp.path());

        assert!(commands.unit.is_none());
        assert!(commands.lint.is_none());
        assert!(commands.type_check.is_none());
        assert!(!commands.has_any());
    }
}
