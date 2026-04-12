//! CLI command handlers.
//! Each handler parses UI input, runs a usecase, and delegates rendering.

use std::fs;
use std::path::Path;

use anyhow::Context;

use crate::base::progress_status::ProgressStatus;
use crate::configuration::runtime::lock_modulus_bits;
use crate::usecases::{calibrate, inspect, lock, unlock, verify};
use crate::userinterfaces::tui;

use super::models::{Commands, InspectArgs, LockArgs, UnlockArgs, VerifyArgs};
use super::progress::ProgressReporter;
use super::render;

pub(crate) struct CommandOptions {
    pub(crate) json_mode: bool,
    pub(crate) quiet: bool,
    pub(crate) no_color: bool,
}

pub(crate) fn run(command: Commands, options: CommandOptions) -> anyhow::Result<()> {
    match command {
        Commands::Lock(args) => run_lock(args, options.json_mode, options.quiet),
        Commands::Unlock(args) => run_unlock(args, options.json_mode, options.quiet),
        Commands::Inspect(args) => run_inspect(args, options.json_mode, options.quiet),
        Commands::Verify(args) => run_verify(args, options.json_mode, options.quiet),
        Commands::Calibrate => run_calibrate(options.json_mode, options.quiet),
        Commands::Tui => run_tui(options.no_color),
    }
}

fn run_tui(no_color: bool) -> anyhow::Result<()> {
    tui::run(tui::TuiOptions { no_color })
}

fn run_lock(args: LockArgs, json_mode: bool, quiet: bool) -> anyhow::Result<()> {
    let LockArgs {
        input,
        input_arg,
        output,
        target,
        iterations,
        hardware_profile,
        creator_name,
        creator_message,
        creator_message_file,
        verify,
    } = args;

    let input_was_explicit = input.is_some();
    let input = required_value(input.or(input_arg), "lock input")?;

    if input_was_explicit && input != "-" && !Path::new(&input).exists() {
        return Err(anyhow::anyhow!("input file does not exist: {input}"));
    }

    let creator_message = resolve_creator_message(creator_message, creator_message_file)?;

    if !quiet && target.is_some() {
        eprintln!(
            "note: delay is estimate-only. Actual runtime depends on hardware, thermals, power mode, and system load."
        );
    }

    run_with_progress(
        json_mode,
        quiet,
        |on_progress| {
            lock::execute(
                lock::LockRequest {
                    input,
                    output,
                    modulus_bits: lock_modulus_bits(),
                    target,
                    iterations,
                    hardware_profile,
                    current_machine_iterations_per_second: None,
                    creator_name,
                    creator_message,
                    verify,
                },
                Some(on_progress),
            )
            .map_err(Into::into)
        },
        |response| render::render_lock_result(&response, json_mode, quiet),
    )
}

fn run_unlock(args: UnlockArgs, json_mode: bool, quiet: bool) -> anyhow::Result<()> {
    let UnlockArgs {
        input,
        input_arg,
        out_dir,
        out,
    } = args;
    let input = required_value(input.or(input_arg), "unlock input")?;

    run_with_progress(
        json_mode,
        quiet,
        |on_progress| {
            unlock::execute(
                unlock::UnlockRequest {
                    input,
                    out_dir,
                    out,
                },
                Some(on_progress),
            )
            .map_err(Into::into)
        },
        |response| render::render_unlock_result(&response, json_mode, quiet),
    )
}

fn run_inspect(args: InspectArgs, json_mode: bool, quiet: bool) -> anyhow::Result<()> {
    let InspectArgs { input, input_arg } = args;
    let input = required_value(input.or(input_arg), "inspect input")?;

    let response = inspect::execute(inspect::InspectRequest {
        input,
        current_machine_iterations_per_second: None,
    })?;
    render::render_inspect_result(&response, json_mode, quiet)
}

fn run_calibrate(json_mode: bool, quiet: bool) -> anyhow::Result<()> {
    let response = calibrate::execute()?;
    render::render_calibrate_result(&response, json_mode, quiet)
}

fn run_verify(args: VerifyArgs, json_mode: bool, quiet: bool) -> anyhow::Result<()> {
    let VerifyArgs { input, input_arg } = args;
    let input = required_value(input.or(input_arg), "verify input")?;

    run_with_progress(
        json_mode,
        quiet,
        |on_progress| {
            verify::execute(verify::VerifyRequest { input }, Some(on_progress)).map_err(Into::into)
        },
        |response| render::render_verify_result(&response, json_mode, quiet),
    )
}

fn required_value<T>(value: Option<T>, label: &str) -> anyhow::Result<T> {
    value.ok_or_else(|| anyhow::anyhow!("internal clap parsing error: missing {label}"))
}

fn resolve_creator_message(
    creator_message: Option<String>,
    creator_message_file: Option<std::path::PathBuf>,
) -> anyhow::Result<Option<String>> {
    match (creator_message, creator_message_file) {
        (Some(message), None) => Ok(Some(message)),
        (None, Some(path)) => Ok(Some(fs::read_to_string(&path).with_context(|| {
            format!("failed to read creator message file {}", path.display())
        })?)),
        (None, None) => Ok(None),
        _ => Ok(None),
    }
}

fn run_with_progress<T>(
    json_mode: bool,
    quiet: bool,
    run_usecase: impl FnOnce(&mut dyn FnMut(ProgressStatus)) -> anyhow::Result<T>,
    render_result: impl FnOnce(T) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let progress = ProgressReporter::new(json_mode, quiet);
    let mut callback = progress.callback();
    let response = run_usecase(&mut callback);
    progress.finish();
    render_result(response?)
}
