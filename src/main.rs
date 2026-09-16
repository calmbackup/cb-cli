mod cli;
mod core;
mod tui;

use clap::{Parser, Subcommand};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(
    name = "calmbackup",
    about = "Zero-knowledge encrypted database backups"
)]
#[command(version = VERSION)]
struct Cli {
    /// Path to config file
    #[arg(long, global = true)]
    config: Option<String>,

    /// Output as JSON (CLI mode only)
    #[arg(long, global = true)]
    json: bool,

    /// Suppress non-error output (CLI mode only)
    #[arg(long, short, global = true)]
    quiet: bool,

    /// Skip the automatic update check before `run` (operator-managed upgrades)
    #[arg(long, global = true)]
    no_auto_update: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a backup now
    Run,

    /// Restore a backup
    Restore {
        /// Backup ID to restore (optional)
        backup_id: Option<String>,

        /// Restore the latest backup automatically
        #[arg(long)]
        latest: bool,

        /// Delete local copy after restore
        #[arg(long)]
        prune_local: bool,
    },

    /// List all backups (local and cloud)
    List,

    /// Show backup status
    Status,

    /// Initialize configuration
    Init,

    /// Show version
    Version,
}

fn should_auto_update(cli: &Cli, version: &str) -> bool {
    matches!(&cli.command, Some(Commands::Run)) && version != "dev" && !cli.no_auto_update
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mode = cli::output::OutputMode::detect(cli.json, cli.quiet);
    let config_path = cli.config.as_deref();

    // Cron and other non-interactive users must receive reliability fixes too.
    // Update before opening the database, then replace this process so the
    // pending backup is performed by the newly installed binary.
    if should_auto_update(&cli, VERSION) {
        match core::updater::check(VERSION).await {
            Ok((tag, true)) => {
                if mode == cli::output::OutputMode::Styled {
                    eprintln!("Updating CalmBackup to {tag} before backup...");
                }

                match core::updater::update(&tag).await {
                    Ok(()) => restart_after_update()?,
                    Err(error) => eprintln!(
                        "Warning: automatic CalmBackup update to {tag} failed; continuing with v{VERSION}: {error}"
                    ),
                }
            }
            Ok((_tag, false)) => {}
            Err(error) => {
                if mode != cli::output::OutputMode::Quiet {
                    eprintln!("Warning: could not check for CalmBackup updates: {error}");
                }
            }
        }
    }

    match cli.command {
        // No subcommand → launch TUI dashboard
        None => {
            let config_path = match config_path {
                Some(p) => std::path::PathBuf::from(p),
                None => core::config::Config::find_config_file().ok_or_else(|| {
                    anyhow::anyhow!("No config file found. Run `calmbackup init` to create one.")
                })?,
            };
            let config = core::config::Config::load(&config_path)?;
            let key = core::crypto::derive_key(&config.encryption_key);
            use std::io::IsTerminal;
            if !std::io::stdin().is_terminal() {
                anyhow::bail!(
                    "TUI requires an interactive terminal. Use `calmbackup run` for non-interactive mode."
                );
            }

            let app = tui::app::App::new(config, key, VERSION.to_string());

            let mut terminal = ratatui::init();
            let result = app.run(&mut terminal).await;
            ratatui::restore();
            result?;
        }

        // Subcommands → CLI mode
        Some(Commands::Run) => {
            cli::run::execute(config_path, mode).await?;
        }
        Some(Commands::Restore {
            backup_id,
            latest,
            prune_local,
        }) => {
            cli::restore::execute(config_path, backup_id.as_deref(), latest, prune_local, mode)
                .await?;
        }
        Some(Commands::List) => {
            cli::list::execute(config_path, mode).await?;
        }
        Some(Commands::Status) => {
            cli::status::execute(config_path, mode).await?;
        }
        Some(Commands::Init) => {
            cli::init::execute().await?;
        }
        Some(Commands::Version) => {
            cli::version::execute(VERSION);
        }
    }

    Ok(())
}

#[cfg(unix)]
fn restart_after_update() -> anyhow::Result<()> {
    use std::os::unix::process::CommandExt;

    let executable = std::env::current_exe()?;
    let error = std::process::Command::new(executable)
        .args(std::env::args_os().skip(1))
        .exec();

    Err(error.into())
}

#[cfg(not(unix))]
fn restart_after_update() -> anyhow::Result<()> {
    anyhow::bail!("CalmBackup was updated; rerun the command to use the new version")
}

#[cfg(test)]
mod update_policy_tests {
    use super::*;

    #[test]
    fn ordinary_backup_keeps_automatic_updates() {
        let cli = Cli::try_parse_from(["calmbackup", "run"]).unwrap();
        assert!(should_auto_update(&cli, "2.0.11"));
        assert!(!should_auto_update(&cli, "dev"));
    }

    #[test]
    fn managed_backup_can_opt_out_before_or_after_subcommand() {
        for args in [
            ["calmbackup", "--no-auto-update", "run"],
            ["calmbackup", "run", "--no-auto-update"],
        ] {
            let cli = Cli::try_parse_from(args).unwrap();
            assert!(!should_auto_update(&cli, "2.0.11"));
        }
    }

    #[test]
    fn other_commands_do_not_gain_automatic_updates() {
        for args in [
            vec!["calmbackup"],
            vec!["calmbackup", "list"],
            vec!["calmbackup", "status"],
            vec!["calmbackup", "version"],
            vec!["calmbackup", "restore", "--latest"],
        ] {
            let cli = Cli::try_parse_from(args).unwrap();
            assert!(!should_auto_update(&cli, "2.0.11"));
        }
    }
}
