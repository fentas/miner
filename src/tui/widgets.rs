use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Sparkline},
    Frame,
};

use std::time::Instant;
use super::state::{LogLevel, TuiState};
use crate::stats::format_hashrate;

const DEFAULT_LOG_TAIL: usize = 100;

// TUI rendering functions are excluded from coverage as they are
// difficult to test meaningfully (visual output verification)
#[cfg(not(tarpaulin_include))]
pub fn draw_ui(f: &mut Frame, state: &TuiState) {
    let size = f.area();

    // Help overlay takes priority
    if state.is_help_visible() {
        draw_help_overlay(f, size);
        return;
    }

    // Check if logs are focused - show full screen logs view
    if state.is_logs_focused() {
        draw_focused_logs(f, size, state);
        return;
    }

    // Main layout: header, body, footer, command
    // Footer only shown when downloading/syncing
    let footer_height = if state.is_downloading() || state.is_syncing() { 3 } else { 0 };
    let cmd_height = if state.is_command_mode() { 1 } else { 0 };
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),  // Header (k9s style with info + shortcuts)
            Constraint::Min(10),    // Body
            Constraint::Length(footer_height),  // Footer (progress bar only)
            Constraint::Length(cmd_height),  // Command input
        ])
        .split(size);

    draw_header(f, main_chunks[0], state);
    draw_body(f, main_chunks[1], state);
    draw_footer(f, main_chunks[2], state);

    if state.is_command_mode() {
        draw_command_input(f, main_chunks[3], state);
    }
}

#[cfg(not(tarpaulin_include))]
fn draw_help_overlay(f: &mut Frame, area: Rect) {
    use ratatui::widgets::Clear;

    let cyan = Style::default().fg(Color::Cyan);
    let magenta = Style::default().fg(Color::Magenta);
    let yellow = Style::default().fg(Color::Yellow);
    let action = Style::default().fg(Color::DarkGray);
    let header = Style::default().fg(Color::White).add_modifier(Modifier::BOLD);

    let help_lines = vec![
        Line::from(vec![Span::styled("GENERAL", header)]),
        Line::from(vec![
            Span::styled("<?>", cyan), Span::styled(" Help", action),
            Span::raw("           "),
            Span::styled("<:>", magenta), Span::styled(" Command Mode", action),
        ]),
        Line::from(vec![
            Span::styled("<p>", cyan), Span::styled(" Pause", action),
            Span::raw("          "),
            Span::styled("<l>", magenta), Span::styled(" Logs", action),
        ]),
        Line::from(vec![
            Span::styled("<Shift+l>", cyan), Span::styled(" Toggle Core Logs", action),
        ]),
        Line::from(vec![
            Span::styled("<h>", cyan), Span::styled(" Toggle Hashrate View (Total/CPU/GPU)", action),
        ]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::styled("NAVIGATION", header)]),
        Line::from(vec![
            Span::styled("<j>", cyan), Span::styled(" Down", action),
            Span::raw("           "),
            Span::styled("<k>", magenta), Span::styled(" Up", action),
        ]),
        Line::from(vec![
            Span::styled("<g>", cyan), Span::styled(" Top", action),
            Span::raw("            "),
            Span::styled("<G>", magenta), Span::styled(" Bottom", action),
        ]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::styled("LOGS (focused)", header)]),
        Line::from(vec![
            Span::styled("<0>", cyan), Span::styled(" tail", action),
            Span::raw("   "),
            Span::styled("<1>", magenta), Span::styled(" 1m", action),
            Span::raw("   "),
            Span::styled("<2>", cyan), Span::styled(" 5m", action),
            Span::raw("   "),
            Span::styled("<3>", magenta), Span::styled(" 15m", action),
        ]),
        Line::from(vec![
            Span::styled("<4>", cyan), Span::styled(" 30m", action),
            Span::raw("   "),
            Span::styled("<5>", magenta), Span::styled(" 1h", action),
            Span::raw("   "),
            Span::styled("<6>", cyan), Span::styled(" all", action),
        ]),
        Line::from(vec![
            Span::styled("<s>", yellow), Span::styled(" AutoScroll", action),
            Span::raw("   "),
            Span::styled("<Shift+c>", yellow), Span::styled(" Clear", action),
        ]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::styled("COMMANDS", header)]),
        Line::from(vec![
            Span::styled(":q", cyan), Span::styled(" Quit", action),
            Span::raw("          "),
            Span::styled(":clear", magenta), Span::styled(" Clear Logs", action),
        ]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::styled("Press any key to close", Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC))]),
    ];

    let help_width = 50;
    let help_height = help_lines.len() as u16 + 2;
    let help_x = area.width.saturating_sub(help_width) / 2;
    let help_y = area.height.saturating_sub(help_height) / 2;
    let help_area = Rect::new(help_x, help_y, help_width.min(area.width), help_height.min(area.height));

    f.render_widget(Clear, help_area);
    let help_block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let help_paragraph = Paragraph::new(help_lines).block(help_block);
    f.render_widget(help_paragraph, help_area);
}

#[cfg(not(tarpaulin_include))]
fn draw_command_input(f: &mut Frame, area: Rect, state: &TuiState) {
    let cmd = state.get_command();
    let line = Line::from(vec![
        Span::styled(":", Style::default().fg(Color::Yellow)),
        Span::raw(&cmd),
        Span::styled("_", Style::default().fg(Color::White).add_modifier(Modifier::SLOW_BLINK)),
    ]);
    let paragraph = Paragraph::new(line);
    f.render_widget(paragraph, area);
}

#[cfg(not(tarpaulin_include))]
fn draw_focused_logs(f: &mut Frame, area: Rect, state: &TuiState) {
    // Layout: full header (with shortcuts) + logs panel + command input
    let cmd_height = if state.is_command_mode() { 1 } else { 0 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),  // Full header with shortcuts
            Constraint::Min(5),     // Logs
            Constraint::Length(cmd_height), // Command input
        ])
        .split(area);

    // Draw full header with shortcuts (same as main view)
    draw_header(f, chunks[0], state);

    // Draw logs panel (full width)
    let showing_bitcoin = state.is_showing_bitcoin_logs();
    let all_logs = if showing_bitcoin {
        state.get_bitcoin_logs()
    } else {
        state.get_logs()
    };
    let scroll = state.get_log_scroll();
    let filter = state.get_log_time_filter();

    // Apply time filter
    let now = Instant::now();
    let filtered_logs: Vec<_> = if filter.is_tail() {
        // Tail: show last N logs
        let skip = all_logs.len().saturating_sub(DEFAULT_LOG_TAIL);
        all_logs.into_iter().skip(skip).collect()
    } else if let Some(max_age) = filter.max_age() {
        // Time-based filter
        all_logs.into_iter()
            .filter(|entry| now.duration_since(entry.created_at) <= max_age)
            .collect()
    } else {
        // All logs
        all_logs
    };

    let log_items: Vec<ListItem> = filtered_logs
        .iter()
        .skip(scroll)
        .map(|entry| {
            let (level_color, level_str) = match entry.level {
                LogLevel::Info => (Color::Blue, "INFO"),
                LogLevel::Warn => (Color::Yellow, "WARN"),
                LogLevel::Error => (Color::Red, "ERR "),
                LogLevel::Success => (Color::Green, " OK "),
                LogLevel::Debug => (Color::Gray, "DBG "),
            };

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{} ", entry.timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("[{}] ", level_str),
                    Style::default().fg(level_color),
                ),
                Span::raw(&entry.message),
            ]))
        })
        .collect();

    let log_count = filtered_logs.len();
    let filter_str = filter.as_str();
    let title = if showing_bitcoin {
        format!(" Bitcoin Core Logs ({}) [{}] ", log_count, filter_str)
    } else {
        format!(" Logs ({}) [{}] ", log_count, filter_str)
    };

    let logs_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White));

    let logs_list = List::new(log_items).block(logs_block);
    f.render_widget(logs_list, chunks[1]);

    // Command input
    if state.is_command_mode() {
        draw_command_input(f, chunks[2], state);
    }
}

#[cfg(not(tarpaulin_include))]
fn draw_header(f: &mut Frame, area: Rect, state: &TuiState) {
    // Split header into left (info) and right (shortcuts)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    draw_header_info(f, chunks[0], state);
    draw_header_shortcuts(f, chunks[1], state);
}

#[cfg(not(tarpaulin_include))]
fn draw_header_info(f: &mut Frame, area: Rect, state: &TuiState) {
    let mode = state.get_mode();
    let simd = state.get_simd_level();
    let workers = state.get_worker_count();
    let web_url = state.get_web_url();

    let status_color = if state.is_connected() {
        Color::Green
    } else if state.is_syncing() {
        Color::Yellow
    } else {
        Color::Red
    };

    let status_text = if state.is_syncing() {
        format!("Syncing {:.1}%", state.get_sync_progress())
    } else if state.is_connected() {
        "Connected".to_string()
    } else {
        "Disconnected".to_string()
    };

    let paused_text = if state.is_paused() { " [PAUSED]" } else { "" };

    // GPU info
    let gpu_configured = state.is_gpu_configured();
    let gpu_available = state.is_gpu_available();
    let gpu_devices = state.get_gpu_devices();

    let (gpu_text, gpu_color) = if gpu_available && !gpu_devices.is_empty() {
        (format!("{} GPU(s)", gpu_devices.len()), Color::Green)
    } else if gpu_configured && !gpu_available {
        ("N/A".to_string(), Color::Red)
    } else if !gpu_configured {
        ("Off".to_string(), Color::DarkGray)
    } else {
        ("No devices".to_string(), Color::Yellow)
    };

    let label_style = Style::default().fg(Color::DarkGray);
    let value_style = Style::default().fg(Color::Cyan);

    let mut lines = vec![
        Line::from(vec![
            Span::styled("BTC Miner: ", label_style),
            Span::styled(format!("v{}", env!("CARGO_PKG_VERSION")), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(paused_text, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Mode:      ", label_style),
            Span::styled(&mode, value_style),
        ]),
        Line::from(vec![
            Span::styled("Status:    ", label_style),
            Span::styled(status_text, Style::default().fg(status_color)),
        ]),
        Line::from(vec![
            Span::styled("Compute:   ", label_style),
            Span::styled(format!("{} CPU ({})", workers, simd), value_style),
            Span::styled(" + ", label_style),
            Span::styled(&gpu_text, Style::default().fg(gpu_color)),
        ]),
    ];

    // Add web URL if present
    if !web_url.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Web:       ", label_style),
            Span::styled(&web_url, Style::default().fg(Color::Blue)),
        ]));
    }

    let info = Paragraph::new(lines);
    f.render_widget(info, area);
}

#[cfg(not(tarpaulin_include))]
fn draw_header_shortcuts(f: &mut Frame, area: Rect, state: &TuiState) {
    // k9s-style: two separate bordered boxes for shortcuts
    let cyan = Style::default().fg(Color::Cyan);
    let magenta = Style::default().fg(Color::Magenta);
    let action_style = Style::default().fg(Color::DarkGray);

    // Split into two boxes side by side
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    if state.is_logs_focused() {
        // Focused logs mode: Time filters box + Actions box
        let filter = state.get_log_time_filter();
        let has_bitcoin = state.is_bitcoin_managed() || state.is_connected() || state.is_syncing() || state.is_downloading();

        // Time filters box (left)
        let filter_lines = vec![
            Line::from(vec![
                Span::styled("<0>", if filter.as_str() == "tail" { Style::default().fg(Color::Yellow) } else { cyan }),
                Span::styled(" tail ", action_style),
                Span::styled("<1>", if filter.as_str() == "1m" { Style::default().fg(Color::Yellow) } else { magenta }),
                Span::styled(" 1m", action_style),
            ]),
            Line::from(vec![
                Span::styled("<2>", if filter.as_str() == "5m" { Style::default().fg(Color::Yellow) } else { cyan }),
                Span::styled(" 5m   ", action_style),
                Span::styled("<3>", if filter.as_str() == "15m" { Style::default().fg(Color::Yellow) } else { magenta }),
                Span::styled(" 15m", action_style),
            ]),
            Line::from(vec![
                Span::styled("<4>", if filter.as_str() == "30m" { Style::default().fg(Color::Yellow) } else { cyan }),
                Span::styled(" 30m  ", action_style),
                Span::styled("<5>", if filter.as_str() == "1h" { Style::default().fg(Color::Yellow) } else { magenta }),
                Span::styled(" 1h", action_style),
            ]),
            Line::from(vec![
                Span::styled("<6>", if filter.as_str() == "all" { Style::default().fg(Color::Yellow) } else { cyan }),
                Span::styled(" all", action_style),
            ]),
        ];
        let filter_block = Block::default()
            .title(" Filters ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
        let filter_para = Paragraph::new(filter_lines).block(filter_block);
        f.render_widget(filter_para, chunks[0]);

        // Actions box (right)
        let mut action_lines = vec![
            Line::from(vec![
                Span::styled("<esc>", cyan),
                Span::styled(" Unfocus", action_style),
            ]),
            Line::from(vec![
                Span::styled("<s>", magenta),
                Span::styled(if state.is_autoscroll() { " Scroll:On" } else { " Scroll:Off" }, action_style),
            ]),
            Line::from(vec![
                Span::styled("<C>", cyan),
                Span::styled(" Clear", action_style),
            ]),
        ];
        if has_bitcoin {
            action_lines.push(Line::from(vec![
                Span::styled("<L>", magenta),
                Span::styled(if state.is_showing_bitcoin_logs() { " Miner" } else { " Core" }, action_style),
            ]));
        }
        let action_block = Block::default()
            .title(" Actions ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
        let action_para = Paragraph::new(action_lines).block(action_block);
        f.render_widget(action_para, chunks[1]);
    } else {
        // Normal mode: Shortcuts box + empty or Bitcoin box
        let shortcut_lines = vec![
            Line::from(vec![
                Span::styled("<?>", cyan),
                Span::styled(" Help", action_style),
            ]),
            Line::from(vec![
                Span::styled("<:>", magenta),
                Span::styled(" Command", action_style),
            ]),
            Line::from(vec![
                Span::styled("<p>", cyan),
                Span::styled(" Pause", action_style),
            ]),
            Line::from(vec![
                Span::styled("<l>", magenta),
                Span::styled(" Logs", action_style),
            ]),
        ];
        let shortcut_block = Block::default()
            .title(" Shortcuts ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
        let shortcut_para = Paragraph::new(shortcut_lines).block(shortcut_block);
        f.render_widget(shortcut_para, chunks[0]);

        // Bitcoin/Actions box (right)
        if state.is_bitcoin_managed() || state.is_connected() || state.is_syncing() || state.is_downloading() {
            let bitcoin_lines = vec![
                Line::from(vec![
                    Span::styled("<L>", cyan),
                    Span::styled(if state.is_showing_bitcoin_logs() { " Miner Logs" } else { " Core Logs" }, action_style),
                ]),
            ];
            let bitcoin_block = Block::default()
                .title(" Bitcoin ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray));
            let bitcoin_para = Paragraph::new(bitcoin_lines).block(bitcoin_block);
            f.render_widget(bitcoin_para, chunks[1]);
        }
    }
}

#[cfg(not(tarpaulin_include))]
fn draw_body(f: &mut Frame, area: Rect, state: &TuiState) {
    // Split body into left (stats) and right (logs)
    // Left side: fixed width of 38 chars for stats panel
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(38), Constraint::Min(30)])
        .split(area);

    draw_stats_panel(f, chunks[0], state);
    draw_logs_panel(f, chunks[1], state);
}

#[cfg(not(tarpaulin_include))]
fn draw_stats_panel(f: &mut Frame, area: Rect, state: &TuiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Hashrate
            Constraint::Length(6),  // Hashrate graph
            Constraint::Length(6),  // Stats
            Constraint::Min(3),     // Connection info
        ])
        .split(area);

    // Hashrate display
    let cpu_hashrate = state.get_hashrate();
    let gpu_hashrate = state.get_gpu_hashrate();
    let total_hashrate = cpu_hashrate + gpu_hashrate;
    let display_mode = state.get_hashrate_display_mode();

    // Determine which hashrate to show based on display mode
    let (primary_hashrate, mode_label): (f64, String) = match display_mode {
        crate::tui::state::HashrateDisplayMode::Total => (total_hashrate, "Total".to_string()),
        crate::tui::state::HashrateDisplayMode::CpuOnly => (cpu_hashrate, "CPU".to_string()),
        crate::tui::state::HashrateDisplayMode::GpuOnly => (gpu_hashrate, "GPU".to_string()),
        crate::tui::state::HashrateDisplayMode::GpuDevice(idx) => {
            // Show per-device hashrate
            let device_rate = state.get_gpu_device_hashrate(idx as u32);
            (device_rate, format!("GPU#{}", idx))
        }
    };
    let hr_str = format_hashrate(primary_hashrate);

    let title = format!(" Hashrate [{}] (h) ", mode_label);
    let hashrate_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    // Show CPU/GPU breakdown if GPU is active
    let hashrate_lines = if gpu_hashrate > 0.0 {
        vec![
            Line::from(vec![
                Span::styled(
                    hr_str,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled(format!("CPU: {}", format_hashrate(cpu_hashrate)), Style::default().fg(Color::DarkGray)),
                Span::raw(" | "),
                Span::styled(format!("GPU: {}", format_hashrate(gpu_hashrate)), Style::default().fg(Color::Magenta)),
            ]),
        ]
    } else {
        vec![Line::from(vec![
            Span::styled(
                hr_str,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ])]
    };

    let hashrate_text = Paragraph::new(hashrate_lines)
        .alignment(Alignment::Center)
        .block(hashrate_block);
    f.render_widget(hashrate_text, chunks[0]);

    // Hashrate sparkline
    let history = state.get_hashrate_history();
    let max_hr = history.iter().cloned().fold(1.0_f64, f64::max);
    let sparkline_data: Vec<u64> = history
        .iter()
        .map(|&h| ((h / max_hr) * 100.0) as u64)
        .collect();

    let sparkline = Sparkline::default()
        .block(
            Block::default()
                .title(" History (2 min) ")
                .borders(Borders::ALL),
        )
        .data(&sparkline_data)
        .style(Style::default().fg(Color::Green))
        .bar_set(symbols::bar::NINE_LEVELS);
    f.render_widget(sparkline, chunks[1]);

    // Mining stats
    let (accepted, rejected) = state.get_shares();
    let total_hashes = state.get_total_hashes();
    let uptime = state.get_uptime();
    let uptime_str = format_duration(uptime);
    let last_share = state
        .get_last_share_ago()
        .map(|d| format_duration(d))
        .unwrap_or_else(|| "N/A".to_string());

    let stats_block = Block::default()
        .title(" Statistics ")
        .borders(Borders::ALL);

    let stats_text = vec![
        Line::from(vec![
            Span::raw("Shares: "),
            Span::styled(accepted.to_string(), Style::default().fg(Color::Green)),
            Span::raw(" / "),
            Span::styled(rejected.to_string(), Style::default().fg(Color::Red)),
        ]),
        Line::from(vec![
            Span::raw("Total Hashes: "),
            Span::styled(format_large_number(total_hashes), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("Uptime: "),
            Span::styled(uptime_str, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("Last Share: "),
            Span::styled(last_share, Style::default().fg(Color::White)),
        ]),
    ];

    let stats = Paragraph::new(stats_text).block(stats_block);
    f.render_widget(stats, chunks[2]);

    // Connection info
    let pool = state.get_pool_address();
    let block_height = state.get_block_height();
    let difficulty = state.get_difficulty();
    let peer_count = state.get_peer_count();

    let conn_block = Block::default()
        .title(" Network ")
        .borders(Borders::ALL);

    let mut conn_text = vec![
        Line::from(vec![
            Span::raw("Pool: "),
            Span::styled(
                if pool.len() > 30 {
                    format!("{}...", &pool[..30])
                } else {
                    pool
                },
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::raw("Block: "),
            Span::styled(
                block_height.to_string(),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::raw("Difficulty: "),
            Span::styled(
                format_large_number(difficulty),
                Style::default().fg(Color::Magenta),
            ),
        ]),
    ];

    // Add peers if in Bitcoin Core mode
    if state.is_bitcoin_managed() {
        conn_text.push(Line::from(vec![
            Span::raw("Peers: "),
            Span::styled(
                peer_count.to_string(),
                Style::default().fg(Color::Green),
            ),
        ]));
    }

    let conn = Paragraph::new(conn_text).block(conn_block);
    f.render_widget(conn, chunks[3]);
}

#[cfg(not(tarpaulin_include))]
fn draw_logs_panel(f: &mut Frame, area: Rect, state: &TuiState) {
    let showing_bitcoin = state.is_showing_bitcoin_logs();
    let logs = if showing_bitcoin {
        state.get_bitcoin_logs()
    } else {
        state.get_logs()
    };
    let scroll = state.get_log_scroll();

    let log_items: Vec<ListItem> = logs
        .iter()
        .skip(scroll)
        .map(|entry| {
            let (level_color, level_str) = match entry.level {
                LogLevel::Info => (Color::Blue, "INFO"),
                LogLevel::Warn => (Color::Yellow, "WARN"),
                LogLevel::Error => (Color::Red, "ERR "),
                LogLevel::Success => (Color::Green, " OK "),
                LogLevel::Debug => (Color::Gray, "DBG "),
            };

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{} ", entry.timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("[{}] ", level_str),
                    Style::default().fg(level_color),
                ),
                Span::raw(&entry.message),
            ]))
        })
        .collect();

    let log_count = logs.len();
    let title = if showing_bitcoin {
        if log_count > 0 {
            format!(" Bitcoin Core Logs ({}) ", log_count)
        } else {
            " Bitcoin Core Logs ".to_string()
        }
    } else if log_count > 0 {
        format!(" Logs ({}) ", log_count)
    } else {
        " Logs ".to_string()
    };

    let logs_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White));

    let logs_list = List::new(log_items).block(logs_block);
    f.render_widget(logs_list, area);
}

#[cfg(not(tarpaulin_include))]
fn draw_footer(f: &mut Frame, area: Rect, state: &TuiState) {
    // Show download/sync progress bar if active
    if state.is_downloading() {
        let (current, total) = state.get_download_progress();
        let name = state.get_download_name();

        // Handle snapshot loading progress
        // Encoding: total contains packed (elapsed_secs << 16) | (pct_100 or 0xFFFF if unknown)
        // When pct_100 == 0xFFFF (65535), show only elapsed time
        // Otherwise show both percentage and elapsed time
        if name.contains("Snapshot") {
            let elapsed_secs = (total >> 16) as u64;
            let pct_100 = (total & 0xFFFF) as u16;

            let title = format!(" {} ", name);

            if pct_100 == 0xFFFF || pct_100 == 0 {
                // No percentage available, show elapsed time only
                let elapsed_str = if elapsed_secs > 0 {
                    format!("Loading... ({} elapsed)", format_duration(std::time::Duration::from_secs(elapsed_secs)))
                } else {
                    "Loading...".to_string()
                };
                let paragraph = Paragraph::new(elapsed_str)
                    .block(Block::default().title(title).borders(Borders::ALL))
                    .style(Style::default().fg(Color::Yellow))
                    .alignment(Alignment::Center);
                f.render_widget(paragraph, area);
            } else {
                // Show progress bar with percentage and elapsed time
                let pct = pct_100 as f64 / 100.0;
                let elapsed_str = format_duration(std::time::Duration::from_secs(elapsed_secs));
                let label = format!("{:.1}% ({} elapsed)", pct, elapsed_str);
                let gauge = Gauge::default()
                    .block(Block::default().title(title).borders(Borders::ALL))
                    .gauge_style(Style::default().fg(Color::Yellow))
                    .percent(pct.min(100.0) as u16)
                    .label(label);
                f.render_widget(gauge, area);
            }
            return;
        }

        // Handle indeterminate progress (total == 0) - e.g., other loading states
        if total == 0 {
            let title = format!(" {} ", name);
            let elapsed_str = if current > 0 {
                format!("Loading... ({} elapsed)", format_duration(std::time::Duration::from_secs(current)))
            } else {
                "Loading...".to_string()
            };
            let paragraph = Paragraph::new(elapsed_str)
                .block(Block::default().title(title).borders(Borders::ALL))
                .style(Style::default().fg(Color::Yellow))
                .alignment(Alignment::Center);
            f.render_widget(paragraph, area);
            return;
        }

        let percent = ((current as f64 / total as f64) * 100.0).min(100.0);

        // Use different format for headers sync vs file download
        let (title, label, color) = if name.contains("Headers") {
            (
                format!(" Syncing {} ", name),
                format!("{:.1}% ({} / {})", percent, format_large_number(current), format_large_number(total)),
                Color::Yellow
            )
        } else {
            (
                format!(" Downloading {} ", name),
                format!("{:.1}% ({:.1} GB / {:.1} GB)", percent, current as f64 / 1_000_000_000.0, total as f64 / 1_000_000_000.0),
                Color::Cyan
            )
        };

        let gauge = Gauge::default()
            .block(Block::default().title(title).borders(Borders::ALL))
            .gauge_style(Style::default().fg(color))
            .percent(percent as u16)
            .label(label);
        f.render_widget(gauge, area);
    } else if state.is_syncing() {
        let progress = state.get_sync_progress();
        let gauge = Gauge::default()
            .block(Block::default().title(" Syncing Blockchain ").borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Yellow))
            .percent((progress) as u16)
            .label(format!("{:.2}%", progress));
        f.render_widget(gauge, area);
    }
    // No footer when not downloading/syncing (shortcuts are in header)
}

fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86400, (secs % 86400) / 3600)
    }
}

fn format_large_number(n: u64) -> String {
    if n >= 1_000_000_000_000 {
        format!("{:.2}T", n as f64 / 1_000_000_000_000.0)
    } else if n >= 1_000_000_000 {
        format!("{:.2}G", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.2}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_format_duration_seconds() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0s");
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(59)), "59s");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(Duration::from_secs(60)), "1m 0s");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_duration(Duration::from_secs(3599)), "59m 59s");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(Duration::from_secs(3600)), "1h 0m");
        assert_eq!(format_duration(Duration::from_secs(7200)), "2h 0m");
        assert_eq!(format_duration(Duration::from_secs(3665)), "1h 1m");
        assert_eq!(format_duration(Duration::from_secs(86399)), "23h 59m");
    }

    #[test]
    fn test_format_duration_days() {
        assert_eq!(format_duration(Duration::from_secs(86400)), "1d 0h");
        assert_eq!(format_duration(Duration::from_secs(172800)), "2d 0h");
        assert_eq!(format_duration(Duration::from_secs(90000)), "1d 1h");
    }

    #[test]
    fn test_format_large_number_plain() {
        assert_eq!(format_large_number(0), "0");
        assert_eq!(format_large_number(100), "100");
        assert_eq!(format_large_number(999), "999");
    }

    #[test]
    fn test_format_large_number_thousands() {
        assert_eq!(format_large_number(1_000), "1.00K");
        assert_eq!(format_large_number(1_500), "1.50K");
        assert_eq!(format_large_number(999_999), "1000.00K");
    }

    #[test]
    fn test_format_large_number_millions() {
        assert_eq!(format_large_number(1_000_000), "1.00M");
        assert_eq!(format_large_number(2_500_000), "2.50M");
        assert_eq!(format_large_number(999_999_999), "1000.00M");
    }

    #[test]
    fn test_format_large_number_billions() {
        assert_eq!(format_large_number(1_000_000_000), "1.00G");
        assert_eq!(format_large_number(7_500_000_000), "7.50G");
    }

    #[test]
    fn test_format_large_number_trillions() {
        assert_eq!(format_large_number(1_000_000_000_000), "1.00T");
        assert_eq!(format_large_number(55_000_000_000_000), "55.00T");
    }
}
