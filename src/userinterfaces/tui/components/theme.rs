//! Semantic styling helpers for the ratatui interface.

use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Padding};

use crate::userinterfaces::tui::app_state::App;

pub(crate) fn plain_block(app: &App) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(border_style(app))
        .padding(Padding::new(1, 1, 0, 0))
}

pub(crate) fn titled_plain_block(title: &str, app: &App) -> Block<'static> {
    plain_block(app).title(format!(" {title} "))
}

pub(crate) fn panel_block(title: &str, app: &App) -> Block<'static> {
    Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_style(border_style(app))
        .padding(Padding::new(1, 1, 1, 1))
}

fn border_style(app: &App) -> Style {
    if app.no_color {
        Style::default()
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

pub(crate) fn base_style(_app: &App) -> Style {
    Style::default()
}

pub(crate) fn label_style(app: &App) -> Style {
    if app.no_color {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Gray)
            .add_modifier(Modifier::BOLD)
    }
}

pub(crate) fn value_style(app: &App) -> Style {
    if app.no_color {
        Style::default()
    } else {
        Style::default().fg(Color::White)
    }
}

pub(crate) fn detail_value_style(app: &App) -> Style {
    if app.no_color {
        value_style(app).add_modifier(Modifier::BOLD)
    } else {
        value_style(app)
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    }
}

pub(crate) fn success_value_style(app: &App) -> Style {
    if app.no_color {
        value_style(app).add_modifier(Modifier::BOLD)
    } else {
        value_style(app)
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    }
}

pub(crate) fn warning_value_style(app: &App) -> Style {
    if app.no_color {
        value_style(app).add_modifier(Modifier::BOLD)
    } else {
        value_style(app)
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    }
}

pub(crate) fn muted_style(app: &App) -> Style {
    if app.no_color {
        Style::default().add_modifier(Modifier::DIM)
    } else {
        Style::default().fg(Color::Gray).add_modifier(Modifier::DIM)
    }
}

pub(crate) fn muted_italic_style(app: &App) -> Style {
    muted_style(app).add_modifier(Modifier::ITALIC)
}

pub(crate) fn focused_control_style(app: &App) -> Style {
    if app.no_color {
        Style::default()
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    }
}

pub(crate) fn accent_style(app: &App) -> Style {
    focused_control_style(app)
}

pub(crate) fn primary_action_style(app: &App) -> Style {
    if app.no_color {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    }
}

pub(crate) fn secondary_action_style(app: &App) -> Style {
    if app.no_color {
        Style::default()
    } else {
        Style::default().fg(Color::Gray)
    }
}

pub(crate) fn warning_style(app: &App) -> Style {
    if app.no_color {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Yellow)
    }
}

pub(crate) fn destructive_style(app: &App) -> Style {
    if app.no_color {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Red)
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::{Color, Modifier};

    use super::{
        detail_value_style, focused_control_style, muted_italic_style, muted_style, value_style,
    };
    use crate::userinterfaces::tui::app_state::App;

    fn test_app(no_color: bool) -> App {
        App::new(no_color)
    }

    #[test]
    fn focused_controls_do_not_use_underline() {
        let color_style = focused_control_style(&test_app(false));
        let no_color_style = focused_control_style(&test_app(true));

        assert!(color_style.add_modifier.contains(Modifier::BOLD));
        assert!(!color_style.add_modifier.contains(Modifier::UNDERLINED));
        assert!(no_color_style.add_modifier.contains(Modifier::BOLD));
        assert!(no_color_style.add_modifier.contains(Modifier::REVERSED));
        assert!(!no_color_style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn muted_text_uses_brighter_gray_when_color_is_enabled() {
        let style = muted_style(&test_app(false));

        assert_eq!(style.fg, Some(Color::Gray));
        assert!(style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn muted_italic_text_preserves_muted_semantics_and_adds_italic() {
        let style = muted_italic_style(&test_app(false));

        assert_eq!(style.fg, Some(Color::Gray));
        assert!(style.add_modifier.contains(Modifier::DIM));
        assert!(style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn editable_values_keep_plain_no_color_fallback() {
        let style = value_style(&test_app(true));

        assert_eq!(style.fg, None);
        assert!(!style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn detail_values_use_semantic_color_when_enabled() {
        let style = detail_value_style(&test_app(false));

        assert_eq!(style.fg, Some(Color::Cyan));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }
}
