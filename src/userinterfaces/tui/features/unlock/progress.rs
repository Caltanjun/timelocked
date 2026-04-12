//! Renders unlock progress and lets users cancel the active worker.

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
    format_unlock_progress_percent, format_unlock_rate, phase_label, unlock_progress_lines,
    unlock_progress_lines_compact,
};
use crate::userinterfaces::tui::components::theme::{
    accent_style, plain_block, titled_plain_block,
};
use crate::userinterfaces::tui::features::shared::form_navigation::{
    cycled_focus, FocusNavigationAxis,
};
use crate::userinterfaces::tui::worker::UnlockWorker;

const NARROW_PROGRESS_WIDTH: u16 = 72;

pub struct UnlockProgressState {
    pub file_display: String,
    pub progress: ProgressStatus,
    pub worker: UnlockWorker,
    pub cancel_requested: bool,
    pub cpu_count: usize,
    pub focus: UnlockProgressFocus,
}

#[derive(Debug, Clone, Copy)]
pub enum UnlockProgressFocus {
    Progress,
    Cancel,
}

impl UnlockProgressFocus {
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
    "Unlock uses one CPU core by design."
}

fn request_unlock_cancel(state: &mut UnlockProgressState) {
    if state.cancel_requested {
        return;
    }
    state.cancel_requested = true;
    state.worker.cancellation.cancel();
}

pub fn render(state: &UnlockProgressState, frame: &mut Frame, area: Rect, app: &App) {
    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);
    let phase = phase_label(&state.progress.phase);
    let eta = state
        .progress
        .eta_seconds
        .map(format_eta)
        .unwrap_or_else(|| "~?".to_string());
    let rate = state
        .progress
        .rate_per_second
        .map(|value| format_unlock_rate(&state.progress.phase, value))
        .unwrap_or_else(|| "-".to_string());
    let lines = if area.width < NARROW_PROGRESS_WIDTH {
        unlock_progress_lines_compact(
            &state.file_display,
            phase,
            &rate,
            &eta,
            state.cancel_requested,
            app,
        )
    } else {
        unlock_progress_lines(
            &state.file_display,
            phase,
            &rate,
            &eta,
            state.cpu_count,
            state.cancel_requested,
            app,
        )
    };
    render_block_paragraph(frame, body[0], "Unlock Running", lines, app);

    let ratio = (state.progress.pct / 100.0).clamp(0.0, 1.0);
    let gauge = Gauge::default()
        .block(titled_plain_block("Progress", app))
        .gauge_style(accent_style(app))
        .ratio(ratio)
        .label(format_unlock_progress_percent(
            state.progress.pct,
            state.progress.eta_seconds,
        ));
    frame.render_widget(gauge, body[1]);

    let button = Paragraph::new(vec![Line::from(vec![button_span(
        "Cancel",
        ActionKind::Secondary,
        state.cancel_requested || matches!(state.focus, UnlockProgressFocus::Cancel),
        app,
    )])])
    .block(plain_block(app));
    frame.render_widget(button, body[2]);
}

pub fn handle_key(mut state: UnlockProgressState, key: KeyEvent, _app: &mut App) -> Screen {
    if let Some(focus) = cycled_focus(
        key,
        state.focus,
        UnlockProgressFocus::next,
        UnlockProgressFocus::prev,
        FocusNavigationAxis::Horizontal,
    ) {
        state.focus = focus;
        return Screen::UnlockProgress(state);
    }

    match key.code {
        KeyCode::Esc => request_unlock_cancel(&mut state),
        KeyCode::Enter => {
            if matches!(state.focus, UnlockProgressFocus::Cancel) {
                request_unlock_cancel(&mut state);
            }
        }
        _ => {}
    }
    Screen::UnlockProgress(state)
}

#[cfg(test)]
mod tests {
    use super::UnlockProgressFocus;

    #[test]
    fn unlock_progress_focus_cycles_between_progress_and_cancel() {
        let focus = UnlockProgressFocus::Progress;
        assert!(matches!(focus.next(), UnlockProgressFocus::Cancel));
        assert!(matches!(focus.prev(), UnlockProgressFocus::Cancel));
    }
}
