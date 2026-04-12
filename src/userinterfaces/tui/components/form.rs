//! Reusable helpers for readable, aligned TUI forms and actions.

use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use crate::userinterfaces::tui::app_state::App;

use super::navigation::{focus_prefix_span, focus_prefix_text, unfocused_prefix_span};
use super::theme::{
    accent_style, detail_value_style, focused_control_style, label_style, muted_italic_style,
    muted_style, primary_action_style, secondary_action_style, success_value_style, value_style,
    warning_value_style,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldChrome {
    Input,
    Selector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionKind {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReadOnlyValueKind {
    Default,
    Detail,
    Success,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InlineButton<'a> {
    pub label: &'a str,
    pub kind: ActionKind,
    pub focused: bool,
}

pub(crate) fn key_hints_line(hints: &str, app: &App) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, group) in hints
        .split("   ")
        .filter(|segment| !segment.trim().is_empty())
        .enumerate()
    {
        if index > 0 {
            spans.push(Span::raw("   "));
        }

        let mut parts = group.splitn(2, ' ');
        let key = parts.next().unwrap_or("").trim();
        let action = parts.next().unwrap_or("").trim();

        if !key.is_empty() {
            spans.push(Span::styled(key.to_string(), accent_style(app)));
        }
        if !action.is_empty() {
            spans.push(Span::raw(format!(" {action}")));
        }
    }

    if spans.is_empty() {
        Line::from(hints.to_string())
    } else {
        Line::from(spans)
    }
}

pub(crate) fn label_width(labels: &[&str]) -> usize {
    labels
        .iter()
        .map(|label| label.chars().count())
        .max()
        .unwrap_or(0)
}

pub(crate) fn helper_line(text: &str, label_width: usize, app: &App) -> Line<'static> {
    helper_line_with_style(text, label_width, muted_style(app), app)
}

pub(crate) fn italic_helper_line(text: &str, label_width: usize, app: &App) -> Line<'static> {
    helper_line_with_style(text, label_width, muted_italic_style(app), app)
}

fn helper_line_with_style(
    text: &str,
    label_width: usize,
    style: ratatui::style::Style,
    app: &App,
) -> Line<'static> {
    let mut spans = vec![unfocused_prefix_span(app)];
    if label_width > 0 {
        spans.push(Span::raw(" ".repeat(label_width)));
        spans.push(Span::raw("   "));
    }
    spans.push(Span::styled(text.to_string(), style));
    Line::from(spans)
}

pub(crate) fn menu_item_with_right_label(
    label: &str,
    description: &str,
    selected: bool,
    description_column: usize,
    app: &App,
) -> Line<'static> {
    let left = format!("{}{label}", focus_prefix_text(selected, app));
    let gap = description_column
        .saturating_sub(left.chars().count())
        .max(3);

    Line::from(vec![
        Span::styled(
            left,
            if selected {
                focused_control_style(app)
            } else {
                label_style(app)
            },
        ),
        Span::raw(" ".repeat(gap)),
        Span::styled(description.to_string(), muted_style(app)),
    ])
}

pub(crate) fn line_with_field(
    label: &str,
    label_width: usize,
    value: &str,
    chrome: FieldChrome,
    focused: bool,
    app: &App,
) -> Line<'static> {
    let mut spans = row_prefix_spans(label, label_width, focused, app);
    spans.push(field_span(value, chrome, focused, app));
    Line::from(spans)
}

pub(crate) fn line_with_field_and_button(
    label: &str,
    label_width: usize,
    value: &str,
    chrome: FieldChrome,
    field_focused: bool,
    button: InlineButton<'_>,
    app: &App,
) -> Line<'static> {
    let mut spans = row_prefix_spans(label, label_width, field_focused || button.focused, app);
    spans.push(field_span(value, chrome, field_focused, app));
    spans.push(Span::raw("  "));
    spans.push(button_span(button.label, button.kind, button.focused, app));
    Line::from(spans)
}

pub(crate) fn focused_line(focused: bool, text: &str, app: &App) -> Line<'static> {
    let style = if focused {
        focused_control_style(app)
    } else {
        label_style(app)
    };
    let text = format!("{}{text}", focus_prefix_text(focused, app));
    Line::from(Span::styled(text, style))
}

pub(crate) fn button_span(
    label: &str,
    kind: ActionKind,
    focused: bool,
    app: &App,
) -> Span<'static> {
    let mut style = match kind {
        ActionKind::Primary => primary_action_style(app),
        ActionKind::Secondary => secondary_action_style(app),
    };

    if focused {
        style = style
            .patch(focused_control_style(app))
            .add_modifier(Modifier::REVERSED);
    }

    Span::styled(format!("[{label}]"), style)
}

pub(crate) fn read_only_row(
    indent: &str,
    label: &str,
    label_width: usize,
    value: &str,
    kind: ReadOnlyValueKind,
    app: &App,
) -> Line<'static> {
    let label = format!("{label:<label_width$}", label_width = label_width);

    Line::from(vec![
        Span::raw(indent.to_string()),
        Span::styled(format!("{label}:"), label_style(app)),
        Span::raw(" "),
        Span::styled(value.to_string(), read_only_value_style(kind, app)),
    ])
}

fn read_only_value_style(kind: ReadOnlyValueKind, app: &App) -> ratatui::style::Style {
    match kind {
        ReadOnlyValueKind::Default => value_style(app),
        ReadOnlyValueKind::Detail => detail_value_style(app),
        ReadOnlyValueKind::Success => success_value_style(app),
        ReadOnlyValueKind::Warning => warning_value_style(app),
    }
}

fn row_prefix_spans(
    label: &str,
    label_width: usize,
    focused: bool,
    app: &App,
) -> Vec<Span<'static>> {
    let label = format!("{label:<label_width$}", label_width = label_width);
    let label_style = if focused {
        focused_control_style(app)
    } else {
        label_style(app)
    };

    vec![
        focus_prefix_span(focused, app),
        Span::styled(label, label_style),
        Span::raw("   "),
    ]
}

fn field_span(value: &str, chrome: FieldChrome, focused: bool, app: &App) -> Span<'static> {
    let (open, close) = match chrome {
        FieldChrome::Input => ("[", "]"),
        FieldChrome::Selector => ("<", ">"),
    };
    let style = if focused {
        focused_control_style(app)
    } else {
        value_style(app)
    };
    Span::styled(format!("{open} {} {close}", display_value(value)), style)
}

fn display_value(value: &str) -> String {
    if value.trim().is_empty() {
        " ".to_string()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use ratatui::text::Span;

    use ratatui::style::{Color, Modifier};

    use super::{
        display_value, focused_line, helper_line, italic_helper_line, label_width, line_with_field,
        menu_item_with_right_label, read_only_row, FieldChrome, ReadOnlyValueKind,
    };
    use crate::userinterfaces::tui::app_state::App;

    fn test_app(no_color: bool) -> App {
        App::new(no_color)
    }

    #[test]
    fn empty_values_render_as_visible_blank_slot() {
        assert_eq!(display_value(""), " ");
        assert_eq!(display_value("   "), " ");
    }

    #[test]
    fn label_width_uses_longest_visible_label() {
        assert_eq!(label_width(&["Input", "Output path", "ETA"]), 11);
    }

    #[test]
    fn focused_main_menu_lines_stay_marker_free_in_color_mode() {
        let line = focused_line(true, "Lock a file", &test_app(false));

        assert_eq!(
            line.spans,
            vec![Span::styled("Lock a file", line.spans[0].style)]
        );
    }

    #[test]
    fn focused_main_menu_lines_restore_textual_marker_in_no_color_mode() {
        let line = focused_line(true, "Lock a file", &test_app(true));

        assert_eq!(
            line.spans,
            vec![Span::styled("> Lock a file", line.spans[0].style)]
        );
    }

    #[test]
    fn two_column_main_menu_items_stay_flush_in_color_mode() {
        let line =
            menu_item_with_right_label("Lock text", "Description", true, 20, &test_app(false));

        assert_eq!(line.spans[0].content.as_ref(), "Lock text");
    }

    #[test]
    fn two_column_main_menu_items_restore_marker_in_no_color_mode() {
        let line =
            menu_item_with_right_label("Lock text", "Description", true, 20, &test_app(true));

        assert_eq!(line.spans[0].content.as_ref(), "> Lock text");
    }

    #[test]
    fn focused_form_rows_drop_marker_but_keep_alignment_in_color_mode() {
        let line = line_with_field(
            "Input",
            8,
            "file.txt",
            FieldChrome::Input,
            true,
            &test_app(false),
        );

        assert_eq!(line.spans[0].content.as_ref(), "");
    }

    #[test]
    fn focused_form_rows_keep_textual_marker_in_no_color_mode() {
        let line = line_with_field(
            "Input",
            8,
            "file.txt",
            FieldChrome::Input,
            true,
            &test_app(true),
        );

        assert_eq!(line.spans[0].content.as_ref(), "> ");
    }

    #[test]
    fn helper_lines_stay_flush_in_color_mode() {
        let line = helper_line("Examples", 0, &test_app(false));

        assert_eq!(line.spans[0].content.as_ref(), "");
        assert_eq!(line.spans[1].content.as_ref(), "Examples");
    }

    #[test]
    fn helper_lines_keep_no_color_alignment_prefix() {
        let line = helper_line("Examples", 0, &test_app(true));

        assert_eq!(line.spans[0].content.as_ref(), "  ");
        assert_eq!(line.spans[1].content.as_ref(), "Examples");
    }

    #[test]
    fn helper_lines_keep_value_alignment_when_label_width_is_present() {
        let line = helper_line("Examples", 8, &test_app(false));

        assert_eq!(line.spans[0].content.as_ref(), "");
        assert_eq!(line.spans[1].content.as_ref(), "        ");
        assert_eq!(line.spans[2].content.as_ref(), "   ");
        assert_eq!(line.spans[3].content.as_ref(), "Examples");
    }

    #[test]
    fn italic_helper_lines_add_italic_without_changing_alignment() {
        let line = italic_helper_line("Examples", 8, &test_app(false));

        assert_eq!(line.spans[0].content.as_ref(), "");
        assert_eq!(line.spans[1].content.as_ref(), "        ");
        assert_eq!(line.spans[2].content.as_ref(), "   ");
        assert_eq!(line.spans[3].content.as_ref(), "Examples");
        assert!(line.spans[3].style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn read_only_rows_align_labels_and_style_values() {
        let line = read_only_row(
            "  ",
            "Chosen delay",
            22,
            "3d",
            ReadOnlyValueKind::Detail,
            &test_app(false),
        );

        assert_eq!(line.spans[0].content.as_ref(), "  ");
        assert_eq!(line.spans[1].content.as_ref(), "Chosen delay          :");
        assert_eq!(line.spans[3].content.as_ref(), "3d");
        assert_eq!(line.spans[3].style.fg, Some(Color::Cyan));
        assert!(line.spans[3].style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn read_only_rows_keep_plain_semantics_in_no_color_mode() {
        let line = read_only_row(
            "",
            "Integrity",
            10,
            "OK",
            ReadOnlyValueKind::Success,
            &test_app(true),
        );

        assert_eq!(line.spans[3].style.fg, None);
        assert!(line.spans[3].style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn editable_fields_keep_previous_non_bold_value_style() {
        let line = line_with_field(
            "Input",
            8,
            "file.txt",
            FieldChrome::Input,
            false,
            &test_app(true),
        );

        assert!(!line.spans[3].style.add_modifier.contains(Modifier::BOLD));
    }
}
