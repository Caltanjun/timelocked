//! Modal and file-browser event handling for the TUI app state.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};

use super::{
    browser_filter_toggle_available, App, BrowserMode, BrowserTarget, FileBrowserState,
    MainMenuState, Modal, PasswordPromptFocus, PasswordPromptState, Screen, TextField,
};
use crate::base::{Error, SecretString};
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
        Modal::PasswordPrompt(mut state) => {
            if handle_password_prompt_key(app, &mut state, key) {
                app.modal = None;
            } else {
                app.modal = Some(Modal::PasswordPrompt(state));
            }
        }
    }
}

fn handle_password_prompt_key(
    app: &mut App,
    state: &mut PasswordPromptState,
    key: KeyEvent,
) -> bool {
    match key.code {
        KeyCode::Tab => {
            state.focus = state.focus.next();
            false
        }
        KeyCode::BackTab => {
            state.focus = state.focus.prev();
            false
        }
        KeyCode::Esc => {
            cancel_password_prompt(app, state);
            true
        }
        KeyCode::Enter => match state.focus {
            PasswordPromptFocus::Password | PasswordPromptFocus::Unlock => {
                submit_password_prompt(state);
                true
            }
            PasswordPromptFocus::Cancel => {
                cancel_password_prompt(app, state);
                true
            }
        },
        _ if matches!(state.focus, PasswordPromptFocus::Password) => {
            state.password.apply_key(key);
            false
        }
        _ => false,
    }
}

fn submit_password_prompt(state: &mut PasswordPromptState) {
    let password = SecretString::new(state.password.expose_secret().to_string());
    state.password.clear();
    let _ = state.reply.send(Ok(password));
}

fn cancel_password_prompt(app: &mut App, state: &mut PasswordPromptState) {
    state.password.clear();
    let _ = state.reply.send(Err(Error::Cancelled));
    if let Screen::UnlockProgress(progress) = &mut app.screen {
        progress.cancel_requested = true;
        progress.worker.cancellation.cancel();
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

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::base::progress_status::ProgressStatus;
    use crate::base::CancellationToken;
    use crate::userinterfaces::tui::features::unlock::progress::{
        UnlockProgressFocus, UnlockProgressState,
    };
    use crate::userinterfaces::tui::worker::UnlockWorker;

    fn password_modal() -> (
        Modal,
        mpsc::Receiver<crate::base::Result<crate::base::SecretString>>,
    ) {
        let (reply, receiver) = mpsc::channel();
        (
            Modal::PasswordPrompt(PasswordPromptState {
                password: crate::userinterfaces::tui::state::SecretTextField::default(),
                focus: PasswordPromptFocus::Password,
                attempt: 1,
                max_attempts: 3,
                previous_attempt_failed: false,
                reply,
            }),
            receiver,
        )
    }

    fn unlock_progress_screen() -> Screen {
        let (_sender, receiver) = mpsc::channel();
        Screen::UnlockProgress(UnlockProgressState {
            file_display: "archive.timelocked".to_string(),
            progress: ProgressStatus::new("unlock-timelock", 1, 2, Some(1), Some(1.0)),
            worker: UnlockWorker {
                receiver,
                cancellation: CancellationToken::default(),
            },
            cancel_requested: false,
            cpu_count: 1,
            focus: UnlockProgressFocus::Progress,
        })
    }

    #[test]
    fn password_modal_submit_replies_to_worker() {
        let mut app = App::new(false);
        let (modal, receiver) = password_modal();
        app.modal = Some(modal);

        app.on_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(app.modal.is_none());
        let password = receiver
            .recv()
            .expect("password reply")
            .expect("password ok");
        assert_eq!(password.expose_secret(), "s3");
    }

    #[test]
    fn password_modal_cancel_cancels_worker() {
        let mut app = App::new(false);
        app.screen = unlock_progress_screen();
        let (modal, receiver) = password_modal();
        app.modal = Some(modal);

        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(app.modal.is_none());
        assert!(matches!(
            receiver.recv().expect("cancel reply"),
            Err(Error::Cancelled)
        ));
        match &app.screen {
            Screen::UnlockProgress(state) => {
                assert!(state.cancel_requested);
                assert!(state.worker.cancellation.is_cancelled());
            }
            _ => panic!("expected unlock progress screen"),
        }
    }
}
