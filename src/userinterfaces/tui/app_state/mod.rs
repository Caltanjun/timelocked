//! Holds TUI session state and delegates routing, modal handling, footer copy,
//! worker polling, and session calibration behavior for the active session.

mod footer;
mod modal;
mod routing;
mod session;
mod worker_poll;

use crate::userinterfaces::tui::worker::{CalibrationWorker, VerifyWorker};

pub(crate) use super::features::inspect::details::{
    handle_key as handle_inspect_details_key, help as inspect_details_help, InspectDetailsFocus,
    InspectDetailsState,
};
pub(crate) use super::features::inspect::form::{
    handle_key as handle_inspect_form_key, help as inspect_form_help, InspectFormState,
};
pub(crate) use super::features::lock::complete::{
    completed_lock_progress, handle_key as handle_lock_complete_key, help as lock_complete_help,
    LockCompleteFocus, LockCompleteState,
};
pub(crate) use super::features::lock::file_form::{
    handle_key as handle_lock_file_form_key, help as lock_file_help, LockFileFormState,
};
pub(crate) use super::features::lock::progress::{
    handle_key as handle_lock_progress_key, help as lock_progress_help,
};
pub(crate) use super::features::lock::text_form::{
    handle_key as handle_lock_text_form_key, help as lock_text_help, LockTextFormState,
};
pub(crate) use super::features::main_menu::screen::{
    handle_key as handle_main_menu_key, help as main_menu_help, MainMenuState,
};
pub(crate) use super::features::unlock::complete::{
    handle_key as handle_unlock_complete_key, help as unlock_complete_help, UnlockCompleteFocus,
    UnlockCompleteState,
};
pub(crate) use super::features::unlock::form::{
    handle_key as handle_unlock_form_key, help as unlock_form_help, UnlockFormState,
};
pub(crate) use super::features::unlock::progress::{
    handle_key as handle_unlock_progress_key, help as unlock_progress_help,
};
pub(crate) use super::features::verify::details::{
    handle_key as handle_verify_details_key, help as verify_details_help,
};
pub(crate) use super::features::verify::form::{
    handle_key as handle_verify_form_key, help as verify_form_help, VerifyFormState,
};
pub(crate) use super::state::{
    browser_filter_toggle_available, BrowserFileFilter, BrowserMode, BrowserTarget,
    FileBrowserState, FooterContent, Modal, Screen, TextField,
};

const DOCS_URL: &str = "https://timelocked.app/";

pub struct App {
    pub screen: Screen,
    pub modal: Option<Modal>,
    pub should_quit: bool,
    pub no_color: bool,
    pub docs_url: &'static str,
    pub session_calibration_iterations_per_second: Option<u64>,
    pub(crate) session_calibration_worker: Option<CalibrationWorker>,
    pub(crate) verify_worker: Option<VerifyWorker>,
    pub(crate) session_calibration_prewarm_started: bool,
}

impl App {
    pub fn new(no_color: bool) -> Self {
        Self {
            screen: Screen::MainMenu(MainMenuState::default()),
            modal: None,
            should_quit: false,
            no_color,
            docs_url: DOCS_URL,
            session_calibration_iterations_per_second: None,
            session_calibration_worker: None,
            verify_worker: None,
            session_calibration_prewarm_started: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{App, BrowserMode, BrowserTarget, FileBrowserState, Modal};

    #[test]
    fn browser_footer_hides_filter_hint_for_lock_input() {
        let mut app = App::new(false);
        app.modal = Some(Modal::Browser(FileBrowserState::new(
            BrowserMode::File,
            BrowserTarget::LockFileInput,
            None,
        )));

        let footer = app.footer_content();
        assert!(!footer.left.contains("f Toggle filter"));
    }

    #[test]
    fn session_calibration_is_cached_in_memory() {
        let mut app = App::new(false);

        let first = app.ensure_session_calibration().expect("first calibration");
        let second = app
            .ensure_session_calibration()
            .expect("cached calibration");

        assert!(first > 0);
        assert_eq!(first, second);
        assert_eq!(app.session_calibration_iterations_per_second, Some(first));
    }
}
