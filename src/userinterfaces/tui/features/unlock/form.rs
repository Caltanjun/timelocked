//! Renders the unlock form and derives pre-unlock ETA hints from inspect
//! metadata plus any session-scoped machine calibration.

use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::usecases::unlock;
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
use crate::userinterfaces::tui::features::shared::progress_screens::new_unlock_progress_screen;
use crate::userinterfaces::tui::features::shared::unlock_estimate::refresh_unlock_estimate_state;
use crate::userinterfaces::tui::state::TextField;
use crate::userinterfaces::tui::worker::spawn_unlock_worker;

#[derive(Debug, Clone)]
pub struct UnlockFormState {
    pub input_path: TextField,
    pub output_dir: TextField,
    pub focus: UnlockFocus,
    pub estimated_duration_label: Option<String>,
    pub estimated_duration_seconds: Option<u64>,
    pub estimated_error: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum UnlockFocus {
    InputPath,
    BrowseInput,
    OutputDir,
    BrowseOutputDir,
    Start,
    Cancel,
}

impl Default for UnlockFormState {
    fn default() -> Self {
        let mut state = Self {
            input_path: TextField::new(String::new()),
            output_dir: TextField::new(String::new()),
            focus: UnlockFocus::InputPath,
            estimated_duration_label: None,
            estimated_duration_seconds: None,
            estimated_error: None,
        };
        state.arm_focus_field_for_replace();
        state
    }
}

impl UnlockFormState {
    pub(crate) fn arm_focus_field_for_replace(&mut self) {
        match self.focus {
            UnlockFocus::InputPath => self.input_path.arm_clear_on_next_edit(),
            UnlockFocus::OutputDir => self.output_dir.arm_clear_on_next_edit(),
            _ => {}
        }
    }
}

impl UnlockFocus {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::InputPath => Self::BrowseInput,
            Self::BrowseInput => Self::OutputDir,
            Self::OutputDir => Self::BrowseOutputDir,
            Self::BrowseOutputDir => Self::Start,
            Self::Start => Self::Cancel,
            Self::Cancel => Self::InputPath,
        }
    }

    pub(crate) fn prev(self) -> Self {
        match self {
            Self::InputPath => Self::Cancel,
            Self::BrowseInput => Self::InputPath,
            Self::OutputDir => Self::BrowseInput,
            Self::BrowseOutputDir => Self::OutputDir,
            Self::Start => Self::BrowseOutputDir,
            Self::Cancel => Self::Start,
        }
    }
}

pub fn help(focus: UnlockFocus) -> &'static str {
    match focus {
        UnlockFocus::InputPath => "Path to a .timelocked file.",
        UnlockFocus::BrowseInput => "Open file browser.",
        UnlockFocus::OutputDir => "Leave empty to use same directory.",
        UnlockFocus::BrowseOutputDir => "Choose output folder.",
        UnlockFocus::Start => "Start unlock operation.",
        UnlockFocus::Cancel => "Back to main menu.",
    }
}

const FORM_LABEL_WIDTH: usize = 16;

pub fn render(state: &UnlockFormState, frame: &mut Frame, area: Rect, app: &App) {
    let estimate = match (&state.estimated_duration_label, &state.estimated_error) {
        (Some(label), _) => label.clone(),
        (_, Some(err)) => format!("Estimate unavailable: {err}"),
        _ => "Estimated time: ~?".to_string(),
    };
    let label_width = FORM_LABEL_WIDTH.max(label_width(&["Timelocked file", "Output folder"]));
    let lines = vec![
        line_with_field_and_button(
            "Timelocked file",
            label_width,
            &state.input_path.value,
            FieldChrome::Input,
            matches!(state.focus, UnlockFocus::InputPath),
            InlineButton {
                label: "Browse",
                kind: ActionKind::Secondary,
                focused: matches!(state.focus, UnlockFocus::BrowseInput),
            },
            app,
        ),
        line_with_field_and_button(
            "Output folder",
            label_width,
            if state.output_dir.value.trim().is_empty() {
                "same directory as timelocked file"
            } else {
                &state.output_dir.value
            },
            FieldChrome::Input,
            matches!(state.focus, UnlockFocus::OutputDir),
            InlineButton {
                label: "Browse",
                kind: ActionKind::Secondary,
                focused: matches!(state.focus, UnlockFocus::BrowseOutputDir),
            },
            app,
        ),
        helper_line(&estimate, label_width, app),
        helper_line("Estimate only - actual runtime may vary.", label_width, app),
        Line::from(""),
        Line::from(vec![
            button_span(
                "Start unlock",
                ActionKind::Primary,
                matches!(state.focus, UnlockFocus::Start),
                app,
            ),
            Span::raw("  "),
            button_span(
                "Cancel",
                ActionKind::Secondary,
                matches!(state.focus, UnlockFocus::Cancel),
                app,
            ),
        ]),
    ];
    render_block_paragraph(frame, area, "Unlock a Timelocked File", lines, app);
}

pub fn handle_key(state: &mut UnlockFormState, key: KeyEvent, app: &mut App) -> Screen {
    if key.code == KeyCode::Esc {
        return Screen::MainMenu(MainMenuState::default());
    }

    if let Some(focus) = cycled_focus(
        key,
        state.focus,
        UnlockFocus::next,
        UnlockFocus::prev,
        FocusNavigationAxis::Vertical,
    ) {
        state.focus = focus;
        state.arm_focus_field_for_replace();
        return Screen::UnlockForm(state.clone());
    }

    match state.focus {
        UnlockFocus::InputPath => {
            if state.input_path.apply_key(key) {
                refresh_unlock_estimate_state(app, state);
            }
            Screen::UnlockForm(state.clone())
        }
        UnlockFocus::BrowseInput => {
            if key.code == KeyCode::Enter {
                app.open_browser(
                    BrowserTarget::UnlockInput,
                    BrowserMode::File,
                    Some(state.input_path.value.trim().into()),
                );
            }
            Screen::UnlockForm(state.clone())
        }
        UnlockFocus::OutputDir => {
            state.output_dir.apply_key(key);
            Screen::UnlockForm(state.clone())
        }
        UnlockFocus::BrowseOutputDir => {
            if key.code == KeyCode::Enter {
                app.open_browser(
                    BrowserTarget::UnlockOutputDir,
                    BrowserMode::Directory,
                    Some(state.output_dir.value.trim().into()),
                );
            }
            Screen::UnlockForm(state.clone())
        }
        UnlockFocus::Start => {
            if key.code == KeyCode::Enter {
                match start_unlock(app, state) {
                    Ok(progress) => progress,
                    Err(err) => {
                        app.modal = Some(Modal::Error(err));
                        Screen::UnlockForm(state.clone())
                    }
                }
            } else {
                Screen::UnlockForm(state.clone())
            }
        }
        UnlockFocus::Cancel => {
            if key.code == KeyCode::Enter {
                Screen::MainMenu(MainMenuState::default())
            } else {
                Screen::UnlockForm(state.clone())
            }
        }
    }
}

pub(crate) fn start_unlock(
    _app: &mut App,
    state: &UnlockFormState,
) -> std::result::Result<Screen, String> {
    let input = state.input_path.value.trim();
    if input.is_empty() {
        return Err("Timelocked file is required.".to_string());
    }

    let out_dir = state.output_dir.value.trim();
    let out_dir = if out_dir.is_empty() {
        None
    } else {
        Some(PathBuf::from(out_dir))
    };

    let worker = spawn_unlock_worker(unlock::UnlockRequest {
        input: PathBuf::from(input),
        out_dir,
        out: None,
    });

    Ok(new_unlock_progress_screen(
        input.to_string(),
        state.estimated_duration_seconds,
        worker,
    ))
}

pub(crate) fn start_unlock_from_path(
    _app: &mut App,
    input: &Path,
    estimated_duration_seconds: Option<u64>,
) -> std::result::Result<Screen, String> {
    if input.as_os_str().is_empty() {
        return Err("Timelocked file is required.".to_string());
    }

    let worker = spawn_unlock_worker(unlock::UnlockRequest {
        input: input.to_path_buf(),
        out_dir: None,
        out: None,
    });

    Ok(new_unlock_progress_screen(
        input.display().to_string(),
        estimated_duration_seconds,
        worker,
    ))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{start_unlock, UnlockFormState};
    use crate::domains::timelocked_file::test_support::SampleTimelockedFileBuilder;
    use crate::userinterfaces::tui::app_state::{App, Screen};
    use crate::userinterfaces::tui::features::shared::unlock_estimate::refresh_unlock_estimate_state;
    use crate::userinterfaces::tui::state::TextField;

    #[test]
    fn unlock_progress_starts_with_estimated_eta_when_available() {
        let mut app = App::new(false);

        let form = UnlockFormState {
            input_path: TextField::new("input.timelocked"),
            estimated_duration_seconds: Some(42),
            ..UnlockFormState::default()
        };

        let screen = start_unlock(&mut app, &form).expect("start unlock");
        match screen {
            Screen::UnlockProgress(state) => {
                assert_eq!(state.progress.phase, "unlock-timelock");
                assert_eq!(state.progress.eta_seconds, Some(42));
            }
            _ => panic!("expected unlock progress screen"),
        }
    }

    #[test]
    fn unlock_estimate_prefers_session_calibration_when_available() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("message.timelocked");
        let mut app = App::new(false);

        SampleTimelockedFileBuilder::new(Vec::<u8>::new())
            .original_filename("note.txt")
            .iterations(642)
            .hardware_profile("desktop-2026")
            .target_seconds(Some(2))
            .write_to(&path)
            .expect("write artifact");

        let mut form = UnlockFormState {
            input_path: TextField::new(path.display().to_string()),
            ..UnlockFormState::default()
        };

        app.session_calibration_iterations_per_second = Some(321);
        refresh_unlock_estimate_state(&mut app, &mut form);

        assert_eq!(form.estimated_duration_seconds, Some(2));
        assert_eq!(
            form.estimated_duration_label.as_deref(),
            Some("Estimated time on this machine: ~2s")
        );
    }
}
