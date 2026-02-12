//! Persistence services for WorkspaceView.

use std::sync::{Arc, Mutex};

use codirigent_core::compaction::CompactionService;
use codirigent_core::StorageService;

/// Groups storage and compaction services for the workspace.
pub(super) struct PersistenceServices {
    /// File-based storage service for session state.
    pub(super) storage: Arc<dyn StorageService>,
    /// Auto-compaction service for context management.
    pub(super) compaction: Arc<Mutex<CompactionService>>,
}
