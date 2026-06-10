//! CLI command handlers.
//! Each handler parses UI input, runs a usecase, and delegates rendering.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use anyhow::Context;

use crate::base::progress_status::ProgressStatus;
use crate::base::{Error, Result, SecretString};
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
        password,
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
                    password: password_to_secret(password)?,
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
    run_unlock_with_password_reader(
        args,
        json_mode,
        quiet,
        read_password_from_terminal,
        io::stderr(),
    )
}

fn run_unlock_with_password_reader<R, W>(
    args: UnlockArgs,
    json_mode: bool,
    quiet: bool,
    mut password_reader: R,
    mut prompt_sink: W,
) -> anyhow::Result<()>
where
    R: FnMut() -> Result<SecretString>,
    W: Write,
{
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
            let mut password_provider = |context: unlock::PasswordPromptContext| {
                write!(prompt_sink, "{}", password_prompt_text(&context))?;
                prompt_sink.flush()?;
                password_reader()
            };

            unlock::execute_with_cancel_and_password_provider(
                unlock::UnlockRequest {
                    input,
                    out_dir,
                    out,
                },
                Some(on_progress),
                None,
                Some(&mut password_provider),
            )
            .map_err(Into::into)
        },
        |response| render::render_unlock_result(&response, json_mode, quiet),
    )
}

fn password_to_secret(password: Option<String>) -> anyhow::Result<Option<SecretString>> {
    match password {
        Some(password) if password.is_empty() => {
            Err(Error::InvalidArgument("password cannot be empty".to_string()).into())
        }
        Some(password) => Ok(Some(SecretString::new(password))),
        None => Ok(None),
    }
}

fn read_password_from_terminal() -> Result<SecretString> {
    rpassword::read_password()
        .map(SecretString::new)
        .map_err(|err| Error::InvalidArgument(format!("password prompt unavailable: {err}")))
}

fn password_prompt_text(context: &unlock::PasswordPromptContext) -> String {
    if context.previous_attempt_failed {
        let remaining_attempts = context
            .max_attempts
            .saturating_sub(context.attempt.saturating_sub(1));
        let attempt_label = if remaining_attempts == 1 {
            "attempt"
        } else {
            "attempts"
        };
        format!(
            "Password did not unlock this file. Try again ({remaining_attempts} {attempt_label} remaining): "
        )
    } else {
        "Password required for this timelocked file: ".to_string()
    }
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

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::fs;
    use std::io::{self, Write};
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    use tempfile::tempdir;

    use crate::base::SecretString;

    use super::*;

    #[test]
    fn cli_unlock_unprotected_does_not_prompt_for_password() {
        let dir = tempdir().expect("tempdir");
        let locked_path = dir.path().join("unprotected.timelocked");
        lock_text_payload(&locked_path, None);
        let args = unlock_args(&locked_path);
        let prompts = Rc::new(RefCell::new(Vec::new()));
        let calls = Rc::new(Cell::new(0));
        let reader_calls = Rc::clone(&calls);
        let reader = move || {
            reader_calls.set(reader_calls.get() + 1);
            Ok(SecretString::new("should not be requested".to_string()))
        };

        run_unlock_with_password_reader(
            args,
            false,
            true,
            reader,
            CaptureSink::new(Rc::clone(&prompts)),
        )
        .expect("unlock unprotected");

        assert_eq!(calls.get(), 0);
        assert!(prompts.borrow().is_empty());
    }

    #[test]
    fn cli_unlock_password_provider_prompt_is_called_for_protected_file() {
        let dir = tempdir().expect("tempdir");
        let locked_path = dir.path().join("protected.timelocked");
        lock_text_payload(&locked_path, Some("correct horse"));
        let args = unlock_args(&locked_path);
        let prompts = Rc::new(RefCell::new(Vec::new()));
        let calls = Rc::new(Cell::new(0));
        let reader_calls = Rc::clone(&calls);
        let reader = move || {
            reader_calls.set(reader_calls.get() + 1);
            Ok(SecretString::new("correct horse".to_string()))
        };

        run_unlock_with_password_reader(
            args,
            false,
            true,
            reader,
            CaptureSink::new(Rc::clone(&prompts)),
        )
        .expect("unlock protected");

        assert_eq!(calls.get(), 1);
        assert_eq!(
            String::from_utf8(prompts.borrow().clone()).expect("prompt utf8"),
            "Password required for this timelocked file: "
        );
    }

    #[test]
    fn cli_unlock_wrong_password_reprompts_with_remaining_attempts() {
        let dir = tempdir().expect("tempdir");
        let locked_path = dir.path().join("retry.timelocked");
        lock_text_payload(&locked_path, Some("correct horse"));
        let args = unlock_args(&locked_path);
        let prompts = Rc::new(RefCell::new(Vec::new()));
        let passwords = Rc::new(RefCell::new(vec![
            "correct horse".to_string(),
            "wrong horse".to_string(),
        ]));
        let reader_passwords = Rc::clone(&passwords);
        let reader = move || {
            Ok(SecretString::new(
                reader_passwords.borrow_mut().pop().expect("password"),
            ))
        };

        run_unlock_with_password_reader(
            args,
            false,
            true,
            reader,
            CaptureSink::new(Rc::clone(&prompts)),
        )
        .expect("unlock after retry");

        let prompts = String::from_utf8(prompts.borrow().clone()).expect("prompt utf8");
        assert!(prompts.contains("Password required for this timelocked file: "));
        assert!(prompts
            .contains("Password did not unlock this file. Try again (2 attempts remaining): "));
    }

    #[test]
    fn cli_unlock_exhausted_password_attempts_reports_generic_authentication_error() {
        let dir = tempdir().expect("tempdir");
        let locked_path = dir.path().join("exhausted.timelocked");
        lock_text_payload(&locked_path, Some("correct horse"));
        let args = unlock_args(&locked_path);
        let prompts = Rc::new(RefCell::new(Vec::new()));
        let calls = Rc::new(Cell::new(0));
        let reader_calls = Rc::clone(&calls);
        let reader = move || {
            let next_call = reader_calls.get() + 1;
            reader_calls.set(next_call);
            Ok(SecretString::new(format!("wrong password {next_call}")))
        };

        let err = run_unlock_with_password_reader(
            args,
            false,
            true,
            reader,
            CaptureSink::new(Rc::clone(&prompts)),
        )
        .expect_err("wrong passwords should fail");

        assert_eq!(calls.get(), unlock::MAX_PASSWORD_ATTEMPTS);
        assert!(err
            .to_string()
            .contains("payload authentication failed (wrong password or file is corrupted)"));
        let prompts = String::from_utf8(prompts.borrow().clone()).expect("prompt utf8");
        assert!(prompts.contains("Try again (2 attempts remaining)"));
        assert!(prompts.contains("Try again (1 attempt remaining)"));
    }

    #[test]
    fn cli_json_unlock_keeps_prompt_off_stdout_for_all_attempts() {
        let dir = tempdir().expect("tempdir");
        let locked_path = dir.path().join("json-retry.timelocked");
        lock_text_payload(&locked_path, Some("correct horse"));
        let args = unlock_args(&locked_path);
        let prompts = Rc::new(RefCell::new(Vec::new()));
        let passwords = Rc::new(RefCell::new(vec![
            "correct horse".to_string(),
            "wrong horse".to_string(),
        ]));
        let reader_passwords = Rc::clone(&passwords);
        let reader = move || {
            Ok(SecretString::new(
                reader_passwords.borrow_mut().pop().expect("password"),
            ))
        };

        run_unlock_with_password_reader(
            args,
            true,
            true,
            reader,
            CaptureSink::new(Rc::clone(&prompts)),
        )
        .expect("json unlock after retry");

        let prompts = String::from_utf8(prompts.borrow().clone()).expect("prompt utf8");
        assert!(prompts.contains("Password required for this timelocked file: "));
        assert!(prompts.contains("Try again (2 attempts remaining)"));
    }

    fn lock_text_payload(output_path: &Path, password: Option<&str>) {
        let _ = fs::remove_file(output_path);
        lock::execute(
            lock::LockRequest {
                input: "hidden message".to_string(),
                output: Some(PathBuf::from(output_path)),
                modulus_bits: 256,
                target: None,
                iterations: Some(1),
                hardware_profile: None,
                current_machine_iterations_per_second: None,
                creator_name: None,
                creator_message: None,
                password: password.map(|value| SecretString::new(value.to_string())),
                verify: false,
            },
            None,
        )
        .expect("lock text payload");
    }

    fn unlock_args(input: &Path) -> UnlockArgs {
        UnlockArgs {
            input: Some(input.to_path_buf()),
            input_arg: None,
            out_dir: None,
            out: None,
        }
    }

    struct CaptureSink {
        bytes: Rc<RefCell<Vec<u8>>>,
    }

    impl CaptureSink {
        fn new(bytes: Rc<RefCell<Vec<u8>>>) -> Self {
            Self { bytes }
        }
    }

    impl Write for CaptureSink {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.bytes.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}
