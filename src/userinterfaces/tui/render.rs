//! TUI rendering entrypoints.
//! This module draws the active screen plus shared chrome like header, footer, and modals.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Frame;

use super::app_state::{App, Modal, Screen};
use super::components::layout::{
    constrained_page_area, render_footer, render_header, render_small_terminal,
};
use super::components::modal::{render_browser_modal, render_message_modal};

use super::features::inspect::details::render as render_inspect_details;
use super::features::inspect::form::render as render_inspect_form;
use super::features::lock::complete::render as render_lock_complete;
use super::features::lock::file_form::render as render_lock_file;
use super::features::lock::progress::render as render_lock_progress;
use super::features::lock::text_form::render as render_lock_text;
use super::features::main_menu::screen::render as render_main_menu;
use super::features::unlock::complete::render as render_unlock_complete;
use super::features::unlock::form::render as render_unlock_form;
use super::features::unlock::progress::render as render_unlock_progress;
use super::features::verify::details::render as render_verify_details;
use super::features::verify::form::render as render_verify_form;

const MIN_VIEWPORT_WIDTH: u16 = 46;
const MIN_VIEWPORT_HEIGHT: u16 = 20;

pub fn draw(frame: &mut Frame, app: &App) {
    let viewport = frame.area();
    if viewport.width < MIN_VIEWPORT_WIDTH || viewport.height < MIN_VIEWPORT_HEIGHT {
        render_small_terminal(frame, viewport, app);
        return;
    }

    let area = constrained_page_area(viewport, 140, 2);

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    render_header(frame, vertical[0], app);
    render_screen(frame, vertical[1], app);
    render_footer(frame, vertical[2], app.footer_content(), app);

    if let Some(modal) = &app.modal {
        match modal {
            Modal::Error(message) => {
                render_message_modal(frame, viewport, "Error", message, app, true)
            }
            Modal::Info(message) => {
                render_message_modal(frame, viewport, "Info", message, app, false)
            }
            Modal::Browser(browser) => render_browser_modal(frame, viewport, browser, app),
        }
    }
}

fn render_screen(frame: &mut Frame, area: Rect, app: &App) {
    match &app.screen {
        Screen::MainMenu(state) => {
            render_main_menu(state, frame, area, app);
        }
        Screen::LockFileForm(state) => {
            render_lock_file(state, frame, area, app);
        }
        Screen::LockTextForm(state) => {
            render_lock_text(state, frame, area, app);
        }
        Screen::LockProgress(state) => {
            render_lock_progress(state, frame, area, app);
        }
        Screen::LockComplete(state) => {
            render_lock_complete(state, frame, area, app);
        }
        Screen::UnlockForm(state) => {
            render_unlock_form(state, frame, area, app);
        }
        Screen::UnlockProgress(state) => {
            render_unlock_progress(state, frame, area, app);
        }
        Screen::UnlockComplete(state) => {
            render_unlock_complete(state, frame, area, app);
        }
        Screen::InspectForm(state) => {
            render_inspect_form(state, frame, area, app);
        }
        Screen::InspectDetails(state) => {
            render_inspect_details(state, frame, area, app);
        }
        Screen::VerifyForm(state) => {
            render_verify_form(state, frame, area, app);
        }
        Screen::VerifyDetails(state) => {
            render_verify_details(state, frame, area, app);
        }
    }
}
