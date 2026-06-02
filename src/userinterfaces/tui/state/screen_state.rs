//! Shared TUI screen and modal enums.
//! They define which feature state is currently active in the single-session app.

use crate::base::{Result, SecretString};
use crate::userinterfaces::tui::features::inspect::details::InspectDetailsState;
use crate::userinterfaces::tui::features::inspect::form::InspectFormState;
use crate::userinterfaces::tui::features::lock::complete::LockCompleteState;
use crate::userinterfaces::tui::features::lock::file_form::LockFileFormState;
use crate::userinterfaces::tui::features::lock::progress::LockProgressState;
use crate::userinterfaces::tui::features::lock::text_form::LockTextFormState;
use crate::userinterfaces::tui::features::main_menu::screen::MainMenuState;
use crate::userinterfaces::tui::features::unlock::complete::UnlockCompleteState;
use crate::userinterfaces::tui::features::unlock::form::UnlockFormState;
use crate::userinterfaces::tui::features::unlock::progress::UnlockProgressState;
use crate::userinterfaces::tui::features::verify::details::VerifyDetailsState;
use crate::userinterfaces::tui::features::verify::form::VerifyFormState;
use crate::userinterfaces::tui::state::SecretTextField;

use super::browser::FileBrowserState;

use std::sync::mpsc::Sender;

#[derive(Debug, Clone)]
pub struct FooterContent {
    pub left: String,
    pub center: String,
}

pub enum Screen {
    MainMenu(MainMenuState),
    LockFileForm(LockFileFormState),
    LockTextForm(LockTextFormState),
    LockProgress(LockProgressState),
    LockComplete(LockCompleteState),
    UnlockForm(UnlockFormState),
    UnlockProgress(UnlockProgressState),
    UnlockComplete(UnlockCompleteState),
    InspectForm(InspectFormState),
    InspectDetails(InspectDetailsState),
    VerifyForm(VerifyFormState),
    VerifyDetails(VerifyDetailsState),
}

pub enum Modal {
    Error(String),
    Info(String),
    Browser(FileBrowserState),
    PasswordPrompt(PasswordPromptState),
}

pub struct PasswordPromptState {
    pub password: SecretTextField,
    pub focus: PasswordPromptFocus,
    pub attempt: u8,
    pub max_attempts: u8,
    pub previous_attempt_failed: bool,
    pub reply: Sender<Result<SecretString>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordPromptFocus {
    Password,
    Unlock,
    Cancel,
}

impl PasswordPromptFocus {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Password => Self::Unlock,
            Self::Unlock => Self::Cancel,
            Self::Cancel => Self::Password,
        }
    }

    pub(crate) fn prev(self) -> Self {
        match self {
            Self::Password => Self::Cancel,
            Self::Unlock => Self::Password,
            Self::Cancel => Self::Unlock,
        }
    }
}
