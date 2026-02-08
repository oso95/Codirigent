//! Skill manager implementation.
//!
//! Discovers and manages Claude Code skills from the filesystem,
//! including token budget tracking and preset management.
//!
//! # Overview
//!
//! This module provides [`DefaultSkillManager`], the default implementation of
//! the [`SkillManager`] trait from `dirigent-core`. It discovers skills from:
//!
//! - Project-level `.claude/commands` directory
//! - Global `~/.claude/commands` directory
//!
//! # Example
//!
//! ```no_run
//! use codirigent_session::DefaultSkillManager;
//! use codirigent_core::traits::SkillManager;
//!
//! let mut manager = DefaultSkillManager::new();
//!
//! // Refresh skills from disk
//! manager.refresh().unwrap();
//!
//! // List all skills
//! for skill in manager.list_skills() {
//!     println!("{}: {}", skill.name, skill.description);
//! }
//!
//! // Check token budget
//! let budget = manager.token_budget();
//! println!("Token usage: {}/{}", budget.used_tokens, budget.max_tokens);
//! ```

use anyhow::{Context, Result};
use codirigent_core::events::CodirigentEvent;
use codirigent_core::skill::{Skill, SkillPreset, SkillType, TokenBudget};
use codirigent_core::traits::SkillManager as SkillManagerTrait;
use std::path::{Path, PathBuf};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

/// Default implementation of the SkillManager trait.
///
/// Discovers and manages Claude Code skills from the filesystem.
/// Skills are loaded from `.claude/commands` directories and can be
/// enabled/disabled individually or via presets.
///
/// # Example
///
/// ```no_run
/// use codirigent_session::DefaultSkillManager;
/// use codirigent_core::traits::SkillManager;
/// use std::path::PathBuf;
///
/// // Create with custom paths for testing
/// let manager = DefaultSkillManager::with_paths(
///     Some(PathBuf::from("/project/.claude")),
///     None,
/// );
/// ```
pub struct DefaultSkillManager {
    /// All discovered skills.
    skills: Vec<Skill>,
    /// Available presets.
    presets: Vec<SkillPreset>,
    /// Current token budget.
    budget: TokenBudget,
    /// Path to project's .claude directory.
    project_claude_dir: Option<PathBuf>,
    /// Path to global .claude directory.
    global_claude_dir: Option<PathBuf>,
    /// Event sender for emitting skill events.
    event_tx: Option<broadcast::Sender<CodirigentEvent>>,
}

impl Default for DefaultSkillManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultSkillManager {
    /// Create a new skill manager with default paths.
    ///
    /// Uses the global `~/.claude` directory if available.
    /// Project directory must be set separately via [`set_project_dir`](Self::set_project_dir).
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_session::DefaultSkillManager;
    /// use codirigent_core::traits::SkillManager;
    ///
    /// let manager = DefaultSkillManager::new();
    /// assert!(manager.list_skills().is_empty());
    /// ```
    pub fn new() -> Self {
        let global_dir = dirs::home_dir().map(|h| h.join(".claude"));

        Self {
            skills: Vec::new(),
            presets: vec![
                SkillPreset::dev_preset(),
                SkillPreset::review_preset(),
                SkillPreset::minimal_preset(),
            ],
            budget: TokenBudget::default(),
            project_claude_dir: None,
            global_claude_dir: global_dir,
            event_tx: None,
        }
    }

    /// Create with specific paths (for testing).
    ///
    /// # Arguments
    ///
    /// * `project_dir` - Path to project's `.claude` directory
    /// * `global_dir` - Path to global `.claude` directory
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_session::DefaultSkillManager;
    /// use codirigent_core::traits::SkillManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = DefaultSkillManager::with_paths(
    ///     Some(PathBuf::from("/project/.claude")),
    ///     Some(PathBuf::from("/home/user/.claude")),
    /// );
    /// ```
    pub fn with_paths(project_dir: Option<PathBuf>, global_dir: Option<PathBuf>) -> Self {
        Self {
            skills: Vec::new(),
            presets: vec![
                SkillPreset::dev_preset(),
                SkillPreset::review_preset(),
                SkillPreset::minimal_preset(),
            ],
            budget: TokenBudget::default(),
            project_claude_dir: project_dir,
            global_claude_dir: global_dir,
            event_tx: None,
        }
    }

    /// Create with an event bus for emitting skill events.
    ///
    /// # Arguments
    ///
    /// * `event_tx` - Broadcast sender for events
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_session::DefaultSkillManager;
    /// use tokio::sync::broadcast;
    /// use codirigent_core::events::CodirigentEvent;
    ///
    /// let (tx, _rx) = broadcast::channel::<CodirigentEvent>(16);
    /// let manager = DefaultSkillManager::with_event_bus(tx);
    /// ```
    pub fn with_event_bus(event_tx: broadcast::Sender<CodirigentEvent>) -> Self {
        Self {
            event_tx: Some(event_tx),
            ..Self::new()
        }
    }

    /// Set the project directory.
    ///
    /// This sets the path to the project's `.claude` directory.
    /// Call [`refresh`](SkillManagerTrait::refresh) after setting to load skills.
    ///
    /// # Arguments
    ///
    /// * `dir` - Path to the project root directory
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_session::DefaultSkillManager;
    /// use codirigent_core::traits::SkillManager;
    /// use std::path::PathBuf;
    ///
    /// let mut manager = DefaultSkillManager::new();
    /// manager.set_project_dir(PathBuf::from("/my/project"));
    /// manager.refresh().unwrap();
    /// ```
    pub fn set_project_dir(&mut self, dir: PathBuf) {
        self.project_claude_dir = Some(dir.join(".claude"));
    }

    /// Get the project claude directory path.
    ///
    /// Returns the path to the project's `.claude` directory if set.
    pub fn project_claude_dir(&self) -> Option<&Path> {
        self.project_claude_dir.as_deref()
    }

    /// Get the global claude directory path.
    ///
    /// Returns the path to the global `~/.claude` directory if set.
    pub fn global_claude_dir(&self) -> Option<&Path> {
        self.global_claude_dir.as_deref()
    }

    /// Emit an event if event bus is configured.
    fn emit(&self, event: CodirigentEvent) {
        if let Some(ref tx) = self.event_tx {
            // Ignore send errors (no receivers)
            let _ = tx.send(event);
        }
    }

    /// Scan a directory for skill files.
    ///
    /// Scans the `commands` subdirectory for `.md` files and parses
    /// them as skills.
    ///
    /// # Arguments
    ///
    /// * `dir` - Path to the `.claude` directory
    /// * `skill_type` - Type to assign to discovered skills
    fn scan_directory(&mut self, dir: &Path, skill_type: SkillType) -> Result<()> {
        let commands_dir = dir.join("commands");

        if !commands_dir.exists() {
            debug!(?commands_dir, "Commands directory does not exist");
            return Ok(());
        }

        info!(?commands_dir, "Scanning for skills");

        let entries = std::fs::read_dir(&commands_dir)
            .with_context(|| format!("Failed to read commands directory: {:?}", commands_dir))?;

        for entry in entries {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "md") {
                match Self::parse_skill_file(&path, skill_type) {
                    Ok(skill) => {
                        debug!(name = %skill.name, tokens = skill.token_count, "Found skill");
                        self.skills.push(skill);
                    }
                    Err(e) => {
                        warn!(?path, error = %e, "Failed to parse skill file");
                    }
                }
            }
        }

        Ok(())
    }

    /// Parse a skill from a markdown file.
    ///
    /// Extracts the skill name from the filename, description from the
    /// first non-header line, tags from YAML frontmatter, and estimates
    /// the token count.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the skill markdown file
    /// * `skill_type` - Type to assign to the skill
    fn parse_skill_file(path: &Path, skill_type: SkillType) -> Result<Skill> {
        let content = std::fs::read_to_string(path).context("Failed to read skill file")?;

        // Extract skill name from filename (e.g., "commit.md" -> "commit")
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("Invalid filename")?
            .to_string();

        // Extract description from first paragraph or use filename
        let description =
            Self::extract_description(&content).unwrap_or_else(|| format!("Skill: {}", name));

        // Estimate token count
        let token_count = Self::estimate_tokens(&content);

        // Extract tags from frontmatter if present
        let tags = Self::extract_tags(&content);

        Ok(Skill {
            name,
            description,
            source_path: path.to_path_buf(),
            token_count,
            enabled: true,
            tags,
            skill_type,
        })
    }

    /// Extract description from markdown content.
    ///
    /// Returns the first non-empty line that is not a header or frontmatter
    /// delimiter. Truncates to 100 characters if longer.
    ///
    /// # Arguments
    ///
    /// * `content` - Markdown content to extract from
    fn extract_description(content: &str) -> Option<String> {
        let mut in_frontmatter = false;

        for line in content.lines() {
            let trimmed = line.trim();

            // Handle frontmatter
            if trimmed == "---" {
                in_frontmatter = !in_frontmatter;
                continue;
            }

            // Skip frontmatter content
            if in_frontmatter {
                continue;
            }

            // Skip empty lines and headers
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                // Truncate to reasonable length
                let desc = if trimmed.len() > 100 {
                    format!("{}...", &trimmed[..97])
                } else {
                    trimmed.to_string()
                };
                return Some(desc);
            }
        }
        None
    }

    /// Extract tags from frontmatter.
    ///
    /// Parses simple YAML frontmatter to extract tags in the format:
    /// `tags: [tag1, tag2]` or `tags: [tag1, "tag 2"]`
    ///
    /// # Arguments
    ///
    /// * `content` - Markdown content to extract from
    fn extract_tags(content: &str) -> Vec<String> {
        // Simple YAML frontmatter extraction
        if !content.starts_with("---") {
            return Vec::new();
        }

        let mut tags = Vec::new();
        let mut in_frontmatter = false;

        for line in content.lines() {
            if line == "---" {
                if in_frontmatter {
                    break;
                }
                in_frontmatter = true;
                continue;
            }

            if in_frontmatter {
                if let Some(tag_str) = line.strip_prefix("tags:") {
                    // Parse simple tag format: tags: [tag1, tag2]
                    let tag_str = tag_str.trim().trim_matches(|c| c == '[' || c == ']');
                    tags = tag_str
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            }
        }

        tags
    }

    /// Estimate token count for content.
    ///
    /// Uses a simple heuristic: approximately 4 characters per token on average.
    /// This is a rough estimate suitable for budget planning purposes.
    ///
    /// # Arguments
    ///
    /// * `content` - Text content to estimate tokens for
    pub fn estimate_tokens(content: &str) -> u32 {
        // Rough estimate: 4 characters per token
        (content.len() / 4) as u32
    }
}

impl SkillManagerTrait for DefaultSkillManager {
    fn list_skills(&self) -> &[Skill] {
        &self.skills
    }

    fn get_skill(&self, name: &str) -> Option<&Skill> {
        self.skills.iter().find(|s| s.name == name)
    }

    fn get_skill_mut(&mut self, name: &str) -> Option<&mut Skill> {
        self.skills.iter_mut().find(|s| s.name == name)
    }

    fn enable_skill(&mut self, name: &str) -> Result<()> {
        let skill = self
            .skills
            .iter_mut()
            .find(|s| s.name == name)
            .context("Skill not found")?;

        if skill.enabled {
            return Ok(()); // Already enabled
        }

        skill.enabled = true;
        self.recalculate_budget();

        info!(name, "Skill enabled");
        self.emit(CodirigentEvent::SkillEnabled {
            name: name.to_string(),
        });

        Ok(())
    }

    fn disable_skill(&mut self, name: &str) -> Result<()> {
        let skill = self
            .skills
            .iter_mut()
            .find(|s| s.name == name)
            .context("Skill not found")?;

        if !skill.enabled {
            return Ok(()); // Already disabled
        }

        skill.enabled = false;
        self.recalculate_budget();

        info!(name, "Skill disabled");
        self.emit(CodirigentEvent::SkillDisabled {
            name: name.to_string(),
        });

        Ok(())
    }

    fn token_budget(&self) -> TokenBudget {
        self.budget
    }

    fn recalculate_budget(&mut self) {
        self.budget.reset();

        let total: u32 = self
            .skills
            .iter()
            .filter(|s| s.enabled)
            .map(|s| s.token_count)
            .sum();

        self.budget.add(total);

        if self.budget.is_exceeded() {
            warn!(
                used = self.budget.used_tokens,
                max = self.budget.max_tokens,
                "Token budget exceeded"
            );
            self.emit(CodirigentEvent::TokenBudgetExceeded {
                budget: self.budget,
            });
        } else if self.budget.is_warning() {
            warn!(
                used = self.budget.used_tokens,
                threshold = self.budget.warning_threshold,
                "Token budget warning threshold reached"
            );
            self.emit(CodirigentEvent::TokenBudgetWarning {
                budget: self.budget,
            });
        }
    }

    fn list_presets(&self) -> &[SkillPreset] {
        &self.presets
    }

    fn apply_preset(&mut self, preset_name: &str) -> Result<()> {
        let preset = self
            .presets
            .iter()
            .find(|p| p.name == preset_name)
            .context("Preset not found")?
            .clone();

        // Disable all skills first
        for skill in &mut self.skills {
            skill.enabled = false;
        }

        // Enable skills in preset
        let mut enabled_count = 0;
        for skill_name in &preset.enabled_skills {
            if let Some(skill) = self.skills.iter_mut().find(|s| s.name == *skill_name) {
                skill.enabled = true;
                enabled_count += 1;
            }
        }

        self.recalculate_budget();

        info!(preset_name, enabled_count, "Applied preset");
        self.emit(CodirigentEvent::SkillPresetApplied {
            preset_name: preset_name.to_string(),
            enabled_count,
        });

        Ok(())
    }

    fn search(&self, query: &str) -> Vec<&Skill> {
        let query_lower = query.to_lowercase();
        self.skills
            .iter()
            .filter(|s| {
                s.name.to_lowercase().contains(&query_lower)
                    || s.description.to_lowercase().contains(&query_lower)
                    || s.tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    fn refresh(&mut self) -> Result<()> {
        self.skills.clear();

        // Scan project directory
        if let Some(ref dir) = self.project_claude_dir.clone() {
            self.scan_directory(dir, SkillType::UserDefined)?;
        }

        // Scan global directory
        if let Some(ref dir) = self.global_claude_dir.clone() {
            self.scan_directory(dir, SkillType::UserDefined)?;
        }

        self.recalculate_budget();

        info!(count = self.skills.len(), "Skills refreshed");
        self.emit(CodirigentEvent::SkillsRefreshed {
            count: self.skills.len(),
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // Task 1 tests: DefaultSkillManager structure
    #[test]
    fn test_skill_manager_new() {
        let manager = DefaultSkillManager::new();
        assert!(manager.skills.is_empty());
        assert!(!manager.presets.is_empty());
        assert_eq!(manager.budget.used_tokens, 0);
    }

    #[test]
    fn test_skill_manager_default() {
        let manager = DefaultSkillManager::default();
        assert!(manager.skills.is_empty());
        assert_eq!(manager.presets.len(), 3); // dev, review, minimal
    }

    #[test]
    fn test_skill_manager_with_paths() {
        let temp = TempDir::new().unwrap();
        let manager =
            DefaultSkillManager::with_paths(Some(temp.path().to_path_buf()), Some(PathBuf::new()));
        assert!(manager.project_claude_dir.is_some());
        assert!(manager.global_claude_dir.is_some());
    }

    #[test]
    fn test_skill_manager_with_event_bus() {
        let (tx, _rx) = broadcast::channel::<CodirigentEvent>(16);
        let manager = DefaultSkillManager::with_event_bus(tx);
        assert!(manager.event_tx.is_some());
    }

    #[test]
    fn test_set_project_dir() {
        let mut manager = DefaultSkillManager::new();
        manager.set_project_dir(PathBuf::from("/my/project"));
        assert_eq!(
            manager.project_claude_dir,
            Some(PathBuf::from("/my/project/.claude"))
        );
    }

    #[test]
    fn test_project_claude_dir_getter() {
        let manager =
            DefaultSkillManager::with_paths(Some(PathBuf::from("/project/.claude")), None);
        assert_eq!(
            manager.project_claude_dir(),
            Some(Path::new("/project/.claude"))
        );
    }

    #[test]
    fn test_global_claude_dir_getter() {
        let manager =
            DefaultSkillManager::with_paths(None, Some(PathBuf::from("/home/user/.claude")));
        assert_eq!(
            manager.global_claude_dir(),
            Some(Path::new("/home/user/.claude"))
        );
    }

    // Task 2 tests: Directory scanning
    fn setup_test_skills(temp: &TempDir) {
        let commands_dir = temp.path().join("commands");
        fs::create_dir_all(&commands_dir).unwrap();

        fs::write(
            commands_dir.join("test-skill.md"),
            "# Test Skill\n\nThis is a test skill description.\n\n```\ncode here\n```",
        )
        .unwrap();

        fs::write(
            commands_dir.join("with-tags.md"),
            "---\ntags: [dev, testing]\n---\n\nSkill with tags",
        )
        .unwrap();

        fs::write(
            commands_dir.join("long-description.md"),
            "This is a very long description that should be truncated because it exceeds one hundred characters in length and we need to make sure it gets properly shortened.",
        )
        .unwrap();
    }

    #[test]
    fn test_scan_directory() {
        let temp = TempDir::new().unwrap();
        setup_test_skills(&temp);

        let mut manager = DefaultSkillManager::with_paths(None, None);
        manager
            .scan_directory(temp.path(), SkillType::UserDefined)
            .unwrap();

        assert_eq!(manager.skills.len(), 3);
    }

    #[test]
    fn test_scan_directory_nonexistent() {
        let temp = TempDir::new().unwrap();
        let mut manager = DefaultSkillManager::with_paths(None, None);

        // Should not error when commands dir doesn't exist
        let result = manager.scan_directory(temp.path(), SkillType::UserDefined);
        assert!(result.is_ok());
        assert!(manager.skills.is_empty());
    }

    #[test]
    fn test_parse_skill_file() {
        let temp = TempDir::new().unwrap();
        let skill_path = temp.path().join("my-skill.md");
        fs::write(&skill_path, "Description of the skill").unwrap();

        let skill =
            DefaultSkillManager::parse_skill_file(&skill_path, SkillType::UserDefined).unwrap();

        assert_eq!(skill.name, "my-skill");
        assert!(skill.description.contains("Description"));
        assert_eq!(skill.skill_type, SkillType::UserDefined);
        assert!(skill.enabled);
    }

    #[test]
    fn test_parse_skill_file_with_frontmatter() {
        let temp = TempDir::new().unwrap();
        let skill_path = temp.path().join("tagged-skill.md");
        fs::write(
            &skill_path,
            "---\ntags: [git, vcs]\n---\n\nA skill with tags",
        )
        .unwrap();

        let skill =
            DefaultSkillManager::parse_skill_file(&skill_path, SkillType::UserDefined).unwrap();

        assert_eq!(skill.name, "tagged-skill");
        assert!(skill.tags.contains(&"git".to_string()));
        assert!(skill.tags.contains(&"vcs".to_string()));
        assert_eq!(skill.description, "A skill with tags");
    }

    #[test]
    fn test_extract_description_simple() {
        let content = "This is a description";
        let desc = DefaultSkillManager::extract_description(content);
        assert_eq!(desc, Some("This is a description".to_string()));
    }

    #[test]
    fn test_extract_description_with_header() {
        let content = "# Header\n\nThis is the description";
        let desc = DefaultSkillManager::extract_description(content);
        assert_eq!(desc, Some("This is the description".to_string()));
    }

    #[test]
    fn test_extract_description_with_frontmatter() {
        let content = "---\ntags: [test]\n---\n\nActual description";
        let desc = DefaultSkillManager::extract_description(content);
        assert_eq!(desc, Some("Actual description".to_string()));
    }

    #[test]
    fn test_extract_description_long() {
        let content = "a".repeat(150);
        let desc = DefaultSkillManager::extract_description(&content);
        assert!(desc.is_some());
        let desc = desc.unwrap();
        assert!(desc.len() <= 100);
        assert!(desc.ends_with("..."));
    }

    #[test]
    fn test_extract_description_empty() {
        let content = "# Just a header\n";
        let desc = DefaultSkillManager::extract_description(content);
        assert!(desc.is_none());
    }

    #[test]
    fn test_extract_tags_simple() {
        let content = "---\ntags: [dev, test]\n---\n\nContent";
        let tags = DefaultSkillManager::extract_tags(content);
        assert_eq!(tags, vec!["dev", "test"]);
    }

    #[test]
    fn test_extract_tags_with_quotes() {
        let content = "---\ntags: [\"multi word\", 'another']\n---\n\nContent";
        let tags = DefaultSkillManager::extract_tags(content);
        assert_eq!(tags, vec!["multi word", "another"]);
    }

    #[test]
    fn test_extract_tags_no_frontmatter() {
        let content = "Just regular content";
        let tags = DefaultSkillManager::extract_tags(content);
        assert!(tags.is_empty());
    }

    #[test]
    fn test_extract_tags_no_tags_field() {
        let content = "---\nauthor: test\n---\n\nContent";
        let tags = DefaultSkillManager::extract_tags(content);
        assert!(tags.is_empty());
    }

    #[test]
    fn test_estimate_tokens() {
        let content = "a".repeat(400); // 400 chars
        let tokens = DefaultSkillManager::estimate_tokens(&content);
        assert_eq!(tokens, 100); // 400 / 4
    }

    #[test]
    fn test_estimate_tokens_empty() {
        let tokens = DefaultSkillManager::estimate_tokens("");
        assert_eq!(tokens, 0);
    }

    #[test]
    fn test_estimate_tokens_short() {
        let tokens = DefaultSkillManager::estimate_tokens("abc");
        assert_eq!(tokens, 0); // 3 / 4 = 0
    }

    // Task 3 tests: SkillManager trait implementation
    fn create_test_manager() -> DefaultSkillManager {
        let mut manager = DefaultSkillManager::new();
        manager.skills = vec![
            Skill {
                name: "skill1".to_string(),
                description: "First skill".to_string(),
                source_path: PathBuf::new(),
                token_count: 1000,
                enabled: true,
                tags: vec!["dev".to_string()],
                skill_type: SkillType::UserDefined,
            },
            Skill {
                name: "skill2".to_string(),
                description: "Second skill".to_string(),
                source_path: PathBuf::new(),
                token_count: 2000,
                enabled: false,
                tags: vec!["test".to_string()],
                skill_type: SkillType::UserDefined,
            },
        ];
        manager.recalculate_budget();
        manager
    }

    #[test]
    fn test_list_skills() {
        let manager = create_test_manager();
        assert_eq!(manager.list_skills().len(), 2);
    }

    #[test]
    fn test_get_skill() {
        let manager = create_test_manager();
        assert!(manager.get_skill("skill1").is_some());
        assert!(manager.get_skill("nonexistent").is_none());
    }

    #[test]
    fn test_get_skill_mut() {
        let mut manager = create_test_manager();
        let skill = manager.get_skill_mut("skill1").unwrap();
        skill.description = "Modified".to_string();
        assert_eq!(manager.get_skill("skill1").unwrap().description, "Modified");
    }

    #[test]
    fn test_enable_skill() {
        let mut manager = create_test_manager();

        manager.enable_skill("skill2").unwrap();
        assert!(manager.get_skill("skill2").unwrap().enabled);
        assert_eq!(manager.budget.used_tokens, 3000); // 1000 + 2000
    }

    #[test]
    fn test_enable_skill_already_enabled() {
        let mut manager = create_test_manager();

        // Should not error when already enabled
        let result = manager.enable_skill("skill1");
        assert!(result.is_ok());
    }

    #[test]
    fn test_enable_skill_not_found() {
        let mut manager = create_test_manager();

        let result = manager.enable_skill("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_disable_skill() {
        let mut manager = create_test_manager();

        manager.disable_skill("skill1").unwrap();
        assert!(!manager.get_skill("skill1").unwrap().enabled);
        assert_eq!(manager.budget.used_tokens, 0);
    }

    #[test]
    fn test_disable_skill_already_disabled() {
        let mut manager = create_test_manager();

        // Should not error when already disabled
        let result = manager.disable_skill("skill2");
        assert!(result.is_ok());
    }

    #[test]
    fn test_disable_skill_not_found() {
        let mut manager = create_test_manager();

        let result = manager.disable_skill("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_token_budget() {
        let manager = create_test_manager();
        let budget = manager.token_budget();
        assert_eq!(budget.used_tokens, 1000); // Only skill1 is enabled
    }

    #[test]
    fn test_recalculate_budget() {
        let mut manager = create_test_manager();
        manager.skills[1].enabled = true;
        manager.recalculate_budget();
        assert_eq!(manager.budget.used_tokens, 3000);
    }

    #[test]
    fn test_list_presets() {
        let manager = DefaultSkillManager::new();
        let presets = manager.list_presets();
        assert_eq!(presets.len(), 3);
        assert!(presets.iter().any(|p| p.name == "dev"));
        assert!(presets.iter().any(|p| p.name == "review"));
        assert!(presets.iter().any(|p| p.name == "minimal"));
    }

    #[test]
    fn test_apply_preset() {
        let mut manager = create_test_manager();

        // Add skills that match dev preset
        manager.skills.push(Skill {
            name: "commit".to_string(),
            description: "Git commit".to_string(),
            source_path: PathBuf::new(),
            token_count: 500,
            enabled: false,
            tags: vec![],
            skill_type: SkillType::UserDefined,
        });

        manager.apply_preset("dev").unwrap();

        // Only "commit" should be enabled (matches dev preset)
        assert!(manager.get_skill("commit").unwrap().enabled);
        assert!(!manager.get_skill("skill1").unwrap().enabled);
        assert!(!manager.get_skill("skill2").unwrap().enabled);
    }

    #[test]
    fn test_apply_preset_minimal() {
        let mut manager = create_test_manager();
        manager.apply_preset("minimal").unwrap();

        // All skills should be disabled
        assert!(!manager.get_skill("skill1").unwrap().enabled);
        assert!(!manager.get_skill("skill2").unwrap().enabled);
        assert_eq!(manager.budget.used_tokens, 0);
    }

    #[test]
    fn test_apply_preset_not_found() {
        let mut manager = create_test_manager();
        let result = manager.apply_preset("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_search_by_name() {
        let manager = create_test_manager();

        let results = manager.search("skill1");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "skill1");
    }

    #[test]
    fn test_search_by_description() {
        let manager = create_test_manager();

        let results = manager.search("first");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "skill1");
    }

    #[test]
    fn test_search_by_tag() {
        let manager = create_test_manager();

        let results = manager.search("dev");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "skill1");
    }

    #[test]
    fn test_search_case_insensitive() {
        let manager = create_test_manager();

        let results = manager.search("SKILL1");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_partial_match() {
        let manager = create_test_manager();

        let results = manager.search("skill");
        assert_eq!(results.len(), 2); // Both skill1 and skill2 match
    }

    #[test]
    fn test_search_no_match() {
        let manager = create_test_manager();

        let results = manager.search("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_refresh() {
        let temp = TempDir::new().unwrap();
        setup_test_skills(&temp);

        let mut manager = DefaultSkillManager::with_paths(Some(temp.path().to_path_buf()), None);
        manager.refresh().unwrap();

        assert_eq!(manager.skills.len(), 3);
    }

    #[test]
    fn test_refresh_clears_existing() {
        let temp = TempDir::new().unwrap();
        setup_test_skills(&temp);

        let mut manager = DefaultSkillManager::with_paths(Some(temp.path().to_path_buf()), None);

        // Add a skill manually
        manager.skills.push(Skill {
            name: "manual".to_string(),
            description: "Manual".to_string(),
            source_path: PathBuf::new(),
            token_count: 100,
            enabled: true,
            tags: vec![],
            skill_type: SkillType::UserDefined,
        });

        assert_eq!(manager.skills.len(), 1);

        // Refresh should clear and reload
        manager.refresh().unwrap();
        assert_eq!(manager.skills.len(), 3); // Only the 3 from disk
    }

    #[test]
    fn test_enabled_count() {
        let manager = create_test_manager();
        assert_eq!(manager.enabled_count(), 1); // Only skill1 is enabled
    }

    #[test]
    fn test_enabled_token_count() {
        let manager = create_test_manager();
        assert_eq!(manager.enabled_token_count(), 1000); // Only skill1
    }

    // Task 5 tests: Event emission
    #[test]
    fn test_emit_skill_enabled_event() {
        let (tx, mut rx) = broadcast::channel::<CodirigentEvent>(16);
        let mut manager = DefaultSkillManager::with_event_bus(tx);
        manager.skills = vec![Skill {
            name: "test".to_string(),
            description: "Test".to_string(),
            source_path: PathBuf::new(),
            token_count: 100,
            enabled: false,
            tags: vec![],
            skill_type: SkillType::UserDefined,
        }];

        manager.enable_skill("test").unwrap();

        // Check that event was emitted
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, CodirigentEvent::SkillEnabled { name } if name == "test"));
    }

    #[test]
    fn test_emit_skill_disabled_event() {
        let (tx, mut rx) = broadcast::channel::<CodirigentEvent>(16);
        let mut manager = DefaultSkillManager::with_event_bus(tx);
        manager.skills = vec![Skill {
            name: "test".to_string(),
            description: "Test".to_string(),
            source_path: PathBuf::new(),
            token_count: 100,
            enabled: true,
            tags: vec![],
            skill_type: SkillType::UserDefined,
        }];

        manager.disable_skill("test").unwrap();

        // Check that event was emitted
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, CodirigentEvent::SkillDisabled { name } if name == "test"));
    }

    #[test]
    fn test_emit_preset_applied_event() {
        let (tx, mut rx) = broadcast::channel::<CodirigentEvent>(16);
        let mut manager = DefaultSkillManager::with_event_bus(tx);

        manager.apply_preset("minimal").unwrap();

        // Check that event was emitted
        let event = rx.try_recv().unwrap();
        assert!(
            matches!(event, CodirigentEvent::SkillPresetApplied { preset_name, .. } if preset_name == "minimal")
        );
    }

    #[test]
    fn test_emit_skills_refreshed_event() {
        let (tx, mut rx) = broadcast::channel::<CodirigentEvent>(16);
        let mut manager = DefaultSkillManager::with_event_bus(tx);

        manager.refresh().unwrap();

        // refresh() may emit budget events before SkillsRefreshed
        // (when skills are found on the host), so drain until we find it.
        let mut found = false;
        while let Ok(event) = rx.try_recv() {
            if matches!(event, CodirigentEvent::SkillsRefreshed { .. }) {
                found = true;
                break;
            }
        }
        assert!(found, "Expected SkillsRefreshed event from refresh()");
    }

    #[test]
    fn test_emit_token_budget_warning() {
        let (tx, mut rx) = broadcast::channel::<CodirigentEvent>(16);
        let mut manager = DefaultSkillManager::with_event_bus(tx);
        manager.skills = vec![Skill {
            name: "heavy".to_string(),
            description: "Heavy".to_string(),
            source_path: PathBuf::new(),
            token_count: 13000, // Above warning threshold (12000)
            enabled: true,
            tags: vec![],
            skill_type: SkillType::UserDefined,
        }];

        manager.recalculate_budget();

        // Check that warning event was emitted
        let event = rx.try_recv().unwrap();
        assert!(
            matches!(event, CodirigentEvent::TokenBudgetWarning { budget } if budget.is_warning())
        );
    }

    #[test]
    fn test_emit_token_budget_exceeded() {
        let (tx, mut rx) = broadcast::channel::<CodirigentEvent>(16);
        let mut manager = DefaultSkillManager::with_event_bus(tx);
        manager.skills = vec![Skill {
            name: "huge".to_string(),
            description: "Huge".to_string(),
            source_path: PathBuf::new(),
            token_count: 16000, // Above max (15000)
            enabled: true,
            tags: vec![],
            skill_type: SkillType::UserDefined,
        }];

        manager.recalculate_budget();

        // Check that exceeded event was emitted
        let event = rx.try_recv().unwrap();
        assert!(
            matches!(event, CodirigentEvent::TokenBudgetExceeded { budget } if budget.is_exceeded())
        );
    }

    #[test]
    fn test_no_event_without_bus() {
        // This test ensures no panic when event bus is not configured
        let mut manager = DefaultSkillManager::new();
        manager.skills = vec![Skill {
            name: "test".to_string(),
            description: "Test".to_string(),
            source_path: PathBuf::new(),
            token_count: 100,
            enabled: false,
            tags: vec![],
            skill_type: SkillType::UserDefined,
        }];

        // These should not panic even without event bus
        manager.enable_skill("test").unwrap();
        manager.disable_skill("test").unwrap();
        manager.apply_preset("minimal").unwrap();
        manager.refresh().unwrap();
    }
}
