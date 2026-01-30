mod state;
mod widgets;
mod logs;
mod history;

pub use state::TuiState;
pub use logs::{LogEntry, LogLevel};
pub use history::HashrateDisplayMode;
pub use widgets::draw_ui;

use std::io;
use std::sync::Arc;
use std::time::Duration;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
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
    ToggleIdle,
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
                    // Only handle key press events, not repeats or releases
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    // Command mode input handling
                    if self.state.is_command_mode() {
                        match key.code {
                            KeyCode::Esc => {
                                self.state.exit_command_mode();
                            }
                            KeyCode::Enter => {
                                let cmd = self.state.get_command();
                                self.state.exit_command_mode();
                                // Execute command
                                match cmd.as_str() {
                                    "q" | "quit" => {
                                        let _ = self.cmd_tx.send(TuiCommand::Quit);
                                        return Ok(());
                                    }
                                    "clear" => {
                                        let _ = self.cmd_tx.send(TuiCommand::ClearLogs);
                                        self.state.clear_logs();
                                    }
                                    _ => {} // Unknown command - ignore
                                }
                            }
                            KeyCode::Backspace => {
                                self.state.command_pop();
                            }
                            KeyCode::Char(c) => {
                                self.state.command_push(c);
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Help overlay - any key dismisses it
                    if self.state.is_help_visible() {
                        self.state.hide_help();
                        continue;
                    }

                    match (key.code, key.modifiers) {
                        // Quit: Ctrl+C only
                        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                            let _ = self.cmd_tx.send(TuiCommand::Quit);
                            return Ok(());
                        }
                        // Escape: unfocus logs/history or do nothing
                        (KeyCode::Esc, _) => {
                            if self.state.is_logs_focused() {
                                self.state.toggle_logs_focus();
                            } else if self.state.is_history_focused() {
                                self.state.toggle_history_focus();
                            }
                        }
                        // Help: ?
                        (KeyCode::Char('?'), _) => {
                            self.state.toggle_help();
                        }
                        // Command mode: :
                        (KeyCode::Char(':'), _) => {
                            self.state.enter_command_mode();
                        }
                        // Pause: p or space (only when logs not focused)
                        (KeyCode::Char('p'), _) | (KeyCode::Char(' '), _) => {
                            if !self.state.is_logs_focused() && !self.state.is_history_focused() {
                                let _ = self.cmd_tx.send(TuiCommand::TogglePause);
                            }
                        }
                        // Toggle idle mode: i (temporary, not saved to config)
                        (KeyCode::Char('i'), KeyModifiers::NONE) => {
                            if !self.state.is_logs_focused() && !self.state.is_history_focused() {
                                let enabled = self.state.toggle_idle_enabled();
                                let status = if enabled { "enabled" } else { "disabled" };
                                self.state.log_info(format!("Idle mode {}", status));
                                let _ = self.cmd_tx.send(TuiCommand::ToggleIdle);
                            }
                        }
                        // Focus/unfocus logs: l
                        (KeyCode::Char('l'), KeyModifiers::NONE) => {
                            if !self.state.is_history_focused() {
                                self.state.toggle_logs_focus();
                            }
                        }
                        // Focus/unfocus history: Shift+H
                        (KeyCode::Char('H'), KeyModifiers::SHIFT) => {
                            self.state.toggle_history_focus();
                        }
                        // Clear logs: Shift+C (only in focused mode)
                        (KeyCode::Char('C'), KeyModifiers::SHIFT) => {
                            if self.state.is_logs_focused() {
                                let _ = self.cmd_tx.send(TuiCommand::ClearLogs);
                                self.state.clear_logs();
                            }
                        }
                        // Toggle Bitcoin Core logs: Shift+L (when Bitcoin Core is running)
                        (KeyCode::Char('L'), KeyModifiers::SHIFT) | (KeyCode::Char('L'), KeyModifiers::NONE) => {
                            if self.state.is_bitcoin_managed() || self.state.is_connected() || self.state.is_syncing() || self.state.is_downloading() {
                                self.state.toggle_bitcoin_logs();
                                let _ = self.cmd_tx.send(TuiCommand::ToggleBitcoinLogs);
                            }
                        }
                        // Toggle autoscroll: s (in focused mode)
                        (KeyCode::Char('s'), KeyModifiers::NONE) => {
                            if self.state.is_logs_focused() {
                                self.state.toggle_autoscroll();
                            }
                        }
                        // Time filters: 0-6 (in logs or history focused mode)
                        (KeyCode::Char(c @ '0'..='6'), KeyModifiers::NONE) => {
                            if self.state.is_logs_focused() {
                                if let Some(filter) = logs::LogTimeFilter::from_key(c) {
                                    self.state.set_log_time_filter(filter);
                                }
                            } else if self.state.is_history_focused() {
                                if let Some(filter) = logs::LogTimeFilter::from_key(c) {
                                    self.state.set_history_time_filter(filter);
                                }
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
                        (KeyCode::Home, _) | (KeyCode::Char('g'), _) => {
                            self.state.scroll_to_top();
                        }
                        (KeyCode::End, _) | (KeyCode::Char('G'), KeyModifiers::SHIFT) => {
                            self.state.scroll_to_bottom();
                        }
                        // Toggle hashrate display mode: h (only when not focused on anything)
                        (KeyCode::Char('h'), KeyModifiers::NONE) => {
                            if !self.state.is_logs_focused() && !self.state.is_history_focused() {
                                let mode = self.state.toggle_hashrate_display_mode();
                                self.state.log_info(format!("Hashrate display: {}", mode.as_str()));
                            }
                        }
                        // Toggle overlay window: o
                        (KeyCode::Char('o'), KeyModifiers::NONE) => {
                            if !self.state.is_logs_focused() && !self.state.is_history_focused() {
                                if self.state.overlay_available() {
                                    let running = self.state.toggle_overlay();
                                    let status = if running { "opened" } else { "closed" };
                                    self.state.log_info(format!("Overlay {}", status));
                                } else {
                                    self.state.log_warn("Overlay not available (build with --features overlay)".to_string());
                                }
                            }
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
