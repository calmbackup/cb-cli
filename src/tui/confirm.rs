use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use crate::tui::app::App;
use crate::tui::theme;

/// Calculate a centered rectangle within the given area.
fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup_width = area.width * percent_x / 100;
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;

    Rect::new(
        x,
        y,
        popup_width.min(area.width),
        height.min(area.height),
    )
}

/// Render a yes/no confirmation dialog overlay.
pub fn draw(app: &App, frame: &mut Frame, area: Rect) {
    let popup_area = centered_rect(60, 5, area);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(Span::styled(" Confirm ", theme::header_style()))
        .borders(Borders::ALL)
        .border_style(theme::border_style());

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    // Split inner into message line and button line
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // spacer
            Constraint::Length(1), // message
            Constraint::Length(1), // buttons
        ])
        .split(inner);

    // Message
    let message = Paragraph::new(Line::from(Span::styled(
        format!("  {}", app.confirm_message),
        theme::value_style(),
    )));
    frame.render_widget(message, chunks[1]);

    // Buttons
    let (no_style, yes_style) = if app.confirm_cursor == 0 {
        (theme::selected_style(), theme::label_style())
    } else {
        (theme::label_style(), theme::selected_style())
    };

    let buttons = Line::from(vec![
        Span::raw("  "),
        Span::styled("[ No ]", no_style),
        Span::raw("    "),
        Span::styled("[ Yes ]", yes_style),
    ]);
    frame.render_widget(Paragraph::new(buttons), chunks[2]);
}
