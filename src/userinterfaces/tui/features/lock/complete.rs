//! Renders the post-lock success screen and follow-up actions.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Gauge;
use ratatui::Frame;

use crate::base::progress_status::ProgressStatus;
use crate::usecases::{inspect, verify};
use crate::userinterfaces::common::output::{format_binary_size, format_eta};
use crate::userinterfaces::tui::app_state::{
    App, InspectDetailsFocus, InspectDetailsState, Modal, Screen,
};
use crate::userinterfaces::tui::components::form::{
    button_span, label_width, read_only_row, ActionKind, ReadOnlyValueKind,
};
use crate::userinterfaces::tui::components::layout::render_block_paragraph;
use crate::userinterfaces::tui::components::progress::{
    format_lock_rate, lock_complete_progress_lines, phase_label,
};
use crate::userinterfaces::tui::components::theme::{accent_style, titled_plain_block};
use crate::userinterfaces::tui::features::main_menu::screen::MainMenuState;
use crate::userinterfaces::tui::features::shared::form_navigation::{
    cycled_focus, FocusNavigationAxis,
};

#[derive(Debug, Clone)]
pub struct LockCompleteState {
    pub output_path: PathBuf,
    pub payload_bytes: u64,
    pub input_display: String,
    pub output_display: String,
    pub progress: ProgressStatus,
    pub focus: LockCompleteFocus,
}

#[derive(Debug, Clone, Copy)]
pub enum LockCompleteFocus {
    Inspect,
    Verify,
    Done,
}

impl LockCompleteFocus {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Inspect => Self::Verify,
            Self::Verify => Self::Done,
            Self::Done => Self::Inspect,
        }
    }

    pub(crate) fn prev(self) -> Self {
        match self {
            Self::Inspect => Self::Done,
            Self::Verify => Self::Inspect,
            Self::Done => Self::Verify,
        }
    }
}

pub fn help(focus: LockCompleteFocus) -> &'static str {
    match focus {
        LockCompleteFocus::Inspect => "Inspect metadata and parameters.",
        LockCompleteFocus::Verify => "Run structural verification.",
        LockCompleteFocus::Done => "Return to main menu.",
    }
}

pub fn completed_lock_progress(progress: &ProgressStatus) -> ProgressStatus {
    let phase = if progress.phase == "starting" {
        "lock-encrypt".to_string()
    } else {
        progress.phase.clone()
    };
    let total = progress.total.max(progress.current).max(1);

    ProgressStatus::new(phase, total, total, Some(0), progress.rate_per_second)
}

pub fn render(state: &LockCompleteState, frame: &mut Frame, area: Rect, app: &App) {
    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(3),
            Constraint::Min(8),
        ])
        .split(area);

    let phase = phase_label(&state.progress.phase);
    let eta = state
        .progress
        .eta_seconds
        .map(format_eta)
        .unwrap_or_else(|| "~0s".to_string());
    let rate = state
        .progress
        .rate_per_second
        .map(format_lock_rate)
        .unwrap_or_else(|| "-".to_string());
    let progress_lines = lock_complete_progress_lines(
        &state.input_display,
        &state.output_display,
        phase,
        &rate,
        &eta,
        app,
    );
    render_block_paragraph(
        frame,
        body[0],
        "Creating Timelocked File",
        progress_lines,
        app,
    );

    let ratio = (state.progress.pct / 100.0).clamp(0.0, 1.0);
    let gauge = Gauge::default()
        .block(titled_plain_block("Progress", app))
        .gauge_style(accent_style(app))
        .ratio(ratio)
        .label(format!("{:.1}%", state.progress.pct));
    frame.render_widget(gauge, body[1]);

    let lines = complete_lines(state, app);
    render_block_paragraph(frame, body[2], "Timelocked File Created", lines, app);
}

fn complete_lines(state: &LockCompleteState, app: &App) -> Vec<Line<'static>> {
    let label_width = label_width(&["Output", "Size"]);

    vec![
        read_only_row(
            "",
            "Output",
            label_width,
            &state.output_path.display().to_string(),
            ReadOnlyValueKind::Default,
            app,
        ),
        read_only_row(
            "",
            "Size",
            label_width,
            &format_binary_size(state.payload_bytes),
            ReadOnlyValueKind::Detail,
            app,
        ),
        Line::from(""),
        Line::from("Keep the timelocked file and Timelocked binaries together for future unlock."),
        Line::from(""),
        Line::from(vec![
            button_span(
                "Inspect",
                ActionKind::Secondary,
                matches!(state.focus, LockCompleteFocus::Inspect),
                app,
            ),
            Span::raw("  "),
            button_span(
                "Structural verify",
                ActionKind::Secondary,
                matches!(state.focus, LockCompleteFocus::Verify),
                app,
            ),
            Span::raw("  "),
            button_span(
                "Done",
                ActionKind::Primary,
                matches!(state.focus, LockCompleteFocus::Done),
                app,
            ),
        ]),
    ]
}

pub fn handle_key(state: &mut LockCompleteState, key: KeyEvent, app: &mut App) -> Screen {
    if key.code == KeyCode::Esc {
        return Screen::MainMenu(MainMenuState::default());
    }

    if let Some(focus) = cycled_focus(
        key,
        state.focus,
        LockCompleteFocus::next,
        LockCompleteFocus::prev,
        FocusNavigationAxis::Horizontal,
    ) {
        state.focus = focus;
        return Screen::LockComplete(state.clone());
    }

    match key.code {
        KeyCode::Enter => match state.focus {
            LockCompleteFocus::Inspect => {
                match inspect::execute(inspect::InspectRequest {
                    input: state.output_path.clone(),
                    current_machine_iterations_per_second: app
                        .estimate_calibration_for_path(state.output_path.as_path()),
                }) {
                    Ok(response) => Screen::InspectDetails(InspectDetailsState {
                        response,
                        focus: InspectDetailsFocus::Unlock,
                    }),
                    Err(err) => {
                        app.modal = Some(Modal::Error(err.to_string()));
                        Screen::LockComplete(state.clone())
                    }
                }
            }
            LockCompleteFocus::Verify => {
                match verify::execute(
                    verify::VerifyRequest {
                        input: state.output_path.clone(),
                    },
                    None,
                ) {
                    Ok(_) => {
                        app.modal = Some(Modal::Info("Structural verification OK".to_string()));
                    }
                    Err(err) => {
                        app.modal = Some(Modal::Error(err.to_string()));
                    }
                }
                Screen::LockComplete(state.clone())
            }
            LockCompleteFocus::Done => Screen::MainMenu(MainMenuState::default()),
        },
        _ => Screen::LockComplete(state.clone()),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ratatui::style::Color;

    use crate::base::progress_status::ProgressStatus;
    use crate::userinterfaces::tui::app_state::App;

    use super::{complete_lines, completed_lock_progress, LockCompleteFocus, LockCompleteState};

    fn test_app(no_color: bool) -> App {
        App::new(no_color)
    }

    #[test]
    fn completed_lock_progress_marks_starting_phase_as_finished() {
        let progress = ProgressStatus::new("starting", 0, 1, None, None);

        let completed = completed_lock_progress(&progress);
        assert_eq!(completed.phase, "lock-encrypt");
        assert_eq!(completed.current, completed.total);
        assert_eq!(completed.pct, 100.0);
        assert_eq!(completed.eta_seconds, Some(0));
    }

    #[test]
    fn complete_screen_highlights_structured_values() {
        let state = LockCompleteState {
            output_path: PathBuf::from("file.timelocked"),
            payload_bytes: 1234,
            input_display: "input.txt".to_string(),
            output_display: "file.timelocked".to_string(),
            progress: ProgressStatus::new("lock-encrypt", 10, 10, Some(0), Some(1.0)),
            focus: LockCompleteFocus::Done,
        };

        let lines = complete_lines(&state, &test_app(false));

        assert_eq!(lines[1].spans[3].style.fg, Some(Color::Cyan));
    }
}
