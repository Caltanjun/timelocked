//! Shared helpers for consistent focus markers and row spacing across TUI navigation widgets.

use ratatui::text::Span;

use crate::userinterfaces::tui::app_state::App;

use super::theme::{accent_style, base_style};

pub(crate) fn focus_prefix_text(focused: bool, app: &App) -> &'static str {
    if app.no_color {
        if focused {
            "> "
        } else {
            "  "
        }
    } else {
        ""
    }
}

pub(crate) fn unfocused_prefix_text(app: &App) -> &'static str {
    focus_prefix_text(false, app)
}

pub(crate) fn focus_prefix_span(focused: bool, app: &App) -> Span<'static> {
    Span::styled(
        focus_prefix_text(focused, app),
        if focused {
            accent_style(app)
        } else {
            base_style(app)
        },
    )
}

pub(crate) fn unfocused_prefix_span(app: &App) -> Span<'static> {
    Span::styled(unfocused_prefix_text(app), base_style(app))
}

pub(crate) fn list_highlight_symbol(app: &App) -> &'static str {
    focus_prefix_text(true, app)
}

#[cfg(test)]
mod tests {
    use super::{focus_prefix_text, list_highlight_symbol, unfocused_prefix_text};
    use crate::userinterfaces::tui::app_state::App;

    fn test_app(no_color: bool) -> App {
        App::new(no_color)
    }

    #[test]
    fn focus_prefix_is_hidden_in_color_mode() {
        assert_eq!(focus_prefix_text(true, &test_app(false)), "");
        assert_eq!(focus_prefix_text(false, &test_app(false)), "");
    }

    #[test]
    fn focus_prefix_stays_visible_in_no_color_mode() {
        assert_eq!(focus_prefix_text(true, &test_app(true)), "> ");
        assert_eq!(focus_prefix_text(false, &test_app(true)), "  ");
    }

    #[test]
    fn unfocused_prefix_is_hidden_in_color_mode() {
        assert_eq!(unfocused_prefix_text(&test_app(false)), "");
    }

    #[test]
    fn unfocused_prefix_keeps_no_color_alignment() {
        assert_eq!(unfocused_prefix_text(&test_app(true)), "  ");
    }

    #[test]
    fn list_highlight_symbol_matches_navigation_prefix_rules() {
        assert_eq!(list_highlight_symbol(&test_app(false)), "");
        assert_eq!(list_highlight_symbol(&test_app(true)), "> ");
    }
}
