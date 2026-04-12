//! Renders the file-lock form and starts worker-backed lock progress screens.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::configuration::runtime::lock_modulus_bits;
use crate::domains::timelock::{
    all_profiles, is_current_machine_profile_id, CURRENT_MACHINE_PROFILE_ID,
};
use crate::domains::timelocked_file::ensure_timelocked_extension;
use crate::usecases::lock;
use crate::userinterfaces::tui::app_state::{App, BrowserMode, BrowserTarget, Modal, Screen};
use crate::userinterfaces::tui::components::form::{
    button_span, helper_line, label_width, line_with_field, line_with_field_and_button, ActionKind,
    FieldChrome, InlineButton,
};
use crate::userinterfaces::tui::components::layout::render_block_paragraph;
use crate::userinterfaces::tui::features::main_menu::screen::MainMenuState;
use crate::userinterfaces::tui::features::shared::form_navigation::{
    cycled_focus, FocusNavigationAxis,
};
use crate::userinterfaces::tui::features::shared::progress_screens::new_lock_progress_screen;
use crate::userinterfaces::tui::state::TextField;
use crate::userinterfaces::tui::worker::spawn_lock_worker;

#[derive(Debug, Clone)]
pub struct LockFileFormState {
    pub input_path: TextField,
    pub output_path: TextField,
    pub target_delay: TextField,
    pub profile_index: usize,
    pub output_touched: bool,
    pub focus: LockFileFocus,
}

#[derive(Debug, Clone, Copy)]
pub enum LockFileFocus {
    InputPath,
    BrowseInput,
    OutputPath,
    TargetDelay,
    HardwareProfile,
    Lock,
    Cancel,
}

impl Default for LockFileFormState {
    fn default() -> Self {
        let mut state = Self {
            input_path: TextField::new(String::new()),
            output_path: TextField::new(String::new()),
            target_delay: TextField::new("3d".to_string()),
            profile_index: default_profile_index(),
            output_touched: false,
            focus: LockFileFocus::InputPath,
        };
        state.arm_focus_field_for_replace();
        state
    }
}

impl LockFileFormState {
    pub(crate) fn arm_focus_field_for_replace(&mut self) {
        match self.focus {
            LockFileFocus::InputPath => self.input_path.arm_clear_on_next_edit(),
            LockFileFocus::OutputPath => self.output_path.arm_clear_on_next_edit(),
            LockFileFocus::TargetDelay => self.target_delay.arm_clear_on_next_edit(),
            _ => {}
        }
    }

    pub(crate) fn profile_next(&mut self) {
        self.profile_index = (self.profile_index + 1) % profile_option_count().max(1);
    }

    pub(crate) fn profile_prev(&mut self) {
        if self.profile_index == 0 {
            self.profile_index = profile_option_count().saturating_sub(1);
        } else {
            self.profile_index -= 1;
        }
    }
}

impl LockFileFocus {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::InputPath => Self::BrowseInput,
            Self::BrowseInput => Self::OutputPath,
            Self::OutputPath => Self::TargetDelay,
            Self::TargetDelay => Self::HardwareProfile,
            Self::HardwareProfile => Self::Lock,
            Self::Lock => Self::Cancel,
            Self::Cancel => Self::InputPath,
        }
    }

    pub(crate) fn prev(self) -> Self {
        match self {
            Self::InputPath => Self::Cancel,
            Self::BrowseInput => Self::InputPath,
            Self::OutputPath => Self::BrowseInput,
            Self::TargetDelay => Self::OutputPath,
            Self::HardwareProfile => Self::TargetDelay,
            Self::Lock => Self::HardwareProfile,
            Self::Cancel => Self::Lock,
        }
    }
}

pub fn default_profile_index() -> usize {
    all_profiles()
        .iter()
        .position(|profile| profile.id == "desktop-2026")
        .unwrap_or(0)
}

pub fn help(focus: LockFileFocus) -> &'static str {
    match focus {
        LockFileFocus::InputPath => "Path to the original file to lock.",
        LockFileFocus::BrowseInput => "Open file browser.",
        LockFileFocus::OutputPath => "Default is <input>.timelocked.",
        LockFileFocus::TargetDelay => "Examples: 6h, 3d, 2w.",
        LockFileFocus::HardwareProfile => {
            "Profile converts delay into iterations. Current machine calibrates once per session."
        }
        LockFileFocus::Lock => "Start locking operation.",
        LockFileFocus::Cancel => "Back to main menu.",
    }
}

pub(crate) fn profile_option_count() -> usize {
    all_profiles().len() + 1
}

const FORM_LABEL_WIDTH: usize = 22;

pub(crate) fn profile_id_for_index(index: usize) -> &'static str {
    all_profiles()
        .get(index)
        .map(|profile| profile.id)
        .unwrap_or(CURRENT_MACHINE_PROFILE_ID)
}

pub(crate) fn profile_label_for_index(index: usize) -> &'static str {
    let profile_id = profile_id_for_index(index);
    if is_current_machine_profile_id(profile_id) {
        "current machine"
    } else {
        profile_id
    }
}

pub fn derive_default_output(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    ensure_timelocked_extension(PathBuf::from(trimmed))
        .to_string_lossy()
        .into_owned()
}

pub fn render(state: &LockFileFormState, frame: &mut Frame, area: Rect, app: &App) {
    let profile_id = profile_label_for_index(state.profile_index);
    let label_width = FORM_LABEL_WIDTH.max(label_width(&[
        "Input file",
        "Output file",
        "Target delay",
        "Hardware profile",
    ]));
    let lines = vec![
        line_with_field_and_button(
            "Input file",
            label_width,
            &state.input_path.value,
            FieldChrome::Input,
            matches!(state.focus, LockFileFocus::InputPath),
            InlineButton {
                label: "Browse",
                kind: ActionKind::Secondary,
                focused: matches!(state.focus, LockFileFocus::BrowseInput),
            },
            app,
        ),
        line_with_field(
            "Output file",
            label_width,
            &state.output_path.value,
            FieldChrome::Input,
            matches!(state.focus, LockFileFocus::OutputPath),
            app,
        ),
        line_with_field(
            "Target delay",
            label_width,
            &state.target_delay.value,
            FieldChrome::Input,
            matches!(state.focus, LockFileFocus::TargetDelay),
            app,
        ),
        helper_line("Examples: 6h, 3d, 2w.", label_width, app),
        line_with_field(
            "Hardware profile",
            label_width,
            profile_id,
            FieldChrome::Selector,
            matches!(state.focus, LockFileFocus::HardwareProfile),
            app,
        ),
        Line::from(""),
        Line::from(vec![
            button_span(
                "Lock",
                ActionKind::Primary,
                matches!(state.focus, LockFileFocus::Lock),
                app,
            ),
            Span::raw("  "),
            button_span(
                "Cancel",
                ActionKind::Secondary,
                matches!(state.focus, LockFileFocus::Cancel),
                app,
            ),
        ]),
    ];
    render_block_paragraph(frame, area, "Lock a File", lines, app);
}

pub fn handle_key(state: &mut LockFileFormState, key: KeyEvent, app: &mut App) -> Screen {
    if key.code == KeyCode::Esc {
        return Screen::MainMenu(MainMenuState::default());
    }

    if let Some(focus) = cycled_focus(
        key,
        state.focus,
        LockFileFocus::next,
        LockFileFocus::prev,
        FocusNavigationAxis::Vertical,
    ) {
        state.focus = focus;
        state.arm_focus_field_for_replace();
        return Screen::LockFileForm(state.clone());
    }

    match state.focus {
        LockFileFocus::InputPath => {
            if state.input_path.apply_key(key) && !state.output_touched {
                state.output_path = TextField::new(derive_default_output(&state.input_path.value));
            }
            Screen::LockFileForm(state.clone())
        }
        LockFileFocus::BrowseInput => {
            if key.code == KeyCode::Enter {
                app.open_browser(
                    BrowserTarget::LockFileInput,
                    BrowserMode::File,
                    Some(PathBuf::from(state.input_path.value.trim())),
                );
            }
            Screen::LockFileForm(state.clone())
        }
        LockFileFocus::OutputPath => {
            if state.output_path.apply_key(key) {
                state.output_touched = true;
            }
            Screen::LockFileForm(state.clone())
        }
        LockFileFocus::TargetDelay => {
            state.target_delay.apply_key(key);
            Screen::LockFileForm(state.clone())
        }
        LockFileFocus::HardwareProfile => {
            if key.code == KeyCode::Left {
                state.profile_prev();
            } else if matches!(key.code, KeyCode::Right | KeyCode::Char(' ')) {
                state.profile_next();
            }
            Screen::LockFileForm(state.clone())
        }
        LockFileFocus::Lock => {
            if key.code == KeyCode::Enter {
                match start_lock_file(app, state) {
                    Ok(progress) => progress,
                    Err(err) => {
                        app.modal = Some(Modal::Error(err));
                        Screen::LockFileForm(state.clone())
                    }
                }
            } else {
                Screen::LockFileForm(state.clone())
            }
        }
        LockFileFocus::Cancel => {
            if key.code == KeyCode::Enter {
                Screen::MainMenu(MainMenuState::default())
            } else {
                Screen::LockFileForm(state.clone())
            }
        }
    }
}

pub(crate) fn start_lock_file(
    app: &mut App,
    state: &LockFileFormState,
) -> std::result::Result<Screen, String> {
    let input = state.input_path.value.trim();
    if input.is_empty() {
        return Err("Input file is required.".to_string());
    }

    let output = state.output_path.value.trim();
    if output.is_empty() {
        return Err("Output timelocked file is required.".to_string());
    }

    let target = state.target_delay.value.trim();
    if target.is_empty() {
        return Err("Target delay is required (examples: 6h, 3d, 2w).".to_string());
    }

    let profile = profile_id_for_index(state.profile_index).to_string();
    let current_machine_iterations_per_second = if is_current_machine_profile_id(&profile) {
        Some(
            app.ensure_session_calibration()
                .map_err(|err: crate::base::Error| err.to_string())?,
        )
    } else {
        None
    };

    let worker = spawn_lock_worker(lock::LockRequest {
        input: input.to_string(),
        output: Some(PathBuf::from(output)),
        modulus_bits: lock_modulus_bits(),
        target: Some(target.to_string()),
        iterations: None,
        hardware_profile: Some(profile),
        current_machine_iterations_per_second,
        creator_name: None,
        creator_message: None,
        verify: false,
    });

    Ok(new_lock_progress_screen(
        input.to_string(),
        output.to_string(),
        worker,
    ))
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{
        derive_default_output, handle_key, profile_id_for_index, profile_option_count,
        LockFileFocus, LockFileFormState,
    };
    use crate::userinterfaces::tui::app_state::{App, Screen};

    fn test_app() -> App {
        App::new(false)
    }

    #[test]
    fn profile_options_include_current_machine() {
        assert_eq!(
            profile_id_for_index(profile_option_count() - 1),
            "current-machine"
        );
    }

    #[test]
    fn left_and_right_change_hardware_profile_selection() {
        let mut app = test_app();
        let mut state = LockFileFormState {
            focus: LockFileFocus::HardwareProfile,
            profile_index: 1,
            ..LockFileFormState::default()
        };

        let screen = handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            &mut app,
        );
        match screen {
            Screen::LockFileForm(updated) => {
                assert!(matches!(updated.focus, LockFileFocus::HardwareProfile));
                assert_eq!(updated.profile_index, 2);
            }
            _ => panic!("expected lock file form"),
        }

        let screen = handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            &mut app,
        );
        match screen {
            Screen::LockFileForm(updated) => {
                assert!(matches!(updated.focus, LockFileFocus::HardwareProfile));
                assert_eq!(updated.profile_index, 1);
            }
            _ => panic!("expected lock file form"),
        }
    }

    #[test]
    fn up_and_down_move_focus_away_from_hardware_profile() {
        let mut app = test_app();
        let mut state = LockFileFormState {
            focus: LockFileFocus::HardwareProfile,
            profile_index: 1,
            ..LockFileFormState::default()
        };

        let screen = handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &mut app,
        );
        match screen {
            Screen::LockFileForm(updated) => {
                assert!(matches!(updated.focus, LockFileFocus::Lock));
                assert_eq!(updated.profile_index, 1);
            }
            _ => panic!("expected lock file form"),
        }

        state.focus = LockFileFocus::HardwareProfile;

        let screen = handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            &mut app,
        );
        match screen {
            Screen::LockFileForm(updated) => {
                assert!(matches!(updated.focus, LockFileFocus::TargetDelay));
                assert_eq!(updated.profile_index, 1);
            }
            _ => panic!("expected lock file form"),
        }
    }

    #[test]
    fn derive_default_output_appends_timelocked_extension() {
        assert_eq!(derive_default_output("story.txt"), "story.txt.timelocked");
    }

    #[test]
    fn derive_default_output_preserves_existing_timelocked_extension() {
        assert_eq!(
            derive_default_output("archive.timelocked"),
            "archive.timelocked"
        );
    }

    #[test]
    fn derive_default_output_appends_after_other_extensions() {
        assert_eq!(
            derive_default_output("archive.tar"),
            "archive.tar.timelocked"
        );
    }

    #[test]
    fn derive_default_output_returns_empty_for_empty_input() {
        assert_eq!(derive_default_output(""), "");
        assert_eq!(derive_default_output("   "), "");
    }
}
