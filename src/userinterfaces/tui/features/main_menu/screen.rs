//! Renders the main menu and routes selected actions to feature entry screens.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::Frame;

use crate::userinterfaces::tui::app_state::{
    App, InspectFormState, LockFileFormState, LockTextFormState, Screen, UnlockFormState,
    VerifyFormState,
};
use crate::userinterfaces::tui::components::form::{
    focused_line, helper_line, italic_helper_line, menu_item_with_right_label,
};
use crate::userinterfaces::tui::components::layout::render_block_paragraph;

const MENU_HELP_LABEL_WIDTH: usize = 24;
const MAIN_MENU_DISCLAIMER_LINES: [&str; 3] = [
    "Timelocked uses sequential computation puzzles to enforce unlock delays.",
    "Unlocking uses a single-core CPU work by design.",
    "Actual duration varies with hardware, thermals, power mode, and load.",
];
const STACKED_MENU_MIN_HEIGHT: u16 = 20;

#[derive(Debug, Clone, Copy)]
pub enum MainMenuAction {
    LockFile,
    LockText,
    Unlock,
    Inspect,
    Verify,
}

#[derive(Debug, Clone, Copy)]
pub struct MainMenuItem {
    pub label: &'static str,
    pub description: &'static str,
    pub action: MainMenuAction,
}

pub const MAIN_MENU_ITEMS: [MainMenuItem; 5] = [
    MainMenuItem {
        label: "Lock a file",
        description: "Create a timelocked file from a source file",
        action: MainMenuAction::LockFile,
    },
    MainMenuItem {
        label: "Lock text",
        description: "Create a timelocked file from a message",
        action: MainMenuAction::LockText,
    },
    MainMenuItem {
        label: "Unlock a timelocked file",
        description: "Recover the original content",
        action: MainMenuAction::Unlock,
    },
    MainMenuItem {
        label: "Inspect a timelocked file",
        description: "Show metadata and delay parameters",
        action: MainMenuAction::Inspect,
    },
    MainMenuItem {
        label: "Verify a timelocked file",
        description: "Check file structure",
        action: MainMenuAction::Verify,
    },
];

#[derive(Debug, Clone, Default)]
pub struct MainMenuState {
    pub selected: usize,
}

impl MainMenuState {
    pub(crate) fn move_up(&mut self) {
        if self.selected == 0 {
            self.selected = MAIN_MENU_ITEMS.len().saturating_sub(1);
        } else {
            self.selected -= 1;
        }
    }

    pub(crate) fn move_down(&mut self) {
        self.selected = (self.selected + 1) % MAIN_MENU_ITEMS.len().max(1);
    }
}

pub fn help() -> &'static str {
    "Choose an action."
}

pub fn render(state: &MainMenuState, frame: &mut Frame, area: Rect, app: &App) {
    let lines = main_menu_lines(state, area.width, area.height, app);
    render_block_paragraph(frame, area, "Main Menu", lines, app);
}

fn main_menu_lines<'a>(
    state: &MainMenuState,
    width: u16,
    height: u16,
    app: &'a App,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    let layout_mode = menu_layout_mode(width, height);
    let content_width = usize::from(width.saturating_sub(4));
    let description_column = content_width.saturating_mul(62) / 100;

    for (index, item) in MAIN_MENU_ITEMS.iter().enumerate() {
        match layout_mode {
            MainMenuLayout::TwoColumn => lines.push(menu_item_with_right_label(
                item.label,
                item.description,
                index == state.selected,
                description_column,
                app,
            )),
            MainMenuLayout::StackedWithDescriptions => {
                lines.push(focused_line(index == state.selected, item.label, app));
                lines.push(helper_line(item.description, 0, app));
                if index + 1 < MAIN_MENU_ITEMS.len() {
                    lines.push(Line::from(""));
                }
            }
            MainMenuLayout::Compact => {
                lines.push(focused_line(index == state.selected, item.label, app));
            }
        }
    }

    lines.push(Line::from(""));
    for copy_line in MAIN_MENU_DISCLAIMER_LINES {
        lines.push(italic_helper_line(copy_line, 0, app));
    }
    lines.push(Line::from(""));
    lines.push(helper_line(
        &format!("More info: {}", app.docs_url),
        MENU_HELP_LABEL_WIDTH,
        app,
    ));

    lines
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MainMenuLayout {
    TwoColumn,
    StackedWithDescriptions,
    Compact,
}

fn menu_layout_mode(width: u16, height: u16) -> MainMenuLayout {
    if height < STACKED_MENU_MIN_HEIGHT {
        if width >= 110 {
            MainMenuLayout::TwoColumn
        } else {
            MainMenuLayout::Compact
        }
    } else if width >= 110 {
        MainMenuLayout::TwoColumn
    } else if width >= 72 {
        MainMenuLayout::StackedWithDescriptions
    } else {
        MainMenuLayout::Compact
    }
}

pub fn handle_key(state: &mut MainMenuState, key: KeyEvent, app: &mut App) -> Screen {
    match key.code {
        KeyCode::Esc => {
            app.should_quit = true;
            Screen::MainMenu(state.clone())
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.move_up();
            Screen::MainMenu(state.clone())
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.move_down();
            Screen::MainMenu(state.clone())
        }
        KeyCode::Enter => match MAIN_MENU_ITEMS[state.selected].action {
            MainMenuAction::LockFile => Screen::LockFileForm(LockFileFormState::default()),
            MainMenuAction::LockText => Screen::LockTextForm(LockTextFormState::default()),
            MainMenuAction::Unlock => Screen::UnlockForm(UnlockFormState::default()),
            MainMenuAction::Inspect => Screen::InspectForm(InspectFormState::default()),
            MainMenuAction::Verify => Screen::VerifyForm(VerifyFormState::default()),
        },
        _ => Screen::MainMenu(state.clone()),
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::Modifier;

    use super::{main_menu_lines, menu_layout_mode, MainMenuLayout, MAIN_MENU_ITEMS};
    use crate::userinterfaces::tui::app_state::App;

    fn line_text(line: &ratatui::text::Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    #[test]
    fn uses_compact_layout_under_72_columns() {
        assert_eq!(menu_layout_mode(50, 20), MainMenuLayout::Compact);
        assert_eq!(menu_layout_mode(71, 20), MainMenuLayout::Compact);
        assert_eq!(
            menu_layout_mode(72, 20),
            MainMenuLayout::StackedWithDescriptions
        );
    }

    #[test]
    fn uses_two_column_layout_from_110_columns() {
        assert_eq!(
            menu_layout_mode(109, 20),
            MainMenuLayout::StackedWithDescriptions
        );
        assert_eq!(menu_layout_mode(110, 20), MainMenuLayout::TwoColumn);
    }

    #[test]
    fn tight_height_uses_shorter_menu_layouts() {
        assert_eq!(menu_layout_mode(72, 14), MainMenuLayout::Compact);
        assert_eq!(menu_layout_mode(110, 14), MainMenuLayout::TwoColumn);
    }

    #[test]
    fn menu_exposes_only_mvp_actions() {
        let labels: Vec<_> = MAIN_MENU_ITEMS.iter().map(|item| item.label).collect();

        assert_eq!(
            labels,
            vec![
                "Lock a file",
                "Lock text",
                "Unlock a timelocked file",
                "Inspect a timelocked file",
                "Verify a timelocked file"
            ]
        );
    }

    #[test]
    fn first_run_copy_appears_before_more_info() {
        let app = App::new(false);
        let lines = main_menu_lines(&Default::default(), 120, 20, &app);
        let texts: Vec<_> = lines.iter().map(line_text).collect();
        let info_index = texts
            .iter()
            .position(|line| line.contains("More info:"))
            .expect("more info line");

        assert!(texts[info_index - 1].is_empty());
        assert!(texts[info_index - 2].contains("Actual duration varies"));
        assert!(texts[info_index - 3].contains("single-core CPU work"));
        assert!(texts[info_index - 4].contains("sequential computation puzzles"));
    }

    #[test]
    fn compact_layout_keeps_disclaimer_and_info_within_tight_height() {
        let app = App::new(false);
        let lines = main_menu_lines(&Default::default(), 72, 14, &app);
        let texts: Vec<_> = lines.iter().map(line_text).collect();

        assert_eq!(lines.len(), 11);
        assert!(texts
            .iter()
            .any(|line| line.contains("sequential computation puzzles")));
        assert!(texts.iter().any(|line| line.contains("More info:")));
    }

    #[test]
    fn disclaimer_copy_is_italic_but_more_info_stays_plain_muted() {
        let app = App::new(false);
        let lines = main_menu_lines(&Default::default(), 120, 20, &app);
        let disclaimer_line = lines
            .iter()
            .find(|line| line_text(line).contains("sequential computation puzzles"))
            .expect("disclaimer line");
        let more_info_line = lines
            .iter()
            .find(|line| line_text(line).contains("More info:"))
            .expect("more info line");

        assert!(disclaimer_line.spans[1]
            .style
            .add_modifier
            .contains(Modifier::ITALIC));
        assert!(!more_info_line.spans[3]
            .style
            .add_modifier
            .contains(Modifier::ITALIC));
    }
}
