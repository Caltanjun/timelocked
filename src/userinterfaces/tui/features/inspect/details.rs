//! Renders inspected timelocked metadata and follow-up actions.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::usecases::inspect;
use crate::userinterfaces::common::output::{format_binary_size, format_eta};
use crate::userinterfaces::tui::app_state::{App, MainMenuState, Modal, Screen};
use crate::userinterfaces::tui::components::form::{
    button_span, label_width, read_only_row, ActionKind, ReadOnlyValueKind,
};
use crate::userinterfaces::tui::components::layout::render_block_paragraph;
use crate::userinterfaces::tui::features::shared::form_navigation::{
    cycled_focus, FocusNavigationAxis,
};
use crate::userinterfaces::tui::features::unlock::form::start_unlock_from_path;

#[derive(Debug, Clone)]
pub struct InspectDetailsState {
    pub response: inspect::InspectResponse,
    pub focus: InspectDetailsFocus,
}

#[derive(Debug, Clone, Copy)]
pub enum InspectDetailsFocus {
    Unlock,
    Back,
}

impl InspectDetailsFocus {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Unlock => Self::Back,
            Self::Back => Self::Unlock,
        }
    }

    pub(crate) fn prev(self) -> Self {
        self.next()
    }
}

pub fn help(focus: InspectDetailsFocus) -> &'static str {
    match focus {
        InspectDetailsFocus::Unlock => "Start unlocking this timelocked file now.",
        InspectDetailsFocus::Back => "Return to main menu.",
    }
}

pub fn render(state: &InspectDetailsState, frame: &mut Frame, area: Rect, app: &App) {
    let lines = details_lines(state, app);
    render_block_paragraph(frame, area, "Inspect", lines, app);
}

fn details_lines(state: &InspectDetailsState, app: &App) -> Vec<Line<'static>> {
    let header = &state.response.header;
    let top_width = label_width(&["File", "Format", "Created", "Content size"]);
    let param_width = label_width(&[
        "Chosen delay",
        "Chosen hardware profile",
        "Estimated delay on this machine",
        "Estimated delay from chosen profile",
        "Estimated delay",
        "Iterations (T)",
    ]);
    let estimate_line =
        if let Some(seconds) = state.response.estimated_duration_on_current_machine_seconds {
            read_only_row(
                "  ",
                "Estimated delay on this machine",
                param_width,
                &format_eta(seconds),
                ReadOnlyValueKind::Warning,
                app,
            )
        } else if let Some(seconds) = state.response.estimated_duration_on_profile_seconds {
            read_only_row(
                "  ",
                "Estimated delay from chosen profile",
                param_width,
                &format_eta(seconds),
                ReadOnlyValueKind::Warning,
                app,
            )
        } else {
            read_only_row(
                "  ",
                "Estimated delay",
                param_width,
                "unavailable",
                ReadOnlyValueKind::Warning,
                app,
            )
        };
    let chosen_delay = header
        .timelock_params
        .target_seconds
        .map(format_eta)
        .unwrap_or_else(|| format!("{} iterations", header.timelock_params.iterations));
    vec![
        read_only_row(
            "",
            "File",
            top_width,
            &state.response.path.display().to_string(),
            ReadOnlyValueKind::Default,
            app,
        ),
        read_only_row(
            "",
            "Format",
            top_width,
            &format!("TLCK v{}", state.response.format_version),
            ReadOnlyValueKind::Detail,
            app,
        ),
        read_only_row(
            "",
            "Created",
            top_width,
            &header.created_at,
            ReadOnlyValueKind::Detail,
            app,
        ),
        read_only_row(
            "",
            "Content size",
            top_width,
            &format_binary_size(header.payload_plaintext_bytes),
            ReadOnlyValueKind::Detail,
            app,
        ),
        Line::from(""),
        Line::from("Params:"),
        read_only_row(
            "  ",
            "Chosen delay",
            param_width,
            &chosen_delay,
            ReadOnlyValueKind::Warning,
            app,
        ),
        read_only_row(
            "  ",
            "Chosen hardware profile",
            param_width,
            &header.timelock_params.hardware_profile,
            ReadOnlyValueKind::Detail,
            app,
        ),
        estimate_line,
        read_only_row(
            "  ",
            "Iterations (T)",
            param_width,
            &header.timelock_params.iterations.to_string(),
            ReadOnlyValueKind::Detail,
            app,
        ),
        Line::from(""),
        Line::from(vec![
            button_span(
                "Unlock",
                ActionKind::Primary,
                matches!(state.focus, InspectDetailsFocus::Unlock),
                app,
            ),
            Span::raw("  "),
            button_span(
                "Back",
                ActionKind::Secondary,
                matches!(state.focus, InspectDetailsFocus::Back),
                app,
            ),
        ]),
    ]
}

pub fn handle_key(mut state: InspectDetailsState, key: KeyEvent, app: &mut App) -> Screen {
    if key.code == KeyCode::Esc {
        return Screen::MainMenu(MainMenuState::default());
    }

    if let Some(focus) = cycled_focus(
        key,
        state.focus,
        InspectDetailsFocus::next,
        InspectDetailsFocus::prev,
        FocusNavigationAxis::Horizontal,
    ) {
        state.focus = focus;
        return Screen::InspectDetails(state);
    }

    match key.code {
        KeyCode::Enter => match state.focus {
            InspectDetailsFocus::Unlock => {
                match start_unlock_from_path(
                    app,
                    &state.response.path,
                    state
                        .response
                        .estimated_duration_on_current_machine_seconds
                        .or(state.response.estimated_duration_on_profile_seconds),
                ) {
                    Ok(next) => next,
                    Err(err) => {
                        app.modal = Some(Modal::Error(err));
                        Screen::InspectDetails(state)
                    }
                }
            }
            InspectDetailsFocus::Back => Screen::MainMenu(MainMenuState::default()),
        },
        _ => Screen::InspectDetails(state),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ratatui::style::Color;

    use super::{details_lines, InspectDetailsFocus, InspectDetailsState};
    use crate::domains::timelocked_file::{
        ChunkingParams, CipherParams, TimelockParams, TimelockedHeader,
    };
    use crate::usecases::inspect::InspectResponse;
    use crate::userinterfaces::tui::app_state::App;

    fn test_app(no_color: bool) -> App {
        App::new(no_color)
    }

    fn sample_state() -> InspectDetailsState {
        InspectDetailsState {
            response: InspectResponse {
                path: PathBuf::from("file.timelocked"),
                payload_len: 1234,
                format_version: 1,
                header: TimelockedHeader {
                    about: "About".to_string(),
                    created_at: "2026-02-16T22:01:00Z".to_string(),
                    original_filename: Some("file.txt".to_string()),
                    creator_name: None,
                    creator_message: None,
                    cipher_params: CipherParams {
                        payload_cipher: "XChaCha20-Poly1305".to_string(),
                        key_wrap: "blake3-xor-v1".to_string(),
                    },
                    chunking_params: ChunkingParams {
                        chunk_size_bytes: 4096,
                    },
                    payload_plaintext_bytes: 1234,
                    timelock_params: TimelockParams {
                        algorithm: "rsw-repeated-squaring-v1".to_string(),
                        iterations: 123_456,
                        modulus_bits: 256,
                        target_seconds: Some(3600),
                        hardware_profile: "high-end-cpu-2026".to_string(),
                    },
                },
                estimated_duration_on_current_machine_seconds: Some(3700),
                estimated_duration_on_profile_seconds: None,
            },
            focus: InspectDetailsFocus::Unlock,
        }
    }

    #[test]
    fn inspect_details_highlight_structured_values() {
        let lines = details_lines(&sample_state(), &test_app(false));

        assert_eq!(lines[1].spans[3].style.fg, Some(Color::Cyan));
        assert_eq!(lines[6].spans[3].style.fg, Some(Color::Yellow));
        assert_eq!(lines[7].spans[3].style.fg, Some(Color::Cyan));
    }

    #[test]
    fn inspect_details_keep_values_emphasized_without_color() {
        let lines = details_lines(&sample_state(), &test_app(true));

        assert_eq!(lines[1].spans[3].style.fg, None);
        assert!(lines[1].spans[3]
            .style
            .add_modifier
            .contains(ratatui::style::Modifier::BOLD));
    }
}
