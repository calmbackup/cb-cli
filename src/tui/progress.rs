use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use crate::tui::app::App;
use crate::tui::theme;

/// Render inline progress within the dashboard (replaces backup list area).
/// Shows current step, detail message, and animated dots.
pub fn draw(app: &App, frame: &mut Frame, area: Rect) {
    // Center content vertically — allocate 3 lines in the middle
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    let center = vertical[1];

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(center);

    // Animated dots based on elapsed time
    let dot_count = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        / 500
        % 4) as usize;
    let dots = ".".repeat(dot_count);

    // Step line: → message...
    let step_line = Line::from(vec![
        Span::styled("  \u{2192} ", theme::step_style()),
        Span::styled(
            format!("{}{}", app.progress_message, dots),
            theme::step_style(),
        ),
    ]);
    frame.render_widget(Paragraph::new(step_line), inner[0]);

    // Detail line (if present)
    if let Some(ref detail) = app.progress_detail {
        let detail_line = Line::from(vec![
            Span::raw("    "),
            Span::styled(detail.as_str(), theme::label_style()),
        ]);
        frame.render_widget(Paragraph::new(detail_line), inner[1]);
    }
}
