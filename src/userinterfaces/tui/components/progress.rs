//! Reusable progress copy and semantic line styling for lock and unlock screens.

use ratatui::text::Line;

use crate::userinterfaces::tui::app_state::App;

use super::form::{read_only_row, ReadOnlyValueKind};

pub(crate) fn phase_label(phase: &str) -> &'static str {
    match phase {
        "lock-primes" => "Generating primes",
        "lock-puzzle" => "Preparing timelock puzzle",
        "lock-encrypt" => "Encrypting payload",
        "unlock-timelock" => "Running timelock puzzle",
        "unlock-decrypt" => "Decrypting payload",
        "starting" => "Starting",
        _ => "Working",
    }
}

fn spinner_frame() -> char {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Simple, dependency-free spinner. Using time keeps it moving without
    // adding a tick counter to app state.
    const FRAMES: [char; 4] = ['|', '/', '-', '\\'];
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let idx = ((ms / 150) % (FRAMES.len() as u128)) as usize;
    FRAMES[idx]
}

pub(crate) fn format_lock_rate(rate: f64) -> String {
    format!("{:.2} MB/s", rate / 1_000_000.0)
}

pub(crate) fn format_unlock_rate(phase: &str, rate: f64) -> String {
    if phase == "unlock-timelock" {
        format!("{:.2}M it/s", rate / 1_000_000.0)
    } else {
        format!("{:.2} MB/s", rate / 1_000_000.0)
    }
}

pub(crate) fn format_unlock_progress_percent(pct: f64, eta_seconds: Option<u64>) -> String {
    let precision = if eta_seconds.is_some_and(|seconds| seconds > 86_400) {
        3
    } else {
        2
    };
    format!("{pct:.precision$}%")
}

pub(crate) fn lock_progress_lines(
    input_display: &str,
    output_display: &str,
    phase: &str,
    rate: &str,
    eta: &str,
    cancel_requested: bool,
    app: &App,
) -> Vec<Line<'static>> {
    let phase_label = phase_label(phase);
    let phase_suffix = if phase == "lock-primes" {
        format!(" {}", spinner_frame())
    } else {
        String::new()
    };

    let mut lines = vec![
        progress_value_row(
            "Phase",
            12,
            &format!("{phase_label}{phase_suffix}"),
            ReadOnlyValueKind::Detail,
            app,
        ),
        progress_value_row("ETA", 12, eta, ReadOnlyValueKind::Warning, app),
        progress_value_row("Throughput", 12, rate, ReadOnlyValueKind::Detail, app),
        progress_value_row(
            "Reading input",
            12,
            input_display,
            ReadOnlyValueKind::Default,
            app,
        ),
        progress_value_row(
            "Writing output",
            12,
            output_display,
            ReadOnlyValueKind::Default,
            app,
        ),
    ];

    lines.push(if cancel_requested {
        Line::from("Cancelling... waiting for safe checkpoint")
    } else {
        Line::from("Press Esc to cancel")
    });

    lines
}

pub(crate) fn lock_progress_lines_compact(
    input_display: &str,
    output_display: &str,
    phase: &str,
    rate: &str,
    eta: &str,
    cancel_requested: bool,
    app: &App,
) -> Vec<Line<'static>> {
    let phase_label = phase_label(phase);
    let phase_suffix = if phase == "lock-primes" {
        format!(" {}", spinner_frame())
    } else {
        String::new()
    };

    let mut lines = vec![
        progress_value_row("ETA", 10, eta, ReadOnlyValueKind::Warning, app),
        progress_value_row(
            "Phase",
            10,
            &format!("{phase_label}{phase_suffix}"),
            ReadOnlyValueKind::Detail,
            app,
        ),
        progress_value_row("Throughput", 10, rate, ReadOnlyValueKind::Detail, app),
        progress_value_row(
            "In",
            10,
            &tail_ellipsis(input_display, 28),
            ReadOnlyValueKind::Default,
            app,
        ),
        progress_value_row(
            "Out",
            10,
            &tail_ellipsis(output_display, 28),
            ReadOnlyValueKind::Default,
            app,
        ),
    ];

    lines.push(if cancel_requested {
        Line::from("Cancelling... waiting for safe checkpoint")
    } else {
        Line::from("Press Esc to cancel")
    });

    lines
}

pub(crate) fn lock_complete_progress_lines(
    input_display: &str,
    output_display: &str,
    phase: &str,
    rate: &str,
    eta: &str,
    app: &App,
) -> Vec<Line<'static>> {
    vec![
        progress_value_row("Phase", 12, phase, ReadOnlyValueKind::Detail, app),
        progress_value_row("ETA", 12, eta, ReadOnlyValueKind::Warning, app),
        progress_value_row("Throughput", 12, rate, ReadOnlyValueKind::Detail, app),
        progress_value_row(
            "Reading input",
            12,
            input_display,
            ReadOnlyValueKind::Default,
            app,
        ),
        progress_value_row(
            "Writing output",
            12,
            output_display,
            ReadOnlyValueKind::Default,
            app,
        ),
    ]
}

pub(crate) fn unlock_progress_lines(
    file_display: &str,
    phase: &str,
    rate: &str,
    eta: &str,
    cpu_count: usize,
    cancel_requested: bool,
    app: &App,
) -> Vec<Line<'static>> {
    let mut lines = vec![
        progress_value_row(
            "Unlocking",
            9,
            file_display,
            ReadOnlyValueKind::Default,
            app,
        ),
        progress_value_row("Phase", 9, phase, ReadOnlyValueKind::Detail, app),
        progress_value_row("ETA", 9, eta, ReadOnlyValueKind::Warning, app),
        progress_value_row("Rate", 9, rate, ReadOnlyValueKind::Detail, app),
        progress_value_row(
            "CPU",
            9,
            &format!("using one core (of {cpu_count})"),
            ReadOnlyValueKind::Default,
            app,
        ),
    ];

    lines.push(if cancel_requested {
        Line::from("Cancelling... waiting for safe checkpoint")
    } else {
        Line::from("Press Esc to cancel")
    });

    lines
}

pub(crate) fn unlock_progress_lines_compact(
    file_display: &str,
    phase: &str,
    rate: &str,
    eta: &str,
    cancel_requested: bool,
    app: &App,
) -> Vec<Line<'static>> {
    let mut lines = vec![
        progress_value_row("ETA", 5, eta, ReadOnlyValueKind::Warning, app),
        progress_value_row("Phase", 5, phase, ReadOnlyValueKind::Detail, app),
        progress_value_row("Rate", 5, rate, ReadOnlyValueKind::Detail, app),
        progress_value_row(
            "File",
            5,
            &tail_ellipsis(file_display, 30),
            ReadOnlyValueKind::Default,
            app,
        ),
    ];

    lines.push(if cancel_requested {
        Line::from("Cancelling... waiting for safe checkpoint")
    } else {
        Line::from("Press Esc to cancel")
    });

    lines
}

fn progress_value_row(
    label: &str,
    label_width: usize,
    value: &str,
    kind: ReadOnlyValueKind,
    app: &App,
) -> Line<'static> {
    read_only_row("", label, label_width, value, kind, app)
}

fn tail_ellipsis(value: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let count = value.chars().count();
    if count <= max_chars {
        return value.to_string();
    }

    if max_chars <= 3 {
        return "...".chars().take(max_chars).collect();
    }

    let keep = max_chars - 3;
    let tail: String = value
        .chars()
        .rev()
        .take(keep)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("...{tail}")
}

#[cfg(test)]
mod tests {
    use ratatui::style::{Color, Modifier};
    use ratatui::text::Line;

    use super::{
        format_unlock_progress_percent, lock_progress_lines, lock_progress_lines_compact,
        unlock_progress_lines, unlock_progress_lines_compact,
    };
    use crate::userinterfaces::tui::app_state::App;

    fn test_app(no_color: bool) -> App {
        App::new(no_color)
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    #[test]
    fn lock_progress_prioritizes_eta_in_top_visible_rows() {
        let lines = lock_progress_lines(
            "input.bin",
            "output.timelocked",
            "lock-encrypt",
            "5.12 MB/s",
            "14m",
            false,
            &test_app(false),
        );

        assert_eq!(line_text(&lines[0]), "Phase       : Encrypting payload");
        assert_eq!(line_text(&lines[1]), "ETA         : 14m");
        assert_eq!(line_text(&lines[2]), "Throughput  : 5.12 MB/s");
    }

    #[test]
    fn unlock_progress_prioritizes_eta_in_top_visible_rows() {
        let lines = unlock_progress_lines(
            "test.timelocked",
            "Running timelock puzzle",
            "0.26M it/s",
            "2h 13m",
            8,
            false,
            &test_app(false),
        );

        assert_eq!(line_text(&lines[0]), "Unlocking: test.timelocked");
        assert_eq!(line_text(&lines[2]), "ETA      : 2h 13m");
        assert_eq!(line_text(&lines[3]), "Rate     : 0.26M it/s");
    }

    #[test]
    fn lock_progress_compact_prioritizes_eta_for_narrow_layouts() {
        let lines = lock_progress_lines_compact(
            "very-long-input-file.bin",
            "very-long-output-file.timelocked",
            "lock-encrypt",
            "5.12 MB/s",
            "14m",
            false,
            &test_app(false),
        );

        assert_eq!(line_text(&lines[0]), "ETA       : 14m");
        assert_eq!(line_text(&lines[1]), "Phase     : Encrypting payload");
    }

    #[test]
    fn unlock_progress_compact_prioritizes_eta_for_narrow_layouts() {
        let lines = unlock_progress_lines_compact(
            "very-long-test.timelocked",
            "Running timelock puzzle",
            "0.26M it/s",
            "2h 13m",
            false,
            &test_app(false),
        );

        assert_eq!(line_text(&lines[0]), "ETA  : 2h 13m");
        assert_eq!(line_text(&lines[2]), "Rate : 0.26M it/s");
    }

    #[test]
    fn unlock_progress_percent_uses_two_fraction_digits_by_default() {
        assert_eq!(
            format_unlock_progress_percent(12.34567, Some(86_400)),
            "12.35%"
        );
        assert_eq!(format_unlock_progress_percent(12.34567, None), "12.35%");
    }

    #[test]
    fn unlock_progress_percent_uses_three_fraction_digits_for_long_duration() {
        assert_eq!(
            format_unlock_progress_percent(12.34567, Some(86_401)),
            "12.346%"
        );
    }

    #[test]
    fn progress_rows_style_eta_and_phase_semantically() {
        let lines = lock_progress_lines(
            "input.bin",
            "output.timelocked",
            "lock-encrypt",
            "5.12 MB/s",
            "14m",
            false,
            &test_app(false),
        );

        assert_eq!(lines[0].spans[3].style.fg, Some(Color::Cyan));
        assert_eq!(lines[1].spans[3].style.fg, Some(Color::Yellow));
        assert!(lines[1].spans[3]
            .style
            .add_modifier
            .contains(Modifier::BOLD));
    }

    #[test]
    fn progress_rows_keep_bold_values_without_color() {
        let lines = unlock_progress_lines_compact(
            "test.timelocked",
            "Running timelock puzzle",
            "0.26M it/s",
            "2h 13m",
            false,
            &test_app(true),
        );

        assert_eq!(lines[0].spans[3].style.fg, None);
        assert!(lines[0].spans[3]
            .style
            .add_modifier
            .contains(Modifier::BOLD));
    }
}
