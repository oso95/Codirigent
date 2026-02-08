//! Broadcast messaging types.
//!
//! Types for sending messages to multiple sessions simultaneously,
//! with template variable support and delivery tracking.
//!
//! # Overview
//!
//! The broadcast module enables sending messages to multiple AI coding sessions
//! at once. This is useful for:
//!
//! - Notifying all sessions about API changes or breaking changes
//! - Sharing context updates across sessions
//! - Coordinating work between multiple AI agents
//!
//! # Template Variables
//!
//! Messages can include template variables that are expanded before sending:
//!
//! - `$SESSION_NAME` - The name of the target session
//! - `$WORKTREE` - The worktree path (if bound)
//! - `$PROJECT` - The project name
//! - Custom variables via the `custom` HashMap
//!
//! # Example
//!
//! ```
//! use codirigent_core::broadcast::{BroadcastId, BroadcastMessage, BroadcastVariables};
//! use codirigent_core::SessionId;
//!
//! // Create a broadcast message
//! let targets = vec![SessionId(1), SessionId(2)];
//! let msg = BroadcastMessage::new(
//!     BroadcastId(1),
//!     "API has changed, please update your code".to_string(),
//!     targets,
//! );
//!
//! assert_eq!(msg.targets.len(), 2);
//! assert_eq!(msg.pending_count(), 2);
//!
//! // Use template variables
//! let mut vars = BroadcastVariables::default();
//! vars.project = Some("my-app".to_string());
//! let expanded = vars.expand("Working on $PROJECT");
//! assert_eq!(expanded, "Working on my-app");
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

use crate::types::SessionId;

/// Unique identifier for a broadcast message.
///
/// Each broadcast message has a unique ID for tracking delivery status
/// and retry operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BroadcastId(pub u64);

impl fmt::Display for BroadcastId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "broadcast-{}", self.0)
    }
}

/// Priority level for broadcast messages.
///
/// Higher priority messages may be processed first or displayed more prominently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BroadcastPriority {
    /// Low priority - informational messages.
    Low,
    /// Normal priority (default).
    #[default]
    Normal,
    /// High priority - important updates.
    High,
    /// Critical priority - urgent messages requiring immediate attention.
    Critical,
}

impl fmt::Display for BroadcastPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BroadcastPriority::Low => write!(f, "low"),
            BroadcastPriority::Normal => write!(f, "normal"),
            BroadcastPriority::High => write!(f, "high"),
            BroadcastPriority::Critical => write!(f, "critical"),
        }
    }
}

/// A broadcast message to be sent to multiple sessions.
///
/// Broadcast messages allow sending the same content to multiple AI sessions
/// simultaneously. Each message tracks delivery status for each target session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastMessage {
    /// Unique message ID.
    pub id: BroadcastId,
    /// Original template content.
    pub template: String,
    /// Expanded content (after variable substitution).
    pub content: String,
    /// Priority level.
    pub priority: BroadcastPriority,
    /// Target session IDs (empty = all sessions).
    pub targets: Vec<SessionId>,
    /// When the message was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Delivery status for each target.
    pub delivery_status: Vec<DeliveryStatus>,
}

impl BroadcastMessage {
    /// Create a new broadcast message.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique broadcast ID
    /// * `content` - Message content (used as both template and content)
    /// * `targets` - List of session IDs to send to
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::broadcast::{BroadcastId, BroadcastMessage};
    /// use codirigent_core::SessionId;
    ///
    /// let msg = BroadcastMessage::new(
    ///     BroadcastId(1),
    ///     "Hello".to_string(),
    ///     vec![SessionId(1), SessionId(2)],
    /// );
    /// assert_eq!(msg.targets.len(), 2);
    /// ```
    pub fn new(id: BroadcastId, content: String, targets: Vec<SessionId>) -> Self {
        Self {
            id,
            template: content.clone(),
            content,
            priority: BroadcastPriority::default(),
            targets: targets.clone(),
            created_at: chrono::Utc::now(),
            delivery_status: targets
                .into_iter()
                .map(|session_id| DeliveryStatus {
                    session_id,
                    delivered: false,
                    error: None,
                    delivered_at: None,
                })
                .collect(),
        }
    }

    /// Create a new broadcast message with priority.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique broadcast ID
    /// * `content` - Message content
    /// * `targets` - List of session IDs to send to
    /// * `priority` - Message priority level
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::broadcast::{BroadcastId, BroadcastMessage, BroadcastPriority};
    /// use codirigent_core::SessionId;
    ///
    /// let msg = BroadcastMessage::with_priority(
    ///     BroadcastId(1),
    ///     "Urgent update".to_string(),
    ///     vec![SessionId(1)],
    ///     BroadcastPriority::Critical,
    /// );
    /// assert_eq!(msg.priority, BroadcastPriority::Critical);
    /// ```
    pub fn with_priority(
        id: BroadcastId,
        content: String,
        targets: Vec<SessionId>,
        priority: BroadcastPriority,
    ) -> Self {
        let mut msg = Self::new(id, content, targets);
        msg.priority = priority;
        msg
    }

    /// Get the number of successful deliveries.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::broadcast::{BroadcastId, BroadcastMessage};
    /// use codirigent_core::SessionId;
    ///
    /// let msg = BroadcastMessage::new(
    ///     BroadcastId(1),
    ///     "Test".to_string(),
    ///     vec![SessionId(1)],
    /// );
    /// assert_eq!(msg.success_count(), 0);
    /// ```
    pub fn success_count(&self) -> usize {
        self.delivery_status.iter().filter(|s| s.delivered).count()
    }

    /// Get the number of failed deliveries.
    ///
    /// A failed delivery is one that has an error message but was not delivered.
    pub fn failure_count(&self) -> usize {
        self.delivery_status
            .iter()
            .filter(|s| !s.delivered && s.error.is_some())
            .count()
    }

    /// Get the number of pending deliveries.
    ///
    /// Pending deliveries are those that have not been delivered and have no error.
    pub fn pending_count(&self) -> usize {
        self.delivery_status
            .iter()
            .filter(|s| !s.delivered && s.error.is_none())
            .count()
    }

    /// Check if all deliveries are complete (either successful or failed).
    ///
    /// Returns `true` when there are no pending deliveries.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::broadcast::{BroadcastId, BroadcastMessage};
    /// use codirigent_core::SessionId;
    ///
    /// let mut msg = BroadcastMessage::new(
    ///     BroadcastId(1),
    ///     "Test".to_string(),
    ///     vec![SessionId(1)],
    /// );
    /// assert!(!msg.is_complete());
    ///
    /// msg.delivery_status[0].mark_delivered();
    /// assert!(msg.is_complete());
    /// ```
    pub fn is_complete(&self) -> bool {
        self.pending_count() == 0
    }

    /// Get failed session IDs for retry.
    ///
    /// Returns a list of session IDs that failed delivery.
    pub fn failed_targets(&self) -> Vec<SessionId> {
        self.delivery_status
            .iter()
            .filter(|s| !s.delivered && s.error.is_some())
            .map(|s| s.session_id)
            .collect()
    }
}

/// Template variables available for broadcast messages.
///
/// Variables are expanded in message templates using `$VARIABLE_NAME` syntax.
/// Custom variables can be added via the `custom` HashMap.
///
/// # Example
///
/// ```
/// use codirigent_core::broadcast::BroadcastVariables;
/// use std::collections::HashMap;
///
/// let vars = BroadcastVariables {
///     session_name: Some("Session 1".to_string()),
///     project: Some("my-app".to_string()),
///     worktree: None,
///     custom: HashMap::new(),
/// };
///
/// let template = "Working on $PROJECT in $SESSION_NAME";
/// let expanded = vars.expand(template);
/// assert_eq!(expanded, "Working on my-app in Session 1");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BroadcastVariables {
    /// Session name placeholder: `$SESSION_NAME`
    pub session_name: Option<String>,
    /// Worktree path placeholder: `$WORKTREE`
    pub worktree: Option<String>,
    /// Project name placeholder: `$PROJECT`
    pub project: Option<String>,
    /// Custom variables (key without `$`).
    pub custom: HashMap<String, String>,
}

impl BroadcastVariables {
    /// Create a new empty variables set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create variables with session name.
    pub fn with_session_name(mut self, name: String) -> Self {
        self.session_name = Some(name);
        self
    }

    /// Create variables with worktree path.
    pub fn with_worktree(mut self, path: String) -> Self {
        self.worktree = Some(path);
        self
    }

    /// Create variables with project name.
    pub fn with_project(mut self, name: String) -> Self {
        self.project = Some(name);
        self
    }

    /// Add a custom variable.
    pub fn with_custom(mut self, key: String, value: String) -> Self {
        self.custom.insert(key, value);
        self
    }

    /// Expand variables in a template string.
    ///
    /// Replaces all occurrences of:
    /// - `$SESSION_NAME` with the session name
    /// - `$WORKTREE` with the worktree path
    /// - `$PROJECT` with the project name
    /// - `$<KEY>` with custom variable values
    ///
    /// Variables that are `None` are left as-is in the template.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::broadcast::BroadcastVariables;
    ///
    /// let vars = BroadcastVariables::new()
    ///     .with_project("my-app".to_string())
    ///     .with_custom("BRANCH".to_string(), "main".to_string());
    ///
    /// let expanded = vars.expand("Project: $PROJECT, Branch: $BRANCH");
    /// assert_eq!(expanded, "Project: my-app, Branch: main");
    /// ```
    pub fn expand(&self, template: &str) -> String {
        let mut result = template.to_string();

        if let Some(ref name) = self.session_name {
            result = result.replace("$SESSION_NAME", name);
        }
        if let Some(ref worktree) = self.worktree {
            result = result.replace("$WORKTREE", worktree);
        }
        if let Some(ref project) = self.project {
            result = result.replace("$PROJECT", project);
        }

        for (key, value) in &self.custom {
            result = result.replace(&format!("${}", key), value);
        }

        result
    }

    /// Check if any variables are set.
    pub fn is_empty(&self) -> bool {
        self.session_name.is_none()
            && self.worktree.is_none()
            && self.project.is_none()
            && self.custom.is_empty()
    }
}

/// Delivery status for a single target session.
///
/// Tracks whether a message was successfully delivered to a session,
/// along with error information if delivery failed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryStatus {
    /// Target session ID.
    pub session_id: SessionId,
    /// Whether delivery succeeded.
    pub delivered: bool,
    /// Error message if failed.
    pub error: Option<String>,
    /// When delivered.
    pub delivered_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl DeliveryStatus {
    /// Create a new pending delivery status.
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            delivered: false,
            error: None,
            delivered_at: None,
        }
    }

    /// Mark as successfully delivered.
    ///
    /// Sets `delivered` to `true`, records the delivery time, and clears any error.
    pub fn mark_delivered(&mut self) {
        self.delivered = true;
        self.delivered_at = Some(chrono::Utc::now());
        self.error = None;
    }

    /// Mark as failed with an error message.
    ///
    /// Sets `delivered` to `false` and records the error.
    pub fn mark_failed(&mut self, error: String) {
        self.delivered = false;
        self.error = Some(error);
    }

    /// Check if this delivery is pending (not delivered and no error).
    pub fn is_pending(&self) -> bool {
        !self.delivered && self.error.is_none()
    }

    /// Check if this delivery failed.
    pub fn is_failed(&self) -> bool {
        !self.delivered && self.error.is_some()
    }
}

/// Broadcast history entry.
///
/// Wraps a completed broadcast message with a summary of delivery results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastHistoryEntry {
    /// The broadcast message.
    pub message: BroadcastMessage,
    /// Summary of delivery results.
    pub summary: String,
}

impl BroadcastHistoryEntry {
    /// Create from a completed broadcast message.
    ///
    /// Generates a summary string describing the delivery results.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::broadcast::{BroadcastId, BroadcastMessage, BroadcastHistoryEntry};
    /// use codirigent_core::SessionId;
    ///
    /// let mut msg = BroadcastMessage::new(
    ///     BroadcastId(1),
    ///     "Test".to_string(),
    ///     vec![SessionId(1), SessionId(2)],
    /// );
    /// msg.delivery_status[0].mark_delivered();
    /// msg.delivery_status[1].mark_failed("Connection error".to_string());
    ///
    /// let entry = BroadcastHistoryEntry::from_message(msg);
    /// assert!(entry.summary.contains("1 delivered"));
    /// assert!(entry.summary.contains("1 failed"));
    /// ```
    pub fn from_message(message: BroadcastMessage) -> Self {
        let success = message.success_count();
        let failed = message.failure_count();
        let summary = format!(
            "Sent to {} sessions ({} delivered, {} failed)",
            message.targets.len(),
            success,
            failed
        );

        Self { message, summary }
    }

    /// Get the broadcast ID.
    pub fn id(&self) -> BroadcastId {
        self.message.id
    }

    /// Check if all deliveries succeeded.
    pub fn all_delivered(&self) -> bool {
        self.message.failure_count() == 0 && self.message.is_complete()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // BroadcastId tests
    #[test]
    fn test_broadcast_id_display() {
        let id = BroadcastId(42);
        assert_eq!(format!("{}", id), "broadcast-42");
    }

    #[test]
    fn test_broadcast_id_equality() {
        assert_eq!(BroadcastId(1), BroadcastId(1));
        assert_ne!(BroadcastId(1), BroadcastId(2));
    }

    #[test]
    fn test_broadcast_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(BroadcastId(1));
        assert!(set.contains(&BroadcastId(1)));
        assert!(!set.contains(&BroadcastId(2)));
    }

    #[test]
    fn test_broadcast_id_serialization() {
        let id = BroadcastId(42);
        let json = serde_json::to_string(&id).unwrap();
        let parsed: BroadcastId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_broadcast_id_clone_copy() {
        let id = BroadcastId(42);
        let cloned = id;
        assert_eq!(id, cloned);
    }

    // BroadcastPriority tests
    #[test]
    fn test_broadcast_priority_default() {
        assert_eq!(BroadcastPriority::default(), BroadcastPriority::Normal);
    }

    #[test]
    fn test_broadcast_priority_display() {
        assert_eq!(format!("{}", BroadcastPriority::Low), "low");
        assert_eq!(format!("{}", BroadcastPriority::Normal), "normal");
        assert_eq!(format!("{}", BroadcastPriority::High), "high");
        assert_eq!(format!("{}", BroadcastPriority::Critical), "critical");
    }

    #[test]
    fn test_broadcast_priority_serialization() {
        let priorities = [
            BroadcastPriority::Low,
            BroadcastPriority::Normal,
            BroadcastPriority::High,
            BroadcastPriority::Critical,
        ];
        for priority in priorities {
            let json = serde_json::to_string(&priority).unwrap();
            let parsed: BroadcastPriority = serde_json::from_str(&json).unwrap();
            assert_eq!(priority, parsed);
        }
    }

    #[test]
    fn test_broadcast_priority_equality() {
        assert_eq!(BroadcastPriority::High, BroadcastPriority::High);
        assert_ne!(BroadcastPriority::High, BroadcastPriority::Low);
    }

    // BroadcastMessage tests
    #[test]
    fn test_broadcast_message_new() {
        let targets = vec![SessionId(1), SessionId(2)];
        let msg = BroadcastMessage::new(BroadcastId(1), "Hello".to_string(), targets);

        assert_eq!(msg.id, BroadcastId(1));
        assert_eq!(msg.content, "Hello");
        assert_eq!(msg.template, "Hello");
        assert_eq!(msg.targets.len(), 2);
        assert_eq!(msg.delivery_status.len(), 2);
        assert_eq!(msg.priority, BroadcastPriority::Normal);
        assert_eq!(msg.success_count(), 0);
        assert_eq!(msg.pending_count(), 2);
        assert_eq!(msg.failure_count(), 0);
        assert!(!msg.is_complete());
    }

    #[test]
    fn test_broadcast_message_with_priority() {
        let msg = BroadcastMessage::with_priority(
            BroadcastId(1),
            "Urgent".to_string(),
            vec![SessionId(1)],
            BroadcastPriority::Critical,
        );
        assert_eq!(msg.priority, BroadcastPriority::Critical);
    }

    #[test]
    fn test_broadcast_message_empty_targets() {
        let msg = BroadcastMessage::new(BroadcastId(1), "Hello".to_string(), vec![]);
        assert_eq!(msg.targets.len(), 0);
        assert_eq!(msg.delivery_status.len(), 0);
        assert!(msg.is_complete()); // No pending deliveries
    }

    #[test]
    fn test_broadcast_message_delivery_counts() {
        let targets = vec![SessionId(1), SessionId(2), SessionId(3)];
        let mut msg = BroadcastMessage::new(BroadcastId(1), "Test".to_string(), targets);

        // Initially all pending
        assert_eq!(msg.pending_count(), 3);
        assert_eq!(msg.success_count(), 0);
        assert_eq!(msg.failure_count(), 0);

        // Deliver to first
        msg.delivery_status[0].mark_delivered();
        assert_eq!(msg.pending_count(), 2);
        assert_eq!(msg.success_count(), 1);

        // Fail second
        msg.delivery_status[1].mark_failed("Error".to_string());
        assert_eq!(msg.pending_count(), 1);
        assert_eq!(msg.failure_count(), 1);

        // Deliver third
        msg.delivery_status[2].mark_delivered();
        assert_eq!(msg.pending_count(), 0);
        assert!(msg.is_complete());
    }

    #[test]
    fn test_broadcast_message_failed_targets() {
        let targets = vec![SessionId(1), SessionId(2), SessionId(3)];
        let mut msg = BroadcastMessage::new(BroadcastId(1), "Test".to_string(), targets);

        msg.delivery_status[0].mark_delivered();
        msg.delivery_status[1].mark_failed("Error 1".to_string());
        msg.delivery_status[2].mark_failed("Error 2".to_string());

        let failed = msg.failed_targets();
        assert_eq!(failed.len(), 2);
        assert!(failed.contains(&SessionId(2)));
        assert!(failed.contains(&SessionId(3)));
    }

    #[test]
    fn test_broadcast_message_serialization() {
        let msg = BroadcastMessage::new(
            BroadcastId(1),
            "Test message".to_string(),
            vec![SessionId(1)],
        );
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: BroadcastMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg.id, parsed.id);
        assert_eq!(msg.content, parsed.content);
    }

    #[test]
    fn test_broadcast_message_clone() {
        let msg = BroadcastMessage::new(BroadcastId(1), "Test".to_string(), vec![SessionId(1)]);
        let cloned = msg.clone();
        assert_eq!(msg.id, cloned.id);
        assert_eq!(msg.content, cloned.content);
    }

    // BroadcastVariables tests
    #[test]
    fn test_broadcast_variables_default() {
        let vars = BroadcastVariables::default();
        assert!(vars.session_name.is_none());
        assert!(vars.worktree.is_none());
        assert!(vars.project.is_none());
        assert!(vars.custom.is_empty());
        assert!(vars.is_empty());
    }

    #[test]
    fn test_broadcast_variables_new() {
        let vars = BroadcastVariables::new();
        assert!(vars.is_empty());
    }

    #[test]
    fn test_broadcast_variables_builders() {
        let vars = BroadcastVariables::new()
            .with_session_name("Session 1".to_string())
            .with_worktree("/path/to/worktree".to_string())
            .with_project("my-project".to_string())
            .with_custom("BRANCH".to_string(), "main".to_string());

        assert_eq!(vars.session_name, Some("Session 1".to_string()));
        assert_eq!(vars.worktree, Some("/path/to/worktree".to_string()));
        assert_eq!(vars.project, Some("my-project".to_string()));
        assert_eq!(vars.custom.get("BRANCH"), Some(&"main".to_string()));
        assert!(!vars.is_empty());
    }

    #[test]
    fn test_broadcast_variables_expand_session_name() {
        let vars = BroadcastVariables {
            session_name: Some("Session 1".to_string()),
            ..Default::default()
        };
        let expanded = vars.expand("Hello $SESSION_NAME");
        assert_eq!(expanded, "Hello Session 1");
    }

    #[test]
    fn test_broadcast_variables_expand_worktree() {
        let vars = BroadcastVariables {
            worktree: Some("/repo/feature".to_string()),
            ..Default::default()
        };
        let expanded = vars.expand("Working in $WORKTREE");
        assert_eq!(expanded, "Working in /repo/feature");
    }

    #[test]
    fn test_broadcast_variables_expand_project() {
        let vars = BroadcastVariables {
            project: Some("my-app".to_string()),
            ..Default::default()
        };
        let expanded = vars.expand("Project: $PROJECT");
        assert_eq!(expanded, "Project: my-app");
    }

    #[test]
    fn test_broadcast_variables_expand_custom() {
        let vars = BroadcastVariables {
            custom: HashMap::from([
                ("BRANCH".to_string(), "main".to_string()),
                ("VERSION".to_string(), "1.0.0".to_string()),
            ]),
            ..Default::default()
        };
        let expanded = vars.expand("On $BRANCH at v$VERSION");
        assert_eq!(expanded, "On main at v1.0.0");
    }

    #[test]
    fn test_broadcast_variables_expand_all() {
        let vars = BroadcastVariables {
            session_name: Some("Session 1".to_string()),
            project: Some("my-app".to_string()),
            worktree: Some("/repo".to_string()),
            custom: HashMap::from([("BRANCH".to_string(), "main".to_string())]),
        };

        let template = "$SESSION_NAME: $PROJECT in $WORKTREE on $BRANCH";
        let expanded = vars.expand(template);
        assert_eq!(expanded, "Session 1: my-app in /repo on main");
    }

    #[test]
    fn test_broadcast_variables_expand_missing() {
        let vars = BroadcastVariables::default();
        let expanded = vars.expand("Hello $SESSION_NAME");
        assert_eq!(expanded, "Hello $SESSION_NAME"); // Not expanded
    }

    #[test]
    fn test_broadcast_variables_expand_multiple_occurrences() {
        let vars = BroadcastVariables {
            project: Some("app".to_string()),
            ..Default::default()
        };
        let expanded = vars.expand("$PROJECT: $PROJECT rocks!");
        assert_eq!(expanded, "app: app rocks!");
    }

    #[test]
    fn test_broadcast_variables_serialization() {
        let vars = BroadcastVariables {
            session_name: Some("Test".to_string()),
            project: Some("proj".to_string()),
            worktree: None,
            custom: HashMap::from([("KEY".to_string(), "value".to_string())]),
        };
        let json = serde_json::to_string(&vars).unwrap();
        let parsed: BroadcastVariables = serde_json::from_str(&json).unwrap();
        assert_eq!(vars, parsed);
    }

    #[test]
    fn test_broadcast_variables_equality() {
        let vars1 = BroadcastVariables::new().with_project("app".to_string());
        let vars2 = BroadcastVariables::new().with_project("app".to_string());
        let vars3 = BroadcastVariables::new().with_project("other".to_string());
        assert_eq!(vars1, vars2);
        assert_ne!(vars1, vars3);
    }

    // DeliveryStatus tests
    #[test]
    fn test_delivery_status_new() {
        let status = DeliveryStatus::new(SessionId(1));
        assert_eq!(status.session_id, SessionId(1));
        assert!(!status.delivered);
        assert!(status.error.is_none());
        assert!(status.delivered_at.is_none());
        assert!(status.is_pending());
        assert!(!status.is_failed());
    }

    #[test]
    fn test_delivery_status_mark_delivered() {
        let mut status = DeliveryStatus::new(SessionId(1));
        status.mark_delivered();
        assert!(status.delivered);
        assert!(status.delivered_at.is_some());
        assert!(status.error.is_none());
        assert!(!status.is_pending());
        assert!(!status.is_failed());
    }

    #[test]
    fn test_delivery_status_mark_failed() {
        let mut status = DeliveryStatus::new(SessionId(1));
        status.mark_failed("Connection error".to_string());
        assert!(!status.delivered);
        assert_eq!(status.error, Some("Connection error".to_string()));
        assert!(!status.is_pending());
        assert!(status.is_failed());
    }

    #[test]
    fn test_delivery_status_mark_delivered_clears_error() {
        let mut status = DeliveryStatus::new(SessionId(1));
        status.mark_failed("Error".to_string());
        assert!(status.is_failed());

        status.mark_delivered();
        assert!(status.delivered);
        assert!(status.error.is_none());
        assert!(!status.is_failed());
    }

    #[test]
    fn test_delivery_status_serialization() {
        let mut status = DeliveryStatus::new(SessionId(1));
        status.mark_delivered();

        let json = serde_json::to_string(&status).unwrap();
        let parsed: DeliveryStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status.session_id, parsed.session_id);
        assert_eq!(status.delivered, parsed.delivered);
    }

    #[test]
    fn test_delivery_status_clone() {
        let mut status = DeliveryStatus::new(SessionId(1));
        status.mark_delivered();
        let cloned = status.clone();
        assert_eq!(status.session_id, cloned.session_id);
        assert_eq!(status.delivered, cloned.delivered);
    }

    // BroadcastHistoryEntry tests
    #[test]
    fn test_broadcast_history_entry_from_message() {
        let mut msg = BroadcastMessage::new(
            BroadcastId(1),
            "Test".to_string(),
            vec![SessionId(1), SessionId(2)],
        );
        msg.delivery_status[0].mark_delivered();
        msg.delivery_status[1].mark_failed("Error".to_string());

        let entry = BroadcastHistoryEntry::from_message(msg);
        assert_eq!(entry.id(), BroadcastId(1));
        assert!(entry.summary.contains("2 sessions"));
        assert!(entry.summary.contains("1 delivered"));
        assert!(entry.summary.contains("1 failed"));
        assert!(!entry.all_delivered());
    }

    #[test]
    fn test_broadcast_history_entry_all_delivered() {
        let mut msg = BroadcastMessage::new(BroadcastId(1), "Test".to_string(), vec![SessionId(1)]);
        msg.delivery_status[0].mark_delivered();

        let entry = BroadcastHistoryEntry::from_message(msg);
        assert!(entry.all_delivered());
    }

    #[test]
    fn test_broadcast_history_entry_serialization() {
        let msg = BroadcastMessage::new(BroadcastId(1), "Test".to_string(), vec![SessionId(1)]);
        let entry = BroadcastHistoryEntry::from_message(msg);

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: BroadcastHistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry.id(), parsed.id());
    }

    #[test]
    fn test_broadcast_history_entry_clone() {
        let msg = BroadcastMessage::new(BroadcastId(1), "Test".to_string(), vec![SessionId(1)]);
        let entry = BroadcastHistoryEntry::from_message(msg);
        let cloned = entry.clone();
        assert_eq!(entry.id(), cloned.id());
        assert_eq!(entry.summary, cloned.summary);
    }

    // Integration tests
    #[test]
    fn test_full_broadcast_workflow() {
        // Create variables
        let vars = BroadcastVariables::new()
            .with_project("my-app".to_string())
            .with_custom("BRANCH".to_string(), "main".to_string());

        // Expand template
        let template = "API changed in $PROJECT on $BRANCH. Please update.";
        let content = vars.expand(template);
        assert_eq!(content, "API changed in my-app on main. Please update.");

        // Create message
        let mut msg = BroadcastMessage::with_priority(
            BroadcastId(1),
            content,
            vec![SessionId(1), SessionId(2), SessionId(3)],
            BroadcastPriority::High,
        );
        msg.template = template.to_string();

        // Simulate delivery
        msg.delivery_status[0].mark_delivered();
        msg.delivery_status[1].mark_failed("Session offline".to_string());
        msg.delivery_status[2].mark_delivered();

        // Check results
        assert_eq!(msg.success_count(), 2);
        assert_eq!(msg.failure_count(), 1);
        assert!(msg.is_complete());

        // Create history entry
        let entry = BroadcastHistoryEntry::from_message(msg);
        assert!(!entry.all_delivered());
        assert!(entry.summary.contains("3 sessions"));
    }
}
