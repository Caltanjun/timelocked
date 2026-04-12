//! Renders the inspect form and forwards metadata lookups to the inspect use case.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::Frame;

use super::details::{InspectDetailsFocus, InspectDetailsState};
use crate::usecases::inspect;
use crate::userinterfaces::tui::app_state::{App, BrowserMode, BrowserTarget, Modal, Screen};
use crate::userinterfaces::tui::components::form::{
    button_span, label_width, line_with_field_and_button, ActionKind, FieldChrome, InlineButton,
};
use crate::userinterfaces::tui::components::layout::render_block_paragraph;
use crate::userinterfaces::tui::features::main_menu::screen::MainMenuState;
use crate::userinterfaces::tui::features::shared::form_navigation::{
    cycled_focus, FocusNavigationAxis,
};
use crate::userinterfaces::tui::state::TextField;

#[derive(Debug, Clone)]
pub struct InspectFormState {
    pub input_path: TextField,
    pub focus: InspectFocus,
}

#[derive(Debug, Clone, Copy)]
pub enum InspectFocus {
    InputPath,
    BrowseInput,
    Inspect,
    Cancel,
}

impl Default for InspectFormState {
    fn default() -> Self {
        let mut state = Self {
            input_path: TextField::new(String::new()),
            focus: InspectFocus::InputPath,
        };
        state.arm_focus_field_for_replace();
        state
    }
}

impl InspectFormState {
    pub(crate) fn arm_focus_field_for_replace(&mut self) {
        if matches!(self.focus, InspectFocus::InputPath) {
            self.input_path.arm_clear_on_next_edit();
        }
    }
}

impl InspectFocus {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::InputPath => Self::BrowseInput,
            Self::BrowseInput => Self::Inspect,
            Self::Inspect => Self::Cancel,
            Self::Cancel => Self::InputPath,
        }
    }

    pub(crate) fn prev(self) -> Self {
        match self {
            Self::InputPath => Self::Cancel,
            Self::BrowseInput => Self::InputPath,
            Self::Inspect => Self::BrowseInput,
            Self::Cancel => Self::Inspect,
        }
    }
}

pub fn help(focus: InspectFocus) -> &'static str {
    match focus {
        InspectFocus::InputPath => "Path to a .timelocked file.",
        InspectFocus::BrowseInput => "Open file browser.",
        InspectFocus::Inspect => "Read metadata without heavy compute.",
        InspectFocus::Cancel => "Back to main menu.",
    }
}

const FORM_LABEL_WIDTH: usize = 16;

pub fn render(state: &InspectFormState, frame: &mut Frame, area: Rect, app: &App) {
    let label_width = FORM_LABEL_WIDTH.max(label_width(&["Timelocked file"]));
    let lines = vec![
        line_with_field_and_button(
            "Timelocked file",
            label_width,
            &state.input_path.value,
            FieldChrome::Input,
            matches!(state.focus, InspectFocus::InputPath),
            InlineButton {
                label: "Browse",
                kind: ActionKind::Secondary,
                focused: matches!(state.focus, InspectFocus::BrowseInput),
            },
            app,
        ),
        Line::from(""),
        Line::from(vec![
            button_span(
                "Inspect",
                ActionKind::Primary,
                matches!(state.focus, InspectFocus::Inspect),
                app,
            ),
            Span::raw("  "),
            button_span(
                "Cancel",
                ActionKind::Secondary,
                matches!(state.focus, InspectFocus::Cancel),
                app,
            ),
        ]),
    ];
    render_block_paragraph(frame, area, "Inspect Timelocked File", lines, app);
}

pub fn handle_key(state: &mut InspectFormState, key: KeyEvent, app: &mut App) -> Screen {
    if key.code == KeyCode::Esc {
        return Screen::MainMenu(MainMenuState::default());
    }

    if let Some(focus) = cycled_focus(
        key,
        state.focus,
        InspectFocus::next,
        InspectFocus::prev,
        FocusNavigationAxis::Vertical,
    ) {
        state.focus = focus;
        state.arm_focus_field_for_replace();
        return Screen::InspectForm(state.clone());
    }

    match state.focus {
        InspectFocus::InputPath => {
            state.input_path.apply_key(key);
            Screen::InspectForm(state.clone())
        }
        InspectFocus::BrowseInput => {
            if key.code == KeyCode::Enter {
                app.open_browser(
                    BrowserTarget::InspectInput,
                    BrowserMode::File,
                    Some(PathBuf::from(state.input_path.value.trim())),
                );
            }
            Screen::InspectForm(state.clone())
        }
        InspectFocus::Inspect => {
            if key.code == KeyCode::Enter {
                let input = state.input_path.value.trim();
                if input.is_empty() {
                    app.modal = Some(Modal::Error("Timelocked file is required.".to_string()));
                    return Screen::InspectForm(state.clone());
                }
                let input_path = PathBuf::from(input);
                let current_machine_iterations_per_second =
                    app.estimate_calibration_for_path(input_path.as_path());
                match inspect::execute(inspect::InspectRequest {
                    input: input_path,
                    current_machine_iterations_per_second,
                }) {
                    Ok(response) => Screen::InspectDetails(InspectDetailsState {
                        response,
                        focus: InspectDetailsFocus::Unlock,
                    }),
                    Err(err) => {
                        app.modal = Some(Modal::Error(err.to_string()));
                        Screen::InspectForm(state.clone())
                    }
                }
            } else {
                Screen::InspectForm(state.clone())
            }
        }
        InspectFocus::Cancel => {
            if key.code == KeyCode::Enter {
                Screen::MainMenu(MainMenuState::default())
            } else {
                Screen::InspectForm(state.clone())
            }
        }
    }
}
