mod cli;
mod core;
mod tui;

use clap::{Parser, Subcommand};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "calmbackup", about = "Zero-knowledge encrypted database backups")]
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mode = cli::output::OutputMode::detect(cli.json, cli.quiet);
    let config_path = cli.config.as_deref();

    match cli.command {
        // No subcommand → launch TUI dashboard
        None => {
            todo!("Load config, derive key, create App, init terminal, run TUI")
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
            cli::restore::execute(
                config_path,
                backup_id.as_deref(),
                latest,
                prune_local,
                mode,
            )
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
