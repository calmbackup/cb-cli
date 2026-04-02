use std::io::Write;

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

/// Print the branded header with a random tagline.
pub fn print_header() {
    todo!("Print 🪷 Calm Backup header with random tagline")
}

/// Print a progress step: "  → message" in cyan.
pub fn print_step(msg: &str) {
    todo!("Print step with cyan arrow")
}

/// Print a completion: "  ✓ message" in green.
pub fn print_done(msg: &str) {
    todo!("Print done with green checkmark")
}

/// Print an info line with padding.
pub fn print_info(msg: &str) {
    todo!("Print padded info line")
}

/// Print a key-value label line.
pub fn print_label(label: &str, value: &str) {
    todo!("Print label-value pair with faint label")
}

/// Print a section header with underline.
pub fn print_section(title: &str) {
    todo!("Print bold underlined section header")
}

/// Print a success banner: "  🪷 message" in bold green.
pub fn print_success(msg: &str) {
    todo!("Print success banner")
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
