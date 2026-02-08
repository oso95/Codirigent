//! Skill manager trait for managing Claude Code skills.
//!
//! This module defines the [`SkillManager`] trait which provides the contract
//! for skill management operations including enabling/disabling skills,
//! token budget tracking, and preset application.

use crate::skill::{Skill, SkillPreset, TokenBudget};
use anyhow::Result;

/// Skill manager service trait.
///
/// Manages Claude Code skills and slash commands, including
/// token budget tracking and skill presets.
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync` to allow sharing across threads.
///
/// # Example
///
/// ```ignore
/// use codirigent_core::traits::SkillManager;
///
/// fn use_skill_manager(manager: &dyn SkillManager) {
///     // List all skills
///     for skill in manager.list_skills() {
///         println!("{}: {} (enabled: {})", skill.name, skill.description, skill.enabled);
///     }
///
///     // Check token budget
///     let budget = manager.token_budget();
///     println!("Token usage: {}/{}", budget.used_tokens, budget.max_tokens);
/// }
/// ```
pub trait SkillManager: Send + Sync {
    /// List all available skills.
    ///
    /// Returns a slice of all skills known to the manager,
    /// including built-in, user-defined, and MCP skills.
    fn list_skills(&self) -> &[Skill];

    /// Get a specific skill by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The skill name to look up
    ///
    /// # Returns
    ///
    /// A reference to the skill if found, `None` otherwise.
    fn get_skill(&self, name: &str) -> Option<&Skill>;

    /// Get a mutable reference to a skill.
    ///
    /// # Arguments
    ///
    /// * `name` - The skill name to look up
    ///
    /// # Returns
    ///
    /// A mutable reference to the skill if found, `None` otherwise.
    fn get_skill_mut(&mut self, name: &str) -> Option<&mut Skill>;

    /// Enable a skill by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The skill name to enable
    ///
    /// # Errors
    ///
    /// Returns an error if the skill is not found.
    fn enable_skill(&mut self, name: &str) -> Result<()>;

    /// Disable a skill by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The skill name to disable
    ///
    /// # Errors
    ///
    /// Returns an error if the skill is not found.
    fn disable_skill(&mut self, name: &str) -> Result<()>;

    /// Get current token budget.
    ///
    /// Returns the current token budget state, including
    /// used tokens, max tokens, and warning threshold.
    fn token_budget(&self) -> TokenBudget;

    /// Recalculate token budget based on enabled skills.
    ///
    /// This should be called after enabling/disabling skills
    /// to update the used token count.
    fn recalculate_budget(&mut self);

    /// List all available presets.
    ///
    /// Returns a slice of all skill presets known to the manager.
    fn list_presets(&self) -> &[SkillPreset];

    /// Apply a preset (enable/disable skills accordingly).
    ///
    /// This will enable skills listed in the preset and disable
    /// all other skills.
    ///
    /// # Arguments
    ///
    /// * `preset_name` - The name of the preset to apply
    ///
    /// # Errors
    ///
    /// Returns an error if the preset is not found.
    fn apply_preset(&mut self, preset_name: &str) -> Result<()>;

    /// Search skills by name or description (case-insensitive).
    ///
    /// # Arguments
    ///
    /// * `query` - The search query string
    ///
    /// # Returns
    ///
    /// A vector of references to matching skills.
    fn search(&self, query: &str) -> Vec<&Skill>;

    /// Refresh skills from disk (re-scan directories).
    ///
    /// This will scan the `.claude/commands` directory for
    /// user-defined skills and update the skill list.
    ///
    /// # Errors
    ///
    /// Returns an error if scanning fails.
    fn refresh(&mut self) -> Result<()>;

    /// Get count of enabled skills.
    ///
    /// This is a convenience method with a default implementation.
    fn enabled_count(&self) -> usize {
        self.list_skills().iter().filter(|s| s.enabled).count()
    }

    /// Get total token count for enabled skills.
    ///
    /// This is a convenience method with a default implementation.
    fn enabled_token_count(&self) -> u32 {
        self.list_skills()
            .iter()
            .filter(|s| s.enabled)
            .map(|s| s.token_count)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // Mock implementation for testing
    struct MockSkillManager {
        skills: Vec<Skill>,
        presets: Vec<SkillPreset>,
        budget: TokenBudget,
    }

    impl MockSkillManager {
        fn new() -> Self {
            Self {
                skills: vec![
                    Skill::new_builtin("commit".to_string(), "Create git commit".to_string())
                        .with_token_count(500),
                    Skill::new_builtin("edit".to_string(), "Edit files".to_string())
                        .with_token_count(300),
                    Skill::new_user(
                        "custom".to_string(),
                        "Custom skill".to_string(),
                        PathBuf::from(".claude/commands/custom.md"),
                    )
                    .with_token_count(200),
                ],
                presets: vec![SkillPreset::dev_preset(), SkillPreset::review_preset()],
                budget: TokenBudget::default_claude(),
            }
        }
    }

    impl SkillManager for MockSkillManager {
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
            if let Some(skill) = self.get_skill_mut(name) {
                skill.enabled = true;
                self.recalculate_budget();
                Ok(())
            } else {
                anyhow::bail!("Skill not found: {}", name)
            }
        }

        fn disable_skill(&mut self, name: &str) -> Result<()> {
            if let Some(skill) = self.get_skill_mut(name) {
                skill.enabled = false;
                self.recalculate_budget();
                Ok(())
            } else {
                anyhow::bail!("Skill not found: {}", name)
            }
        }

        fn token_budget(&self) -> TokenBudget {
            self.budget
        }

        fn recalculate_budget(&mut self) {
            self.budget.reset();
            self.budget.add(self.enabled_token_count());
        }

        fn list_presets(&self) -> &[SkillPreset] {
            &self.presets
        }

        fn apply_preset(&mut self, preset_name: &str) -> Result<()> {
            let preset = self
                .presets
                .iter()
                .find(|p| p.name == preset_name)
                .ok_or_else(|| anyhow::anyhow!("Preset not found: {}", preset_name))?
                .clone();

            for skill in &mut self.skills {
                skill.enabled = preset.enabled_skills.contains(&skill.name);
            }
            self.recalculate_budget();
            Ok(())
        }

        fn search(&self, query: &str) -> Vec<&Skill> {
            self.skills
                .iter()
                .filter(|s| s.matches_query(query))
                .collect()
        }

        fn refresh(&mut self) -> Result<()> {
            // Mock: do nothing
            Ok(())
        }
    }

    #[test]
    fn test_skill_manager_trait_is_object_safe() {
        // This compiles only if SkillManager is object-safe
        fn _takes_skill_manager(_: &dyn SkillManager) {}
    }

    #[test]
    fn test_mock_skill_manager_list_skills() {
        let manager = MockSkillManager::new();
        assert_eq!(manager.list_skills().len(), 3);
    }

    #[test]
    fn test_mock_skill_manager_get_skill() {
        let manager = MockSkillManager::new();
        assert!(manager.get_skill("commit").is_some());
        assert!(manager.get_skill("nonexistent").is_none());
    }

    #[test]
    fn test_mock_skill_manager_enable_disable() {
        let mut manager = MockSkillManager::new();

        // Disable a skill
        manager.disable_skill("commit").unwrap();
        assert!(!manager.get_skill("commit").unwrap().enabled);

        // Enable it again
        manager.enable_skill("commit").unwrap();
        assert!(manager.get_skill("commit").unwrap().enabled);

        // Try to enable non-existent skill
        assert!(manager.enable_skill("nonexistent").is_err());
    }

    #[test]
    fn test_mock_skill_manager_token_budget() {
        let mut manager = MockSkillManager::new();
        manager.recalculate_budget();

        // All skills enabled: 500 + 300 + 200 = 1000
        let budget = manager.token_budget();
        assert_eq!(budget.used_tokens, 1000);

        // Disable one skill
        manager.disable_skill("commit").unwrap();
        let budget = manager.token_budget();
        assert_eq!(budget.used_tokens, 500); // 300 + 200
    }

    #[test]
    fn test_mock_skill_manager_apply_preset() {
        let mut manager = MockSkillManager::new();

        // Apply review preset (should enable review-pr and explain, but those don't exist)
        // In our mock, this will disable all skills since none match the preset
        manager.apply_preset("review").unwrap();

        // None of our mock skills are in the review preset
        assert!(!manager.get_skill("commit").unwrap().enabled);
        assert!(!manager.get_skill("edit").unwrap().enabled);

        // Apply dev preset
        manager.apply_preset("dev").unwrap();
        assert!(manager.get_skill("commit").unwrap().enabled);
        assert!(manager.get_skill("edit").unwrap().enabled);

        // Invalid preset
        assert!(manager.apply_preset("nonexistent").is_err());
    }

    #[test]
    fn test_mock_skill_manager_search() {
        let manager = MockSkillManager::new();

        let results = manager.search("commit");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "commit");

        let results = manager.search("git");
        assert_eq!(results.len(), 1); // "commit" has "git" in description

        let results = manager.search("custom");
        assert_eq!(results.len(), 1);

        let results = manager.search("builtin");
        assert_eq!(results.len(), 2); // commit and edit have "builtin" tag
    }

    #[test]
    fn test_mock_skill_manager_enabled_count() {
        let mut manager = MockSkillManager::new();
        assert_eq!(manager.enabled_count(), 3);

        manager.disable_skill("commit").unwrap();
        assert_eq!(manager.enabled_count(), 2);
    }

    #[test]
    fn test_mock_skill_manager_enabled_token_count() {
        let mut manager = MockSkillManager::new();
        assert_eq!(manager.enabled_token_count(), 1000); // 500 + 300 + 200

        manager.disable_skill("commit").unwrap();
        assert_eq!(manager.enabled_token_count(), 500); // 300 + 200
    }

    #[test]
    fn test_mock_skill_manager_list_presets() {
        let manager = MockSkillManager::new();
        assert_eq!(manager.list_presets().len(), 2);
    }

    #[test]
    fn test_mock_skill_manager_refresh() {
        let mut manager = MockSkillManager::new();
        assert!(manager.refresh().is_ok());
    }
}
