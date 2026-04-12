//! Clap models for the Timelocked CLI.
//! These types describe the command surface and stay separate from execution logic.

use std::path::PathBuf;

use clap::{ArgGroup, Args, Parser, Subcommand};

use crate::domains::timelock::{all_profiles, CURRENT_MACHINE_PROFILE_ID};

#[derive(Debug, Parser)]
#[command(
    name = "timelocked",
    version,
    about = "Timelocked - create and unlock timed-release files (.timelocked)",
    long_about = "Timelocked - create and unlock timed-release files (.timelocked)\n\nIMPORTANT:\n  Timelocked enforces sequential work, not an exact unlock time.\n  Actual duration depends on hardware, thermals, power mode, and load."
)]
pub(crate) struct Cli {
    #[arg(long, global = true)]
    pub(crate) verbose: bool,

    #[arg(long, global = true)]
    pub(crate) quiet: bool,

    #[arg(long, global = true)]
    pub(crate) json: bool,

    #[arg(long, global = true)]
    pub(crate) no_color: bool,

    #[command(subcommand)]
    pub(crate) command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    Lock(LockArgs),
    Unlock(UnlockArgs),
    Inspect(InspectArgs),
    Verify(VerifyArgs),
    Calibrate,
    Tui,
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("difficulty")
        .required(true)
        .args(["target", "iterations"])
))]
pub(crate) struct LockArgs {
    #[arg(
        long = "in",
        value_name = "INPUT",
        required_unless_present = "input_arg",
        conflicts_with = "input_arg"
    )]
    pub(crate) input: Option<String>,

    #[arg(
        value_name = "INPUT",
        required_unless_present = "input",
        conflicts_with = "input"
    )]
    pub(crate) input_arg: Option<String>,

    #[arg(long = "out")]
    pub(crate) output: Option<PathBuf>,

    #[arg(long = "target")]
    pub(crate) target: Option<String>,

    #[arg(long = "iterations")]
    pub(crate) iterations: Option<u64>,

    #[arg(long = "hardware-profile")]
    pub(crate) hardware_profile: Option<String>,

    #[arg(long = "creator-name")]
    pub(crate) creator_name: Option<String>,

    #[arg(long = "creator-message", conflicts_with = "creator_message_file")]
    pub(crate) creator_message: Option<String>,

    #[arg(long = "creator-message-file", conflicts_with = "creator_message")]
    pub(crate) creator_message_file: Option<PathBuf>,

    #[arg(
        long = "verify",
        help = "Run structural verification after writing without unlocking the payload"
    )]
    pub(crate) verify: bool,
}

#[derive(Debug, Args)]
pub(crate) struct UnlockArgs {
    #[arg(
        long = "in",
        value_name = "PATH",
        required_unless_present = "input_arg",
        conflicts_with = "input_arg"
    )]
    pub(crate) input: Option<PathBuf>,

    #[arg(
        value_name = "PATH",
        required_unless_present = "input",
        conflicts_with = "input"
    )]
    pub(crate) input_arg: Option<PathBuf>,

    #[arg(long = "out-dir")]
    pub(crate) out_dir: Option<PathBuf>,

    #[arg(long = "out")]
    pub(crate) out: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct InspectArgs {
    #[arg(
        long = "in",
        value_name = "PATH",
        required_unless_present = "input_arg",
        conflicts_with = "input_arg"
    )]
    pub(crate) input: Option<PathBuf>,

    #[arg(
        value_name = "PATH",
        required_unless_present = "input",
        conflicts_with = "input"
    )]
    pub(crate) input_arg: Option<PathBuf>,
}

#[derive(Debug, Args)]
#[command(
    about = "Structurally verify a .timelocked file without unlocking",
    long_about = "Structurally verify a .timelocked file without unlocking. This validates the recoverable protected-stream structure and metadata consistency. Use unlock for full payload authentication and recovery."
)]
pub(crate) struct VerifyArgs {
    #[arg(
        long = "in",
        value_name = "PATH",
        required_unless_present = "input_arg",
        conflicts_with = "input_arg"
    )]
    pub(crate) input: Option<PathBuf>,

    #[arg(
        value_name = "PATH",
        required_unless_present = "input",
        conflicts_with = "input"
    )]
    pub(crate) input_arg: Option<PathBuf>,
}

#[allow(dead_code)]
pub(crate) fn profile_choices_for_help() -> String {
    let mut profile_ids = all_profiles()
        .iter()
        .map(|profile| profile.id)
        .collect::<Vec<_>>();
    profile_ids.push(CURRENT_MACHINE_PROFILE_ID);
    profile_ids.join(", ")
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Commands};

    #[test]
    fn parses_without_subcommand_for_default_tui() {
        let cli = Cli::try_parse_from(["timelocked"]).expect("parse cli");
        assert!(cli.command.is_none());
    }

    #[test]
    fn still_parses_explicit_tui_subcommand() {
        let cli = Cli::try_parse_from(["timelocked", "tui"]).expect("parse cli");
        assert!(matches!(cli.command, Some(Commands::Tui)));
    }

    #[test]
    fn parses_calibrate_subcommand() {
        let cli = Cli::try_parse_from(["timelocked", "calibrate"]).expect("parse cli");
        assert!(matches!(cli.command, Some(Commands::Calibrate)));
    }
}
