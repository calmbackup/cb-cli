use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table};
use crate::tui::app::App;

/// Render the main dashboard view: header + status panel + backup list + keybinds.
pub fn draw(app: &App, frame: &mut Frame, area: Rect) {
    todo!("Layout: header (3 lines), status panel, backup list (or progress), keybind bar")
}

/// Render the branded header bar.
fn draw_header(app: &App, frame: &mut Frame, area: Rect) {
    todo!("🪷 Calm Backup vX.X.X with tagline")
}

/// Render the status overview panel.
fn draw_status(app: &App, frame: &mut Frame, area: Rect) {
    todo!("Last backup, local count/size, cloud status, retention, DB driver, path")
}

/// Render the compact backup list table.
fn draw_backup_list(app: &App, frame: &mut Frame, area: Rect) {
    todo!("Scrollable table: filename, date, size, location (local/cloud/both)")
}

/// Render the keybind bar at the bottom.
fn draw_keybinds(app: &App, frame: &mut Frame, area: Rect) {
    todo!("b backup  r restore  l list  s status  i init  q quit")
}
