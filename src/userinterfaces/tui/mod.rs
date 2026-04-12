//! TUI runtime setup and event loop.
//! It boots directly into the main menu and keeps terminal lifecycle concerns local.

mod app_state;
mod components;
mod features;
mod render;
mod state;
mod worker;

use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use self::app_state::App;

pub struct TuiOptions {
    pub no_color: bool,
}

pub fn run(options: TuiOptions) -> anyhow::Result<()> {
    let no_color = options.no_color || std::env::var_os("NO_COLOR").is_some();

    let mut app = App::new(no_color);
    let mut terminal = setup_terminal()?;

    let run_result = event_loop(&mut terminal, &mut app);
    let restore_result = restore_terminal(&mut terminal);

    run_result?;
    restore_result?;
    Ok(())
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> anyhow::Result<()> {
    while !app.should_quit {
        app.poll_workers();
        terminal.draw(|frame| render::draw(frame, app))?;
        app.on_frame_rendered();

        if app.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key)
                    if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                {
                    app.on_key(key);
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }

    Ok(())
}

fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
