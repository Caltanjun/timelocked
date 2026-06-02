//! Renders the text-lock form and starts worker-backed lock progress screens.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::configuration::runtime::lock_modulus_bits;
use crate::domains::timelock::is_current_machine_profile_id;
use crate::usecases::lock;
use crate::userinterfaces::tui::app_state::{App, Modal, Screen};
use crate::userinterfaces::tui::components::form::{
    button_span, helper_line, label_width, line_with_field, line_with_secret_field, ActionKind,
    FieldChrome,
};
use crate::userinterfaces::tui::components::layout::render_block_paragraph;
use crate::userinterfaces::tui::features::main_menu::screen::MainMenuState;
use crate::userinterfaces::tui::features::shared::form_navigation::{
    cycled_focus, FocusNavigationAxis,
};
use crate::userinterfaces::tui::features::shared::progress_screens::new_lock_progress_screen;
use crate::userinterfaces::tui::state::{SecretTextField, TextField};
use crate::userinterfaces::tui::worker::spawn_lock_worker;

use super::file_form::{
    default_profile_index, password_from_fields, profile_id_for_index, profile_label_for_index,
    profile_option_count,
};

const FORM_LABEL_WIDTH: usize = 22;

#[derive(Debug, Clone)]
pub struct LockTextFormState {
    pub input_text: TextField,
    pub output_path: TextField,
    pub target_delay: TextField,
    pub password: SecretTextField,
    pub password_confirmation: SecretTextField,
    pub profile_index: usize,
    pub focus: LockTextFocus,
}

#[derive(Debug, Clone, Copy)]
pub enum LockTextFocus {
    InputText,
    OutputPath,
    TargetDelay,
    Password,
    ConfirmPassword,
    HardwareProfile,
    Lock,
    Cancel,
}

impl Default for LockTextFormState {
    fn default() -> Self {
        let mut state = Self {
            input_text: TextField::new(String::new()),
            output_path: TextField::new(String::new()),
            target_delay: TextField::new("3d".to_string()),
            password: SecretTextField::default(),
            password_confirmation: SecretTextField::default(),
            profile_index: default_profile_index(),
            focus: LockTextFocus::InputText,
        };
        state.arm_focus_field_for_replace();
        state
    }
}

impl LockTextFormState {
    pub(crate) fn arm_focus_field_for_replace(&mut self) {
        match self.focus {
            LockTextFocus::InputText => self.input_text.arm_clear_on_next_edit(),
            LockTextFocus::OutputPath => self.output_path.arm_clear_on_next_edit(),
            LockTextFocus::TargetDelay => self.target_delay.arm_clear_on_next_edit(),
            LockTextFocus::Password => self.password.arm_clear_on_next_edit(),
            LockTextFocus::ConfirmPassword => self.password_confirmation.arm_clear_on_next_edit(),
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

impl LockTextFocus {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::InputText => Self::OutputPath,
            Self::OutputPath => Self::TargetDelay,
            Self::TargetDelay => Self::Password,
            Self::Password => Self::ConfirmPassword,
            Self::ConfirmPassword => Self::HardwareProfile,
            Self::HardwareProfile => Self::Lock,
            Self::Lock => Self::Cancel,
            Self::Cancel => Self::InputText,
        }
    }

    pub(crate) fn prev(self) -> Self {
        match self {
            Self::InputText => Self::Cancel,
            Self::OutputPath => Self::InputText,
            Self::TargetDelay => Self::OutputPath,
            Self::Password => Self::TargetDelay,
            Self::ConfirmPassword => Self::Password,
            Self::HardwareProfile => Self::ConfirmPassword,
            Self::Lock => Self::HardwareProfile,
            Self::Cancel => Self::Lock,
        }
    }
}

pub fn help(focus: LockTextFocus) -> &'static str {
    match focus {
        LockTextFocus::InputText => "Text message to lock.",
        LockTextFocus::OutputPath => "Path to the output .timelocked file.",
        LockTextFocus::TargetDelay => "Examples: 6h, 3d, 2w.",
        LockTextFocus::Password => "Optional password. Leave empty for no password protection.",
        LockTextFocus::ConfirmPassword => "Re-enter the optional password exactly.",
        LockTextFocus::HardwareProfile => "Higher-end profiles will require more CPU work.",
        LockTextFocus::Lock => "Start locking operation.",
        LockTextFocus::Cancel => "Back to main menu.",
    }
}

pub fn render(state: &LockTextFormState, frame: &mut Frame, area: Rect, app: &App) {
    let profile_id = profile_label_for_index(state.profile_index);
    let label_width = FORM_LABEL_WIDTH.max(label_width(&[
        "Input text",
        "Output file",
        "Target delay",
        "Password",
        "Confirm password",
        "Hardware profile",
    ]));
    let lines = vec![
        line_with_field(
            "Input text",
            label_width,
            &state.input_text.value,
            FieldChrome::Input,
            matches!(state.focus, LockTextFocus::InputText),
            app,
        ),
        line_with_field(
            "Output file",
            label_width,
            &state.output_path.value,
            FieldChrome::Input,
            matches!(state.focus, LockTextFocus::OutputPath),
            app,
        ),
        line_with_field(
            "Target delay",
            label_width,
            &state.target_delay.value,
            FieldChrome::Input,
            matches!(state.focus, LockTextFocus::TargetDelay),
            app,
        ),
        helper_line("Examples: 6h, 3d, 2w.", label_width, app),
        line_with_secret_field(
            "Password",
            label_width,
            state.password.secret_char_count(),
            matches!(state.focus, LockTextFocus::Password),
            app,
        ),
        helper_line(
            "Optional. Leave empty for no password protection.",
            label_width,
            app,
        ),
        line_with_secret_field(
            "Confirm password",
            label_width,
            state.password_confirmation.secret_char_count(),
            matches!(state.focus, LockTextFocus::ConfirmPassword),
            app,
        ),
        line_with_field(
            "Hardware profile",
            label_width,
            profile_id,
            FieldChrome::Selector,
            matches!(state.focus, LockTextFocus::HardwareProfile),
            app,
        ),
        Line::from(""),
        Line::from(vec![
            button_span(
                "Lock",
                ActionKind::Primary,
                matches!(state.focus, LockTextFocus::Lock),
                app,
            ),
            Span::raw("  "),
            button_span(
                "Cancel",
                ActionKind::Secondary,
                matches!(state.focus, LockTextFocus::Cancel),
                app,
            ),
        ]),
    ];
    render_block_paragraph(frame, area, "Lock Text Message", lines, app);
}

pub fn handle_key(state: &mut LockTextFormState, key: KeyEvent, app: &mut App) -> Screen {
    if key.code == KeyCode::Esc {
        return Screen::MainMenu(MainMenuState::default());
    }

    if let Some(focus) = cycled_focus(
        key,
        state.focus,
        LockTextFocus::next,
        LockTextFocus::prev,
        FocusNavigationAxis::Vertical,
    ) {
        state.focus = focus;
        state.arm_focus_field_for_replace();
        return Screen::LockTextForm(state.clone());
    }

    match state.focus {
        LockTextFocus::InputText => {
            state.input_text.apply_key(key);
            Screen::LockTextForm(state.clone())
        }
        LockTextFocus::OutputPath => {
            state.output_path.apply_key(key);
            Screen::LockTextForm(state.clone())
        }
        LockTextFocus::TargetDelay => {
            state.target_delay.apply_key(key);
            Screen::LockTextForm(state.clone())
        }
        LockTextFocus::Password => {
            state.password.apply_key(key);
            Screen::LockTextForm(state.clone())
        }
        LockTextFocus::ConfirmPassword => {
            state.password_confirmation.apply_key(key);
            Screen::LockTextForm(state.clone())
        }
        LockTextFocus::HardwareProfile => {
            if key.code == KeyCode::Left {
                state.profile_prev();
            } else if matches!(key.code, KeyCode::Right | KeyCode::Char(' ')) {
                state.profile_next();
            }
            Screen::LockTextForm(state.clone())
        }
        LockTextFocus::Lock => {
            if key.code == KeyCode::Enter {
                match start_lock_text(app, state) {
                    Ok(progress) => progress,
                    Err(err) => {
                        app.modal = Some(Modal::Error(err));
                        Screen::LockTextForm(state.clone())
                    }
                }
            } else {
                Screen::LockTextForm(state.clone())
            }
        }
        LockTextFocus::Cancel => {
            if key.code == KeyCode::Enter {
                Screen::MainMenu(MainMenuState::default())
            } else {
                Screen::LockTextForm(state.clone())
            }
        }
    }
}

pub(crate) fn start_lock_text(
    app: &mut App,
    state: &mut LockTextFormState,
) -> std::result::Result<Screen, String> {
    let request = build_lock_text_request(app, state)?;
    let output_display = request
        .output
        .as_ref()
        .expect("text lock requests always include output")
        .to_string_lossy()
        .into_owned();

    let worker = spawn_lock_worker(request);
    state.password.clear();
    state.password_confirmation.clear();

    Ok(new_lock_progress_screen(
        "inline text".to_string(),
        output_display,
        worker,
    ))
}

pub(crate) fn build_lock_text_request(
    app: &mut App,
    state: &LockTextFormState,
) -> std::result::Result<lock::LockRequest, String> {
    let input_text = state.input_text.value.trim();
    if input_text.is_empty() {
        return Err("Input text is required.".to_string());
    }

    let output = state.output_path.value.trim();
    if output.is_empty() {
        return Err("Output timelocked file is required for text lock.".to_string());
    }

    let target = state.target_delay.value.trim();
    if target.is_empty() {
        return Err("Target delay is required (examples: 6h, 3d, 2w).".to_string());
    }

    let password = password_from_fields(&state.password, &state.password_confirmation)?;

    let profile = profile_id_for_index(state.profile_index).to_string();
    let current_machine_iterations_per_second = if is_current_machine_profile_id(&profile) {
        Some(
            app.ensure_session_calibration()
                .map_err(|err: crate::base::Error| err.to_string())?,
        )
    } else {
        None
    };

    Ok(lock::LockRequest {
        input: input_text.to_string(),
        output: Some(PathBuf::from(output)),
        modulus_bits: lock_modulus_bits(),
        target: Some(target.to_string()),
        iterations: None,
        hardware_profile: Some(profile),
        current_machine_iterations_per_second,
        creator_name: None,
        creator_message: None,
        password,
        verify: false,
    })
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::Terminal;

    use super::{build_lock_text_request, handle_key, render, LockTextFocus, LockTextFormState};
    use crate::userinterfaces::tui::app_state::{App, Modal, Screen};
    use crate::userinterfaces::tui::state::{SecretTextField, TextField};

    fn test_app() -> App {
        App::new(false)
    }

    fn buffer_text(buffer: &Buffer) -> String {
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn left_and_right_change_hardware_profile_selection() {
        let mut app = test_app();
        let mut state = LockTextFormState {
            focus: LockTextFocus::HardwareProfile,
            profile_index: 1,
            ..LockTextFormState::default()
        };

        let screen = handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            &mut app,
        );
        match screen {
            Screen::LockTextForm(updated) => {
                assert!(matches!(updated.focus, LockTextFocus::HardwareProfile));
                assert_eq!(updated.profile_index, 2);
            }
            _ => panic!("expected lock text form"),
        }

        let screen = handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            &mut app,
        );
        match screen {
            Screen::LockTextForm(updated) => {
                assert!(matches!(updated.focus, LockTextFocus::HardwareProfile));
                assert_eq!(updated.profile_index, 1);
            }
            _ => panic!("expected lock text form"),
        }
    }

    #[test]
    fn up_and_down_move_focus_away_from_hardware_profile() {
        let mut app = test_app();
        let mut state = LockTextFormState {
            focus: LockTextFocus::HardwareProfile,
            profile_index: 1,
            ..LockTextFormState::default()
        };

        let screen = handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &mut app,
        );
        match screen {
            Screen::LockTextForm(updated) => {
                assert!(matches!(updated.focus, LockTextFocus::Lock));
                assert_eq!(updated.profile_index, 1);
            }
            _ => panic!("expected lock text form"),
        }

        state.focus = LockTextFocus::HardwareProfile;

        let screen = handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            &mut app,
        );
        match screen {
            Screen::LockTextForm(updated) => {
                assert!(matches!(updated.focus, LockTextFocus::ConfirmPassword));
                assert_eq!(updated.profile_index, 1);
            }
            _ => panic!("expected lock text form"),
        }
    }

    #[test]
    fn lock_text_form_masks_password_value() {
        let app = test_app();
        let state = LockTextFormState {
            password: SecretTextField::new("open sesame".to_string()),
            password_confirmation: SecretTextField::new("open sesame".to_string()),
            ..LockTextFormState::default()
        };
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).expect("test backend should initialize");

        terminal
            .draw(|frame| render(&state, frame, frame.area(), &app))
            .expect("lock text form should render");
        let rendered = buffer_text(terminal.backend().buffer());

        assert!(!rendered.contains("open sesame"));
        assert!(rendered.contains("***********"));
    }

    #[test]
    fn lock_text_form_password_confirmation_mismatch_shows_error() {
        let mut app = test_app();
        let mut state = LockTextFormState {
            input_text: TextField::new("message"),
            output_path: TextField::new("message.timelocked"),
            password: SecretTextField::new("one".to_string()),
            password_confirmation: SecretTextField::new("two".to_string()),
            focus: LockTextFocus::Lock,
            ..LockTextFormState::default()
        };

        let screen = handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app,
        );

        assert!(matches!(screen, Screen::LockTextForm(_)));
        assert!(
            matches!(app.modal, Some(Modal::Error(ref message)) if message.contains("do not match"))
        );
    }

    #[test]
    fn lock_text_form_passes_password_to_worker_when_present() {
        let mut app = test_app();
        let state = LockTextFormState {
            input_text: TextField::new("message"),
            output_path: TextField::new("message.timelocked"),
            password: SecretTextField::new(" exact password ".to_string()),
            password_confirmation: SecretTextField::new(" exact password ".to_string()),
            ..LockTextFormState::default()
        };

        let request = build_lock_text_request(&mut app, &state).expect("request should build");

        assert_eq!(
            request
                .password
                .expect("password should be present")
                .expose_secret(),
            " exact password "
        );
    }
}
