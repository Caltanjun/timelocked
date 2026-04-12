//! Worker polling and terminal-screen transitions for the TUI app state.

use super::{
    completed_lock_progress, App, LockCompleteFocus, LockCompleteState, MainMenuState, Modal,
    Screen, UnlockCompleteFocus, UnlockCompleteState,
};
use crate::base::Error;
use crate::userinterfaces::tui::features::shared::unlock_estimate::refresh_unlock_estimate_state;
use crate::userinterfaces::tui::features::verify::details::{
    VerifyDetailsFocus, VerifyDetailsState,
};
use crate::userinterfaces::tui::features::verify::form::{VerifyFormState, VerifyRunState};
use crate::userinterfaces::tui::worker::{LockWorkerEvent, UnlockWorkerEvent, VerifyWorkerEvent};

fn poll_session_calibration_worker(app: &mut App) {
    if app.try_complete_session_calibration().is_some() {
        let placeholder = Screen::MainMenu(MainMenuState::default());
        let screen = std::mem::replace(&mut app.screen, placeholder);
        app.screen = match screen {
            Screen::UnlockForm(mut state) => {
                refresh_unlock_estimate_state(app, &mut state);
                Screen::UnlockForm(state)
            }
            other => other,
        };
    }
}

fn poll_lock_worker(
    app: &mut App,
    mut state: crate::userinterfaces::tui::features::lock::progress::LockProgressState,
) -> Screen {
    let mut terminal = None;
    while let Ok(event) = state.worker.receiver.try_recv() {
        match event {
            LockWorkerEvent::Progress(progress) => {
                state.progress = progress;
            }
            LockWorkerEvent::Finished(result) => {
                terminal = Some(result);
                break;
            }
        }
    }

    if let Some(result) = terminal {
        match result {
            Ok(response) => {
                let output_display = response.output_path.display().to_string();
                Screen::LockComplete(LockCompleteState {
                    output_path: response.output_path,
                    payload_bytes: response.payload_bytes,
                    input_display: state.input_display,
                    output_display,
                    progress: completed_lock_progress(&state.progress),
                    focus: LockCompleteFocus::Inspect,
                })
            }
            Err(Error::Cancelled) => {
                app.modal = Some(Modal::Info("Lock operation cancelled.".to_string()));
                Screen::MainMenu(MainMenuState::default())
            }
            Err(err) => {
                app.modal = Some(Modal::Error(err.to_string()));
                Screen::MainMenu(MainMenuState::default())
            }
        }
    } else {
        Screen::LockProgress(state)
    }
}

fn poll_unlock_worker(
    app: &mut App,
    mut state: crate::userinterfaces::tui::features::unlock::progress::UnlockProgressState,
) -> Screen {
    let mut terminal = None;
    while let Ok(event) = state.worker.receiver.try_recv() {
        match event {
            UnlockWorkerEvent::Progress(progress) => {
                state.progress = progress;
            }
            UnlockWorkerEvent::Finished(result) => {
                terminal = Some(result);
                break;
            }
        }
    }

    if let Some(result) = terminal {
        match result {
            Ok(response) => Screen::UnlockComplete(UnlockCompleteState {
                recovered_payload: response.recovered_payload,
                recovered_bytes: response.recovered_bytes,
                focus: UnlockCompleteFocus::OpenFolder,
            }),
            Err(Error::Cancelled) => {
                app.modal = Some(Modal::Info("Unlock operation cancelled.".to_string()));
                Screen::MainMenu(MainMenuState::default())
            }
            Err(err) => {
                app.modal = Some(Modal::Error(err.to_string()));
                Screen::MainMenu(MainMenuState::default())
            }
        }
    } else {
        Screen::UnlockProgress(state)
    }
}

fn poll_verify_worker(app: &mut App, mut state: VerifyFormState) -> Screen {
    let Some(worker) = app.verify_worker.take() else {
        if !matches!(state.status, VerifyRunState::Idle) {
            state.status = VerifyRunState::Idle;
        }
        return Screen::VerifyForm(state);
    };

    match worker.receiver.try_recv() {
        Ok(VerifyWorkerEvent::Finished(result)) => {
            state.status = VerifyRunState::Idle;
            match result {
                Ok(response) => Screen::VerifyDetails(VerifyDetailsState {
                    response,
                    focus: VerifyDetailsFocus::Done,
                }),
                Err(Error::Cancelled) => {
                    app.modal = Some(Modal::Info(
                        "Structural verification cancelled.".to_string(),
                    ));
                    Screen::VerifyForm(state)
                }
                Err(err) => {
                    app.modal = Some(Modal::Error(err.to_string()));
                    Screen::VerifyForm(state)
                }
            }
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {
            app.verify_worker = Some(worker);
            Screen::VerifyForm(state)
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            state.status = VerifyRunState::Idle;
            app.modal = Some(Modal::Error(
                "Structural verification worker disconnected unexpectedly.".to_string(),
            ));
            Screen::VerifyForm(state)
        }
    }
}

impl App {
    pub fn poll_workers(&mut self) {
        poll_session_calibration_worker(self);

        let placeholder = Screen::MainMenu(MainMenuState::default());
        let screen = std::mem::replace(&mut self.screen, placeholder);
        self.screen = match screen {
            Screen::LockProgress(state) => poll_lock_worker(self, state),
            Screen::UnlockProgress(state) => poll_unlock_worker(self, state),
            Screen::VerifyForm(state) => poll_verify_worker(self, state),
            other => other,
        };
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::mpsc;

    use tempfile::tempdir;

    use super::*;
    use crate::base::progress_status::ProgressStatus;
    use crate::base::CancellationToken;
    use crate::domains::timelocked_file::test_support::SampleTimelockedFileBuilder;
    use crate::usecases::{lock, unlock, verify};
    use crate::userinterfaces::tui::features::lock::progress::{
        LockProgressFocus, LockProgressState,
    };
    use crate::userinterfaces::tui::features::unlock::form::{UnlockFocus, UnlockFormState};
    use crate::userinterfaces::tui::features::unlock::progress::{
        UnlockProgressFocus, UnlockProgressState,
    };
    use crate::userinterfaces::tui::features::verify::form::{VerifyFormState, VerifyRunState};
    use crate::userinterfaces::tui::state::TextField;
    use crate::userinterfaces::tui::worker::{
        CalibrationWorker, LockWorker, UnlockWorker, VerifyWorker, VerifyWorkerEvent,
    };

    fn test_app() -> App {
        App::new(false)
    }

    fn lock_worker_with_events(events: Vec<LockWorkerEvent>) -> LockWorker {
        let (sender, receiver) = mpsc::channel();
        for event in events {
            sender.send(event).expect("send lock event");
        }
        drop(sender);
        LockWorker {
            receiver,
            cancellation: CancellationToken::default(),
        }
    }

    fn unlock_worker_with_events(events: Vec<UnlockWorkerEvent>) -> UnlockWorker {
        let (sender, receiver) = mpsc::channel();
        for event in events {
            sender.send(event).expect("send unlock event");
        }
        drop(sender);
        UnlockWorker {
            receiver,
            cancellation: CancellationToken::default(),
        }
    }

    fn verify_worker_with_events(events: Vec<VerifyWorkerEvent>) -> VerifyWorker {
        let (sender, receiver) = mpsc::channel();
        for event in events {
            sender.send(event).expect("send verify event");
        }
        drop(sender);
        VerifyWorker {
            receiver,
            cancellation: CancellationToken::default(),
        }
    }

    fn write_timelocked_fixture(path: &std::path::Path) {
        SampleTimelockedFileBuilder::new(Vec::<u8>::new())
            .original_filename("note.txt")
            .iterations(642)
            .hardware_profile("desktop-2026")
            .target_seconds(Some(2))
            .write_to(path)
            .expect("write artifact");
    }

    #[test]
    fn lock_worker_completion_transitions_to_complete_screen() {
        let mut app = test_app();
        let output_path = PathBuf::from("result.timelocked");
        app.screen = Screen::LockProgress(LockProgressState {
            input_display: "input.txt".to_string(),
            output_display: output_path.display().to_string(),
            progress: ProgressStatus::new("lock-persist", 5, 5, Some(1), Some(12.0)),
            worker: lock_worker_with_events(vec![LockWorkerEvent::Finished(Ok(
                lock::LockResponse {
                    output_path: output_path.clone(),
                    iterations: 8,
                    hardware_profile: "desktop-2026".to_string(),
                    payload_bytes: 64,
                },
            ))]),
            cancel_requested: false,
            focus: LockProgressFocus::Progress,
        });

        app.poll_workers();

        match &app.screen {
            Screen::LockComplete(state) => {
                assert_eq!(state.output_path, output_path);
                assert_eq!(state.payload_bytes, 64);
                assert_eq!(state.progress.pct, 100.0);
                assert_eq!(state.progress.eta_seconds, Some(0));
            }
            _ => panic!("expected lock complete screen"),
        }
        assert!(app.modal.is_none());
    }

    #[test]
    fn lock_worker_cancellation_returns_to_main_menu_with_info_modal() {
        let mut app = test_app();
        app.screen = Screen::LockProgress(LockProgressState {
            input_display: "input.txt".to_string(),
            output_display: "result.timelocked".to_string(),
            progress: ProgressStatus::new("lock-encrypt", 1, 2, Some(3), Some(4.0)),
            worker: lock_worker_with_events(vec![LockWorkerEvent::Finished(Err(Error::Cancelled))]),
            cancel_requested: true,
            focus: LockProgressFocus::Cancel,
        });

        app.poll_workers();

        assert!(matches!(app.screen, Screen::MainMenu(_)));
        match &app.modal {
            Some(Modal::Info(message)) => assert_eq!(message, "Lock operation cancelled."),
            _ => panic!("expected cancellation info modal"),
        }
    }

    #[test]
    fn unlock_worker_error_returns_to_main_menu_with_error_modal() {
        let mut app = test_app();
        app.screen = Screen::UnlockProgress(UnlockProgressState {
            file_display: "archive.timelocked".to_string(),
            progress: ProgressStatus::new("unlock-decrypt", 1, 2, Some(7), Some(8.0)),
            worker: unlock_worker_with_events(vec![UnlockWorkerEvent::Finished(Err(
                Error::InvalidFormat("bad container".to_string()),
            ))]),
            cancel_requested: false,
            cpu_count: 1,
            focus: UnlockProgressFocus::Progress,
        });

        app.poll_workers();

        assert!(matches!(app.screen, Screen::MainMenu(_)));
        match &app.modal {
            Some(Modal::Error(message)) => assert!(message.contains("bad container")),
            _ => panic!("expected error modal"),
        }
    }

    #[test]
    fn unlock_worker_completion_transitions_to_complete_screen() {
        let mut app = test_app();
        app.screen = Screen::UnlockProgress(UnlockProgressState {
            file_display: "archive.timelocked".to_string(),
            progress: ProgressStatus::new("unlock-decrypt", 2, 2, Some(0), Some(9.0)),
            worker: unlock_worker_with_events(vec![UnlockWorkerEvent::Finished(Ok(
                unlock::UnlockResponse {
                    recovered_payload: unlock::RecoveredPayload::Text {
                        text: "hello future".to_string(),
                    },
                    recovered_bytes: 12,
                },
            ))]),
            cancel_requested: false,
            cpu_count: 1,
            focus: UnlockProgressFocus::Progress,
        });

        app.poll_workers();

        match &app.screen {
            Screen::UnlockComplete(state) => {
                assert_eq!(state.recovered_bytes, 12);
                assert!(matches!(
                    state.recovered_payload,
                    unlock::RecoveredPayload::Text { ref text } if text == "hello future"
                ));
            }
            _ => panic!("expected unlock complete screen"),
        }
        assert!(app.modal.is_none());
    }

    #[test]
    fn completed_session_calibration_refreshes_visible_unlock_estimate() {
        let mut app = test_app();
        let dir = tempdir().expect("tempdir");
        let input_path = dir.path().join("message.timelocked");
        write_timelocked_fixture(&input_path);

        app.screen = Screen::UnlockForm(UnlockFormState {
            input_path: TextField::new(input_path.display().to_string()),
            output_dir: TextField::new(String::new()),
            focus: UnlockFocus::InputPath,
            estimated_duration_label: Some("Estimated time from chosen profile: ~2s".to_string()),
            estimated_duration_seconds: Some(2),
            estimated_error: None,
        });

        let (sender, receiver) = mpsc::channel();
        sender.send(Ok(321)).expect("send calibration result");
        app.session_calibration_worker = Some(CalibrationWorker { receiver });

        app.poll_workers();

        assert_eq!(app.session_calibration_iterations_per_second, Some(321));
        match &app.screen {
            Screen::UnlockForm(state) => {
                assert_eq!(state.estimated_duration_seconds, Some(2));
                assert_eq!(
                    state.estimated_duration_label.as_deref(),
                    Some("Estimated time on this machine: ~2s")
                );
            }
            _ => panic!("expected unlock form screen"),
        }
    }

    #[test]
    fn verify_worker_completion_transitions_to_details_screen_from_form() {
        let mut app = test_app();
        app.verify_worker = Some(verify_worker_with_events(vec![
            VerifyWorkerEvent::Finished(Ok(verify::VerifyResponse {
                path: PathBuf::from("archive.timelocked"),
                chunk_count: 2,
                payload_plaintext_bytes: 12,
            })),
        ]));
        app.screen = Screen::VerifyForm(VerifyFormState {
            input_path: TextField::new("archive.timelocked"),
            status: VerifyRunState::Running,
            ..VerifyFormState::default()
        });

        app.poll_workers();

        match &app.screen {
            Screen::VerifyDetails(state) => {
                assert_eq!(state.response.chunk_count, 2);
                assert_eq!(state.response.payload_plaintext_bytes, 12);
            }
            _ => panic!("expected verify details screen"),
        }
        assert!(app.verify_worker.is_none());
        assert!(app.modal.is_none());
    }

    #[test]
    fn verify_worker_error_keeps_form_and_shows_error_modal() {
        let mut app = test_app();
        app.verify_worker = Some(verify_worker_with_events(vec![
            VerifyWorkerEvent::Finished(Err(Error::InvalidFormat("bad container".to_string()))),
        ]));
        app.screen = Screen::VerifyForm(VerifyFormState {
            input_path: TextField::new("archive.timelocked"),
            status: VerifyRunState::Running,
            ..VerifyFormState::default()
        });

        app.poll_workers();

        match &app.screen {
            Screen::VerifyForm(state) => assert!(matches!(state.status, VerifyRunState::Idle)),
            _ => panic!("expected verify form screen"),
        }
        match &app.modal {
            Some(Modal::Error(message)) => assert!(message.contains("bad container")),
            _ => panic!("expected verify error modal"),
        }
        assert!(app.verify_worker.is_none());
    }

    #[test]
    fn verify_worker_cancellation_keeps_form_and_shows_info_modal() {
        let mut app = test_app();
        app.verify_worker = Some(verify_worker_with_events(vec![
            VerifyWorkerEvent::Finished(Err(Error::Cancelled)),
        ]));
        app.screen = Screen::VerifyForm(VerifyFormState {
            input_path: TextField::new("archive.timelocked"),
            status: VerifyRunState::Cancelling,
            ..VerifyFormState::default()
        });

        app.poll_workers();

        match &app.screen {
            Screen::VerifyForm(state) => assert!(matches!(state.status, VerifyRunState::Idle)),
            _ => panic!("expected verify form screen"),
        }
        match &app.modal {
            Some(Modal::Info(message)) => {
                assert_eq!(message, "Structural verification cancelled.")
            }
            _ => panic!("expected verify info modal"),
        }
        assert!(app.verify_worker.is_none());
    }

    #[test]
    fn failed_session_calibration_stays_silent() {
        let mut app = test_app();
        let (sender, receiver) = mpsc::channel();
        sender
            .send(Err(Error::InvalidArgument("boom".to_string())))
            .expect("send calibration error");
        app.session_calibration_worker = Some(CalibrationWorker { receiver });

        app.poll_workers();

        assert!(app.session_calibration_iterations_per_second.is_none());
        assert!(app.session_calibration_worker.is_none());
        assert!(app.modal.is_none());
    }
}
