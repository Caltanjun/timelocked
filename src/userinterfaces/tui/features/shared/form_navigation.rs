//! Shared helpers for cycling focus on form-style TUI screens.

use crossterm::event::{KeyCode, KeyEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusNavigationAxis {
    Vertical,
    Horizontal,
}

pub(crate) fn cycled_focus<F: Copy>(
    key: KeyEvent,
    focus: F,
    next: impl Fn(F) -> F,
    prev: impl Fn(F) -> F,
    axis: FocusNavigationAxis,
) -> Option<F> {
    match key.code {
        KeyCode::Tab => Some(next(focus)),
        KeyCode::BackTab => Some(prev(focus)),
        KeyCode::Down if matches!(axis, FocusNavigationAxis::Vertical) => Some(next(focus)),
        KeyCode::Up if matches!(axis, FocusNavigationAxis::Vertical) => Some(prev(focus)),
        KeyCode::Right if matches!(axis, FocusNavigationAxis::Horizontal) => Some(next(focus)),
        KeyCode::Left if matches!(axis, FocusNavigationAxis::Horizontal) => Some(prev(focus)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{cycled_focus, FocusNavigationAxis};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Focus {
        First,
        Second,
    }

    impl Focus {
        fn next(self) -> Self {
            match self {
                Self::First => Self::Second,
                Self::Second => Self::First,
            }
        }

        fn prev(self) -> Self {
            self.next()
        }
    }

    #[test]
    fn vertical_navigation_supports_up_and_down() {
        let focus = Focus::First;

        assert_eq!(
            cycled_focus(
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
                focus,
                Focus::next,
                Focus::prev,
                FocusNavigationAxis::Vertical,
            ),
            Some(Focus::Second)
        );
        assert_eq!(
            cycled_focus(
                KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
                focus,
                Focus::next,
                Focus::prev,
                FocusNavigationAxis::Vertical,
            ),
            Some(Focus::Second)
        );
    }

    #[test]
    fn horizontal_navigation_supports_left_and_right() {
        let focus = Focus::First;

        assert_eq!(
            cycled_focus(
                KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
                focus,
                Focus::next,
                Focus::prev,
                FocusNavigationAxis::Horizontal,
            ),
            Some(Focus::Second)
        );
        assert_eq!(
            cycled_focus(
                KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
                focus,
                Focus::next,
                Focus::prev,
                FocusNavigationAxis::Horizontal,
            ),
            Some(Focus::Second)
        );
    }

    #[test]
    fn vertical_navigation_does_not_consume_horizontal_text_keys() {
        assert_eq!(
            cycled_focus(
                KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
                Focus::First,
                Focus::next,
                Focus::prev,
                FocusNavigationAxis::Vertical,
            ),
            None
        );
        assert_eq!(
            cycled_focus(
                KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
                Focus::First,
                Focus::next,
                Focus::prev,
                FocusNavigationAxis::Vertical,
            ),
            None
        );
    }

    #[test]
    fn tab_navigation_works_for_each_axis() {
        let focus = Focus::First;

        assert_eq!(
            cycled_focus(
                KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
                focus,
                Focus::next,
                Focus::prev,
                FocusNavigationAxis::Vertical,
            ),
            Some(Focus::Second)
        );
        assert_eq!(
            cycled_focus(
                KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
                focus,
                Focus::next,
                Focus::prev,
                FocusNavigationAxis::Horizontal,
            ),
            Some(Focus::Second)
        );
    }
}
