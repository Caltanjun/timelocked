//! Shared modal renderers for file browsing and message dialogs.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::userinterfaces::tui::app_state::{
    App, BrowserFileFilter, BrowserMode, BrowserTarget, FileBrowserState,
};

use super::form::{button_span, key_hints_line, ActionKind};
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

fn browser_filter_toggle_available(mode: BrowserMode, target: BrowserTarget) -> bool {
    matches!(mode, BrowserMode::File)
        && matches!(
            target,
            BrowserTarget::UnlockInput | BrowserTarget::InspectInput
        )
}
