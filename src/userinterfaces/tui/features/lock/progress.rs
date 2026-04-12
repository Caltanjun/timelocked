//! Renders lock progress and lets users cancel the active worker.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Gauge, Paragraph};
use ratatui::Frame;

use crate::base::progress_status::ProgressStatus;
use crate::userinterfaces::common::output::format_eta;
use crate::userinterfaces::tui::app_state::{App, Screen};
use crate::userinterfaces::tui::components::form::{button_span, ActionKind};
use crate::userinterfaces::tui::components::layout::render_block_paragraph;
use crate::userinterfaces::tui::components::progress::{
    format_lock_rate, lock_progress_lines, lock_progress_lines_compact,
};
use crate::userinterfaces::tui::components::theme::{
    accent_style, plain_block, titled_plain_block,
};
use crate::userinterfaces::tui::features::shared::form_navigation::{
    cycled_focus, FocusNavigationAxis,
};
use crate::userinterfaces::tui::worker::LockWorker;

const NARROW_PROGRESS_WIDTH: u16 = 72;

pub struct LockProgressState {
    pub input_display: String,
    pub output_display: String,
    pub progress: ProgressStatus,
    pub worker: LockWorker,
    pub cancel_requested: bool,
    pub focus: LockProgressFocus,
}

#[derive(Debug, Clone, Copy)]
pub enum LockProgressFocus {
    Progress,
    Cancel,
}

impl LockProgressFocus {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Progress => Self::Cancel,
            Self::Cancel => Self::Progress,
        }
    }

    pub(crate) fn prev(self) -> Self {
        self.next()
    }
}

pub fn help() -> &'static str {
    "Cancelling may take a moment between chunk boundaries."
}

fn request_lock_cancel(state: &mut LockProgressState) {
    if state.cancel_requested {
        return;
    }
    state.cancel_requested = true;
    state.worker.cancellation.cancel();
}

pub fn render(state: &LockProgressState, frame: &mut Frame, area: Rect, app: &App) {
    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);
    let eta = state
        .progress
        .eta_seconds
        .map(format_eta)
        .unwrap_or_else(|| "~?".to_string());
    let rate = state
        .progress
        .rate_per_second
        .map(format_lock_rate)
        .unwrap_or_else(|| "-".to_string());
    let lines = if area.width < NARROW_PROGRESS_WIDTH {
        lock_progress_lines_compact(
            &state.input_display,
            &state.output_display,
            &state.progress.phase,
            &rate,
            &eta,
            state.cancel_requested,
            app,
        )
    } else {
        lock_progress_lines(
            &state.input_display,
            &state.output_display,
            &state.progress.phase,
            &rate,
            &eta,
            state.cancel_requested,
            app,
        )
    };
    render_block_paragraph(frame, body[0], "Creating Timelocked File", lines, app);

    let ratio = (state.progress.pct / 100.0).clamp(0.0, 1.0);
    let gauge = Gauge::default()
        .block(titled_plain_block("Progress", app))
        .gauge_style(accent_style(app))
        .ratio(ratio)
        .label(format!("{:.1}%", state.progress.pct));
    frame.render_widget(gauge, body[1]);

    let button = Paragraph::new(vec![Line::from(vec![button_span(
        "Cancel",
        ActionKind::Secondary,
        state.cancel_requested || matches!(state.focus, LockProgressFocus::Cancel),
        app,
    )])])
    .block(plain_block(app));
    frame.render_widget(button, body[2]);
}

pub fn handle_key(mut state: LockProgressState, key: KeyEvent, _app: &mut App) -> Screen {
    if let Some(focus) = cycled_focus(
        key,
        state.focus,
        LockProgressFocus::next,
        LockProgressFocus::prev,
        FocusNavigationAxis::Horizontal,
    ) {
        state.focus = focus;
        return Screen::LockProgress(state);
    }

    match key.code {
        KeyCode::Esc => request_lock_cancel(&mut state),
        KeyCode::Enter => {
            if matches!(state.focus, LockProgressFocus::Cancel) {
                request_lock_cancel(&mut state);
            }
        }
        _ => {}
    }
    Screen::LockProgress(state)
}

#[cfg(test)]
mod tests {
    use super::LockProgressFocus;

    #[test]
    fn lock_progress_focus_cycles_between_progress_and_cancel() {
        let focus = LockProgressFocus::Progress;
        assert!(matches!(focus.next(), LockProgressFocus::Cancel));
        assert!(matches!(focus.prev(), LockProgressFocus::Cancel));
    }
}
