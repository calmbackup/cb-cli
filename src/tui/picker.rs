use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use crate::core::types::format_size;
use crate::tui::app::App;
use crate::tui::theme;

/// Render the backup selection picker (full-screen list for restore).
pub fn draw(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(Span::styled(
            " Select Backup to Restore ",
            theme::header_style(),
        ))
        .borders(Borders::ALL)
        .border_style(theme::border_style());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.backups.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(
            "  No backups available.",
            theme::label_style(),
        )));
        frame.render_widget(empty, inner);
        return;
    }

    // Split inner into table area + footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    // Build rows
    let rows: Vec<Row> = app
        .backups
        .iter()
        .enumerate()
        .map(|(i, backup)| {
            let date = {
                use chrono::{DateTime, Utc};
                backup
                    .created_at
                    .parse::<DateTime<Utc>>()
                    .map(|dt| dt.format("%b %-d, %Y %H:%M").to_string())
                    .unwrap_or_else(|_| backup.created_at.clone())
            };
            let size = format_size(backup.size);

            let style = if i == app.selected_backup {
                theme::selected_style()
            } else {
                theme::value_style()
            };

            let prefix = if i == app.selected_backup {
                "> "
            } else {
                "  "
            };

            Row::new(vec![
                Cell::from(format!("{}\u{1f512} {}", prefix, backup.filename)),
                Cell::from(date),
                Cell::from(size),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Min(30),
        Constraint::Length(20),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths);
    frame.render_widget(table, chunks[0]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("  Enter", theme::keybind_key_style()),
        Span::styled(" to select \u{00b7} ", theme::keybind_desc_style()),
        Span::styled("Esc", theme::keybind_key_style()),
        Span::styled(" to cancel", theme::keybind_desc_style()),
    ]));
    frame.render_widget(footer, chunks[1]);
}
