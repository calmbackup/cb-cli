use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};
use crate::tui::app::{App, ProgressState, StepStatus};
use crate::tui::theme;

/// Muted terminal green palette for encryption animation.
const CRYPT_BRIGHT: Color = Color::Rgb(100, 160, 100);
const CRYPT_MID: Color = Color::Rgb(70, 120, 70);
const CRYPT_DIM: Color = Color::Rgb(50, 90, 50);
const CRYPT_FAINT: Color = Color::Rgb(35, 60, 35);

/// Source text that gets "encrypted" — fragments of readable data morphing into hex.
const PLAINTEXT_FRAGMENTS: &[&str] = &[
    "SELECT * FROM users",
    "INSERT INTO backups",
    "database.sqlite",
    "encryption_key",
    "created_at timestamp",
    "DROP TABLE sessions",
    "BEGIN TRANSACTION",
    "COMMIT",
    "api_key varchar(64)",
    "password_hash text",
];

/// Render the full progress view: step checklist + encryption animation + gauge bar.
pub fn draw(app: &App, frame: &mut Frame, area: Rect) {
    let Some(ref progress) = app.progress else {
        return;
    };

    let block = Block::default()
        .title(Span::styled(
            if progress.completed {
                " Backup Complete "
            } else {
                " Backup in Progress "
            },
            if progress.completed {
                theme::title_style()
            } else {
                theme::header_style()
            },
        ))
        .borders(Borders::ALL)
        .border_style(if progress.completed {
            Style::default().fg(theme::BRAND_GREEN)
        } else {
            theme::border_style()
        });

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Check if encryption step is active
    let encrypt_active = !progress.completed
        && progress.current_step < progress.steps.len()
        && progress.steps[progress.current_step].label.contains("ncrypt");

    let has_result = progress.completed && progress.backup_result.is_some();

    let constraints = if encrypt_active {
        vec![
            Constraint::Length(progress.steps.len() as u16 + 1),
            Constraint::Length(5),
            Constraint::Min(0),
            Constraint::Length(1),
        ]
    } else if has_result {
        vec![
            Constraint::Length(progress.steps.len() as u16 + 1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(5),
        ]
    } else {
        vec![
            Constraint::Length(progress.steps.len() as u16 + 1),
            Constraint::Min(0),
            Constraint::Length(1),
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    draw_steps(progress, frame, chunks[0]);

    if encrypt_active {
        draw_encryption_animation(frame, chunks[1]);
        draw_gauge(progress, frame, chunks[3]);
    } else if has_result {
        draw_gauge(progress, frame, chunks[2]);
        draw_result(progress, frame, chunks[3]);
    } else {
        draw_gauge(progress, frame, chunks[2]);
    }
}

/// Draw the step checklist.
fn draw_steps(progress: &ProgressState, frame: &mut Frame, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    for step in &progress.steps {
        let (icon, icon_style, label_style) = match step.status {
            StepStatus::Complete => (
                "  \u{2713} ",
                Style::default().fg(theme::BRAND_GREEN),
                if step.label.contains("ncrypt") {
                    Style::default().fg(theme::BRAND_YELLOW).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme::BRAND_FG)
                },
            ),
            StepStatus::Active => (
                "  \u{2192} ",
                Style::default().fg(theme::BRAND_CYAN).add_modifier(Modifier::BOLD),
                Style::default().fg(theme::BRAND_CYAN).add_modifier(Modifier::BOLD),
            ),
            StepStatus::Pending => (
                "  \u{25CB} ",
                Style::default().fg(theme::BRAND_BORDER),
                Style::default().fg(theme::BRAND_BORDER),
            ),
        };

        let label_text = if step.status == StepStatus::Active {
            let dot_count = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                / 300
                % 4) as usize;
            format!("{}{}", step.label, ".".repeat(dot_count))
        } else {
            step.label.clone()
        };

        let mut spans = vec![
            Span::styled(icon, icon_style),
            Span::styled(label_text, label_style),
        ];

        if let Some(dur) = step.duration {
            spans.push(Span::styled(
                format!("  {:.1}s", dur.as_secs_f64()),
                Style::default().fg(theme::BRAND_DIM),
            ));
        }

        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

/// Draw the encryption animation — readable text morphing character-by-character into hex.
fn draw_encryption_animation(frame: &mut Frame, area: Rect) {
    let time_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let animation_area = Rect {
        x: area.x + 4,
        y: area.y,
        width: area.width.saturating_sub(8),
        height: area.height,
    };

    let usable_width = animation_area.width.saturating_sub(6) as usize;
    let hex_chars: &[u8] = b"0123456789abcdef";

    // How far through the encryption we are (cycles every ~3s)
    let cycle_ms = 3000u128;
    let progress_in_cycle = (time_ms % cycle_ms) as f64 / cycle_ms as f64;

    let mut lines: Vec<Line> = Vec::new();

    for row in 0..2u128 {
        // Pick a plaintext fragment for this row (cycles through them)
        let frag_idx = ((time_ms / cycle_ms + row) as usize) % PLAINTEXT_FRAGMENTS.len();
        let plaintext = PLAINTEXT_FRAGMENTS[frag_idx];

        // Pad or truncate to fill the row
        let padded: String = if plaintext.len() >= usable_width {
            plaintext[..usable_width].to_string()
        } else {
            format!("{:width$}", plaintext, width = usable_width)
        };

        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::styled("  ", Style::default()));

        let chars: Vec<char> = padded.chars().collect();
        for (col, &ch) in chars.iter().enumerate() {
            // Each character has a threshold — when progress passes it, it becomes hex
            // Characters encrypt left-to-right with some randomness
            let col_u128 = col as u128;
            let char_hash = col_u128
                .wrapping_mul(7919)
                .wrapping_add(row.wrapping_mul(104729))
                .wrapping_mul(1103515245)
                .wrapping_add(12345);
            let jitter = (char_hash % 30) as f64 / 100.0; // 0.0–0.3 random offset
            let threshold = (col as f64 / chars.len() as f64) * 0.7 + jitter;

            if progress_in_cycle > threshold {
                // This character is "encrypted" — show as hex
                let hex_seed = time_ms / 120; // changes every 120ms
                let h = hex_seed
                    .wrapping_add(col_u128.wrapping_mul(53))
                    .wrapping_add(row.wrapping_mul(97))
                    .wrapping_mul(1103515245);
                let hc = hex_chars[((h >> 4) % 16) as usize] as char;

                let color = match (char_hash / 7) % 4 {
                    0 => CRYPT_BRIGHT,
                    1 => CRYPT_MID,
                    2 => CRYPT_DIM,
                    _ => CRYPT_FAINT,
                };

                spans.push(Span::styled(
                    String::from(hc),
                    Style::default().fg(color),
                ));
            } else {
                // Still plaintext
                spans.push(Span::styled(
                    String::from(ch),
                    Style::default().fg(theme::BRAND_DIM),
                ));
            }
        }

        lines.push(Line::from(spans));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "    AES-256-GCM \u{00b7} zero-knowledge",
        Style::default().fg(CRYPT_DIM),
    )));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(CRYPT_FAINT));

    frame.render_widget(Paragraph::new(lines).block(block), animation_area);
}

/// Draw the progress gauge bar.
fn draw_gauge(progress: &ProgressState, frame: &mut Frame, area: Rect) {
    let total = progress.steps.len() as f64;
    let completed = progress.steps.iter().filter(|s| s.status == StepStatus::Complete).count() as f64;
    let active_bonus = if !progress.completed && progress.steps.iter().any(|s| s.status == StepStatus::Active) {
        0.5
    } else {
        0.0
    };
    let ratio = ((completed + active_bonus) / total).min(1.0);

    let gauge_area = Rect {
        x: area.x + 2,
        y: area.y,
        width: area.width.saturating_sub(4),
        height: 1,
    };

    if progress.completed {
        let gauge = Gauge::default()
            .ratio(1.0)
            .gauge_style(Style::default().fg(theme::BRAND_GREEN).bg(theme::BRAND_BORDER))
            .label(Span::styled(
                " Done \u{2713} ",
                Style::default()
                    .fg(Color::Rgb(29, 32, 33))
                    .bg(theme::BRAND_GREEN)
                    .add_modifier(Modifier::BOLD),
            ));
        frame.render_widget(gauge, gauge_area);
    } else {
        let label = format!(
            " {}/{} \u{00b7} {}% ",
            completed as u32,
            total as u32,
            (ratio * 100.0) as u32,
        );
        let gauge = Gauge::default()
            .ratio(ratio)
            .gauge_style(Style::default().fg(theme::BRAND_GREEN).bg(theme::BRAND_BORDER))
            .label(Span::styled(label, Style::default().fg(theme::BRAND_FG)));
        frame.render_widget(gauge, gauge_area);
    }
}

/// Draw the backup result card after completion.
fn draw_result(progress: &ProgressState, frame: &mut Frame, area: Rect) {
    let Some(ref result) = progress.backup_result else {
        return;
    };

    let result_area = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(1),
    };

    let now = chrono::Local::now();
    let lines = vec![
        Line::from(Span::styled(
            "  \u{1fab7} Backup complete",
            Style::default().fg(theme::BRAND_GREEN).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  \u{1f512} ", Style::default()),
            Span::styled(&result.filename, Style::default().fg(theme::BRAND_FG)),
        ]),
        Line::from(Span::styled(
            format!(
                "  {} \u{00b7} {:.1}s \u{00b7} {}",
                crate::core::types::format_size(result.size),
                result.duration.as_secs_f64(),
                now.format("%b %-d, %Y %H:%M"),
            ),
            Style::default().fg(theme::BRAND_DIM),
        )),
        Line::from(Span::styled(
            "  Press any key to continue",
            Style::default().fg(theme::BRAND_BORDER),
        )),
    ];

    frame.render_widget(Paragraph::new(lines), result_area);
}
