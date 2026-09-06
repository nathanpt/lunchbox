use anyhow::{Context, Result};
use crossterm::event;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, Stdout};

pub type Screen = Terminal<CrosstermBackend<Stdout>>;

pub fn install() -> Result<Screen> {
    let _ = color_eyre::install();
    enable_raw_mode().context("failed to enable raw mode")?;
    let screen = open_screen();
    if screen.is_err() {
        let _ = disable_raw_mode();
    }
    screen
}

fn open_screen() -> Result<Screen> {
    crossterm::execute!(io::stdout(), EnterAlternateScreen)
        .context("failed to enter alternate screen")?;
    Terminal::new(CrosstermBackend::new(io::stdout()))
        .context("failed to open terminal")
}
pub struct Restore;

impl Drop for Restore {
    fn drop(&mut self) {
        let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen);
        let _ = disable_raw_mode();
        ratatui::restore();
    }
}

pub fn next_event() -> Result<event::Event> {
    event::read().context("failed to read terminal event")
}
