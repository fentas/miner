mod state;
mod widgets;

pub use state::TuiState;
pub use widgets::draw_ui;

use std::io;
use std::sync::Arc;
use std::time::Duration;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use tokio::sync::mpsc;

/// Commands that can be sent to the TUI
#[derive(Debug)]
pub enum TuiCommand {
    Quit,
    TogglePause,
    ClearLogs,
    ToggleBitcoinLogs,
}

/// Run the TUI in a separate thread
pub struct TuiRunner {
    state: Arc<TuiState>,
    cmd_tx: mpsc::UnboundedSender<TuiCommand>,
}

impl TuiRunner {
    pub fn new(state: Arc<TuiState>) -> (Self, mpsc::UnboundedReceiver<TuiCommand>) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        (Self { state, cmd_tx }, cmd_rx)
    }

    /// Run the TUI event loop (blocking)
    #[cfg(not(tarpaulin_include))]
    pub fn run(&self) -> io::Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Clear terminal
        terminal.clear()?;

        let result = self.run_loop(&mut terminal);

        // Restore terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    #[cfg(not(tarpaulin_include))]
    fn run_loop(&self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        loop {
            // Draw UI
            terminal.draw(|f| {
                draw_ui(f, &self.state);
            })?;

            // Handle input with timeout
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    match (key.code, key.modifiers) {
                        // Quit: q, Ctrl+C, Escape
                        (KeyCode::Char('q'), _)
                        | (KeyCode::Char('c'), KeyModifiers::CONTROL)
                        | (KeyCode::Esc, _) => {
                            let _ = self.cmd_tx.send(TuiCommand::Quit);
                            return Ok(());
                        }
                        // Pause: p or space
                        (KeyCode::Char('p'), _) | (KeyCode::Char(' '), _) => {
                            let _ = self.cmd_tx.send(TuiCommand::TogglePause);
                        }
                        // Clear logs: c
                        (KeyCode::Char('c'), KeyModifiers::NONE) => {
                            let _ = self.cmd_tx.send(TuiCommand::ClearLogs);
                            self.state.clear_logs();
                        }
                        // Toggle Bitcoin Core logs: l (only if managed mode)
                        (KeyCode::Char('l'), KeyModifiers::NONE) => {
                            if self.state.is_bitcoin_managed() {
                                self.state.toggle_bitcoin_logs();
                                let _ = self.cmd_tx.send(TuiCommand::ToggleBitcoinLogs);
                            }
                        }
                        // Scroll logs: up/down, j/k, Page Up/Down
                        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                            self.state.scroll_up(1);
                        }
                        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                            self.state.scroll_down(1);
                        }
                        (KeyCode::PageUp, _) => {
                            self.state.scroll_up(10);
                        }
                        (KeyCode::PageDown, _) => {
                            self.state.scroll_down(10);
                        }
                        (KeyCode::Home, _) => {
                            self.state.scroll_to_top();
                        }
                        (KeyCode::End, _) => {
                            self.state.scroll_to_bottom();
                        }
                        _ => {}
                    }
                }
            }

            // Check if we should exit
            if self.state.should_quit() {
                return Ok(());
            }
        }
    }
}
