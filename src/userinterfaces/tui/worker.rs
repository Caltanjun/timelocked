//! Background worker helpers for long-running TUI actions and session-scoped
//! calibration prewarm tasks.

use std::sync::mpsc::{self, Receiver};

use crate::base::progress_status::ProgressStatus;
use crate::base::{CancellationToken, Error};
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
        let mut progress = |status: ProgressStatus| {
            let _ = sender.send(UnlockWorkerEvent::Progress(status));
        };
        let result = unlock::execute_with_cancel(
            request,
            Some(&mut progress),
            Some(&cancellation_for_worker),
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
