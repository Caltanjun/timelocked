//! Renders structural verification success details and a simple exit action.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::Frame;

use crate::usecases::verify;
use crate::userinterfaces::common::output::format_binary_size;
use crate::userinterfaces::tui::app_state::{App, MainMenuState, Screen};
use crate::userinterfaces::tui::components::form::{
    button_span, label_width, read_only_row, ActionKind, ReadOnlyValueKind,
};
use crate::userinterfaces::tui::components::layout::render_block_paragraph;

#[derive(Debug, Clone)]
pub struct VerifyDetailsState {
    pub response: verify::VerifyResponse,
    pub focus: VerifyDetailsFocus,
}

#[derive(Debug, Clone, Copy)]
pub enum VerifyDetailsFocus {
    Done,
}

pub fn help(focus: VerifyDetailsFocus) -> &'static str {
    match focus {
        VerifyDetailsFocus::Done => "Return to main menu.",
    }
}

pub fn render(state: &VerifyDetailsState, frame: &mut Frame, area: Rect, app: &App) {
    let lines = details_lines(state, app);
    render_block_paragraph(frame, area, "Structural Verification", lines, app);
}

fn details_lines(state: &VerifyDetailsState, app: &App) -> Vec<Line<'static>> {
    let label_width = label_width(&[
        "Status",
        "File",
        "Chunks",
        "Payload plaintext size",
        "Next step",
    ]);
    vec![
        read_only_row(
            "",
            "Status",
            label_width,
            "Structural verification OK",
            ReadOnlyValueKind::Success,
            app,
        ),
        read_only_row(
            "",
            "File",
            label_width,
            &state.response.path.display().to_string(),
            ReadOnlyValueKind::Default,
            app,
        ),
        read_only_row(
            "",
            "Chunks",
            label_width,
            &state.response.chunk_count.to_string(),
            ReadOnlyValueKind::Detail,
            app,
        ),
        read_only_row(
            "",
            "Next step",
            label_width,
            "Use unlock for full payload authentication and recovery.",
            ReadOnlyValueKind::Default,
            app,
        ),
        read_only_row(
            "",
            "Payload plaintext size",
            label_width,
            &format_binary_size(state.response.payload_plaintext_bytes),
            ReadOnlyValueKind::Detail,
            app,
        ),
        Line::from(""),
        Line::from(vec![button_span(
            "Done",
            ActionKind::Primary,
            matches!(state.focus, VerifyDetailsFocus::Done),
            app,
        )]),
    ]
}

pub fn handle_key(state: VerifyDetailsState, key: KeyEvent, _app: &mut App) -> Screen {
    match key.code {
        KeyCode::Esc | KeyCode::Enter => Screen::MainMenu(MainMenuState::default()),
        _ => Screen::VerifyDetails(state),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ratatui::style::Color;

    use super::{details_lines, VerifyDetailsFocus, VerifyDetailsState};
    use crate::usecases::verify::VerifyResponse;
    use crate::userinterfaces::tui::app_state::App;

    #[test]
    fn verification_details_highlight_success_and_metadata() {
        let app = App::new(false);
        let state = VerifyDetailsState {
            response: VerifyResponse {
                path: PathBuf::from("file.timelocked"),
                chunk_count: 3,
                payload_plaintext_bytes: 4096,
            },
            focus: VerifyDetailsFocus::Done,
        };

        let lines = details_lines(&state, &app);

        assert_eq!(lines[0].spans[3].style.fg, Some(Color::Green));
        assert_eq!(lines[2].spans[3].style.fg, Some(Color::Cyan));
        assert_eq!(lines[4].spans[3].style.fg, Some(Color::Cyan));
    }
}
