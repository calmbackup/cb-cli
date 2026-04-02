use std::time::{Duration, Instant};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{DefaultTerminal, Frame};
use tokio::sync::mpsc;
use crate::core::config::Config;
use crate::core::types::{BackupEntry, Result};

use super::{confirm, dashboard, picker};

/// Labels for backup pipeline steps (must match the order progress messages arrive).
pub const BACKUP_STEPS: &[&str] = &[
    "Dumping database",
    "Verifying dump",
    "Creating archive",
    "Encrypting backup",
    "Syncing backups",
    "Saving locally",
    "Computing checksum",
    "Uploading to cloud",
    "Pruning old backups",
];

pub const RESTORE_STEPS: &[&str] = &[
    "Fetching backup details",
    "Checking local cache",
    "Downloading backup",
    "Decrypting backup",
    "Extracting archive",
    "Restoring database",
    "Restoring directories",
];

/// Minimum display time per step (ms) so each step is clearly visible.
const MIN_STEP_MS: u64 = 2000;
/// Minimum display time for encryption step (ms) — the animation needs time to shine.
const MIN_ENCRYPT_MS: u64 = 3000;

#[derive(Debug, Clone)]
pub struct ProgressStep {
    pub label: String,
    pub status: StepStatus,
    pub duration: Option<Duration>,
    pub started_at: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StepStatus {
    Pending,
    Active,
    Complete,
}

/// Tracks the full progress pipeline.
#[derive(Debug, Clone)]
pub struct ProgressState {
    pub steps: Vec<ProgressStep>,
    pub current_step: usize,
    pub step_visible_since: Option<Instant>,
    /// Number of advances queued (messages arrived while min-display time not yet elapsed).
    pub pending_advances: usize,
    pub completed: bool,
    pub backup_result: Option<crate::core::types::BackupResult>,
    pub restore_result: Option<crate::core::types::RestoreResult>,
}

impl ProgressState {
    pub fn new(step_labels: &[&str]) -> Self {
        Self {
            steps: step_labels
                .iter()
                .map(|label| ProgressStep {
                    label: label.to_string(),
                    status: StepStatus::Pending,
                    duration: None,
                    started_at: None,
                })
                .collect(),
            current_step: 0,
            step_visible_since: None,
            pending_advances: 0,
            completed: false,
            backup_result: None,
            restore_result: None,
        }
    }

    /// Called when a new progress message arrives (meaning previous step is done).
    pub fn advance(&mut self) {
        if self.current_step == 0 && self.steps[0].status == StepStatus::Pending {
            self.steps[0].status = StepStatus::Active;
            self.steps[0].started_at = Some(Instant::now());
            self.step_visible_since = Some(Instant::now());
            return;
        }

        let min_ms = self.min_duration_for_current();
        let elapsed = self.step_visible_since.map(|t| t.elapsed()).unwrap_or_default();

        if elapsed < Duration::from_millis(min_ms) {
            self.pending_advances += 1;
        } else {
            self.do_advance();
        }
    }

    /// Actually move to the next step.
    fn do_advance(&mut self) {
        self.finalize_current();
        let next = self.current_step + 1;
        if next < self.steps.len() {
            self.current_step = next;
            self.steps[next].status = StepStatus::Active;
            self.steps[next].started_at = Some(Instant::now());
            self.step_visible_since = Some(Instant::now());
            if self.pending_advances > 0 {
                self.pending_advances -= 1;
            }
        }
    }

    /// Mark the current step as complete.
    pub fn finalize_current(&mut self) {
        let idx = self.current_step;
        if idx < self.steps.len() && self.steps[idx].status == StepStatus::Active {
            self.steps[idx].status = StepStatus::Complete;
            self.steps[idx].duration = self.steps[idx].started_at.map(|t| t.elapsed());
        }
    }

    /// Called every frame — advances delayed steps when their min time is up.
    pub fn tick(&mut self) {
        if self.completed {
            // Finalize the last step if still active
            self.finalize_current();
            return;
        }

        if self.pending_advances > 0 {
            let min_ms = self.min_duration_for_current();
            let elapsed = self.step_visible_since.map(|t| t.elapsed()).unwrap_or_default();
            if elapsed >= Duration::from_millis(min_ms) {
                self.do_advance();
            }
        }
    }

    fn min_duration_for_current(&self) -> u64 {
        if self.current_step < self.steps.len() {
            if self.steps[self.current_step].label.contains("ncrypt") {
                MIN_ENCRYPT_MS
            } else {
                MIN_STEP_MS
            }
        } else {
            MIN_STEP_MS
        }
    }
}

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
    /// A newer version is available.
    UpdateAvailable(String),
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
    pub progress: Option<ProgressState>,

    // Error state
    pub last_error: Option<String>,

    // Update notice
    pub update_available: Option<String>,

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
        let (tx, rx) = mpsc::unbounded_channel();

        // Scan local backup directory
        let mut local_backup_count: u32 = 0;
        let mut local_backup_size: u64 = 0;
        let mut latest_local: Option<String> = None;
        let mut latest_modified: Option<std::time::SystemTime> = None;

        let local_dir = std::path::Path::new(&config.local_path);
        if local_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(local_dir) {
                for entry in entries.flatten() {
                    let fname = entry.file_name();
                    let fname_str = fname.to_string_lossy();
                    if fname_str.ends_with(".tar.gz.enc") {
                        if let Ok(meta) = entry.metadata() {
                            local_backup_count += 1;
                            local_backup_size += meta.len();
                            let modified = meta.modified().ok();
                            if latest_modified.is_none()
                                || (modified.is_some() && modified > latest_modified)
                            {
                                latest_modified = modified;
                                latest_local = Some(fname_str.to_string());
                            }
                        }
                    }
                }
            }
        }

        Self {
            config,
            key,
            version,
            view: AppView::Dashboard,
            should_quit: false,
            backups: Vec::new(),
            selected_backup: 0,
            api_connected: false,
            account: None,
            local_backup_count,
            local_backup_size,
            latest_local,
            operation_running: false,
            progress: None,
            last_error: None,
            update_available: None,
            confirm_message: String::new(),
            confirm_cursor: 0,
            rx,
            tx,
        }
    }

    /// Run the TUI application main loop.
    pub async fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        // Load all initial data in a single task to avoid request bursts
        self.load_initial_data();
        self.check_for_updates();

        loop {
            terminal.draw(|frame| self.draw(frame))?;

            // Check for crossterm events with a small timeout (50ms)
            if crossterm::event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = crossterm::event::read()? {
                    self.handle_key(key);
                }
            }

            // Drain messages from background tasks
            while let Ok(msg) = self.rx.try_recv() {
                self.handle_message(msg);
            }

            // Tick progress: advance delayed steps whose min-display time has elapsed
            self.tick_progress();

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// Draw the current frame.
    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        match self.view {
            AppView::Dashboard => dashboard::draw(self, frame, area),
            AppView::Picker => picker::draw(self, frame, area),
            AppView::Confirm => {
                // Draw dashboard underneath, then overlay confirm dialog
                dashboard::draw(self, frame, area);
                confirm::draw(self, frame, area);
            }
        }
    }

    /// Handle a key event.
    fn handle_key(&mut self, key: KeyEvent) {
        match self.view {
            AppView::Dashboard => self.handle_dashboard_key(key),
            AppView::Picker => self.handle_picker_key(key),
            AppView::Confirm => self.handle_confirm_key(key),
        }
    }

    fn handle_dashboard_key(&mut self, key: KeyEvent) {
        // If progress is showing and completed, any key dismisses it
        if let Some(ref p) = self.progress {
            if p.completed {
                self.operation_running = false;
                self.progress = None;
                return;
            }
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => {
                self.should_quit = true;
            }
            (KeyCode::Char('b'), _) => {
                if !self.operation_running {
                    self.start_backup();
                }
            }
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                self.refresh_backups();
            }
            (KeyCode::Char('r'), _) => {
                if !self.backups.is_empty() && !self.operation_running {
                    let backup = &self.backups[self.selected_backup];
                    self.confirm_message =
                        format!("Restore {}?", backup.filename);
                    self.confirm_cursor = 0;
                    self.view = AppView::Confirm;
                }
            }
            (KeyCode::Char('j'), _) | (KeyCode::Down, _) => {
                if !self.backups.is_empty() {
                    self.selected_backup =
                        (self.selected_backup + 1) % self.backups.len();
                }
            }
            (KeyCode::Char('k'), _) | (KeyCode::Up, _) => {
                if !self.backups.is_empty() {
                    if self.selected_backup == 0 {
                        self.selected_backup = self.backups.len() - 1;
                    } else {
                        self.selected_backup -= 1;
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_picker_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.view = AppView::Dashboard;
            }
            KeyCode::Enter => {
                if !self.backups.is_empty() {
                    let backup_id = self.backups[self.selected_backup].id.clone();
                    self.view = AppView::Dashboard;
                    self.start_restore(backup_id);
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.backups.is_empty() {
                    self.selected_backup =
                        (self.selected_backup + 1) % self.backups.len();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if !self.backups.is_empty() {
                    if self.selected_backup == 0 {
                        self.selected_backup = self.backups.len() - 1;
                    } else {
                        self.selected_backup -= 1;
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.view = AppView::Dashboard;
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.confirm_cursor = 0;
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
                self.confirm_cursor = 1;
            }
            KeyCode::Enter => {
                if self.confirm_cursor == 1 && !self.backups.is_empty() {
                    let backup_id = self.backups[self.selected_backup].id.clone();
                    self.view = AppView::Dashboard;
                    self.start_restore(backup_id);
                } else {
                    self.view = AppView::Dashboard;
                }
            }
            _ => {}
        }
    }

    /// Handle a message from a background task.
    fn handle_message(&mut self, msg: AppMessage) {
        match msg {
            AppMessage::Progress(_message, _detail) => {
                if let Some(ref mut p) = self.progress {
                    p.advance();
                }
            }
            AppMessage::BackupComplete(result) => {
                if let Some(ref mut p) = self.progress {
                    p.finalize_current();
                    p.completed = true;
                    p.backup_result = Some(result.clone());
                }
                self.last_error = None;
                self.local_backup_count += 1;
                self.local_backup_size += result.size;
                self.latest_local = Some(result.filename);
                self.refresh_backups();
            }
            AppMessage::RestoreComplete(result) => {
                if let Some(ref mut p) = self.progress {
                    p.finalize_current();
                    p.completed = true;
                    p.restore_result = Some(result);
                }
                self.last_error = None;
            }
            AppMessage::Error(err) => {
                self.operation_running = false;
                self.progress = None;
                self.last_error = Some(err);
            }
            AppMessage::BackupsLoaded(backups) => {
                self.backups = backups;
                if self.selected_backup >= self.backups.len() && !self.backups.is_empty() {
                    self.selected_backup = self.backups.len() - 1;
                }
            }
            AppMessage::ApiStatus(connected) => {
                self.api_connected = connected;
            }
            AppMessage::AccountLoaded(info) => {
                self.account = Some(info);
                self.api_connected = true;
            }
            AppMessage::UpdateAvailable(tag) => {
                self.update_available = Some(tag);
            }
        }
    }

    /// Initialize progress state for backup or restore.
    fn init_progress(&mut self, step_labels: &[&str]) {
        self.progress = Some(ProgressState::new(step_labels));
    }

    /// Tick: check if delayed step advances should fire now.
    fn tick_progress(&mut self) {
        if let Some(ref mut p) = self.progress {
            p.tick();
        }
    }

    /// Start a backup operation in the background.
    fn start_backup(&mut self) {
        self.operation_running = true;
        self.init_progress(BACKUP_STEPS);
        self.last_error = None;

        let tx = self.tx.clone();
        let config = self.config.clone();
        let key = self.key;
        let version = self.version.clone();

        tokio::spawn(async move {
            let dumper = match crate::core::dumper::new_dumper(&config.database) {
                Ok(d) => d,
                Err(e) => {
                    let _ = tx.send(AppMessage::Error(e.to_string()));
                    return;
                }
            };
            let api = crate::core::api::ApiClient::new(
                &config.api_key,
                &config.api_url,
                &version,
            );
            let service = crate::core::backup::BackupService {
                config,
                dumper,
                key,
                api,
            };

            let tx_progress = tx.clone();
            let progress_fn: crate::core::types::ProgressFn =
                Box::new(move |msg: &str, detail: Option<&str>| {
                    let _ = tx_progress.send(AppMessage::Progress(
                        msg.to_string(),
                        detail.map(|s| s.to_string()),
                    ));
                });

            match service.backup(progress_fn).await {
                Ok(result) => {
                    let _ = tx.send(AppMessage::BackupComplete(result));
                }
                Err(e) => {
                    let _ = tx.send(AppMessage::Error(e.to_string()));
                }
            }
        });
    }

    /// Start a restore operation in the background.
    fn start_restore(&mut self, backup_id: String) {
        self.operation_running = true;
        self.init_progress(RESTORE_STEPS);
        self.last_error = None;

        let tx = self.tx.clone();
        let config = self.config.clone();
        let key = self.key;
        let version = self.version.clone();

        tokio::spawn(async move {
            let dumper = match crate::core::dumper::new_dumper(&config.database) {
                Ok(d) => d,
                Err(e) => {
                    let _ = tx.send(AppMessage::Error(e.to_string()));
                    return;
                }
            };
            let api = crate::core::api::ApiClient::new(
                &config.api_key,
                &config.api_url,
                &version,
            );
            let service = crate::core::restore::RestoreService {
                config,
                dumper,
                key,
                api,
            };

            let tx_progress = tx.clone();
            let progress_fn: crate::core::types::ProgressFn =
                Box::new(move |msg: &str, detail: Option<&str>| {
                    let _ = tx_progress.send(AppMessage::Progress(
                        msg.to_string(),
                        detail.map(|s| s.to_string()),
                    ));
                });

            match service.restore(&backup_id, false, progress_fn).await {
                Ok(result) => {
                    let _ = tx.send(AppMessage::RestoreComplete(result));
                }
                Err(e) => {
                    let _ = tx.send(AppMessage::Error(e.to_string()));
                }
            }
        });
    }

    /// Load all initial data sequentially in one task to avoid request bursts.
    fn load_initial_data(&self) {
        let tx = self.tx.clone();
        let config = self.config.clone();
        let version = self.version.clone();

        tokio::spawn(async move {
            let api = crate::core::api::ApiClient::new(
                &config.api_key,
                &config.api_url,
                &version,
            );

            // 1. Account info (also serves as health check)
            match api.get_account().await {
                Ok(info) => {
                    let _ = tx.send(AppMessage::AccountLoaded(info));
                }
                Err(_) => {
                    let _ = tx.send(AppMessage::ApiStatus(false));
                }
            }

            // 2. Backup list
            match api.list_backups(1, 50).await {
                Ok(backups) => {
                    let _ = tx.send(AppMessage::BackupsLoaded(backups));
                }
                Err(_) => {
                    // Silent — dashboard will show empty list
                }
            }
        });
    }

    /// Refresh just the backup list (e.g. after a backup completes).
    fn refresh_backups(&self) {
        let tx = self.tx.clone();
        let config = self.config.clone();
        let version = self.version.clone();

        tokio::spawn(async move {
            let api = crate::core::api::ApiClient::new(
                &config.api_key,
                &config.api_url,
                &version,
            );
            if let Ok(backups) = api.list_backups(1, 50).await {
                let _ = tx.send(AppMessage::BackupsLoaded(backups));
            }
        });
    }

    /// Check for updates in the background.
    fn check_for_updates(&self) {
        let tx = self.tx.clone();
        let version = self.version.clone();

        tokio::spawn(async move {
            if version == "dev" {
                return;
            }
            if let Ok((tag, needs_update)) = crate::core::updater::check(&version).await {
                if needs_update {
                    let _ = tx.send(AppMessage::UpdateAvailable(tag));
                }
            }
        });
    }
}
