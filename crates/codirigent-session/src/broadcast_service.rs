//! Broadcast service implementation.
//!
//! This module provides the [`DefaultBroadcastService`] which implements
//! the [`BroadcastService`] trait for sending messages to multiple AI
//! coding sessions simultaneously.
//!
//! # Overview
//!
//! The broadcast service enables:
//! - Sending messages to specific sessions
//! - Template variable expansion
//! - Priority levels for messages
//! - Delivery tracking and retry
//! - History management
//!
//! # Example
//!
//! ```
//! use codirigent_session::DefaultBroadcastService;
//! use codirigent_core::{BroadcastService, BroadcastVariables, SessionId};
//!
//! let mut service = DefaultBroadcastService::new();
//!
//! // Send a simple message
//! let targets = vec![SessionId(1), SessionId(2)];
//! let id = service.send("Hello everyone!", targets).unwrap();
//!
//! // Check delivery status
//! let msg = service.get_broadcast(id).unwrap();
//! println!("Sent to {} sessions", msg.targets.len());
//! ```

use anyhow::{Context, Result};
use codirigent_core::{
    BroadcastHistoryEntry, BroadcastId, BroadcastMessage, BroadcastPriority, BroadcastService,
    BroadcastVariables, CodirigentEvent, SessionId,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

/// Callback type for sending messages to sessions.
///
/// This function is called for each target session when delivering a broadcast.
/// It should return `Ok(())` if delivery succeeded, or an error if it failed.
pub type SendCallback = Box<dyn Fn(SessionId, &str) -> Result<()> + Send + Sync>;

/// Callback type for getting all active session IDs.
///
/// This function is called when sending to all sessions.
pub type SessionListCallback = Box<dyn Fn() -> Vec<SessionId> + Send + Sync>;

/// Default implementation of [`BroadcastService`].
///
/// Provides broadcast messaging to multiple AI coding sessions with:
/// - Unique ID generation
/// - Delivery tracking per session
/// - History management with configurable size limit
/// - Retry support for failed deliveries
/// - Event emission for delivery status
pub struct DefaultBroadcastService {
    /// ID counter for broadcasts.
    next_id: AtomicU64,
    /// Active/pending broadcasts.
    broadcasts: HashMap<BroadcastId, BroadcastMessage>,
    /// Broadcast history.
    history: Vec<BroadcastHistoryEntry>,
    /// Maximum history entries to keep.
    max_history: usize,
    /// Callback to send messages to sessions.
    send_callback: Option<SendCallback>,
    /// Callback to get active session list.
    session_list_callback: Option<SessionListCallback>,
    /// Event sender.
    event_tx: Option<broadcast::Sender<CodirigentEvent>>,
}

impl Default for DefaultBroadcastService {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultBroadcastService {
    /// Create a new broadcast service.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_session::DefaultBroadcastService;
    /// use codirigent_core::BroadcastService;
    ///
    /// let service = DefaultBroadcastService::new();
    /// assert!(service.history().is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            broadcasts: HashMap::new(),
            history: Vec::new(),
            max_history: 100,
            send_callback: None,
            session_list_callback: None,
            event_tx: None,
        }
    }

    /// Create with a maximum history size.
    ///
    /// # Arguments
    ///
    /// * `max_history` - Maximum number of history entries to keep
    pub fn with_max_history(mut self, max_history: usize) -> Self {
        self.max_history = max_history;
        self
    }

    /// Create with a send callback.
    ///
    /// The callback is invoked for each target session when delivering a broadcast.
    ///
    /// # Arguments
    ///
    /// * `callback` - Function to send messages to sessions
    pub fn with_callback(callback: SendCallback) -> Self {
        Self {
            send_callback: Some(callback),
            ..Self::new()
        }
    }

    /// Create with an event bus.
    ///
    /// Events will be emitted for broadcast delivery status changes.
    ///
    /// # Arguments
    ///
    /// * `event_tx` - Event bus sender
    pub fn with_event_bus(event_tx: broadcast::Sender<CodirigentEvent>) -> Self {
        Self {
            event_tx: Some(event_tx),
            ..Self::new()
        }
    }

    /// Set the send callback.
    ///
    /// # Arguments
    ///
    /// * `callback` - Function to send messages to sessions
    pub fn set_callback(&mut self, callback: SendCallback) {
        self.send_callback = Some(callback);
    }

    /// Set the session list callback.
    ///
    /// # Arguments
    ///
    /// * `callback` - Function to get active session IDs
    pub fn set_session_list_callback(&mut self, callback: SessionListCallback) {
        self.session_list_callback = Some(callback);
    }

    /// Set the event bus.
    ///
    /// # Arguments
    ///
    /// * `event_tx` - Event bus sender
    pub fn set_event_bus(&mut self, event_tx: broadcast::Sender<CodirigentEvent>) {
        self.event_tx = Some(event_tx);
    }

    /// Get the maximum history size.
    pub fn max_history(&self) -> usize {
        self.max_history
    }

    /// Set the maximum history size.
    ///
    /// If the current history exceeds the new limit, it will be truncated.
    pub fn set_max_history(&mut self, max_history: usize) {
        self.max_history = max_history;
        if self.history.len() > max_history {
            self.history.truncate(max_history);
        }
    }

    /// Emit an event.
    fn emit(&self, event: CodirigentEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event);
        }
    }

    /// Deliver message to a single session.
    fn deliver_to_session(&self, session_id: SessionId, content: &str) -> Result<()> {
        if let Some(ref callback) = self.send_callback {
            callback(session_id, content)
        } else {
            // No callback set - just log
            debug!(?session_id, "Would send: {}", content);
            Ok(())
        }
    }

    /// Add to history, trimming if needed.
    fn add_to_history(&mut self, entry: BroadcastHistoryEntry) {
        self.history.insert(0, entry);
        if self.history.len() > self.max_history {
            self.history.truncate(self.max_history);
        }
    }

    /// Internal send implementation with priority.
    fn send_internal(
        &mut self,
        content: &str,
        targets: Vec<SessionId>,
        priority: BroadcastPriority,
    ) -> Result<BroadcastId> {
        let id = self.next_id();
        let mut message =
            BroadcastMessage::with_priority(id, content.to_string(), targets.clone(), priority);

        info!(
            ?id,
            targets = targets.len(),
            priority = %priority,
            "Sending broadcast"
        );

        self.emit(CodirigentEvent::BroadcastSent {
            id,
            target_count: targets.len(),
            priority,
        });

        // Deliver to each target
        for status in &mut message.delivery_status {
            match self.deliver_to_session(status.session_id, content) {
                Ok(()) => {
                    status.mark_delivered();
                    self.emit(CodirigentEvent::BroadcastDelivered {
                        id,
                        session_id: status.session_id,
                    });
                }
                Err(e) => {
                    let error = e.to_string();
                    status.mark_failed(error.clone());
                    self.emit(CodirigentEvent::BroadcastDeliveryFailed {
                        id,
                        session_id: status.session_id,
                        error,
                    });
                }
            }
        }

        self.emit(CodirigentEvent::BroadcastComplete {
            id,
            success_count: message.success_count(),
            failure_count: message.failure_count(),
        });

        // Store and add to history
        let entry = BroadcastHistoryEntry::from_message(message.clone());
        self.add_to_history(entry);
        self.broadcasts.insert(id, message);

        Ok(id)
    }
}

impl BroadcastService for DefaultBroadcastService {
    fn send(&mut self, content: &str, targets: Vec<SessionId>) -> Result<BroadcastId> {
        self.send_internal(content, targets, BroadcastPriority::Normal)
    }

    fn send_with_variables(
        &mut self,
        template: &str,
        targets: Vec<SessionId>,
        variables: BroadcastVariables,
    ) -> Result<BroadcastId> {
        let content = variables.expand(template);
        self.send(&content, targets)
    }

    fn send_with_priority(
        &mut self,
        content: &str,
        targets: Vec<SessionId>,
        priority: BroadcastPriority,
    ) -> Result<BroadcastId> {
        self.send_internal(content, targets, priority)
    }

    fn send_to_all(&mut self, content: &str) -> Result<BroadcastId> {
        let targets = if let Some(ref callback) = self.session_list_callback {
            callback()
        } else {
            warn!("send_to_all called but no session list callback set");
            vec![]
        };

        if targets.is_empty() {
            info!("send_to_all: no active sessions to send to");
        }

        self.send(content, targets)
    }

    fn history(&self) -> &[BroadcastHistoryEntry] {
        &self.history
    }

    fn get_broadcast(&self, id: BroadcastId) -> Option<&BroadcastMessage> {
        self.broadcasts.get(&id)
    }

    fn clear_history(&mut self) {
        self.history.clear();
        info!("Broadcast history cleared");
    }

    fn retry_failed(&mut self, id: BroadcastId) -> Result<()> {
        // First, get the content and failed sessions
        let (content, failed_sessions) = {
            let message = self.broadcasts.get(&id).context("Broadcast not found")?;

            let failed: Vec<SessionId> = message
                .delivery_status
                .iter()
                .filter(|s| !s.delivered)
                .map(|s| s.session_id)
                .collect();

            (message.content.clone(), failed)
        };

        if failed_sessions.is_empty() {
            debug!(?id, "No failed deliveries to retry");
            return Ok(());
        }

        info!(
            ?id,
            count = failed_sessions.len(),
            "Retrying failed deliveries"
        );

        // Collect delivery results
        let mut results: Vec<(SessionId, Result<()>)> = Vec::new();
        for session_id in &failed_sessions {
            let result = self.deliver_to_session(*session_id, &content);
            results.push((*session_id, result));
        }

        // Update the message with results and collect events to emit
        let events_to_emit: Vec<SessionId> = {
            let message = self
                .broadcasts
                .get_mut(&id)
                .context("Broadcast not found")?;

            let mut delivered_sessions = Vec::new();
            for (session_id, result) in results {
                if let Some(status) = message
                    .delivery_status
                    .iter_mut()
                    .find(|s| s.session_id == session_id)
                {
                    match result {
                        Ok(()) => {
                            status.mark_delivered();
                            delivered_sessions.push(session_id);
                        }
                        Err(e) => {
                            status.mark_failed(e.to_string());
                        }
                    }
                }
            }
            delivered_sessions
        };

        // Emit events after releasing the mutable borrow
        for session_id in events_to_emit {
            self.emit(CodirigentEvent::BroadcastDelivered { id, session_id });
        }

        Ok(())
    }

    fn next_id(&mut self) -> BroadcastId {
        BroadcastId(self.next_id.fetch_add(1, Ordering::SeqCst))
    }
}

// Make DefaultBroadcastService Send + Sync
unsafe impl Send for DefaultBroadcastService {}
unsafe impl Sync for DefaultBroadcastService {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_default_broadcast_service_new() {
        let service = DefaultBroadcastService::new();
        assert!(service.history().is_empty());
        assert_eq!(service.max_history(), 100);
    }

    #[test]
    fn test_with_max_history() {
        let service = DefaultBroadcastService::new().with_max_history(50);
        assert_eq!(service.max_history(), 50);
    }

    #[test]
    fn test_set_max_history_truncates() {
        let mut service = DefaultBroadcastService::new();

        // Add some history entries
        for i in 0..10 {
            service
                .send(&format!("Message {}", i), vec![SessionId(1)])
                .unwrap();
        }

        assert_eq!(service.history().len(), 10);

        // Truncate to 5
        service.set_max_history(5);
        assert_eq!(service.history().len(), 5);
    }

    #[test]
    fn test_send_broadcast() {
        let mut service = DefaultBroadcastService::new();
        let targets = vec![SessionId(1), SessionId(2)];

        let id = service.send("Hello!", targets).unwrap();

        let msg = service.get_broadcast(id).unwrap();
        assert_eq!(msg.content, "Hello!");
        assert_eq!(msg.targets.len(), 2);
        assert_eq!(msg.priority, BroadcastPriority::Normal);
    }

    #[test]
    fn test_send_with_priority() {
        let mut service = DefaultBroadcastService::new();

        let id = service
            .send_with_priority("Urgent!", vec![SessionId(1)], BroadcastPriority::Critical)
            .unwrap();

        let msg = service.get_broadcast(id).unwrap();
        assert_eq!(msg.priority, BroadcastPriority::Critical);
    }

    #[test]
    fn test_send_with_variables() {
        let mut service = DefaultBroadcastService::new();
        let vars = BroadcastVariables::new()
            .with_project("test-project".to_string())
            .with_custom("BRANCH".to_string(), "main".to_string());

        let id = service
            .send_with_variables(
                "Working on $PROJECT branch $BRANCH",
                vec![SessionId(1)],
                vars,
            )
            .unwrap();

        let msg = service.get_broadcast(id).unwrap();
        assert_eq!(msg.content, "Working on test-project branch main");
    }

    #[test]
    fn test_send_to_all_no_callback() {
        let mut service = DefaultBroadcastService::new();

        // Should succeed with empty targets when no callback is set
        let id = service.send_to_all("Hello everyone!").unwrap();
        let msg = service.get_broadcast(id).unwrap();
        assert_eq!(msg.targets.len(), 0);
    }

    #[test]
    fn test_send_to_all_with_callback() {
        let mut service = DefaultBroadcastService::new();

        // Set up session list callback
        let sessions = vec![SessionId(1), SessionId(2), SessionId(3)];
        let sessions_clone = sessions.clone();
        service.set_session_list_callback(Box::new(move || sessions_clone.clone()));

        let id = service.send_to_all("Hello everyone!").unwrap();
        let msg = service.get_broadcast(id).unwrap();
        assert_eq!(msg.targets.len(), 3);
    }

    #[test]
    fn test_with_callback_delivery() {
        let received: Arc<Mutex<Vec<(SessionId, String)>>> = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let callback: SendCallback = Box::new(move |session_id, content| {
            received_clone
                .lock()
                .unwrap()
                .push((session_id, content.to_string()));
            Ok(())
        });

        let mut service = DefaultBroadcastService::with_callback(callback);
        service
            .send("Test message", vec![SessionId(1), SessionId(2)])
            .unwrap();

        let received = received.lock().unwrap();
        assert_eq!(received.len(), 2);
        assert!(received.iter().all(|(_, msg)| msg == "Test message"));
    }

    #[test]
    fn test_callback_failure() {
        let call_count: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
        let call_count_clone = call_count.clone();

        let callback: SendCallback = Box::new(move |session_id, _content| {
            *call_count_clone.lock().unwrap() += 1;
            if session_id == SessionId(2) {
                Err(anyhow::anyhow!("Session 2 offline"))
            } else {
                Ok(())
            }
        });

        let mut service = DefaultBroadcastService::with_callback(callback);
        let id = service
            .send("Test", vec![SessionId(1), SessionId(2), SessionId(3)])
            .unwrap();

        let msg = service.get_broadcast(id).unwrap();
        assert_eq!(msg.success_count(), 2);
        assert_eq!(msg.failure_count(), 1);
        assert!(msg.is_complete());
    }

    #[test]
    fn test_history_ordering() {
        let mut service = DefaultBroadcastService::new();

        service.send("First", vec![SessionId(1)]).unwrap();
        service.send("Second", vec![SessionId(2)]).unwrap();
        service.send("Third", vec![SessionId(3)]).unwrap();

        let history = service.history();
        assert_eq!(history.len(), 3);
        // Most recent first
        assert_eq!(history[0].message.content, "Third");
        assert_eq!(history[1].message.content, "Second");
        assert_eq!(history[2].message.content, "First");
    }

    #[test]
    fn test_history_max_size() {
        let mut service = DefaultBroadcastService::new().with_max_history(3);

        for i in 0..5 {
            service
                .send(&format!("Message {}", i), vec![SessionId(1)])
                .unwrap();
        }

        assert_eq!(service.history().len(), 3);
        // Should have most recent 3
        assert_eq!(service.history()[0].message.content, "Message 4");
        assert_eq!(service.history()[1].message.content, "Message 3");
        assert_eq!(service.history()[2].message.content, "Message 2");
    }

    #[test]
    fn test_clear_history() {
        let mut service = DefaultBroadcastService::new();
        service.send("Test", vec![SessionId(1)]).unwrap();

        assert!(!service.history().is_empty());
        service.clear_history();
        assert!(service.history().is_empty());
    }

    #[test]
    fn test_get_broadcast_not_found() {
        let service = DefaultBroadcastService::new();
        assert!(service.get_broadcast(BroadcastId(999)).is_none());
    }

    #[test]
    fn test_retry_failed() {
        let call_count: Arc<Mutex<HashMap<SessionId, u32>>> = Arc::new(Mutex::new(HashMap::new()));
        let call_count_clone = call_count.clone();

        // Fail on first call, succeed on retry
        let callback: SendCallback = Box::new(move |session_id, _content| {
            let mut counts = call_count_clone.lock().unwrap();
            let count = counts.entry(session_id).or_insert(0);
            *count += 1;

            if *count == 1 && session_id == SessionId(1) {
                Err(anyhow::anyhow!("First attempt failed"))
            } else {
                Ok(())
            }
        });

        let mut service = DefaultBroadcastService::with_callback(callback);

        let id = service.send("Test", vec![SessionId(1)]).unwrap();

        // First send should fail
        let msg = service.get_broadcast(id).unwrap();
        assert_eq!(msg.failure_count(), 1);
        assert_eq!(msg.success_count(), 0);

        // Retry should succeed
        service.retry_failed(id).unwrap();
        let msg = service.get_broadcast(id).unwrap();
        assert_eq!(msg.success_count(), 1);
        assert_eq!(msg.failure_count(), 0);
    }

    #[test]
    fn test_retry_failed_not_found() {
        let mut service = DefaultBroadcastService::new();
        let result = service.retry_failed(BroadcastId(999));
        assert!(result.is_err());
    }

    #[test]
    fn test_retry_no_failures() {
        let mut service = DefaultBroadcastService::new();
        let id = service.send("Test", vec![SessionId(1)]).unwrap();

        // All deliveries succeeded (no callback means auto-success)
        let msg = service.get_broadcast(id).unwrap();
        assert_eq!(msg.failure_count(), 0);

        // Retry should be a no-op
        service.retry_failed(id).unwrap();
    }

    #[test]
    fn test_next_id_increment() {
        let mut service = DefaultBroadcastService::new();

        let id1 = service.next_id();
        let id2 = service.next_id();
        let id3 = service.next_id();

        assert_eq!(id1, BroadcastId(1));
        assert_eq!(id2, BroadcastId(2));
        assert_eq!(id3, BroadcastId(3));
    }

    #[test]
    fn test_event_emission() {
        let (tx, mut rx) = broadcast::channel::<CodirigentEvent>(16);

        let mut service = DefaultBroadcastService::with_event_bus(tx);

        service.send("Test", vec![SessionId(1)]).unwrap();

        // Should receive BroadcastSent event
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, CodirigentEvent::BroadcastSent { .. }));

        // Should receive BroadcastDelivered event
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, CodirigentEvent::BroadcastDelivered { .. }));

        // Should receive BroadcastComplete event
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, CodirigentEvent::BroadcastComplete { .. }));
    }

    #[test]
    fn test_full_broadcast_workflow() {
        let delivered: Arc<Mutex<HashMap<SessionId, String>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let delivered_clone = delivered.clone();

        let callback: SendCallback = Box::new(move |session_id, content| {
            delivered_clone
                .lock()
                .unwrap()
                .insert(session_id, content.to_string());
            Ok(())
        });

        let mut service = DefaultBroadcastService::with_callback(callback);

        // Send initial broadcast with variables
        let targets = vec![SessionId(1), SessionId(2), SessionId(3)];
        let vars = BroadcastVariables::new()
            .with_project("my-project".to_string())
            .with_custom("VERSION".to_string(), "2.0".to_string());

        let id = service
            .send_with_variables(
                "Note: API changed in $PROJECT v$VERSION. Update your code.",
                targets,
                vars,
            )
            .unwrap();

        // Verify all delivered
        let msg = service.get_broadcast(id).unwrap();
        assert_eq!(msg.success_count(), 3);
        assert_eq!(msg.failure_count(), 0);
        assert!(msg.is_complete());

        // Verify content was expanded
        let delivered = delivered.lock().unwrap();
        let content = delivered.get(&SessionId(1)).unwrap();
        assert!(content.contains("my-project"));
        assert!(content.contains("v2.0"));
    }

    #[test]
    fn test_empty_targets() {
        let mut service = DefaultBroadcastService::new();

        let id = service.send("Hello", vec![]).unwrap();
        let msg = service.get_broadcast(id).unwrap();

        assert_eq!(msg.targets.len(), 0);
        assert!(msg.is_complete()); // No pending deliveries
        assert_eq!(msg.success_count(), 0);
        assert_eq!(msg.failure_count(), 0);
    }

    #[test]
    fn test_set_callback() {
        let received: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
        let received_clone = received.clone();

        let mut service = DefaultBroadcastService::new();

        // No callback initially
        service.send("Test 1", vec![SessionId(1)]).unwrap();

        // Set callback
        service.set_callback(Box::new(move |_session_id, _content| {
            *received_clone.lock().unwrap() = true;
            Ok(())
        }));

        service.send("Test 2", vec![SessionId(1)]).unwrap();

        assert!(*received.lock().unwrap());
    }

    #[test]
    fn test_clone_and_debug() {
        let service = DefaultBroadcastService::new();

        // Test that the service can be used with debug
        let _ = format!("{:?}", service.history());
    }
}
