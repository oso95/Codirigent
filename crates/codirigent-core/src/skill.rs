//! Skill management types.
//!
//! Defines types for managing Claude Code skills and slash commands,
//! including token budget tracking and skill presets.
//!
//! Skills are reusable prompts/capabilities that can be loaded into AI coding
//! sessions. They support:
//! - Built-in skills from Claude Code
//! - User-defined skills from `.claude/commands`
//! - Skills from MCP servers
//!
//! # Example
//!
//! ```
//! use codirigent_core::skill::{Skill, SkillType, TokenBudget, SkillPreset};
//! use std::path::PathBuf;
//!
//! // Create a user-defined skill
//! let skill = Skill::new_user(
//!     "my-skill".to_string(),
//!     "My custom skill".to_string(),
//!     PathBuf::from(".claude/commands/my-skill.md"),
//! );
//! assert_eq!(skill.skill_type, SkillType::UserDefined);
//!
//! // Track token budget
//! let mut budget = TokenBudget::default_claude();
//! budget.add(5000);
//! assert_eq!(budget.remaining(), 10000);
//! ```

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents a Claude Code skill (slash command).
///
/// Skills are reusable prompts that can be enabled or disabled to control
/// which capabilities are available in a session. Each skill has an
/// estimated token count that contributes to the system prompt budget.
///
/// # Example
///
/// ```
/// use codirigent_core::skill::{Skill, SkillType};
/// use std::path::PathBuf;
///
/// let skill = Skill::new_user(
///     "commit".to_string(),
///     "Create a git commit with AI-generated message".to_string(),
///     PathBuf::from(".claude/commands/commit.md"),
/// );
/// assert!(skill.enabled);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Skill {
    /// Skill name (e.g., "commit", "review-pr").
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Path to the skill definition file.
    pub source_path: PathBuf,
    /// Estimated token count for the skill prompt.
    pub token_count: u32,
    /// Whether the skill is currently enabled.
    pub enabled: bool,
    /// Tags for categorization.
    pub tags: Vec<String>,
    /// Skill type/source.
    pub skill_type: SkillType,
}

impl Skill {
    /// Create a new user-defined skill.
    ///
    /// User-defined skills are loaded from `.claude/commands` directory.
    ///
    /// # Arguments
    ///
    /// * `name` - The skill name (used as the slash command)
    /// * `description` - Human-readable description
    /// * `source_path` - Path to the skill definition file
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::{Skill, SkillType};
    /// use std::path::PathBuf;
    ///
    /// let skill = Skill::new_user(
    ///     "deploy".to_string(),
    ///     "Deploy to production".to_string(),
    ///     PathBuf::from(".claude/commands/deploy.md"),
    /// );
    /// assert_eq!(skill.skill_type, SkillType::UserDefined);
    /// assert!(skill.enabled);
    /// ```
    pub fn new_user(name: String, description: String, source_path: PathBuf) -> Self {
        Self {
            name,
            description,
            source_path,
            token_count: 0,
            enabled: true,
            tags: Vec::new(),
            skill_type: SkillType::UserDefined,
        }
    }

    /// Create a new built-in skill.
    ///
    /// Built-in skills are provided by Claude Code itself.
    ///
    /// # Arguments
    ///
    /// * `name` - The skill name (used as the slash command)
    /// * `description` - Human-readable description
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::{Skill, SkillType};
    ///
    /// let skill = Skill::new_builtin(
    ///     "commit".to_string(),
    ///     "Create a git commit".to_string(),
    /// );
    /// assert_eq!(skill.skill_type, SkillType::BuiltIn);
    /// assert!(skill.tags.contains(&"builtin".to_string()));
    /// ```
    pub fn new_builtin(name: String, description: String) -> Self {
        Self {
            name,
            description,
            source_path: PathBuf::new(),
            token_count: 0,
            enabled: true,
            tags: vec!["builtin".to_string()],
            skill_type: SkillType::BuiltIn,
        }
    }

    /// Create a new MCP skill.
    ///
    /// MCP skills are provided by Model Context Protocol servers.
    ///
    /// # Arguments
    ///
    /// * `name` - The skill name
    /// * `description` - Human-readable description
    /// * `server_name` - Name of the MCP server providing this skill
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::{Skill, SkillType};
    ///
    /// let skill = Skill::new_mcp(
    ///     "github-search".to_string(),
    ///     "Search GitHub repositories".to_string(),
    ///     "github-mcp".to_string(),
    /// );
    /// assert_eq!(skill.skill_type, SkillType::Mcp);
    /// assert!(skill.tags.contains(&"mcp".to_string()));
    /// ```
    pub fn new_mcp(name: String, description: String, server_name: String) -> Self {
        Self {
            name,
            description,
            source_path: PathBuf::new(),
            token_count: 0,
            enabled: true,
            tags: vec!["mcp".to_string(), server_name],
            skill_type: SkillType::Mcp,
        }
    }

    /// Set the token count for this skill.
    ///
    /// # Arguments
    ///
    /// * `count` - Estimated token count for the skill prompt
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::Skill;
    ///
    /// let skill = Skill::new_builtin("commit".to_string(), "Create commit".to_string())
    ///     .with_token_count(500);
    /// assert_eq!(skill.token_count, 500);
    /// ```
    pub fn with_token_count(mut self, count: u32) -> Self {
        self.token_count = count;
        self
    }

    /// Add a tag to this skill.
    ///
    /// # Arguments
    ///
    /// * `tag` - Tag to add
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::Skill;
    ///
    /// let skill = Skill::new_builtin("commit".to_string(), "Create commit".to_string())
    ///     .with_tag("git".to_string())
    ///     .with_tag("vcs".to_string());
    /// assert!(skill.tags.contains(&"git".to_string()));
    /// ```
    pub fn with_tag(mut self, tag: String) -> Self {
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
        self
    }

    /// Check if this skill matches a search query.
    ///
    /// Matches against name, description, and tags (case-insensitive).
    ///
    /// # Arguments
    ///
    /// * `query` - Search query string
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::Skill;
    ///
    /// let skill = Skill::new_builtin("commit".to_string(), "Create a git commit".to_string())
    ///     .with_tag("git".to_string());
    /// assert!(skill.matches_query("commit"));
    /// assert!(skill.matches_query("GIT"));
    /// assert!(skill.matches_query("create"));
    /// assert!(!skill.matches_query("deploy"));
    /// ```
    pub fn matches_query(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        self.name.to_lowercase().contains(&query_lower)
            || self.description.to_lowercase().contains(&query_lower)
            || self
                .tags
                .iter()
                .any(|t| t.to_lowercase().contains(&query_lower))
    }
}

/// Type of skill based on its source.
///
/// Skills can come from different sources:
/// - Built-in skills provided by Claude Code
/// - User-defined skills from `.claude/commands`
/// - MCP skills from Model Context Protocol servers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SkillType {
    /// Built-in skill from Claude Code.
    #[default]
    BuiltIn,
    /// User-defined skill from `.claude/commands`.
    UserDefined,
    /// Skill from an MCP server.
    Mcp,
}

/// Token budget information for skill loading.
///
/// Claude Code has a 15,000 token limit for the system prompt.
/// This tracks usage and provides warnings when approaching limits.
///
/// # Example
///
/// ```
/// use codirigent_core::skill::TokenBudget;
///
/// let mut budget = TokenBudget::default_claude();
/// assert_eq!(budget.max_tokens, 15000);
///
/// budget.add(12500);
/// assert!(budget.is_warning());
/// assert!(!budget.is_exceeded());
///
/// budget.add(3000);
/// assert!(budget.is_exceeded());
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct TokenBudget {
    /// Maximum tokens allowed for system prompt.
    pub max_tokens: u32,
    /// Currently used tokens.
    pub used_tokens: u32,
    /// Warning threshold.
    pub warning_threshold: u32,
}

impl Default for TokenBudget {
    fn default() -> Self {
        Self::default_claude()
    }
}

impl TokenBudget {
    /// Create a new token budget with Claude Code's default limit.
    ///
    /// Claude Code uses a 15,000 token limit for the system prompt,
    /// with a warning threshold at 12,000 tokens.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::TokenBudget;
    ///
    /// let budget = TokenBudget::default_claude();
    /// assert_eq!(budget.max_tokens, 15000);
    /// assert_eq!(budget.warning_threshold, 12000);
    /// assert_eq!(budget.used_tokens, 0);
    /// ```
    pub fn default_claude() -> Self {
        Self {
            max_tokens: 15000,
            used_tokens: 0,
            warning_threshold: 12000,
        }
    }

    /// Create a custom token budget.
    ///
    /// # Arguments
    ///
    /// * `max_tokens` - Maximum tokens allowed
    /// * `warning_threshold` - Threshold for warning alerts
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::TokenBudget;
    ///
    /// let budget = TokenBudget::new(10000, 8000);
    /// assert_eq!(budget.max_tokens, 10000);
    /// assert_eq!(budget.warning_threshold, 8000);
    /// ```
    pub fn new(max_tokens: u32, warning_threshold: u32) -> Self {
        Self {
            max_tokens,
            used_tokens: 0,
            warning_threshold,
        }
    }

    /// Add tokens to the used count.
    ///
    /// Uses saturating addition to prevent overflow.
    ///
    /// # Arguments
    ///
    /// * `tokens` - Number of tokens to add
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::TokenBudget;
    ///
    /// let mut budget = TokenBudget::default_claude();
    /// budget.add(5000);
    /// assert_eq!(budget.used_tokens, 5000);
    /// ```
    pub fn add(&mut self, tokens: u32) {
        self.used_tokens = self.used_tokens.saturating_add(tokens);
    }

    /// Remove tokens from the used count.
    ///
    /// Uses saturating subtraction to prevent underflow.
    ///
    /// # Arguments
    ///
    /// * `tokens` - Number of tokens to remove
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::TokenBudget;
    ///
    /// let mut budget = TokenBudget::default_claude();
    /// budget.add(5000);
    /// budget.remove(2000);
    /// assert_eq!(budget.used_tokens, 3000);
    /// ```
    pub fn remove(&mut self, tokens: u32) {
        self.used_tokens = self.used_tokens.saturating_sub(tokens);
    }

    /// Reset the used token count.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::TokenBudget;
    ///
    /// let mut budget = TokenBudget::default_claude();
    /// budget.add(5000);
    /// budget.reset();
    /// assert_eq!(budget.used_tokens, 0);
    /// ```
    pub fn reset(&mut self) {
        self.used_tokens = 0;
    }

    /// Check if budget is exceeded.
    ///
    /// Returns true when used tokens exceed the maximum.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::TokenBudget;
    ///
    /// let mut budget = TokenBudget::default_claude();
    /// assert!(!budget.is_exceeded());
    /// budget.used_tokens = 15001;
    /// assert!(budget.is_exceeded());
    /// ```
    pub fn is_exceeded(&self) -> bool {
        self.used_tokens > self.max_tokens
    }

    /// Check if warning threshold is reached.
    ///
    /// Returns true when used tokens are at or above the warning threshold,
    /// but not yet exceeded. Once exceeded, this returns false.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::TokenBudget;
    ///
    /// let mut budget = TokenBudget::default_claude();
    /// assert!(!budget.is_warning());
    ///
    /// budget.used_tokens = 12000;
    /// assert!(budget.is_warning());
    ///
    /// budget.used_tokens = 15001;
    /// assert!(!budget.is_warning()); // Exceeded, not warning
    /// ```
    pub fn is_warning(&self) -> bool {
        self.used_tokens >= self.warning_threshold && !self.is_exceeded()
    }

    /// Remaining tokens available.
    ///
    /// Uses saturating subtraction to prevent underflow when exceeded.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::TokenBudget;
    ///
    /// let mut budget = TokenBudget::default_claude();
    /// assert_eq!(budget.remaining(), 15000);
    /// budget.add(10000);
    /// assert_eq!(budget.remaining(), 5000);
    /// ```
    pub fn remaining(&self) -> u32 {
        self.max_tokens.saturating_sub(self.used_tokens)
    }

    /// Usage percentage (0.0 - 1.0+).
    ///
    /// Can exceed 1.0 if tokens are over budget.
    /// Returns 0.0 if max_tokens is 0 to avoid division by zero.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::TokenBudget;
    ///
    /// let mut budget = TokenBudget::default_claude();
    /// assert_eq!(budget.usage_percent(), 0.0);
    ///
    /// budget.used_tokens = 7500;
    /// assert_eq!(budget.usage_percent(), 0.5);
    ///
    /// budget.used_tokens = 15000;
    /// assert_eq!(budget.usage_percent(), 1.0);
    /// ```
    pub fn usage_percent(&self) -> f32 {
        if self.max_tokens == 0 {
            return 0.0;
        }
        self.used_tokens as f32 / self.max_tokens as f32
    }
}

/// Skill preset for quick mode switching.
///
/// Presets allow quickly enabling/disabling sets of skills
/// for different workflows (development, review, testing, etc.).
///
/// # Example
///
/// ```
/// use codirigent_core::skill::SkillPreset;
///
/// let preset = SkillPreset::dev_preset();
/// assert_eq!(preset.name, "dev");
/// assert!(preset.enabled_skills.contains(&"commit".to_string()));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillPreset {
    /// Preset name (e.g., "dev", "review", "test").
    pub name: String,
    /// Description of the preset.
    pub description: String,
    /// List of enabled skill names.
    pub enabled_skills: Vec<String>,
}

impl SkillPreset {
    /// Create a new empty preset.
    ///
    /// # Arguments
    ///
    /// * `name` - Preset name
    /// * `description` - Human-readable description
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::SkillPreset;
    ///
    /// let preset = SkillPreset::new("custom".to_string(), "Custom preset".to_string());
    /// assert!(preset.enabled_skills.is_empty());
    /// ```
    pub fn new(name: String, description: String) -> Self {
        Self {
            name,
            description,
            enabled_skills: Vec::new(),
        }
    }

    /// Create the default "dev" preset.
    ///
    /// The dev preset enables common development skills like
    /// commit, edit, and create.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::SkillPreset;
    ///
    /// let preset = SkillPreset::dev_preset();
    /// assert_eq!(preset.name, "dev");
    /// assert!(preset.enabled_skills.contains(&"commit".to_string()));
    /// ```
    pub fn dev_preset() -> Self {
        Self {
            name: "dev".to_string(),
            description: "Development mode - all coding skills enabled".to_string(),
            enabled_skills: vec![
                "commit".to_string(),
                "edit".to_string(),
                "create".to_string(),
            ],
        }
    }

    /// Create the default "review" preset.
    ///
    /// The review preset enables read-only skills suitable for
    /// code review workflows.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::SkillPreset;
    ///
    /// let preset = SkillPreset::review_preset();
    /// assert_eq!(preset.name, "review");
    /// assert!(preset.enabled_skills.contains(&"review-pr".to_string()));
    /// ```
    pub fn review_preset() -> Self {
        Self {
            name: "review".to_string(),
            description: "Code review mode - read-only skills".to_string(),
            enabled_skills: vec!["review-pr".to_string(), "explain".to_string()],
        }
    }

    /// Create the default "minimal" preset.
    ///
    /// The minimal preset has no skills enabled, useful for
    /// conserving token budget.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::SkillPreset;
    ///
    /// let preset = SkillPreset::minimal_preset();
    /// assert_eq!(preset.name, "minimal");
    /// assert!(preset.enabled_skills.is_empty());
    /// ```
    pub fn minimal_preset() -> Self {
        Self {
            name: "minimal".to_string(),
            description: "Minimal mode - no skills enabled".to_string(),
            enabled_skills: Vec::new(),
        }
    }

    /// Add a skill to the preset.
    ///
    /// Duplicates are ignored.
    ///
    /// # Arguments
    ///
    /// * `name` - Skill name to add
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::SkillPreset;
    ///
    /// let mut preset = SkillPreset::new("custom".to_string(), "Custom".to_string());
    /// preset.add_skill("commit".to_string());
    /// preset.add_skill("commit".to_string()); // Duplicate ignored
    /// assert_eq!(preset.enabled_skills.len(), 1);
    /// ```
    pub fn add_skill(&mut self, name: String) {
        if !self.enabled_skills.contains(&name) {
            self.enabled_skills.push(name);
        }
    }

    /// Remove a skill from the preset.
    ///
    /// Does nothing if the skill is not in the preset.
    ///
    /// # Arguments
    ///
    /// * `name` - Skill name to remove
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::SkillPreset;
    ///
    /// let mut preset = SkillPreset::dev_preset();
    /// preset.remove_skill("commit");
    /// assert!(!preset.enabled_skills.contains(&"commit".to_string()));
    /// ```
    pub fn remove_skill(&mut self, name: &str) {
        self.enabled_skills.retain(|s| s != name);
    }

    /// Check if a skill is enabled in this preset.
    ///
    /// # Arguments
    ///
    /// * `name` - Skill name to check
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::skill::SkillPreset;
    ///
    /// let preset = SkillPreset::dev_preset();
    /// assert!(preset.has_skill("commit"));
    /// assert!(!preset.has_skill("review-pr"));
    /// ```
    pub fn has_skill(&self, name: &str) -> bool {
        self.enabled_skills.iter().any(|s| s == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Skill tests
    #[test]
    fn test_skill_new_user() {
        let skill = Skill::new_user(
            "my-skill".to_string(),
            "My custom skill".to_string(),
            PathBuf::from(".claude/commands/my-skill.md"),
        );
        assert_eq!(skill.name, "my-skill");
        assert_eq!(skill.description, "My custom skill");
        assert_eq!(
            skill.source_path,
            PathBuf::from(".claude/commands/my-skill.md")
        );
        assert_eq!(skill.skill_type, SkillType::UserDefined);
        assert!(skill.enabled);
        assert_eq!(skill.token_count, 0);
        assert!(skill.tags.is_empty());
    }

    #[test]
    fn test_skill_new_builtin() {
        let skill = Skill::new_builtin("commit".to_string(), "Create a git commit".to_string());
        assert_eq!(skill.name, "commit");
        assert_eq!(skill.skill_type, SkillType::BuiltIn);
        assert!(skill.tags.contains(&"builtin".to_string()));
        assert!(skill.source_path.as_os_str().is_empty());
    }

    #[test]
    fn test_skill_new_mcp() {
        let skill = Skill::new_mcp(
            "github-search".to_string(),
            "Search GitHub".to_string(),
            "github-mcp".to_string(),
        );
        assert_eq!(skill.name, "github-search");
        assert_eq!(skill.skill_type, SkillType::Mcp);
        assert!(skill.tags.contains(&"mcp".to_string()));
        assert!(skill.tags.contains(&"github-mcp".to_string()));
    }

    #[test]
    fn test_skill_with_token_count() {
        let skill =
            Skill::new_builtin("test".to_string(), "Test".to_string()).with_token_count(500);
        assert_eq!(skill.token_count, 500);
    }

    #[test]
    fn test_skill_with_tag() {
        let skill = Skill::new_builtin("test".to_string(), "Test".to_string())
            .with_tag("git".to_string())
            .with_tag("vcs".to_string())
            .with_tag("git".to_string()); // Duplicate
        assert!(skill.tags.contains(&"git".to_string()));
        assert!(skill.tags.contains(&"vcs".to_string()));
        // "builtin" + "git" + "vcs" = 3 (duplicate ignored)
        assert_eq!(skill.tags.len(), 3);
    }

    #[test]
    fn test_skill_matches_query_name() {
        let skill = Skill::new_builtin("commit".to_string(), "Create a git commit".to_string());
        assert!(skill.matches_query("commit"));
        assert!(skill.matches_query("COMMIT"));
        assert!(skill.matches_query("comm"));
    }

    #[test]
    fn test_skill_matches_query_description() {
        let skill = Skill::new_builtin("commit".to_string(), "Create a git commit".to_string());
        assert!(skill.matches_query("git"));
        assert!(skill.matches_query("create"));
    }

    #[test]
    fn test_skill_matches_query_tags() {
        let skill = Skill::new_builtin("commit".to_string(), "Create a commit".to_string())
            .with_tag("version-control".to_string());
        assert!(skill.matches_query("version"));
        assert!(skill.matches_query("builtin"));
    }

    #[test]
    fn test_skill_matches_query_no_match() {
        let skill = Skill::new_builtin("commit".to_string(), "Create a commit".to_string());
        assert!(!skill.matches_query("deploy"));
        assert!(!skill.matches_query("test"));
    }

    #[test]
    fn test_skill_serialization() {
        let skill = Skill::new_user(
            "test".to_string(),
            "Test skill".to_string(),
            PathBuf::from("/path/to/skill"),
        );
        let json = serde_json::to_string(&skill).unwrap();
        let parsed: Skill = serde_json::from_str(&json).unwrap();
        assert_eq!(skill, parsed);
    }

    #[test]
    fn test_skill_clone() {
        let skill = Skill::new_builtin("test".to_string(), "Test".to_string());
        let cloned = skill.clone();
        assert_eq!(skill, cloned);
    }

    #[test]
    fn test_skill_debug() {
        let skill = Skill::new_builtin("test".to_string(), "Test".to_string());
        let debug_str = format!("{:?}", skill);
        assert!(debug_str.contains("Skill"));
        assert!(debug_str.contains("test"));
    }

    // SkillType tests
    #[test]
    fn test_skill_type_default() {
        assert_eq!(SkillType::default(), SkillType::BuiltIn);
    }

    #[test]
    fn test_skill_type_equality() {
        assert_eq!(SkillType::BuiltIn, SkillType::BuiltIn);
        assert_ne!(SkillType::BuiltIn, SkillType::UserDefined);
        assert_ne!(SkillType::UserDefined, SkillType::Mcp);
    }

    #[test]
    fn test_skill_type_serialization() {
        let types = [SkillType::BuiltIn, SkillType::UserDefined, SkillType::Mcp];
        for skill_type in types {
            let json = serde_json::to_string(&skill_type).unwrap();
            let parsed: SkillType = serde_json::from_str(&json).unwrap();
            assert_eq!(skill_type, parsed);
        }
    }

    // TokenBudget tests
    #[test]
    fn test_token_budget_default() {
        let budget = TokenBudget::default();
        assert_eq!(budget.max_tokens, 15000);
        assert_eq!(budget.warning_threshold, 12000);
        assert_eq!(budget.used_tokens, 0);
    }

    #[test]
    fn test_token_budget_default_claude() {
        let budget = TokenBudget::default_claude();
        assert_eq!(budget.max_tokens, 15000);
        assert_eq!(budget.warning_threshold, 12000);
        assert_eq!(budget.used_tokens, 0);
    }

    #[test]
    fn test_token_budget_new() {
        let budget = TokenBudget::new(10000, 8000);
        assert_eq!(budget.max_tokens, 10000);
        assert_eq!(budget.warning_threshold, 8000);
        assert_eq!(budget.used_tokens, 0);
    }

    #[test]
    fn test_token_budget_add() {
        let mut budget = TokenBudget::default();
        budget.add(5000);
        assert_eq!(budget.used_tokens, 5000);
        budget.add(3000);
        assert_eq!(budget.used_tokens, 8000);
    }

    #[test]
    fn test_token_budget_remove() {
        let mut budget = TokenBudget::default();
        budget.add(5000);
        budget.remove(2000);
        assert_eq!(budget.used_tokens, 3000);
    }

    #[test]
    fn test_token_budget_reset() {
        let mut budget = TokenBudget::default();
        budget.add(5000);
        budget.reset();
        assert_eq!(budget.used_tokens, 0);
    }

    #[test]
    fn test_token_budget_is_exceeded() {
        let mut budget = TokenBudget::default();
        assert!(!budget.is_exceeded());

        budget.used_tokens = 15000;
        assert!(!budget.is_exceeded());

        budget.used_tokens = 15001;
        assert!(budget.is_exceeded());
    }

    #[test]
    fn test_token_budget_is_warning() {
        let mut budget = TokenBudget::default();
        assert!(!budget.is_warning());

        budget.used_tokens = 11999;
        assert!(!budget.is_warning());

        budget.used_tokens = 12000;
        assert!(budget.is_warning());

        budget.used_tokens = 15000;
        assert!(budget.is_warning());

        budget.used_tokens = 15001;
        assert!(!budget.is_warning()); // Exceeded, not warning
    }

    #[test]
    fn test_token_budget_remaining() {
        let mut budget = TokenBudget::default();
        assert_eq!(budget.remaining(), 15000);

        budget.add(10000);
        assert_eq!(budget.remaining(), 5000);

        budget.add(10000); // 20000 used, max 15000
        assert_eq!(budget.remaining(), 0); // Saturates at 0
    }

    #[test]
    fn test_token_budget_usage_percent() {
        let mut budget = TokenBudget::default();
        assert_eq!(budget.usage_percent(), 0.0);

        budget.used_tokens = 7500;
        assert_eq!(budget.usage_percent(), 0.5);

        budget.used_tokens = 15000;
        assert_eq!(budget.usage_percent(), 1.0);

        budget.used_tokens = 30000;
        assert_eq!(budget.usage_percent(), 2.0); // Can exceed 1.0
    }

    #[test]
    fn test_token_budget_usage_percent_zero_max() {
        let budget = TokenBudget::new(0, 0);
        assert_eq!(budget.usage_percent(), 0.0); // Avoid division by zero
    }

    #[test]
    fn test_token_budget_saturating_add() {
        let mut budget = TokenBudget::default();
        budget.add(u32::MAX);
        budget.add(1000);
        assert_eq!(budget.used_tokens, u32::MAX);
    }

    #[test]
    fn test_token_budget_saturating_remove() {
        let mut budget = TokenBudget::default();
        budget.remove(1000); // Can't go below 0
        assert_eq!(budget.used_tokens, 0);
    }

    #[test]
    fn test_token_budget_serialization() {
        let mut budget = TokenBudget::default();
        budget.add(5000);
        let json = serde_json::to_string(&budget).unwrap();
        let parsed: TokenBudget = serde_json::from_str(&json).unwrap();
        assert_eq!(budget, parsed);
    }

    #[test]
    fn test_token_budget_equality() {
        let budget1 = TokenBudget::default();
        let budget2 = TokenBudget::default();
        let budget3 = TokenBudget::new(10000, 8000);
        assert_eq!(budget1, budget2);
        assert_ne!(budget1, budget3);
    }

    // SkillPreset tests
    #[test]
    fn test_skill_preset_new() {
        let preset = SkillPreset::new("custom".to_string(), "Custom preset".to_string());
        assert_eq!(preset.name, "custom");
        assert_eq!(preset.description, "Custom preset");
        assert!(preset.enabled_skills.is_empty());
    }

    #[test]
    fn test_skill_preset_dev() {
        let preset = SkillPreset::dev_preset();
        assert_eq!(preset.name, "dev");
        assert!(preset.enabled_skills.contains(&"commit".to_string()));
        assert!(preset.enabled_skills.contains(&"edit".to_string()));
        assert!(preset.enabled_skills.contains(&"create".to_string()));
    }

    #[test]
    fn test_skill_preset_review() {
        let preset = SkillPreset::review_preset();
        assert_eq!(preset.name, "review");
        assert!(preset.enabled_skills.contains(&"review-pr".to_string()));
        assert!(preset.enabled_skills.contains(&"explain".to_string()));
    }

    #[test]
    fn test_skill_preset_minimal() {
        let preset = SkillPreset::minimal_preset();
        assert_eq!(preset.name, "minimal");
        assert!(preset.enabled_skills.is_empty());
    }

    #[test]
    fn test_skill_preset_add_skill() {
        let mut preset = SkillPreset::new("test".to_string(), "Test".to_string());
        preset.add_skill("skill1".to_string());
        preset.add_skill("skill2".to_string());
        preset.add_skill("skill1".to_string()); // Duplicate ignored
        assert_eq!(preset.enabled_skills.len(), 2);
        assert!(preset.enabled_skills.contains(&"skill1".to_string()));
        assert!(preset.enabled_skills.contains(&"skill2".to_string()));
    }

    #[test]
    fn test_skill_preset_remove_skill() {
        let mut preset = SkillPreset::dev_preset();
        preset.remove_skill("commit");
        assert!(!preset.enabled_skills.contains(&"commit".to_string()));
        assert!(preset.enabled_skills.contains(&"edit".to_string()));

        // Remove non-existent skill (no-op)
        preset.remove_skill("nonexistent");
    }

    #[test]
    fn test_skill_preset_has_skill() {
        let preset = SkillPreset::dev_preset();
        assert!(preset.has_skill("commit"));
        assert!(preset.has_skill("edit"));
        assert!(!preset.has_skill("review-pr"));
    }

    #[test]
    fn test_skill_preset_serialization() {
        let preset = SkillPreset::dev_preset();
        let json = serde_json::to_string(&preset).unwrap();
        let parsed: SkillPreset = serde_json::from_str(&json).unwrap();
        assert_eq!(preset, parsed);
    }

    #[test]
    fn test_skill_preset_equality() {
        let preset1 = SkillPreset::dev_preset();
        let preset2 = SkillPreset::dev_preset();
        let preset3 = SkillPreset::review_preset();
        assert_eq!(preset1, preset2);
        assert_ne!(preset1, preset3);
    }

    #[test]
    fn test_skill_preset_clone() {
        let preset = SkillPreset::dev_preset();
        let cloned = preset.clone();
        assert_eq!(preset, cloned);
    }

    #[test]
    fn test_skill_preset_debug() {
        let preset = SkillPreset::dev_preset();
        let debug_str = format!("{:?}", preset);
        assert!(debug_str.contains("SkillPreset"));
        assert!(debug_str.contains("dev"));
    }
}
