//! Shared modal renderers for file browsing and message dialogs.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::userinterfaces::tui::app_state::{
    App, BrowserFileFilter, BrowserMode, BrowserTarget, FileBrowserState, PasswordPromptFocus,
    PasswordPromptState,
};

use super::form::{
    button_span, helper_line, key_hints_line, label_width, line_with_secret_field, ActionKind,
};
use super::layout::centered_rect;
use super::navigation::list_highlight_symbol;
use super::theme::{accent_style, destructive_style, panel_block, plain_block, warning_style};

pub(crate) fn render_message_modal(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    message: &str,
    app: &App,
    is_error: bool,
) {
    let modal_area = centered_rect(area, 70, 40);
    frame.render_widget(Clear, modal_area);
    let style = if is_error {
        destructive_style(app)
    } else {
        accent_style(app)
    };
    let lines = vec![
        Line::from(Span::styled(message.to_string(), style)),
        Line::from(""),
        Line::from(button_span("Back", ActionKind::Secondary, true, app)),
    ];
    let paragraph = Paragraph::new(lines)
        .block(panel_block(title, app))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, modal_area);
}

pub(crate) fn render_browser_modal(
    frame: &mut Frame,
    area: Rect,
    browser: &FileBrowserState,
    app: &App,
) {
    let modal_area = centered_rect(area, 80, 70);
    frame.render_widget(Clear, modal_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(5),
        ])
        .split(modal_area);

    let header = Paragraph::new(format!("{}", browser.current_dir.display()))
        .block(panel_block("Browse", app))
        .wrap(Wrap { trim: true });
    frame.render_widget(header, chunks[0]);

    let items: Vec<ListItem> = if browser.entries.is_empty() {
        vec![ListItem::new(Line::from("(no entries)"))]
    } else {
        browser
            .entries
            .iter()
            .map(|entry| {
                let suffix = if entry.is_dir { "/" } else { "" };
                ListItem::new(Line::from(format!("{}{}", entry.name, suffix)))
            })
            .collect()
    };

    let list = List::new(items)
        .block(panel_block(
            match browser.mode {
                BrowserMode::File
                    if matches!(browser.file_filter, BrowserFileFilter::TimelockedOnly) =>
                {
                    "Select file (.timelocked)"
                }
                BrowserMode::File => "Select file",
                BrowserMode::Directory => "Select directory",
            },
            app,
        ))
        .highlight_symbol(list_highlight_symbol(app))
        .highlight_style(accent_style(app));

    let mut state = ListState::default();
    if !browser.entries.is_empty() {
        state.select(Some(browser.selected));
    }
    frame.render_stateful_widget(list, chunks[1], &mut state);

    let mut footer_lines = vec![key_hints_line(
        "Esc Close   ← Parent   → Open   Enter Select",
        app,
    )];
    let hidden_hint = if browser.show_hidden {
        "Hide hidden entries"
    } else {
        "Show hidden entries"
    };
    footer_lines.push(Line::from(vec![
        Span::styled("h", accent_style(app)),
        Span::raw(format!(" {hidden_hint}")),
    ]));
    if browser_filter_toggle_available(browser.mode, browser.target) {
        let filter_hint = match browser.file_filter {
            BrowserFileFilter::TimelockedOnly => "Show all files",
            BrowserFileFilter::AllFiles => "Show only .timelocked files",
        };
        footer_lines.push(Line::from(vec![
            Span::styled("f", accent_style(app)),
            Span::raw(format!(" {filter_hint}")),
        ]));
    }
    if matches!(browser.mode, BrowserMode::Directory) {
        footer_lines.push(key_hints_line("s Use current directory", app));
    }
    if let Some(error) = &browser.error {
        footer_lines.push(Line::from(Span::styled(error.clone(), warning_style(app))));
    }

    let footer = Paragraph::new(footer_lines)
        .block(plain_block(app))
        .wrap(Wrap { trim: true });
    frame.render_widget(footer, chunks[2]);
}

pub(crate) fn render_password_prompt_modal(
    frame: &mut Frame,
    area: Rect,
    state: &PasswordPromptState,
    app: &App,
) {
    let modal_area = centered_rect(area, 70, 34);
    frame.render_widget(Clear, modal_area);

    let labels = ["Password"];
    let label_width = label_width(&labels);
    let remaining_attempts = state
        .max_attempts
        .saturating_sub(state.attempt)
        .saturating_add(1);
    let intro = if state.previous_attempt_failed {
        let attempt_noun = if remaining_attempts == 1 {
            "attempt"
        } else {
            "attempts"
        };
        format!("Password did not unlock this file. {remaining_attempts} {attempt_noun} remaining.")
    } else {
        "This file requires a password after the time-lock is solved.".to_string()
    };

    let lines = vec![
        Line::from(intro),
        helper_line(
            "Enter the file password to continue unlocking.",
            label_width,
            app,
        ),
        Line::from(""),
        line_with_secret_field(
            "Password",
            label_width,
            state.password.secret_char_count(),
            matches!(state.focus, PasswordPromptFocus::Password),
            app,
        ),
        Line::from(""),
        Line::from(vec![
            button_span(
                "Unlock",
                ActionKind::Primary,
                matches!(state.focus, PasswordPromptFocus::Unlock),
                app,
            ),
            Span::raw("  "),
            button_span(
                "Cancel",
                ActionKind::Secondary,
                matches!(state.focus, PasswordPromptFocus::Cancel),
                app,
            ),
        ]),
        Line::from(""),
        key_hints_line("Tab Focus   Enter Select   Esc Cancel", app),
    ];

    let paragraph = Paragraph::new(lines)
        .block(panel_block("Password Required", app))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, modal_area);
}

fn browser_filter_toggle_available(mode: BrowserMode, target: BrowserTarget) -> bool {
    matches!(mode, BrowserMode::File)
        && matches!(
            target,
            BrowserTarget::UnlockInput | BrowserTarget::InspectInput
        )
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::Terminal;

    use super::*;
    use crate::userinterfaces::tui::state::SecretTextField;

    fn buffer_text(buffer: &Buffer) -> String {
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn prompt_state(
        password: &str,
        attempt: u8,
        previous_attempt_failed: bool,
    ) -> PasswordPromptState {
        let (reply, _receiver) = mpsc::channel();
        PasswordPromptState {
            password: SecretTextField::new(password.to_string()),
            focus: PasswordPromptFocus::Password,
            attempt,
            max_attempts: 3,
            previous_attempt_failed,
            reply,
        }
    }

    #[test]
    fn password_modal_masks_input() {
        let app = App::new(false);
        let state = prompt_state("raw-secret", 1, false);
        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).expect("test backend should initialize");

        terminal
            .draw(|frame| render_password_prompt_modal(frame, frame.area(), &state, &app))
            .expect("draw password modal");

        let rendered = buffer_text(terminal.backend().buffer());
        assert!(!rendered.contains("raw-secret"));
        assert!(rendered.contains("**********"));
    }

    #[test]
    fn password_modal_retry_shows_remaining_attempts() {
        let app = App::new(false);
        let state = prompt_state("", 2, true);
        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).expect("test backend should initialize");

        terminal
            .draw(|frame| render_password_prompt_modal(frame, frame.area(), &state, &app))
            .expect("draw password modal");

        let rendered = buffer_text(terminal.backend().buffer());
        assert!(rendered.contains("Password did not unlock this file. 2 attempts remaining."));
    }

    #[test]
    fn password_modal_retry_pluralizes_single_remaining_attempt() {
        let app = App::new(false);
        let state = prompt_state("", 3, true);
        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).expect("test backend should initialize");

        terminal
            .draw(|frame| render_password_prompt_modal(frame, frame.area(), &state, &app))
            .expect("draw password modal");

        let rendered = buffer_text(terminal.backend().buffer());
        assert!(rendered.contains("Password did not unlock this file. 1 attempt remaining."));
    }
}
