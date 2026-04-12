//! Footer-copy helpers for the TUI app state.

use super::{
    inspect_details_help, inspect_form_help, lock_complete_help, lock_file_help,
    lock_progress_help, lock_text_help, main_menu_help, unlock_complete_help, unlock_form_help,
    unlock_progress_help, verify_details_help, verify_form_help, App, FooterContent, Modal, Screen,
};

pub(super) fn footer_content(app: &App) -> FooterContent {
    if let Some(modal) = &app.modal {
        if matches!(modal, Modal::Error(_) | Modal::Info(_)) {
            return FooterContent {
                left: "Enter Close   Esc Close".to_string(),
                center: "Message".to_string(),
            };
        }
    }

    let (left, center) = match &app.screen {
        Screen::MainMenu(_) => (
            "↑/↓ Move   Enter Select   Esc Quit".to_string(),
            main_menu_help().to_string(),
        ),
        Screen::LockFileForm(state) => (
            "Tab Focus   Enter Activate   Esc Back".to_string(),
            lock_file_help(state.focus).to_string(),
        ),
        Screen::LockTextForm(state) => (
            "Tab Focus   Enter Activate   Esc Back".to_string(),
            lock_text_help(state.focus).to_string(),
        ),
        Screen::LockProgress(_) => (
            "Tab Focus   Enter Activate   Esc Cancel".to_string(),
            lock_progress_help().to_string(),
        ),
        Screen::LockComplete(state) => (
            "←/→ Move   Enter Select   Esc Back".to_string(),
            lock_complete_help(state.focus).to_string(),
        ),
        Screen::UnlockForm(state) => (
            "Tab Focus   Enter Activate   Esc Back".to_string(),
            unlock_form_help(state.focus).to_string(),
        ),
        Screen::UnlockProgress(_) => (
            "Tab Focus   Enter Activate   Esc Cancel".to_string(),
            unlock_progress_help().to_string(),
        ),
        Screen::UnlockComplete(state) => (
            "←/→ Move   Enter Select   Esc Back".to_string(),
            unlock_complete_help(state).to_string(),
        ),
        Screen::InspectForm(state) => (
            "Tab Focus   Enter Activate   Esc Back".to_string(),
            inspect_form_help(state.focus).to_string(),
        ),
        Screen::InspectDetails(state) => (
            "←/→ Move   Enter Activate   Esc Back".to_string(),
            inspect_details_help(state.focus).to_string(),
        ),
        Screen::VerifyForm(state) => (
            if matches!(
                state.status,
                crate::userinterfaces::tui::features::verify::form::VerifyRunState::Idle
            ) {
                "Tab Focus   Enter Activate   Esc Back".to_string()
            } else {
                "Tab Focus   Enter Activate   Esc Cancel".to_string()
            },
            verify_form_help(state).to_string(),
        ),
        Screen::VerifyDetails(state) => (
            "Enter Select   Esc Back".to_string(),
            verify_details_help(state.focus).to_string(),
        ),
    };

    FooterContent { left, center }
}

impl App {
    pub fn footer_content(&self) -> FooterContent {
        footer_content(self)
    }
}
