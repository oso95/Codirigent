//! Codirigent Core
//!
//! Core types, traits, events, and services for the Codirigent application.
//!
//! ## Modules
//!
//! - [`types`] - Core data types (SessionId, Session, Task, etc.)
//! - [`events`] - Event types for cross-module communication
//! - [`traits`] - Service trait definitions
//! - [`event_bus`] - Default EventBus implementation
//! - [`storage`] - File-based storage service
//! - [`plugin`] - Plugin system types and traits
//! - [`verification`] - Verification types and events
//! - [`context`] - Context window tracking for AI sessions
//! - [`config`] - Configuration types (ProjectConfig, UserSettings)
//! - [`config_service`] - Configuration loading and saving service
//! - [`skill`] - Skill management types (Skill, SkillPreset, TokenBudget)
//! - [`change_summary`] - Change detection and risk assessment types
//! - [`persistence`] - Session persistence types
//! - [`persistence_service`] - Persistence service trait and implementation
//! - [`auto_save`] - Automatic state saving manager
//! - [`assignment`] - Task assignment management and routing
//! - [`session_notes`] - Session notes generation and learnings extraction
//! - [`ralph`] - Ralph Loop for autonomous task execution
//! - [`task_manager`] - Unified task management coordinator
//! - [`session_advanced`] - Advanced session features (templates, handoff, groups, overnight mode)
//! - [`broadcast`] - Broadcast messaging to multiple sessions
//! - [`pipeline`] - Verification pipeline types and traits
//! - [`error`] - Error types
//!
//! ## Quick Start
//!
//! ```
//! use codirigent_core::{
//!     SessionId, Session, SessionStatus,
//!     CodirigentEvent, DefaultEventBus, EventBus,
//! };
//! use std::path::PathBuf;
//!
//! // Create an event bus
//! let bus = DefaultEventBus::new(16);
//!
//! // Subscribe to events
//! let mut rx = bus.subscribe();
//!
//! // Publish an event
//! bus.publish(CodirigentEvent::SessionCreated { id: SessionId(1) });
//! ```
//!
//! ## Storage Example
//!
//! ```no_run
//! use codirigent_core::{FileStorageService, StorageService};
//! use std::path::Path;
//!
//! let storage = FileStorageService::new(Path::new("/path/to/project")).unwrap();
//! let state = storage.load_state().unwrap();
//! println!("Loaded {} sessions", state.sessions.len());
//! ```
//!
//! ## Verification Example
//!
//! ```
//! use codirigent_core::verification::{
//!     VerificationResult, VerificationCheckType, VerificationConfig,
//! };
//!
//! // Create a passed verification result
//! let result = VerificationResult::passed(VerificationCheckType::UnitTest, 1500);
//! assert!(result.passed);
//!
//! // Use default verification config
//! let config = VerificationConfig::default();
//! assert!(config.enabled);
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod assignment;
pub mod auto_save;
pub mod broadcast;
pub mod change_summary;
pub mod clipboard_types;
pub mod config;
pub mod config_service;
pub mod context;
pub mod error;
pub mod event_bus;
pub mod events;
pub mod persistence;
pub mod persistence_service;
pub mod pipeline;
pub mod plugin;
pub mod ralph;
pub mod scheduler;
pub mod session_advanced;
pub mod session_notes;
pub mod skill;
pub mod storage;
pub mod task_manager;
pub mod traits;
pub mod types;
pub mod verification;

// Re-export commonly used items
pub use change_summary::{
    ChangeDetector, ChangeSummary, ChangeType, FileCategory, FileChange, RiskAssessment,
    RiskAssessor, RiskLevel,
};
pub use config::{ProjectConfig, UserSettings};
pub use config_service::{ConfigChange, ConfigService, DefaultConfigService, EffectiveConfig};
pub use context::{
    ContextConfig, ContextPattern, ContextTracker, ContextTrackingService, ContextUsage,
};
pub use error::{CodirigentError, Result};
pub use event_bus::DefaultEventBus;
pub use events::{ClipboardContentType, CodirigentEvent};
pub use scheduler::{SchedulerConfig, SchedulerMode, TaskQueue, TaskQueueService};
pub use skill::{Skill, SkillPreset, SkillType, TokenBudget};
pub use storage::{ContextFileData, FileStorageService};
pub use traits::{
    EventBus, ProcessMonitor, RalphLoopController, SessionManager, SkillManager, StorageService,
};
pub use traits::{FailureFormatter, ProjectType, VerificationDetector, Verifier};
pub use types::*;

// Re-export verification runner types
pub use verification::{
    DetectionRule, OutputParser, ParsedTestFailure, ParsedTestResults, TestCommandDetector,
    VerificationRunner, VerificationRunnerConfig, VerificationService,
};

// Re-export persistence types
pub use auto_save::AutoSaveManager;
pub use persistence::{Checkpoint, PersistentSession, PersistentState, RecoveryResult};
pub use persistence_service::{AutoSaveConfig, DefaultPersistenceService, PersistenceService};

// Re-export assignment types
pub use assignment::{
    AssignmentAction, AssignmentConfig, AssignmentManager, AssignmentService, PendingAssignment,
    DEFAULT_PROMPT_TEMPLATE,
};

// Re-export session notes types
pub use session_notes::{
    CompletionStatus, Learning, LearningCategory, LearningsExtractor, NotesGenerator, SessionNote,
    SessionNotesConfig, SummaryMode,
};

// Re-export Ralph Loop types
pub use ralph::{IterationResult, RalphLoopConfig, RalphLoopState, RalphLoopStatus};

// Re-export task manager types
pub use task_manager::{
    TaskCompletionResult, TaskManagementService, TaskManager, TaskManagerConfig,
};

// Re-export broadcast types
pub use broadcast::{
    BroadcastHistoryEntry, BroadcastId, BroadcastMessage, BroadcastPriority, BroadcastVariables,
    DeliveryStatus,
};
pub use traits::BroadcastService;

// Re-export advanced session types
pub use session_advanced::{
    ContextHandoff, HandoffStatus, OvernightConfig, OvernightSummary, SessionGroup, SessionTemplate,
};

// Re-export pipeline types
pub use pipeline::{
    FailureMessageFormatter, PipelineEvent, PipelineStage, PipelineState, ReviewDecision,
    VerificationPipeline,
};

// Re-export clipboard types
pub use clipboard_types::{CliType, ClipboardContent, ImageData, ImageFormat};
