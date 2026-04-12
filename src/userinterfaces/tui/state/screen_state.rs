//! Shared TUI screen and modal enums.
//! They define which feature state is currently active in the single-session app.

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

use super::browser::FileBrowserState;

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
}
