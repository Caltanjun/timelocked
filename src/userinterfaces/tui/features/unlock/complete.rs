//! Renders the post-unlock result screen for files and recovered text.

use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::Frame;
use zeroize::Zeroize;

use crate::base::open_directory_in_file_manager;
use crate::usecases::unlock;
use crate::userinterfaces::common::output::format_binary_size;
use crate::userinterfaces::tui::app_state::{App, Modal, Screen};
use crate::userinterfaces::tui::components::form::{
    button_span, label_width, read_only_row, ActionKind, ReadOnlyValueKind,
};
use crate::userinterfaces::tui::components::layout::render_block_paragraph;
use crate::userinterfaces::tui::features::main_menu::screen::MainMenuState;
use crate::userinterfaces::tui::features::shared::form_navigation::{
    cycled_focus, FocusNavigationAxis,
};

#[derive(Debug)]
pub struct UnlockCompleteState {
    pub recovered_payload: unlock::RecoveredPayload,
    pub recovered_bytes: u64,
    pub focus: UnlockCompleteFocus,
}

#[derive(Debug, Clone, Copy)]
pub enum UnlockCompleteFocus {
    OpenFolder,
    Done,
}

impl UnlockCompleteState {
    pub(crate) fn zeroize_sensitive(&mut self) {
        if let unlock::RecoveredPayload::Text { text } = &mut self.recovered_payload {
            text.zeroize();
        }
    }
}

impl Drop for UnlockCompleteState {
    fn drop(&mut self) {
        self.zeroize_sensitive();
    }
}

impl UnlockCompleteFocus {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::OpenFolder => Self::Done,
            Self::Done => Self::OpenFolder,
        }
    }

    pub(crate) fn prev(self) -> Self {
        self.next()
    }
}

pub fn help(state: &UnlockCompleteState) -> &'static str {
    match &state.recovered_payload {
        unlock::RecoveredPayload::Text { .. } => "Press Enter or Esc to return.",
        unlock::RecoveredPayload::File { .. } => match state.focus {
            UnlockCompleteFocus::OpenFolder => "Open destination folder.",
            UnlockCompleteFocus::Done => "Return to main menu.",
        },
    }
}

pub fn render(state: &UnlockCompleteState, frame: &mut Frame, area: Rect, app: &App) {
    let lines = complete_lines(state, app);
    render_block_paragraph(frame, area, "Unlock Complete", lines, app);
}

fn complete_lines<'a>(state: &'a UnlockCompleteState, app: &App) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    match &state.recovered_payload {
        unlock::RecoveredPayload::File { path } => {
            let label_width = label_width(&["Output file", "Integrity", "Recovered size"]);

            lines.push(read_only_row(
                "",
                "Output file",
                label_width,
                &path.display().to_string(),
                ReadOnlyValueKind::Default,
                app,
            ));
            lines.push(read_only_row(
                "",
                "Integrity",
                label_width,
                "OK",
                ReadOnlyValueKind::Success,
                app,
            ));
            lines.push(read_only_row(
                "",
                "Recovered size",
                label_width,
                &format_binary_size(state.recovered_bytes),
                ReadOnlyValueKind::Detail,
                app,
            ));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                button_span(
                    "Open folder",
                    ActionKind::Secondary,
                    matches!(state.focus, UnlockCompleteFocus::OpenFolder),
                    app,
                ),
                Span::raw("  "),
                button_span(
                    "Done",
                    ActionKind::Primary,
                    matches!(state.focus, UnlockCompleteFocus::Done),
                    app,
                ),
            ]));
        }
        unlock::RecoveredPayload::Text { text } => {
            lines.push(Line::from("Recovered message:"));
            lines.push(Line::from(text.as_str()));
            lines.push(Line::from(""));
            lines.push(read_only_row(
                "",
                "Recovered size",
                "Recovered size".chars().count(),
                &format_binary_size(state.recovered_bytes),
                ReadOnlyValueKind::Detail,
                app,
            ));
            lines.push(Line::from(""));
            lines.push(Line::from(
                "Save this message now to avoid having to unlock again.",
            ));
            lines.push(Line::from(
                "Nothing is saved to disk. Closing this screen clears this content from memory.",
            ));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![button_span(
                "Done",
                ActionKind::Primary,
                true,
                app,
            )]));
        }
    }
    lines
}

pub fn handle_key(mut state: UnlockCompleteState, key: KeyEvent, app: &mut App) -> Screen {
    if key.code == KeyCode::Esc {
        state.zeroize_sensitive();
        return Screen::MainMenu(MainMenuState::default());
    }

    if matches!(
        &state.recovered_payload,
        unlock::RecoveredPayload::Text { .. }
    ) {
        if key.code == KeyCode::Enter {
            state.zeroize_sensitive();
            return Screen::MainMenu(MainMenuState::default());
        }
        return Screen::UnlockComplete(state);
    }

    if let Some(focus) = cycled_focus(
        key,
        state.focus,
        UnlockCompleteFocus::next,
        UnlockCompleteFocus::prev,
        FocusNavigationAxis::Horizontal,
    ) {
        state.focus = focus;
        return Screen::UnlockComplete(state);
    }

    match key.code {
        KeyCode::Enter => match state.focus {
            UnlockCompleteFocus::OpenFolder => {
                if let unlock::RecoveredPayload::File { path } = &state.recovered_payload {
                    let parent = path.parent().unwrap_or(Path::new("."));
                    match open_directory_in_file_manager(parent) {
                        Ok(()) => {
                            app.modal = Some(Modal::Info("Opened output folder.".to_string()));
                        }
                        Err(err) => {
                            app.modal = Some(Modal::Error(err.to_string()));
                        }
                    }
                }
                Screen::UnlockComplete(state)
            }
            UnlockCompleteFocus::Done => Screen::MainMenu(MainMenuState::default()),
        },
        _ => Screen::UnlockComplete(state),
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::path::PathBuf;

    use ratatui::style::Color;

    use super::{complete_lines, UnlockCompleteFocus, UnlockCompleteState};
    use crate::usecases::unlock::RecoveredPayload;
    use crate::userinterfaces::tui::app_state::App;

    fn test_app(no_color: bool) -> App {
        App::new(no_color)
    }

    #[test]
    fn unlock_file_complete_highlights_integrity_and_size() {
        let state = UnlockCompleteState {
            recovered_payload: RecoveredPayload::File {
                path: PathBuf::from("file.txt"),
            },
            recovered_bytes: 128,
            focus: UnlockCompleteFocus::Done,
        };

        let lines = complete_lines(&state, &test_app(false));

        assert_eq!(lines[1].spans[3].style.fg, Some(Color::Green));
        assert_eq!(lines[2].spans[3].style.fg, Some(Color::Cyan));
    }

    #[test]
    fn unlock_text_complete_keeps_size_emphasized_without_color() {
        let state = UnlockCompleteState {
            recovered_payload: RecoveredPayload::Text {
                text: "secret".to_string(),
            },
            recovered_bytes: 6,
            focus: UnlockCompleteFocus::Done,
        };

        let lines = complete_lines(&state, &test_app(true));

        assert_eq!(lines[3].spans[3].style.fg, None);
        assert!(lines[3].spans[3]
            .style
            .add_modifier
            .contains(ratatui::style::Modifier::BOLD));
    }

    #[test]
    fn unlock_text_complete_borrows_recovered_plaintext() {
        let state = UnlockCompleteState {
            recovered_payload: RecoveredPayload::Text {
                text: "secret".to_string(),
            },
            recovered_bytes: 6,
            focus: UnlockCompleteFocus::Done,
        };

        let lines = complete_lines(&state, &test_app(false));

        assert!(matches!(
            &lines[1].spans[0].content,
            Cow::Borrowed("secret")
        ));
    }
}
