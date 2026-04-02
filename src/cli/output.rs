use std::io::Write;
use std::path::PathBuf;

use crate::core::config::Config;
use crate::core::types::{AppError, Result};

/// Output mode determined by TTY detection and flags.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputMode {
    /// Styled terminal output (colors, arrows, checkmarks).
    Styled,
    /// JSON output for machine consumption.
    Json,
    /// Quiet mode — errors only.
    Quiet,
}

impl OutputMode {
    /// Detect output mode from flags and TTY state.
    pub fn detect(json: bool, quiet: bool) -> Self {
        if json {
            OutputMode::Json
        } else if quiet {
            OutputMode::Quiet
        } else {
            OutputMode::Styled
        }
    }
}

/// 2-space left padding for all styled output.
const PAD: &str = "  ";

// ANSI escape codes
const CYAN: &str = "\x1b[36m";
const GREEN: &str = "\x1b[32m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const UNDERLINE: &str = "\x1b[4m";
const RESET: &str = "\x1b[0m";

/// Print the branded header with a random tagline.
pub fn print_header() {
    use rand::Rng;
    let tagline = TAGLINES[rand::thread_rng().gen_range(0..TAGLINES.len())];
    println!("{PAD}{BOLD}{GREEN}🪷 Calm Backup{RESET}  {DIM}{tagline}{RESET}");
    println!();
}

/// Print a progress step: "  → message" in cyan.
pub fn print_step(msg: &str) {
    println!("{PAD}{CYAN}→{RESET} {msg}");
}

/// Print a completion: "  ✓ message" in green.
pub fn print_done(msg: &str) {
    println!("{PAD}{GREEN}✓{RESET} {msg}");
}

/// Print an info line with padding.
pub fn print_info(msg: &str) {
    println!("{PAD}{msg}");
}

/// Print a key-value label line.
pub fn print_label(label: &str, value: &str) {
    println!("{PAD}{DIM}{label}{RESET}  {value}");
}

/// Print a section header with underline.
pub fn print_section(title: &str) {
    println!();
    println!("{PAD}{BOLD}{UNDERLINE}{title}{RESET}");
}

/// Print a success banner: "  🪷 message" in bold green.
pub fn print_success(msg: &str) {
    println!();
    println!("{PAD}{BOLD}{GREEN}🪷 {msg}{RESET}");
    println!();
}

/// Load config from the given path or auto-detect.
pub fn load_config(config_path: Option<&str>) -> Result<Config> {
    let path = match config_path {
        Some(p) => PathBuf::from(p),
        None => Config::find_config_file()
            .ok_or_else(|| AppError::Config("No config file found. Run 'calmbackup init' first.".into()))?,
    };
    Config::load(&path)
}

/// Prompt the user for a line of text input.
pub fn prompt(message: &str) -> Result<String> {
    print!("{PAD}{message}");
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

/// Random taglines for the branded header.
const TAGLINES: &[&str] = &[
    "Your data is safe with us.",
    "Encrypted. Automated. Calm.",
    "Sleep well, your backups are handled.",
    "Zero-knowledge. Full confidence.",
    "Backups you can trust.",
    "Set it and forget it.",
    "Your database's safety net.",
    "Peaceful backups, every time.",
    "Secure by default.",
    "One less thing to worry about.",
];
