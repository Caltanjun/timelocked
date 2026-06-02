//! Background worker helpers for long-running TUI actions and session-scoped
//! calibration prewarm tasks.

use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use crate::base::progress_status::ProgressStatus;
use crate::base::{CancellationToken, Error, Result as BaseResult, SecretString};
use crate::usecases::{calibrate, lock, unlock, verify};

pub enum LockWorkerEvent {
    Progress(ProgressStatus),
    Finished(Result<lock::LockResponse, Error>),
}

pub struct LockWorker {
    pub receiver: Receiver<LockWorkerEvent>,
    pub cancellation: CancellationToken,
}

pub enum UnlockWorkerEvent {
    Progress(ProgressStatus),
    PasswordRequired {
        attempt: u8,
        max_attempts: u8,
        previous_attempt_failed: bool,
        reply: Sender<BaseResult<SecretString>>,
    },
    Finished(Result<unlock::UnlockResponse, Error>),
}

pub struct UnlockWorker {
    pub receiver: Receiver<UnlockWorkerEvent>,
    pub cancellation: CancellationToken,
}

pub enum VerifyWorkerEvent {
    Finished(Result<verify::VerifyResponse, Error>),
}

pub struct VerifyWorker {
    pub receiver: Receiver<VerifyWorkerEvent>,
    pub cancellation: CancellationToken,
}

pub struct CalibrationWorker {
    pub receiver: Receiver<Result<u64, Error>>,
}

pub fn spawn_lock_worker(request: lock::LockRequest) -> LockWorker {
    let (sender, receiver) = mpsc::channel();
    let cancellation = CancellationToken::default();
    let cancellation_for_worker = cancellation.clone();

    std::thread::spawn(move || {
        let mut progress = |status: ProgressStatus| {
            let _ = sender.send(LockWorkerEvent::Progress(status));
        };
        let result =
            lock::execute_with_cancel(request, Some(&mut progress), Some(&cancellation_for_worker));
        let _ = sender.send(LockWorkerEvent::Finished(result));
    });

    LockWorker {
        receiver,
        cancellation,
    }
}

pub fn spawn_unlock_worker(request: unlock::UnlockRequest) -> UnlockWorker {
    let (sender, receiver) = mpsc::channel();
    let cancellation = CancellationToken::default();
    let cancellation_for_worker = cancellation.clone();

    std::thread::spawn(move || {
        let event_sender = sender.clone();
        let mut progress = |status: ProgressStatus| {
            let _ = sender.send(UnlockWorkerEvent::Progress(status));
        };
        let mut password_provider = |context: unlock::PasswordPromptContext| {
            let (reply_sender, reply_receiver) = mpsc::channel();
            event_sender
                .send(UnlockWorkerEvent::PasswordRequired {
                    attempt: context.attempt,
                    max_attempts: context.max_attempts,
                    previous_attempt_failed: context.previous_attempt_failed,
                    reply: reply_sender,
                })
                .map_err(|_| Error::Cancelled)?;

            loop {
                if cancellation_for_worker.is_cancelled() {
                    return Err(Error::Cancelled);
                }

                match reply_receiver.recv_timeout(Duration::from_millis(50)) {
                    Ok(result) => return result,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => return Err(Error::Cancelled),
                }
            }
        };
        let result = unlock::execute_with_cancel_and_password_provider(
            request,
            Some(&mut progress),
            Some(&cancellation_for_worker),
            Some(&mut password_provider),
        );
        let _ = sender.send(UnlockWorkerEvent::Finished(result));
    });

    UnlockWorker {
        receiver,
        cancellation,
    }
}

pub fn spawn_verify_worker(request: verify::VerifyRequest) -> VerifyWorker {
    let (sender, receiver) = mpsc::channel();
    let cancellation = CancellationToken::default();
    let cancellation_for_worker = cancellation.clone();

    std::thread::spawn(move || {
        let result = verify::execute_with_cancel(request, None, Some(&cancellation_for_worker));
        let _ = sender.send(VerifyWorkerEvent::Finished(result));
    });

    VerifyWorker {
        receiver,
        cancellation,
    }
}

pub fn spawn_calibration_worker() -> CalibrationWorker {
    let (sender, receiver) = mpsc::channel();

    std::thread::spawn(move || {
        let result = calibrate::execute().map(|response| response.iterations_per_second);
        let _ = sender.send(result);
    });

    CalibrationWorker { receiver }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tempfile::tempdir;

    use super::*;
    use crate::domains::timelocked_file::test_support::SampleTimelockedFileBuilder;
    use crate::domains::timelocked_file::PasswordProtectionParams;

    #[test]
    fn unlock_worker_emits_password_required_for_protected_file() {
        let dir = tempdir().expect("tempdir");
        let input = dir.path().join("protected.timelocked");
        let params = PasswordProtectionParams::new(8, 1, 1, vec![7; 16]).expect("params");
        SampleTimelockedFileBuilder::new(b"secret note".to_vec())
            .iterations(1)
            .password_protection(SecretString::new("correct".to_string()), params)
            .write_to(&input)
            .expect("write protected file");

        let worker = spawn_unlock_worker(unlock::UnlockRequest {
            input,
            out_dir: None,
            out: None,
        });

        let mut password_reply = None;
        for _ in 0..40 {
            match worker
                .receiver
                .recv_timeout(Duration::from_millis(250))
                .expect("worker event")
            {
                UnlockWorkerEvent::Progress(_) => {}
                UnlockWorkerEvent::PasswordRequired {
                    attempt,
                    max_attempts,
                    previous_attempt_failed,
                    reply,
                } => {
                    assert_eq!(attempt, 1);
                    assert_eq!(max_attempts, unlock::MAX_PASSWORD_ATTEMPTS);
                    assert!(!previous_attempt_failed);
                    password_reply = Some(reply);
                    break;
                }
                UnlockWorkerEvent::Finished(result) => {
                    panic!("finished before password prompt: {result:?}")
                }
            }
        }

        password_reply
            .expect("password required event")
            .send(Err(Error::Cancelled))
            .expect("send cancel reply");

        loop {
            match worker
                .receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("finished event")
            {
                UnlockWorkerEvent::Progress(_) | UnlockWorkerEvent::PasswordRequired { .. } => {}
                UnlockWorkerEvent::Finished(result) => {
                    assert!(matches!(result, Err(Error::Cancelled)));
                    break;
                }
            }
        }
    }
}
