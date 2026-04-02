use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use crate::core::types::{format_size, format_time};
use crate::tui::app::App;
use crate::tui::{progress, theme};

const TAGLINES: &[&str] = &[
    "Encrypted. Automated.",
    "Sleep well tonight.",
    "Your data, your keys.",
    "Backups without worry.",
    "Peace of mind, encrypted.",
];

/// Render the main dashboard view: header + status panel + backup list + keybinds.
pub fn draw(app: &App, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Length(8),  // status panel (with padding)
            Constraint::Min(5),    // backup list / progress
            Constraint::Length(1), // keybind bar
        ])
        .split(area);

    draw_header(app, frame, chunks[0]);
    draw_status(app, frame, chunks[1]);
    draw_backup_list(app, frame, chunks[2]);
    draw_keybinds(app, frame, chunks[3]);
}

/// Render the branded header bar.
fn draw_header(app: &App, frame: &mut Frame, area: Rect) {
    // Pick a tagline based on a simple hash of the version to keep it stable per session
    let tagline_idx = app.version.len() % TAGLINES.len();
    let tagline = TAGLINES[tagline_idx];

    let mut spans = vec![
        Span::styled(
            format!("  \u{1fab7} Calm Backup v{}", app.version),
            theme::title_style(),
        ),
        Span::raw("    "),
        Span::styled(tagline, theme::label_style()),
    ];

    if app.update_done {
        spans.push(Span::raw("    "));
        spans.push(Span::styled(
            "\u{2713} Updated — restart to use new version",
            Style::default().fg(theme::BRAND_GREEN).add_modifier(Modifier::BOLD),
        ));
    } else if app.updating {
        let dots = ".".repeat(
            (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() / 400 % 4) as usize,
        );
        spans.push(Span::raw("    "));
        spans.push(Span::styled(
            format!("{}{}", app.update_step, dots),
            Style::default().fg(theme::BRAND_CYAN),
        ));
    } else if let Some(ref tag) = app.update_available {
        spans.push(Span::raw("    "));
        spans.push(Span::styled(
            format!("Updating to {}...", tag),
            Style::default().fg(theme::BRAND_YELLOW),
        ));
    }

    let title = Line::from(spans);

    let header = Paragraph::new(title)
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(theme::border_style()),
        );

    frame.render_widget(header, area);
}

/// Fixed-width label helper — right-pads the label so values align.
fn status_label(label: &str, width: usize) -> Span<'static> {
    Span::styled(format!("{:>width$}  ", label, width = width), theme::label_style())
}

/// Render the status overview panel.
fn draw_status(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(Span::styled(" Status ", theme::header_style()))
        .borders(Borders::ALL)
        .border_style(theme::border_style());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Vertical layout: top padding, row 1, spacing, row 2, bottom padding
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // top padding
            Constraint::Length(1), // row 1
            Constraint::Length(1), // spacing
            Constraint::Length(1), // row 2
            Constraint::Min(0),   // bottom padding
        ])
        .split(inner);

    let label_width = 12; // fixed width for all labels

    // Row 1: Last backup | Local | Cloud
    let row1_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .split(rows[1]);

    // Last backup
    let last_backup_text = match &app.account {
        Some(info) => match &info.last_backup_at {
            Some(ts) => format_time(ts),
            None => "Never".to_string(),
        },
        None => "...".to_string(),
    };
    let last_backup = Line::from(vec![
        Span::raw("  "),
        status_label("Last backup", label_width),
        Span::styled(last_backup_text, theme::value_style()),
    ]);
    frame.render_widget(Paragraph::new(last_backup), row1_cols[0]);

    let local_text = format!(
        "{} files \u{00b7} {}",
        app.local_backup_count,
        format_size(app.local_backup_size),
    );
    let local_line = Line::from(vec![
        status_label("Local", label_width),
        Span::styled(local_text, theme::value_style()),
    ]);
    frame.render_widget(Paragraph::new(local_line), row1_cols[1]);

    let (cloud_icon, cloud_style) = if app.api_connected {
        ("\u{2713} ", theme::success_style())
    } else {
        ("\u{2717} ", theme::error_style())
    };
    let cloud_count = app
        .account
        .as_ref()
        .map(|a| format!("{} backups", a.backup_count))
        .unwrap_or_else(|| "...".to_string());
    let cloud_line = Line::from(vec![
        status_label("Cloud", label_width),
        Span::styled(cloud_icon, cloud_style),
        Span::styled(cloud_count, theme::value_style()),
    ]);
    frame.render_widget(Paragraph::new(cloud_line), row1_cols[2]);

    // Row 2: Retention | Database | Path
    let row2_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .split(rows[3]);

    let retention = Line::from(vec![
        Span::raw("  "),
        status_label("Retention", label_width),
        Span::styled(
            format!("{} days", app.config.local_retention_days),
            theme::value_style(),
        ),
    ]);
    frame.render_widget(Paragraph::new(retention), row2_cols[0]);

    let driver_label = match app.config.database.driver.as_str() {
        "pgsql" => "PostgreSQL",
        "mysql" => "MySQL",
        "sqlite" => "SQLite",
        other => other,
    };
    let db_line = Line::from(vec![
        status_label("Database", label_width),
        Span::styled(driver_label, theme::value_style()),
    ]);
    frame.render_widget(Paragraph::new(db_line), row2_cols[1]);

    let path = &app.config.local_path;
    let max_len = row2_cols[2].width.saturating_sub(label_width as u16 + 4) as usize;
    let display_path = if path.len() > max_len && max_len > 3 {
        format!("..{}", &path[path.len() - (max_len - 2)..])
    } else {
        path.clone()
    };
    let path_line = Line::from(vec![
        status_label("Path", label_width),
        Span::styled(display_path, theme::value_style()),
    ]);
    frame.render_widget(Paragraph::new(path_line), row2_cols[2]);
}

/// Render the compact backup list table or progress.
fn draw_backup_list(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(Span::styled(" Recent Backups ", theme::header_style()))
        .borders(Borders::ALL)
        .border_style(theme::border_style());

    if app.operation_running {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        progress::draw(app, frame, inner);
        return;
    }

    // Show error if present
    if let Some(ref err) = app.last_error {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let err_line = Line::from(vec![
            Span::styled("  Error: ", theme::error_style()),
            Span::styled(err.as_str(), theme::value_style()),
        ]);
        frame.render_widget(Paragraph::new(err_line), inner);
        return;
    }

    if app.backups.is_empty() {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let empty = Paragraph::new(Line::from(vec![
            Span::styled("  No backups yet. Press ", theme::label_style()),
            Span::styled("b", theme::keybind_key_style()),
            Span::styled(" to create one.", theme::label_style()),
        ]));
        frame.render_widget(empty, inner);
        return;
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Check which filenames exist locally
    let local_dir = std::path::Path::new(&app.config.local_path);
    let local_files: std::collections::HashSet<String> = if local_dir.exists() {
        std::fs::read_dir(local_dir)
            .ok()
            .map(|entries| {
                entries
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect()
            })
            .unwrap_or_default()
    } else {
        std::collections::HashSet::new()
    };

    // Build rows
    let rows: Vec<Row> = app
        .backups
        .iter()
        .enumerate()
        .map(|(i, backup)| {
            let is_local = local_files.contains(&backup.filename);
            let location = if is_local {
                "Local + Cloud"
            } else {
                "Cloud"
            };

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
                Cell::from(location),
            ])
            .style(style)
        })
        .collect();

    let total = app.backups.len();
    let showing = total;

    // Reserve 1 line for footer text
    let table_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let widths = [
        Constraint::Min(30),
        Constraint::Length(20),
        Constraint::Length(10),
        Constraint::Length(14),
    ];

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec![
                Cell::from("  Filename"),
                Cell::from("Date"),
                Cell::from("Size"),
                Cell::from("Location"),
            ])
            .style(theme::label_style()),
        )
        .row_highlight_style(theme::selected_style());

    frame.render_widget(table, table_area[0]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(
                "  \u{2191}\u{2193} navigate \u{00b7} showing {} of {}",
                showing, total
            ),
            theme::label_style(),
        ),
    ]));
    frame.render_widget(footer, table_area[1]);
}

/// Render the keybind bar at the bottom.
fn draw_keybinds(_app: &App, frame: &mut Frame, area: Rect) {
    let keybinds = Line::from(vec![
        Span::raw("  "),
        Span::styled("b", theme::keybind_key_style()),
        Span::styled(" backup  ", theme::keybind_desc_style()),
        Span::styled("r", theme::keybind_key_style()),
        Span::styled(" restore  ", theme::keybind_desc_style()),
        Span::styled("\u{2191}\u{2193}", theme::keybind_key_style()),
        Span::styled(" navigate  ", theme::keybind_desc_style()),
        Span::styled("q", theme::keybind_key_style()),
        Span::styled(" quit", theme::keybind_desc_style()),
    ]);

    frame.render_widget(Paragraph::new(keybinds), area);
}
