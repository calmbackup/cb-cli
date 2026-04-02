use std::time::Duration;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{DefaultTerminal, Frame};
use tokio::sync::mpsc;
use crate::core::config::Config;
use crate::core::types::{BackupEntry, Result};

use super::{confirm, dashboard, picker};

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

    // Error state
    pub last_error: Option<String>,

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
            progress_message: String::new(),
            progress_detail: None,
            last_error: None,
            confirm_message: String::new(),
            confirm_cursor: 0,
            rx,
            tx,
        }
    }

    /// Run the TUI application main loop.
    pub async fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        // Spawn initial data loading tasks
        self.refresh_backups();
        self.check_health();
        self.load_account();

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
            AppMessage::Progress(message, detail) => {
                self.progress_message = message;
                self.progress_detail = detail;
            }
            AppMessage::BackupComplete(result) => {
                self.operation_running = false;
                self.progress_message = format!(
                    "Backup complete: {} ({})",
                    result.filename,
                    crate::core::types::format_size(result.size)
                );
                self.progress_detail = None;
                self.last_error = None;
                // Update local stats
                self.local_backup_count += 1;
                self.local_backup_size += result.size;
                self.latest_local = Some(result.filename);
                // Refresh backup list from cloud
                self.refresh_backups();
            }
            AppMessage::RestoreComplete(result) => {
                self.operation_running = false;
                self.progress_message = format!(
                    "Restore complete: {}",
                    result.filename,
                );
                self.progress_detail = None;
                self.last_error = None;
            }
            AppMessage::Error(err) => {
                self.operation_running = false;
                self.last_error = Some(err);
                self.progress_detail = None;
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
        }
    }

    /// Start a backup operation in the background.
    fn start_backup(&mut self) {
        self.operation_running = true;
        self.progress_message = "Starting backup...".to_string();
        self.progress_detail = None;
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
        self.progress_message = "Starting restore...".to_string();
        self.progress_detail = None;
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

    /// Refresh the backup list from API.
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
            match api.list_backups(1, 50).await {
                Ok(backups) => {
                    let _ = tx.send(AppMessage::BackupsLoaded(backups));
                }
                Err(e) => {
                    let _ = tx.send(AppMessage::Error(format!("Failed to load backups: {}", e)));
                }
            }
        });
    }

    /// Check API health in the background.
    fn check_health(&self) {
        let tx = self.tx.clone();
        let config = self.config.clone();
        let version = self.version.clone();

        tokio::spawn(async move {
            let api = crate::core::api::ApiClient::new(
                &config.api_key,
                &config.api_url,
                &version,
            );
            let connected = api.health_check().await;
            let _ = tx.send(AppMessage::ApiStatus(connected));
        });
    }

    /// Load account info in the background.
    fn load_account(&self) {
        let tx = self.tx.clone();
        let config = self.config.clone();
        let version = self.version.clone();

        tokio::spawn(async move {
            let api = crate::core::api::ApiClient::new(
                &config.api_key,
                &config.api_url,
                &version,
            );
            match api.get_account().await {
                Ok(info) => {
                    let _ = tx.send(AppMessage::AccountLoaded(info));
                }
                Err(_) => {
                    let _ = tx.send(AppMessage::ApiStatus(false));
                }
            }
        });
    }
}
