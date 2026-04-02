use ratatui::style::{Color, Modifier, Style};

// Calm Backup color palette
pub const BRAND_GREEN: Color = Color::Rgb(142, 192, 124);
pub const BRAND_CYAN: Color = Color::Rgb(131, 165, 152);
pub const BRAND_YELLOW: Color = Color::Rgb(250, 189, 47);
pub const BRAND_RED: Color = Color::Rgb(251, 73, 52);
pub const BRAND_DIM: Color = Color::Rgb(146, 131, 116);
pub const BRAND_FG: Color = Color::Rgb(235, 219, 178);
pub const BRAND_BG: Color = Color::Rgb(40, 40, 40);
pub const BRAND_SURFACE: Color = Color::Rgb(50, 48, 47);
pub const BRAND_BORDER: Color = Color::Rgb(80, 73, 69);

pub fn title_style() -> Style {
    Style::default()
        .fg(BRAND_GREEN)
        .add_modifier(Modifier::BOLD)
}

pub fn header_style() -> Style {
    Style::default()
        .fg(BRAND_FG)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
}

pub fn label_style() -> Style {
    Style::default().fg(BRAND_DIM)
}

pub fn value_style() -> Style {
    Style::default().fg(BRAND_FG)
}

pub fn success_style() -> Style {
    Style::default().fg(BRAND_GREEN)
}

pub fn step_style() -> Style {
    Style::default().fg(BRAND_CYAN)
}

pub fn error_style() -> Style {
    Style::default().fg(BRAND_RED)
}

pub fn selected_style() -> Style {
    Style::default()
        .fg(BRAND_YELLOW)
        .add_modifier(Modifier::BOLD)
}

pub fn border_style() -> Style {
    Style::default().fg(BRAND_BORDER)
}

pub fn keybind_key_style() -> Style {
    Style::default().fg(BRAND_CYAN)
}

pub fn keybind_desc_style() -> Style {
    Style::default().fg(BRAND_DIM)
}
