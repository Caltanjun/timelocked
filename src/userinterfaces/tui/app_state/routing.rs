//! Screen routing and global key handling for the TUI app state.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{
    handle_inspect_details_key, handle_inspect_form_key, handle_lock_complete_key,
    handle_lock_file_form_key, handle_lock_progress_key, handle_lock_text_form_key,
    handle_main_menu_key, handle_unlock_complete_key, handle_unlock_form_key,
    handle_unlock_progress_key, handle_verify_details_key, handle_verify_form_key,
    modal::handle_modal_key, App, MainMenuState, Screen,
};

impl App {
    pub fn on_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        if self.modal.is_some() {
            handle_modal_key(self, key);
            return;
        }

        let placeholder = Screen::MainMenu(MainMenuState::default());
        let screen = std::mem::replace(&mut self.screen, placeholder);
        self.screen = match screen {
            Screen::MainMenu(mut state) => handle_main_menu_key(&mut state, key, self),
            Screen::LockFileForm(mut state) => handle_lock_file_form_key(&mut state, key, self),
            Screen::LockTextForm(mut state) => handle_lock_text_form_key(&mut state, key, self),
            Screen::LockProgress(state) => handle_lock_progress_key(state, key, self),
            Screen::LockComplete(mut state) => handle_lock_complete_key(&mut state, key, self),
            Screen::UnlockForm(mut state) => handle_unlock_form_key(&mut state, key, self),
            Screen::UnlockProgress(state) => handle_unlock_progress_key(state, key, self),
            Screen::UnlockComplete(state) => handle_unlock_complete_key(state, key, self),
            Screen::InspectForm(mut state) => handle_inspect_form_key(&mut state, key, self),
            Screen::InspectDetails(state) => handle_inspect_details_key(state, key, self),
            Screen::VerifyForm(mut state) => handle_verify_form_key(&mut state, key, self),
            Screen::VerifyDetails(state) => handle_verify_details_key(state, key, self),
        };
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::App;

    #[test]
    fn main_menu_esc_quits_and_q_does_not() {
        let mut app = App::new(false);

        app.on_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(!app.should_quit);

        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.should_quit);
    }
}
