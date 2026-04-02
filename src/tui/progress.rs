use ratatui::Frame;
use ratatui::layout::Rect;
use crate::tui::app::App;

/// Render inline progress within the dashboard (replaces backup list area).
/// Shows current step, detail message, and a gauge bar.
pub fn draw(app: &App, frame: &mut Frame, area: Rect) {
    todo!("Step label + detail + gauge/spinner")
}
