//! Session-scoped calibration helpers for ETA and current-machine profile use.

use std::path::Path;

use crate::base::Error;
use crate::usecases::calibrate;
use crate::userinterfaces::tui::worker::spawn_calibration_worker;

use super::{App, Screen};

impl App {
    fn cache_session_calibration(&mut self, rate: u64) -> u64 {
        self.session_calibration_iterations_per_second = Some(rate);
        rate
    }

    pub(crate) fn try_complete_session_calibration(&mut self) -> Option<u64> {
        let worker = self.session_calibration_worker.take()?;
        match worker.receiver.try_recv() {
            Ok(Ok(rate)) => Some(self.cache_session_calibration(rate)),
            Ok(Err(_)) => None,
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                self.session_calibration_worker = Some(worker);
                None
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => None,
        }
    }

    pub(crate) fn on_frame_rendered(&mut self) {
        if matches!(self.screen, Screen::MainMenu(_)) {
            self.start_session_calibration_prewarm();
        }
    }

    pub(crate) fn start_session_calibration_prewarm(&mut self) {
        if self.session_calibration_prewarm_started
            || self.session_calibration_iterations_per_second.is_some()
            || self.session_calibration_worker.is_some()
        {
            return;
        }

        self.session_calibration_prewarm_started = true;
        self.session_calibration_worker = Some(spawn_calibration_worker());
    }

    pub(crate) fn ensure_session_calibration(&mut self) -> Result<u64, Error> {
        if let Some(rate) = self.session_calibration_iterations_per_second {
            return Ok(rate);
        }

        if let Some(rate) = self.try_complete_session_calibration() {
            return Ok(rate);
        }

        if let Some(worker) = self.session_calibration_worker.take() {
            if let Ok(Ok(rate)) = worker.receiver.recv() {
                return Ok(self.cache_session_calibration(rate));
            }
        }

        let response = calibrate::execute()?;
        Ok(self.cache_session_calibration(response.iterations_per_second))
    }

    pub(crate) fn calibration_for_estimate(&mut self) -> Option<u64> {
        self.session_calibration_iterations_per_second
            .or_else(|| self.try_complete_session_calibration())
    }

    pub(crate) fn estimate_calibration_for_path(&mut self, path: &Path) -> Option<u64> {
        if path.exists() {
            self.calibration_for_estimate()
        } else {
            self.session_calibration_iterations_per_second
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::App;
    use crate::base::Error;
    use crate::userinterfaces::tui::features::lock::file_form::LockFileFormState;
    use crate::userinterfaces::tui::state::Screen;
    use crate::userinterfaces::tui::worker::CalibrationWorker;

    fn test_app() -> App {
        App::new(false)
    }

    #[test]
    fn main_menu_render_starts_background_calibration_once() {
        let mut app = test_app();

        app.on_frame_rendered();

        assert!(app.session_calibration_prewarm_started);
        assert!(app.session_calibration_worker.is_some());

        let first_worker = app.session_calibration_worker.take();
        app.session_calibration_worker = first_worker;
        app.on_frame_rendered();

        assert!(app.session_calibration_worker.is_some());
    }

    #[test]
    fn non_main_menu_render_does_not_start_background_calibration() {
        let mut app = test_app();
        app.screen = Screen::LockFileForm(LockFileFormState::default());

        app.on_frame_rendered();

        assert!(!app.session_calibration_prewarm_started);
        assert!(app.session_calibration_worker.is_none());
    }

    #[test]
    fn ensure_session_calibration_waits_for_in_flight_worker_and_caches_rate() {
        let mut app = test_app();
        let (sender, receiver) = mpsc::channel();
        sender.send(Ok(4321)).expect("send calibration result");
        app.session_calibration_worker = Some(CalibrationWorker { receiver });
        app.session_calibration_prewarm_started = true;

        let rate = app
            .ensure_session_calibration()
            .expect("calibration from worker");

        assert_eq!(rate, 4321);
        assert_eq!(app.session_calibration_iterations_per_second, Some(4321));
        assert!(app.session_calibration_worker.is_none());
    }

    #[test]
    fn calibration_for_estimate_returns_cached_rate_without_worker() {
        let mut app = test_app();
        app.session_calibration_iterations_per_second = Some(2468);

        let rate = app.calibration_for_estimate();

        assert_eq!(rate, Some(2468));
        assert!(app.session_calibration_worker.is_none());
    }

    #[test]
    fn calibration_for_estimate_consumes_finished_worker_without_blocking() {
        let mut app = test_app();
        let (sender, receiver) = mpsc::channel();
        sender.send(Ok(2468)).expect("send calibration result");
        app.session_calibration_worker = Some(CalibrationWorker { receiver });

        let rate = app.calibration_for_estimate();

        assert_eq!(rate, Some(2468));
        assert_eq!(app.session_calibration_iterations_per_second, Some(2468));
        assert!(app.session_calibration_worker.is_none());
    }

    #[test]
    fn calibration_for_estimate_returns_none_while_worker_is_in_flight() {
        let mut app = test_app();
        let (_sender, receiver) = mpsc::channel();
        app.session_calibration_worker = Some(CalibrationWorker { receiver });

        let rate = app.calibration_for_estimate();

        assert_eq!(rate, None);
        assert!(app.session_calibration_iterations_per_second.is_none());
        assert!(app.session_calibration_worker.is_some());
    }

    #[test]
    fn calibration_for_estimate_discards_failed_finished_worker() {
        let mut app = test_app();
        let (sender, receiver) = mpsc::channel();
        sender
            .send(Err(Error::InvalidArgument("boom".to_string())))
            .expect("send calibration error");
        app.session_calibration_worker = Some(CalibrationWorker { receiver });

        let rate = app.calibration_for_estimate();

        assert_eq!(rate, None);
        assert!(app.session_calibration_iterations_per_second.is_none());
        assert!(app.session_calibration_worker.is_none());
    }

    #[test]
    fn ensure_session_calibration_retries_synchronously_after_background_failure() {
        let mut app = test_app();
        let (sender, receiver) = mpsc::channel();
        sender
            .send(Err(Error::InvalidArgument("boom".to_string())))
            .expect("send calibration error");
        app.session_calibration_worker = Some(CalibrationWorker { receiver });
        app.session_calibration_prewarm_started = true;

        let rate = app
            .ensure_session_calibration()
            .expect("fallback synchronous calibration");

        assert!(rate > 0);
        assert_eq!(app.session_calibration_iterations_per_second, Some(rate));
        assert!(app.session_calibration_worker.is_none());
    }
}
