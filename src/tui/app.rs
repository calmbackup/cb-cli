use std::time::Duration;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::{DefaultTerminal, Frame};
use tokio::sync::mpsc;
use crate::core::config::Config;
use crate::core::types::{BackupEntry, Result};

/// Messages sent from background tasks to the TUI.
#[derive(Debug)]
pub enum AppMessage {
    /// Progress update from a backup/restore operation.
    Progress(String, Option<String>),
    /// Backup completed successfully.
    BackupComplete(crate::core::types::BackupResult),
    /// Restore completed successfully.
    RestoreComplete(crate::core::types::RestoreResult),
    /// Operation failed.
    Error(String),
    /// Backup list refreshed.
    BackupsLoaded(Vec<BackupEntry>),
    /// API connectivity status.
    ApiStatus(bool),
    /// Account info loaded.
    AccountLoaded(crate::core::types::AccountInfo),
}

/// The active view/mode of the TUI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppView {
    Dashboard,
    Picker,
    Confirm,
}

/// Main TUI application state.
pub struct App {
    pub config: Config,
    pub key: [u8; 32],
    pub version: String,
    pub view: AppView,
    pub should_quit: bool,

    // Dashboard state
    pub backups: Vec<BackupEntry>,
    pub selected_backup: usize,
    pub api_connected: bool,
    pub account: Option<crate::core::types::AccountInfo>,
    pub local_backup_count: u32,
    pub local_backup_size: u64,
    pub latest_local: Option<String>,

    // Progress state
    pub operation_running: bool,
    pub progress_message: String,
    pub progress_detail: Option<String>,

    // Confirm dialog state
    pub confirm_message: String,
    pub confirm_cursor: usize, // 0=No, 1=Yes

    // Channel for receiving messages from background tasks
    pub rx: mpsc::UnboundedReceiver<AppMessage>,
    pub tx: mpsc::UnboundedSender<AppMessage>,
}

impl App {
    /// Create a new App instance from config.
    pub fn new(config: Config, key: [u8; 32], version: String) -> Self {
        todo!("Initialize app state, create mpsc channel")
    }

    /// Run the TUI application main loop.
    pub async fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        todo!("Event loop: crossterm events + mpsc messages, render, handle input")
    }

    /// Draw the current frame.
    fn draw(&self, frame: &mut Frame) {
        todo!("Dispatch to dashboard/picker/confirm draw based on self.view")
    }

    /// Handle a key event.
    fn handle_key(&mut self, key: KeyEvent) {
        todo!("Dispatch to view-specific key handler")
    }

    /// Handle a message from a background task.
    fn handle_message(&mut self, msg: AppMessage) {
        todo!("Update state based on message type")
    }

    /// Start a backup operation in the background.
    fn start_backup(&mut self) {
        todo!("Spawn tokio task for BackupService.backup(), send progress via tx")
    }

    /// Start a restore operation in the background.
    fn start_restore(&mut self, backup_id: String) {
        todo!("Spawn tokio task for RestoreService.restore(), send progress via tx")
    }

    /// Refresh the backup list from API.
    fn refresh_backups(&self) {
        todo!("Spawn tokio task to list backups, send via tx")
    }
}
