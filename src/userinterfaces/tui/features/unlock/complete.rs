//! Renders the post-unlock result screen for files and recovered text.

use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
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
use crate::userinterfaces::tui::components::theme::{base_style, panel_block, titled_plain_block};
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
    match &state.recovered_payload {
        unlock::RecoveredPayload::File { path } => {
            let lines = file_complete_lines(path, state, app);
            render_block_paragraph(frame, area, "Unlock Complete", lines, app);
        }
        unlock::RecoveredPayload::Text { text } => {
            render_text_complete(text, state, frame, area, app)
        }
    }
}

fn render_text_complete(
    recovered_text: &str,
    state: &UnlockCompleteState,
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    let outer_block = panel_block("Unlock Complete", app);
    let inner = outer_block.inner(area);
    frame.render_widget(outer_block, area);

    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(TEXT_COMPLETE_DETAIL_ROWS),
        ])
        .split(inner);

    let recovered_message = Paragraph::new(recovered_message_lines(recovered_text))
        .block(titled_plain_block("Recovered message", app))
        .wrap(Wrap { trim: true })
        .style(base_style(app));
    frame.render_widget(recovered_message, body[0]);

    let details = Paragraph::new(text_complete_detail_lines(state, app)).style(base_style(app));
    frame.render_widget(details, body[1]);
}

fn recovered_message_lines(recovered_text: &str) -> Vec<Line<'_>> {
    vec![Line::from(recovered_text)]
}

fn file_complete_lines(path: &Path, state: &UnlockCompleteState, app: &App) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
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
    lines
}

const TEXT_COMPLETE_DETAIL_ROWS: u16 = 7;

fn text_complete_detail_lines(state: &UnlockCompleteState, app: &App) -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        read_only_row(
            "",
            "Recovered size",
            "Recovered size".chars().count(),
            &format_binary_size(state.recovered_bytes),
            ReadOnlyValueKind::Detail,
            app,
        ),
        Line::from(""),
        Line::from("Save this message now to avoid having to unlock again."),
        Line::from(
            "Nothing is saved to disk. Closing this screen clears this content from memory.",
        ),
        Line::from(""),
        Line::from(vec![button_span(
            "Done",
            ActionKind::Primary,
            matches!(state.focus, UnlockCompleteFocus::Done),
            app,
        )]),
    ]
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

    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::Color;
    use ratatui::Terminal;

    use super::{
        file_complete_lines, recovered_message_lines, render, text_complete_detail_lines,
        UnlockCompleteFocus, UnlockCompleteState,
    };
    use crate::usecases::unlock::RecoveredPayload;
    use crate::userinterfaces::tui::app_state::App;

    fn test_app(no_color: bool) -> App {
        App::new(no_color)
    }

    fn buffer_line(buffer: &Buffer, y: u16) -> String {
        (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect::<Vec<_>>()
            .join("")
    }

    fn buffer_lines(buffer: &Buffer) -> Vec<String> {
        (0..buffer.area.height)
            .map(|y| buffer_line(buffer, y))
            .collect()
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

        let RecoveredPayload::File { path } = &state.recovered_payload else {
            panic!("test state should use a file payload");
        };
        let lines = file_complete_lines(path, &state, &test_app(false));

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

        let lines = text_complete_detail_lines(&state, &test_app(true));

        assert_eq!(lines[1].spans[3].style.fg, None);
        assert!(lines[1].spans[3]
            .style
            .add_modifier
            .contains(ratatui::style::Modifier::BOLD));
    }

    #[test]
    fn unlock_text_complete_highlights_done_when_focused() {
        let state = UnlockCompleteState {
            recovered_payload: RecoveredPayload::Text {
                text: "secret".to_string(),
            },
            recovered_bytes: 6,
            focus: UnlockCompleteFocus::Done,
        };

        let lines = text_complete_detail_lines(&state, &test_app(false));

        assert!(lines[6].spans[0]
            .style
            .add_modifier
            .contains(ratatui::style::Modifier::REVERSED));
    }

    #[test]
    fn unlock_text_complete_renders_recovered_plaintext_inside_bordered_panel() {
        let state = UnlockCompleteState {
            recovered_payload: RecoveredPayload::Text {
                text: "secret".to_string(),
            },
            recovered_bytes: 6,
            focus: UnlockCompleteFocus::Done,
        };
        let app = test_app(true);
        let backend = TestBackend::new(60, 14);
        let mut terminal = Terminal::new(backend).expect("test backend should initialize");

        terminal
            .draw(|frame| render(&state, frame, frame.area(), &app))
            .expect("unlock complete screen should render");
        let buffer = terminal.backend().buffer();
        let lines = buffer_lines(buffer);

        let title_row = lines
            .iter()
            .position(|line| line.contains("┌") && line.contains("Recovered message"))
            .expect("message panel should have a titled top border");
        let content_row = lines
            .iter()
            .position(|line| line.contains("│ secret"))
            .expect("recovered message should be inside the bordered panel");
        let bottom_border_row = lines
            .iter()
            .position(|line| line.contains("└"))
            .expect("message panel should have a bottom border");

        assert!(title_row < content_row);
        assert!(content_row < bottom_border_row);
        assert!(lines.iter().any(|line| line.contains("Done")));
    }

    #[test]
    fn unlock_text_complete_borrows_recovered_plaintext() {
        let lines = recovered_message_lines("secret");

        assert!(matches!(
            &lines[0].spans[0].content,
            Cow::Borrowed("secret")
        ));
    }
}
