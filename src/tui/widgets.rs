use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Sparkline},
    Frame,
};

use super::state::{LogLevel, TuiState};
use crate::stats::format_hashrate;

const BANNER: &str = r#" ____  _____ ____    __  __ _
| __ )|_   _/ ___|  |  \/  (_)_ __   ___ _ __
|  _ \  | || |      | |\/| | | '_ \ / _ \ '__|
| |_) | | || |___   | |  | | | | | |  __/ |
|____/  |_| \____|  |_|  |_|_|_| |_|\___|_|   "#;

pub fn draw_ui(f: &mut Frame, state: &TuiState) {
    let size = f.area();

    // Main layout: header, body, footer
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),  // Header with banner
            Constraint::Min(10),    // Body
            Constraint::Length(3),  // Footer
        ])
        .split(size);

    draw_header(f, main_chunks[0], state);
    draw_body(f, main_chunks[1], state);
    draw_footer(f, main_chunks[2], state);
}

fn draw_header(f: &mut Frame, area: Rect, state: &TuiState) {
    // Split header into banner and status
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(50), Constraint::Min(30)])
        .split(area);

    // Banner
    let banner_text: Vec<Line> = BANNER
        .lines()
        .map(|l| Line::from(Span::styled(l, Style::default().fg(Color::Yellow))))
        .collect();
    let banner = Paragraph::new(banner_text);
    f.render_widget(banner, chunks[0]);

    // Status info
    let mode = state.get_mode();
    let simd = state.get_simd_level();
    let workers = state.get_worker_count();

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

    let paused_indicator = if state.is_paused() {
        Span::styled(" [PAUSED]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    } else {
        Span::raw("")
    };

    // GPU info
    let gpu_enabled = state.is_gpu_enabled();
    let gpu_devices = state.get_gpu_devices();
    let gpu_text = if gpu_enabled && !gpu_devices.is_empty() {
        format!("{} GPU(s)", gpu_devices.len())
    } else if gpu_enabled {
        "Enabled (no devices)".to_string()
    } else {
        "Disabled".to_string()
    };
    let gpu_color = if gpu_enabled && !gpu_devices.is_empty() {
        Color::Green
    } else if gpu_enabled {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let status_lines = vec![
        Line::from(vec![
            Span::styled(
                format!("v{}", env!("CARGO_PKG_VERSION")),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
            paused_indicator,
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Mode: "),
            Span::styled(&mode, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("SIMD: "),
            Span::styled(&simd, Style::default().fg(Color::Cyan)),
            Span::raw("  GPU: "),
            Span::styled(gpu_text, Style::default().fg(gpu_color)),
        ]),
        Line::from(vec![
            Span::raw("Workers: "),
            Span::styled(workers.to_string(), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("Status: "),
            Span::styled(status_text, Style::default().fg(status_color)),
        ]),
    ];

    let status = Paragraph::new(status_lines)
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(status, chunks[1]);
}

fn draw_body(f: &mut Frame, area: Rect, state: &TuiState) {
    // Split body into left (stats) and right (logs)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    draw_stats_panel(f, chunks[0], state);
    draw_logs_panel(f, chunks[1], state);
}

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
    let hr_str = format_hashrate(total_hashrate);

    let hashrate_block = Block::default()
        .title(" Hashrate ")
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

    let conn_block = Block::default()
        .title(" Network ")
        .borders(Borders::ALL);

    let conn_text = vec![
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

    let conn = Paragraph::new(conn_text).block(conn_block);
    f.render_widget(conn, chunks[3]);
}

fn draw_logs_panel(f: &mut Frame, area: Rect, state: &TuiState) {
    let logs = state.get_logs();
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
    let title = if log_count > 0 {
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

fn draw_footer(f: &mut Frame, area: Rect, state: &TuiState) {
    // Sync progress bar if syncing
    if state.is_syncing() {
        let progress = state.get_sync_progress();
        let gauge = Gauge::default()
            .block(Block::default().title(" Syncing Blockchain ").borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Yellow))
            .percent((progress) as u16)
            .label(format!("{:.2}%", progress));
        f.render_widget(gauge, area);
    } else {
        // Key bindings
        let help_text = vec![Line::from(vec![
            Span::styled(" q ", Style::default().fg(Color::Black).bg(Color::White)),
            Span::raw(" Quit  "),
            Span::styled(" p ", Style::default().fg(Color::Black).bg(Color::White)),
            Span::raw(" Pause  "),
            Span::styled(" c ", Style::default().fg(Color::Black).bg(Color::White)),
            Span::raw(" Clear Logs  "),
            Span::styled(" ↑↓ ", Style::default().fg(Color::Black).bg(Color::White)),
            Span::raw(" Scroll  "),
            Span::styled(" PgUp/PgDn ", Style::default().fg(Color::Black).bg(Color::White)),
            Span::raw(" Page Scroll"),
        ])];

        let help = Paragraph::new(help_text)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(help, area);
    }
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
