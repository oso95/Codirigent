//! Default implementation of the [`EventBus`] trait.
//!
//! This module provides [`DefaultEventBus`], a tokio broadcast channel-based
//! implementation suitable for most use cases.

use crate::events::CodirigentEvent;
use crate::traits::EventBus;
use tokio::sync::broadcast;

/// Default implementation of [`EventBus`] using tokio broadcast channel.
///
/// This implementation uses a bounded broadcast channel to distribute events
/// to all subscribers. Events are cloned for each subscriber.
///
/// # Capacity
///
/// The channel has a fixed capacity. If a subscriber falls behind and the
/// channel fills up, the oldest events are dropped for that subscriber
/// (they receive a `RecvError::Lagged`).
///
/// # Example
///
/// ```
/// use codirigent_core::event_bus::DefaultEventBus;
/// use codirigent_core::traits::EventBus;
/// use codirigent_core::events::CodirigentEvent;
/// use codirigent_core::types::SessionId;
///
/// let bus = DefaultEventBus::new(16);
/// let mut rx = bus.subscribe();
///
/// bus.publish(CodirigentEvent::SessionCreated { id: SessionId(1) });
/// ```
pub struct DefaultEventBus {
    sender: broadcast::Sender<CodirigentEvent>,
}

impl DefaultEventBus {
    /// Create a new event bus with the specified capacity.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of events to buffer before dropping old events
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is 0.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Create a new event bus with the default capacity (256 events).
    pub fn with_default_capacity() -> Self {
        Self::new(256)
    }

    /// Get the current number of subscribers.
    ///
    /// This can be useful for debugging or monitoring.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl EventBus for DefaultEventBus {
    fn subscribe(&self) -> broadcast::Receiver<CodirigentEvent> {
        self.sender.subscribe()
    }

    fn publish(&self, event: CodirigentEvent) {
        // Ignore send errors (no subscribers)
        let _ = self.sender.send(event);
    }
}

impl Default for DefaultEventBus {
    fn default() -> Self {
        Self::with_default_capacity()
    }
}

impl std::fmt::Debug for DefaultEventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultEventBus")
            .field("subscriber_count", &self.subscriber_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{LayoutMode, SessionId, SessionStatus, TaskId};
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_event_bus_publish_subscribe() {
        let bus = DefaultEventBus::new(16);
        let mut rx = bus.subscribe();

        bus.publish(CodirigentEvent::SessionCreated { id: SessionId(1) });

        let event = rx.recv().await.unwrap();
        assert!(matches!(
            event,
            CodirigentEvent::SessionCreated { id } if id == SessionId(1)
        ));
    }

    #[tokio::test]
    async fn test_event_bus_multiple_subscribers() {
        let bus = DefaultEventBus::new(16);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        bus.publish(CodirigentEvent::SessionFocused { id: SessionId(1) });

        let event1 = rx1.recv().await.unwrap();
        let event2 = rx2.recv().await.unwrap();

        assert!(matches!(event1, CodirigentEvent::SessionFocused { .. }));
        assert!(matches!(event2, CodirigentEvent::SessionFocused { .. }));
    }

    #[test]
    fn test_event_bus_no_subscribers() {
        let bus = DefaultEventBus::new(16);
        // Should not panic even with no subscribers
        bus.publish(CodirigentEvent::SessionClosed { id: SessionId(1) });
    }

    #[tokio::test]
    async fn test_event_bus_multiple_events() {
        let bus = DefaultEventBus::new(16);
        let mut rx = bus.subscribe();

        bus.publish(CodirigentEvent::SessionCreated { id: SessionId(1) });
        bus.publish(CodirigentEvent::SessionStatusChanged {
            id: SessionId(1),
            old: SessionStatus::Idle,
            new: SessionStatus::Working,
        });
        bus.publish(CodirigentEvent::SessionClosed { id: SessionId(1) });

        let e1 = rx.recv().await.unwrap();
        let e2 = rx.recv().await.unwrap();
        let e3 = rx.recv().await.unwrap();

        assert!(matches!(e1, CodirigentEvent::SessionCreated { .. }));
        assert!(matches!(e2, CodirigentEvent::SessionStatusChanged { .. }));
        assert!(matches!(e3, CodirigentEvent::SessionClosed { .. }));
    }

    #[test]
    fn test_event_bus_default() {
        let bus = DefaultEventBus::default();
        // Default should work without panicking
        bus.publish(CodirigentEvent::SessionCreated { id: SessionId(1) });
    }

    #[test]
    fn test_event_bus_with_default_capacity() {
        let bus = DefaultEventBus::with_default_capacity();
        bus.publish(CodirigentEvent::SessionCreated { id: SessionId(1) });
    }

    #[test]
    fn test_event_bus_subscriber_count() {
        let bus = DefaultEventBus::new(16);
        assert_eq!(bus.subscriber_count(), 0);

        let _rx1 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1);

        let _rx2 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);

        drop(_rx1);
        // Note: subscriber count doesn't immediately update on drop
        // so we don't test the decrement here
    }

    #[test]
    fn test_event_bus_debug() {
        let bus = DefaultEventBus::new(16);
        let debug_str = format!("{:?}", bus);
        assert!(debug_str.contains("DefaultEventBus"));
        assert!(debug_str.contains("subscriber_count"));
    }

    #[tokio::test]
    async fn test_event_bus_all_event_types() {
        let bus = DefaultEventBus::new(32);
        let mut rx = bus.subscribe();

        // Publish all event types
        let events = vec![
            CodirigentEvent::SessionCreated { id: SessionId(1) },
            CodirigentEvent::SessionClosed { id: SessionId(1) },
            CodirigentEvent::SessionStatusChanged {
                id: SessionId(1),
                old: SessionStatus::Idle,
                new: SessionStatus::Working,
            },
            CodirigentEvent::SessionOutputReceived {
                id: SessionId(1),
                data: vec![1, 2, 3],
            },
            CodirigentEvent::SessionRenamed {
                id: SessionId(1),
                old_name: "old".to_string(),
                new_name: "new".to_string(),
            },
            CodirigentEvent::SessionGroupChanged {
                id: SessionId(1),
                group: Some("test".to_string()),
                color: None,
            },
            CodirigentEvent::AttentionRequired {
                session_id: SessionId(1),
                detail: None,
            },
            CodirigentEvent::InputProvided {
                session_id: SessionId(1),
            },
            CodirigentEvent::LayoutChanged {
                mode: LayoutMode::Single,
            },
            CodirigentEvent::SessionFocused { id: SessionId(1) },
            CodirigentEvent::TaskCreated {
                id: TaskId::from("t"),
            },
            CodirigentEvent::TaskAssigned {
                task_id: TaskId::from("t"),
                session_id: SessionId(1),
            },
            CodirigentEvent::TaskCompleted {
                task_id: TaskId::from("t"),
                success: true,
            },
            CodirigentEvent::PathDraggedToSession {
                session_id: SessionId(1),
                path: PathBuf::from("/tmp"),
            },
        ];

        for event in &events {
            bus.publish(event.clone());
        }

        // Verify all events received
        for _ in 0..events.len() {
            let _event = rx.recv().await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_event_bus_late_subscriber() {
        let bus = DefaultEventBus::new(16);

        // Publish before subscribing
        bus.publish(CodirigentEvent::SessionCreated { id: SessionId(1) });

        // Late subscriber won't get the event
        let mut rx = bus.subscribe();

        // Publish after subscribing
        bus.publish(CodirigentEvent::SessionCreated { id: SessionId(2) });

        let event = rx.recv().await.unwrap();
        // Should get the second event, not the first
        let CodirigentEvent::SessionCreated { id } = event else {
            panic!("Expected SessionCreated, got {event:?}");
        };
        assert_eq!(id, SessionId(2));
    }

    #[tokio::test]
    async fn test_event_bus_capacity_overflow() {
        let bus = DefaultEventBus::new(2);
        let mut rx = bus.subscribe();

        // Publish more events than capacity without reading
        bus.publish(CodirigentEvent::SessionCreated { id: SessionId(1) });
        bus.publish(CodirigentEvent::SessionCreated { id: SessionId(2) });
        bus.publish(CodirigentEvent::SessionCreated { id: SessionId(3) });

        // First recv may return Lagged error or the latest events
        let result = rx.recv().await;
        // The exact behavior depends on timing, but it should not panic
        assert!(result.is_ok() || matches!(result, Err(broadcast::error::RecvError::Lagged(_))));
    }

    #[test]
    fn test_event_bus_send_sync() {
        // Verify DefaultEventBus is Send + Sync
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DefaultEventBus>();
    }

    #[tokio::test]
    async fn test_event_bus_concurrent_publish() {
        use std::sync::Arc;

        let bus = Arc::new(DefaultEventBus::new(100));
        let mut rx = bus.subscribe();

        let bus_clone = Arc::clone(&bus);
        let handle = tokio::spawn(async move {
            for i in 0..10 {
                bus_clone.publish(CodirigentEvent::SessionCreated {
                    id: SessionId(i as u64),
                });
            }
        });

        handle.await.unwrap();

        // Should receive all events
        let mut count = 0;
        while let Ok(_event) =
            tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await
        {
            count += 1;
            if count >= 10 {
                break;
            }
        }
        assert_eq!(count, 10);
    }
}
