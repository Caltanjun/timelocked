//! Modal and file-browser event handling for the TUI app state.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};

use super::{
    browser_filter_toggle_available, App, BrowserMode, BrowserTarget, FileBrowserState,
    MainMenuState, Modal, Screen, TextField,
};
use crate::userinterfaces::tui::features::shared::unlock_estimate::refresh_unlock_estimate_state;

pub(super) fn handle_modal_key(app: &mut App, key: KeyEvent) {
    let Some(modal) = app.modal.take() else {
        return;
    };

    match modal {
        Modal::Error(message) => {
            if matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
                app.modal = None;
            } else {
                app.modal = Some(Modal::Error(message));
            }
        }
        Modal::Info(message) => {
            if matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
                app.modal = None;
            } else {
                app.modal = Some(Modal::Info(message));
            }
        }
        Modal::Browser(mut browser) => {
            if handle_browser_key(app, &mut browser, key) {
                app.modal = None;
            } else {
                app.modal = Some(Modal::Browser(browser));
            }
        }
    }
}

fn handle_browser_key(app: &mut App, browser: &mut FileBrowserState, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Esc => true,
        KeyCode::Up | KeyCode::Char('k') => {
            browser.move_up();
            false
        }
        KeyCode::Down | KeyCode::Char('j') => {
            browser.move_down();
            false
        }
        KeyCode::Left => {
            browser.navigate_parent();
            false
        }
        KeyCode::Right => {
            browser.navigate_selected();
            false
        }
        KeyCode::Char('h') => {
            browser.toggle_hidden_entries();
            false
        }
        KeyCode::Char('f') => {
            if browser_filter_toggle_available(browser.mode, browser.target) {
                browser.toggle_file_filter();
            }
            false
        }
        KeyCode::Char('s') => {
            if matches!(browser.mode, BrowserMode::Directory) {
                apply_browser_selection(app, browser.target, browser.current_dir.clone());
                true
            } else {
                false
            }
        }
        KeyCode::Enter => {
            let Some(entry) = browser.selected_entry().cloned() else {
                return false;
            };

            match browser.mode {
                BrowserMode::File => {
                    if entry.is_dir {
                        browser.navigate_selected();
                        false
                    } else {
                        apply_browser_selection(app, browser.target, entry.path);
                        true
                    }
                }
                BrowserMode::Directory => {
                    if entry.is_dir {
                        apply_browser_selection(app, browser.target, entry.path);
                        true
                    } else {
                        false
                    }
                }
            }
        }
        _ => false,
    }
}

fn apply_browser_selection(app: &mut App, target: BrowserTarget, path: PathBuf) {
    let path_string = path.to_string_lossy().to_string();
    let mut refresh_unlock_estimate = false;

    match &mut app.screen {
        Screen::LockFileForm(state) => {
            if matches!(target, BrowserTarget::LockFileInput) {
                state.input_path = TextField::new(path_string.clone());
                if !state.output_touched {
                    state.output_path = TextField::new(
                        super::super::features::lock::file_form::derive_default_output(
                            &path_string,
                        ),
                    );
                }
            }
        }
        Screen::UnlockForm(state) => match target {
            BrowserTarget::UnlockInput => {
                state.input_path = TextField::new(path_string);
                refresh_unlock_estimate = true;
            }
            BrowserTarget::UnlockOutputDir => {
                state.output_dir = TextField::new(path_string);
            }
            _ => {}
        },
        Screen::InspectForm(state) => {
            if matches!(target, BrowserTarget::InspectInput) {
                state.input_path = TextField::new(path_string);
            }
        }
        Screen::VerifyForm(state) => {
            if matches!(target, BrowserTarget::VerifyInput) {
                state.input_path = TextField::new(path_string);
            }
        }
        _ => {}
    }

    if refresh_unlock_estimate {
        let placeholder = Screen::MainMenu(MainMenuState::default());
        let screen = std::mem::replace(&mut app.screen, placeholder);
        app.screen = match screen {
            Screen::UnlockForm(mut state) => {
                refresh_unlock_estimate_state(app, &mut state);
                Screen::UnlockForm(state)
            }
            other => other,
        };
    }
}

impl App {
    pub(crate) fn open_browser(
        &mut self,
        target: BrowserTarget,
        mode: BrowserMode,
        preferred_path: Option<PathBuf>,
    ) {
        let browser = FileBrowserState::new(mode, target, preferred_path);
        self.modal = Some(Modal::Browser(browser));
    }
}
