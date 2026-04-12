//! CLI entrypoint wiring for Timelocked.
//! It parses top-level flags, handles shared setup, and dispatches commands.

mod handlers;
mod models;
mod progress;
mod render;

use clap::Parser;
use tracing_subscriber::EnvFilter;

pub fn run() -> anyhow::Result<()> {
    let cli = models::Cli::parse();
    init_tracing(cli.verbose);
    let command = cli.command.unwrap_or(models::Commands::Tui);

    handlers::run(
        command,
        handlers::CommandOptions {
            json_mode: cli.json,
            quiet: cli.quiet,
            no_color: cli.no_color,
        },
    )
}

fn init_tracing(verbose: bool) {
    let filter = if verbose { "debug" } else { "warn" };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(filter))
        .with_target(false)
        .try_init();
}
