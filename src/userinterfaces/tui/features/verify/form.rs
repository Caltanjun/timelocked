//! Renders the structural verify form and manages inline structural verification state.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::usecases::verify;
use crate::userinterfaces::tui::app_state::{App, BrowserMode, BrowserTarget, Modal, Screen};
use crate::userinterfaces::tui::components::form::{
    button_span, helper_line, label_width, line_with_field_and_button, ActionKind, FieldChrome,
    InlineButton,
};
use crate::userinterfaces::tui::components::layout::render_block_paragraph;
use crate::userinterfaces::tui::features::main_menu::screen::MainMenuState;
use crate::userinterfaces::tui::features::shared::form_navigation::{
    cycled_focus, FocusNavigationAxis,
};
use crate::userinterfaces::tui::state::TextField;
use crate::userinterfaces::tui::worker::spawn_verify_worker;

#[derive(Debug, Clone)]
pub struct VerifyFormState {
    pub input_path: TextField,
    pub focus: VerifyFocus,
    pub status: VerifyRunState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyRunState {
    Idle,
    Running,
    Cancelling,
}

#[derive(Debug, Clone, Copy)]
pub enum VerifyFocus {
    InputPath,
    BrowseInput,
    Verify,
    Cancel,
}

impl Default for VerifyFormState {
    fn default() -> Self {
        let mut state = Self {
            input_path: TextField::new(String::new()),
            focus: VerifyFocus::InputPath,
            status: VerifyRunState::Idle,
        };
        state.arm_focus_field_for_replace();
        state
    }
}

impl VerifyFormState {
    pub(crate) fn arm_focus_field_for_replace(&mut self) {
        if matches!(self.focus, VerifyFocus::InputPath) {
            self.input_path.arm_clear_on_next_edit();
        }
    }
}

impl VerifyFocus {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::InputPath => Self::BrowseInput,
            Self::BrowseInput => Self::Verify,
            Self::Verify => Self::Cancel,
            Self::Cancel => Self::InputPath,
        }
    }

    pub(crate) fn prev(self) -> Self {
        match self {
            Self::InputPath => Self::Cancel,
            Self::BrowseInput => Self::InputPath,
            Self::Verify => Self::BrowseInput,
            Self::Cancel => Self::Verify,
        }
    }
}

pub fn help(state: &VerifyFormState) -> &'static str {
    if matches!(state.status, VerifyRunState::Cancelling) {
        return "Cancellation requested. Waiting for structural verification to stop.";
    }

    if matches!(state.status, VerifyRunState::Running) {
        return match state.focus {
            VerifyFocus::Cancel => "Cancel structural verification.",
            _ => "Structural verification is running in the background.",
        };
    }

    match state.focus {
        VerifyFocus::InputPath => "Path to a .timelocked file.",
        VerifyFocus::BrowseInput => "Open file browser.",
        VerifyFocus::Verify => "Start structural verification.",
        VerifyFocus::Cancel => "Back to main menu.",
    }
}

const FORM_LABEL_WIDTH: usize = 16;

pub fn render(state: &VerifyFormState, frame: &mut Frame, area: Rect, app: &App) {
    let label_width = FORM_LABEL_WIDTH.max(label_width(&["Timelocked file"]));
    let verify_label = match state.status {
        VerifyRunState::Idle => "Verify",
        VerifyRunState::Running => "Verifying...",
        VerifyRunState::Cancelling => "Cancelling...",
    };
    let cancel_label = if matches!(state.status, VerifyRunState::Idle) {
        "Cancel"
    } else {
        "Cancel verify"
    };
    let mut lines = vec![line_with_field_and_button(
        "Timelocked file",
        label_width,
        &state.input_path.value,
        FieldChrome::Input,
        matches!(state.focus, VerifyFocus::InputPath),
        InlineButton {
            label: "Browse",
            kind: ActionKind::Secondary,
            focused: matches!(state.focus, VerifyFocus::BrowseInput),
        },
        app,
    )];
    let helper_text = match state.status {
        VerifyRunState::Idle => {
            "Checks file structure without unlocking the payload."
        }
        VerifyRunState::Running => "Structural verification is running. You can cancel safely.",
        VerifyRunState::Cancelling => {
            "Stopping structural verification at the next safe cancellation point."
        }
    };
    lines.push(helper_line(helper_text, label_width, app));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        button_span(
            verify_label,
            ActionKind::Primary,
            matches!(state.focus, VerifyFocus::Verify),
            app,
        ),
        Span::raw("  "),
        button_span(
            cancel_label,
            ActionKind::Secondary,
            matches!(state.focus, VerifyFocus::Cancel),
            app,
        ),
    ]));
    render_block_paragraph(frame, area, "Structural Verification", lines, app);
}

pub fn handle_key(state: &mut VerifyFormState, key: KeyEvent, app: &mut App) -> Screen {
    if key.code == KeyCode::Esc {
        if matches!(
            state.status,
            VerifyRunState::Running | VerifyRunState::Cancelling
        ) {
            request_verify_cancel(app, state);
            return Screen::VerifyForm(state.clone());
        }
        return Screen::MainMenu(MainMenuState::default());
    }

    if let Some(focus) = cycled_focus(
        key,
        state.focus,
        VerifyFocus::next,
        VerifyFocus::prev,
        FocusNavigationAxis::Vertical,
    ) {
        state.focus = focus;
        state.arm_focus_field_for_replace();
        return Screen::VerifyForm(state.clone());
    }

    match state.focus {
        VerifyFocus::InputPath => {
            if !matches!(state.status, VerifyRunState::Idle) {
                return Screen::VerifyForm(state.clone());
            }
            state.input_path.apply_key(key);
            Screen::VerifyForm(state.clone())
        }
        VerifyFocus::BrowseInput => {
            if key.code == KeyCode::Enter && matches!(state.status, VerifyRunState::Idle) {
                app.open_browser(
                    BrowserTarget::VerifyInput,
                    BrowserMode::File,
                    Some(PathBuf::from(state.input_path.value.trim())),
                );
            }
            Screen::VerifyForm(state.clone())
        }
        VerifyFocus::Verify => {
            if key.code == KeyCode::Enter {
                match state.status {
                    VerifyRunState::Idle => match start_verify(app, state) {
                        Ok(()) => Screen::VerifyForm(state.clone()),
                        Err(err) => {
                            app.modal = Some(Modal::Error(err));
                            Screen::VerifyForm(state.clone())
                        }
                    },
                    VerifyRunState::Running | VerifyRunState::Cancelling => {
                        Screen::VerifyForm(state.clone())
                    }
                }
            } else {
                Screen::VerifyForm(state.clone())
            }
        }
        VerifyFocus::Cancel => {
            if key.code == KeyCode::Enter {
                if matches!(state.status, VerifyRunState::Idle) {
                    Screen::MainMenu(MainMenuState::default())
                } else {
                    request_verify_cancel(app, state);
                    Screen::VerifyForm(state.clone())
                }
            } else {
                Screen::VerifyForm(state.clone())
            }
        }
    }
}

pub(crate) fn start_verify(
    app: &mut App,
    state: &mut VerifyFormState,
) -> std::result::Result<(), String> {
    let input = state.input_path.value.trim();
    if input.is_empty() {
        return Err("Timelocked file is required.".to_string());
    }

    let request = verify::VerifyRequest {
        input: PathBuf::from(input),
    };
    app.verify_worker = Some(spawn_verify_worker(request));
    state.status = VerifyRunState::Running;
    Ok(())
}

pub(crate) fn request_verify_cancel(app: &mut App, state: &mut VerifyFormState) {
    if matches!(state.status, VerifyRunState::Cancelling) {
        return;
    }

    if let Some(worker) = &app.verify_worker {
        worker.cancellation.cancel();
        state.status = VerifyRunState::Cancelling;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use tempfile::tempdir;

    use crate::base::CancellationToken;
    use crate::domains::timelocked_file::test_support::SampleTimelockedFileBuilder;

    use super::{request_verify_cancel, start_verify, VerifyFormState, VerifyRunState};
    use crate::userinterfaces::tui::app_state::{App, Screen};
    use crate::userinterfaces::tui::state::TextField;
    use crate::userinterfaces::tui::worker::VerifyWorker;

    #[test]
    fn verify_starts_background_worker_and_marks_form_running() {
        let mut app = App::new(false);
        let dir = tempdir().expect("tempdir");
        let input_path = dir.path().join("input.timelocked");
        SampleTimelockedFileBuilder::new(b"verify me")
            .write_to(&input_path)
            .expect("write sample container");
        let mut form = VerifyFormState {
            input_path: TextField::new(input_path.display().to_string()),
            ..VerifyFormState::default()
        };

        start_verify(&mut app, &mut form).expect("start verify");

        assert!(matches!(form.status, VerifyRunState::Running));
        assert!(app.verify_worker.is_some());
    }

    #[test]
    fn verify_requires_non_empty_input_before_starting_worker() {
        let mut app = App::new(false);
        let mut form = VerifyFormState {
            input_path: TextField::new(""),
            ..VerifyFormState::default()
        };

        let error = start_verify(&mut app, &mut form)
            .err()
            .expect("verify should fail");

        assert_eq!(error, "Timelocked file is required.");
        assert!(matches!(form.status, VerifyRunState::Idle));
        assert!(app.verify_worker.is_none());
    }

    #[test]
    fn request_verify_cancel_marks_form_cancelling_and_cancels_worker() {
        let mut app = App::new(false);
        let (_sender, receiver) = mpsc::channel();
        let cancellation = CancellationToken::default();
        let cancellation_for_assert = cancellation.clone();
        app.verify_worker = Some(VerifyWorker {
            receiver,
            cancellation,
        });
        let mut form = VerifyFormState {
            status: VerifyRunState::Running,
            ..VerifyFormState::default()
        };

        request_verify_cancel(&mut app, &mut form);

        assert!(matches!(form.status, VerifyRunState::Cancelling));
        assert!(cancellation_for_assert.is_cancelled());
    }

    #[test]
    fn verify_run_state_is_part_of_screen_form_state() {
        let form = VerifyFormState {
            status: VerifyRunState::Running,
            ..VerifyFormState::default()
        };

        match Screen::VerifyForm(form.clone()) {
            Screen::VerifyForm(state) => assert!(matches!(state.status, VerifyRunState::Running)),
            _ => panic!("expected verify form screen"),
        }
    }
}
