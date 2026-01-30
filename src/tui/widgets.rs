use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Borders, Chart, Dataset, Gauge, GraphType, List, ListItem, Paragraph, Sparkline},
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

    // Check if history is focused - show expanded hashrate chart
    if state.is_history_focused() {
        draw_focused_history(f, size, state);
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
        Line::from(vec![
            Span::styled("<Shift+h>", cyan), Span::styled(" Hashrate History (expanded chart)", action),
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

    // Calculate available lines (accounting for border)
    let available_lines = chunks[1].height.saturating_sub(2) as usize;

    // Autoscroll: show most recent logs if enabled
    let scroll = if state.is_autoscroll() {
        filtered_logs.len().saturating_sub(available_lines)
    } else {
        state.get_log_scroll()
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
    let autoscroll_str = if state.is_autoscroll() { "Autoscroll:On" } else { "Autoscroll:Off" };
    let log_type = if showing_bitcoin { "Bitcoin Core" } else { "Miner" };

    // k9s style: title on left with count, status on right
    let title = format!(" {} Logs ({}) [{}] ", log_type, log_count, filter_str);
    let title_right = format!(" {}  FullScreen ", autoscroll_str);

    let logs_block = Block::default()
        .title(title)
        .title_bottom(Line::from(title_right).alignment(Alignment::Right))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White));

    let logs_list = List::new(log_items).block(logs_block);
    f.render_widget(logs_list, chunks[1]);

    // Command input
    if state.is_command_mode() {
        draw_command_input(f, chunks[2], state);
    }
}

/// Draw focused history view with expanded chart
#[cfg(not(tarpaulin_include))]
fn draw_focused_history(f: &mut Frame, area: Rect, state: &TuiState) {
    let cmd_height = if state.is_command_mode() { 1 } else { 0 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),  // Header (includes filters and actions)
            Constraint::Min(8),     // Chart (reduced height)
            Constraint::Length(cmd_height),
        ])
        .split(area);

    draw_header(f, chunks[0], state);

    // Get filtered samples
    let samples = state.get_filtered_history_samples();
    let filter = state.get_history_time_filter();

    if samples.is_empty() {
        let empty_msg = Paragraph::new("No hashrate data yet...")
            .alignment(Alignment::Center)
            .block(Block::default()
                .title(" Hashrate History ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)));
        f.render_widget(empty_msg, chunks[1]);
    } else {
        // Build datasets for chart
        let now = Instant::now();
        let oldest = samples.first().map(|s| s.timestamp).unwrap_or(now);
        let time_range = now.duration_since(oldest).as_secs_f64().max(1.0);

        // Convert samples to chart data points (x=time, y=hashrate)
        let total_data: Vec<(f64, f64)> = samples.iter()
            .map(|s| {
                let x = now.duration_since(s.timestamp).as_secs_f64();
                let x_normalized = time_range - x; // Reverse so oldest is at left
                (x_normalized, s.total_hashrate / 1_000_000.0) // Convert to MH/s
            })
            .collect();

        let cpu_data: Vec<(f64, f64)> = samples.iter()
            .map(|s| {
                let x = now.duration_since(s.timestamp).as_secs_f64();
                let x_normalized = time_range - x;
                (x_normalized, s.cpu_hashrate / 1_000_000.0)
            })
            .collect();

        let gpu_data: Vec<(f64, f64)> = samples.iter()
            .map(|s| {
                let x = now.duration_since(s.timestamp).as_secs_f64();
                let x_normalized = time_range - x;
                (x_normalized, s.gpu_hashrate / 1_000_000.0)
            })
            .collect();

        // Find block markers
        let block_markers: Vec<(f64, u64)> = samples.iter()
            .filter(|s| s.new_block)
            .map(|s| {
                let x = now.duration_since(s.timestamp).as_secs_f64();
                let x_normalized = time_range - x;
                (x_normalized, s.block_height)
            })
            .collect();

        // Calculate Y axis bounds (consider total, CPU, and GPU separately)
        let max_hashrate = samples.iter()
            .map(|s| s.total_hashrate.max(s.cpu_hashrate).max(s.gpu_hashrate))
            .fold(0.0_f64, f64::max) / 1_000_000.0;
        let y_max = (max_hashrate * 1.1).max(1.0); // 10% headroom

        // Build datasets
        let mut datasets = vec![
            Dataset::default()
                .name("Total")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::White))
                .data(&total_data),
        ];

        // Only add CPU/GPU datasets if we have both
        let has_gpu = samples.iter().any(|s| s.gpu_hashrate > 0.0);
        if has_gpu {
            datasets.push(
                Dataset::default()
                    .name("CPU")
                    .marker(symbols::Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(Color::Cyan))
                    .data(&cpu_data)
            );
            datasets.push(
                Dataset::default()
                    .name("GPU")
                    .marker(symbols::Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(Color::Green))
                    .data(&gpu_data)
            );
        }

        // Format X axis labels based on time range (left=oldest, right=now)
        let x_labels: Vec<Span> = if time_range <= 120.0 {
            vec![
                Span::raw(format!("-{:.0}s", time_range)),
                Span::raw(format!("-{:.0}s", time_range / 2.0)),
                Span::raw("now"),
            ]
        } else if time_range <= 3600.0 {
            vec![
                Span::raw(format!("-{:.0}m", time_range / 60.0)),
                Span::raw(format!("-{:.0}m", time_range / 120.0)),
                Span::raw("now"),
            ]
        } else {
            vec![
                Span::raw(format!("-{:.1}h", time_range / 3600.0)),
                Span::raw(format!("-{:.1}h", time_range / 7200.0)),
                Span::raw("now"),
            ]
        };

        let x_axis = Axis::default()
            .style(Style::default().fg(Color::Gray))
            .bounds([0.0, time_range])
            .labels(x_labels);

        let y_axis = Axis::default()
            .style(Style::default().fg(Color::Gray))
            .bounds([0.0, y_max])
            .labels(vec![
                Span::raw("0"),
                Span::raw(format!("{:.1}", y_max / 2.0)),
                Span::raw(format!("{:.1} MH/s", y_max)),
            ]);

        // Build title with filter info
        let filter_str = filter.as_str();
        let sample_count = samples.len();
        let block_count = block_markers.len();
        let title = if block_count > 0 {
            format!(" Hashrate History ({} samples, {} blocks) [{}] ", sample_count, block_count, filter_str)
        } else {
            format!(" Hashrate History ({} samples) [{}] ", sample_count, filter_str)
        };

        let chart = Chart::new(datasets)
            .block(Block::default()
                .title(title)
                .borders(Borders::NONE))
            .x_axis(x_axis)
            .y_axis(y_axis);

        f.render_widget(chart, chunks[1]);

        // Draw block markers as labels (vertical lines simulation)
        if !block_markers.is_empty() {
            let chart_area = chunks[1];
            let inner_width = chart_area.width.saturating_sub(7) as f64; // Account for Y axis
            let inner_x_start = chart_area.x + 6;

            for (x_pos, height) in block_markers.iter().take(5) { // Max 5 markers
                let x_ratio = x_pos / time_range;
                let screen_x = inner_x_start + (x_ratio * inner_width) as u16;
                if screen_x > inner_x_start && screen_x < chart_area.x + chart_area.width {
                    // Draw a vertical marker
                    let marker_area = Rect::new(screen_x, chart_area.y, 1, chart_area.height.saturating_sub(2));
                    for y in marker_area.y..marker_area.y + marker_area.height {
                        f.render_widget(
                            Paragraph::new("│").style(Style::default().fg(Color::Yellow)),
                            Rect::new(screen_x, y, 1, 1)
                        );
                    }
                    // Draw height label at top
                    let label = format!("{}", height);
                    let label_len = label.len() as u16;
                    if screen_x + label_len <= chart_area.x + chart_area.width {
                        f.render_widget(
                            Paragraph::new(label).style(Style::default().fg(Color::Yellow)),
                            Rect::new(screen_x, chart_area.y, label_len, 1)
                        );
                    }
                }
            }
        }
    }

    if state.is_command_mode() {
        draw_command_input(f, chunks[2], state);
    }
}

#[cfg(not(tarpaulin_include))]
fn draw_header(f: &mut Frame, area: Rect, state: &TuiState) {
    // k9s style: Info | Filters | Actions (no borders, colored titles)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(42),  // Info section
            Constraint::Length(22),  // Filters section
            Constraint::Min(20),     // Actions section
        ])
        .split(area);

    draw_header_info(f, chunks[0], state);
    draw_header_filters(f, chunks[1], state);
    draw_header_actions(f, chunks[2], state);
}

#[cfg(not(tarpaulin_include))]
fn draw_header_info(f: &mut Frame, area: Rect, state: &TuiState) {
    let mode = state.get_mode();
    let simd = state.get_simd_level();
    let workers = state.get_worker_count();
    let web_url = state.get_web_url();

    let status_color = if state.is_syncing() {
        Color::Yellow
    } else if state.is_connected() {
        // Check if idle mode is preventing mining
        if state.is_idle_enabled() && !state.is_idle_active() && state.is_idle_cpu() {
            Color::Cyan  // Ready but waiting for idle
        } else if state.is_paused() {
            Color::Yellow  // Paused
        } else {
            Color::Green  // Mining
        }
    } else {
        Color::Red  // Disconnected
    };

    let status_text = if state.is_syncing() {
        format!("Syncing {:.1}%", state.get_sync_progress())
    } else if state.is_connected() {
        // Check if idle mode is preventing mining
        if state.is_idle_enabled() && !state.is_idle_active() && state.is_idle_cpu() {
            "Ready (idle waiting)".to_string()
        } else if state.is_paused() {
            "Mining [PAUSED]".to_string()
        } else {
            "Mining".to_string()
        }
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

    // k9s style: label in one color, value in another, aligned with spaces
    let label_style = Style::default().fg(Color::White);
    let value_style = Style::default().fg(Color::Cyan);

    let mut lines = vec![
        Line::from(vec![
            Span::styled("BTC Miner:", label_style),
            Span::raw("  "),
            Span::styled(format!("v{}", env!("CARGO_PKG_VERSION")), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(paused_text, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Mode:", label_style),
            Span::raw("       "),
            Span::styled(&mode, value_style),
        ]),
        Line::from(vec![
            Span::styled("Status:", label_style),
            Span::raw("     "),
            Span::styled(status_text, Style::default().fg(status_color)),
        ]),
        Line::from(vec![
            Span::styled("Compute:", label_style),
            Span::raw("    "),
            Span::styled(format!("{} CPU ({})", workers, simd), value_style),
            Span::raw(" + "),
            Span::styled(&gpu_text, Style::default().fg(gpu_color)),
        ]),
    ];

    // Add idle mode status if enabled
    if state.is_idle_enabled() {
        let idle_active = state.is_idle_active();
        let idle_cpu = state.is_idle_cpu();
        let idle_gpu = state.is_idle_gpu();

        let status_text = if idle_active {
            "Active (system idle)"
        } else {
            "Waiting"
        };
        let status_color = if idle_active { Color::Green } else { Color::Yellow };

        // Build target text (what follows idle state)
        let target_text = match (idle_cpu, idle_gpu) {
            (true, true) => "CPU+GPU",
            (true, false) => "CPU",
            (false, true) => "GPU",
            (false, false) => "None",
        };

        lines.push(Line::from(vec![
            Span::styled("Idle:", label_style),
            Span::raw("       "),
            Span::styled(status_text, Style::default().fg(status_color)),
            Span::raw(" ["),
            Span::styled(target_text, value_style),
            Span::raw("]"),
        ]));
    }

    // Add web URL if present
    if !web_url.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Web:", label_style),
            Span::raw("        "),
            Span::styled(&web_url, Style::default().fg(Color::Blue)),
        ]));
    }

    let info = Paragraph::new(lines);
    f.render_widget(info, area);
}

/// Draw Filters section (k9s style: magenta title, no border)
#[cfg(not(tarpaulin_include))]
fn draw_header_filters(f: &mut Frame, area: Rect, state: &TuiState) {
    let key_style = Style::default().fg(Color::Magenta);
    let desc_style = Style::default().fg(Color::DarkGray);
    let yellow = Style::default().fg(Color::Yellow);

    // Title line with magenta color (k9s style)
    let title_style = Style::default().fg(Color::Gray);

    if state.is_logs_focused() {
        // Log focused mode: show log time filters
        let filter = state.get_log_time_filter();
        let active = |name: &str| if filter.as_str() == name { yellow } else { key_style };

        let lines = vec![
            Line::from(vec![Span::styled("Filters ", title_style), Span::raw("─".repeat(12))]),
            Line::from(vec![
                Span::styled("<0>", active("tail")),
                Span::styled(" tail  ", desc_style),
                Span::styled("<1>", active("1m")),
                Span::styled(" 1m", desc_style),
            ]),
            Line::from(vec![
                Span::styled("<2>", active("5m")),
                Span::styled(" 5m    ", desc_style),
                Span::styled("<3>", active("15m")),
                Span::styled(" 15m", desc_style),
            ]),
            Line::from(vec![
                Span::styled("<4>", active("30m")),
                Span::styled(" 30m   ", desc_style),
                Span::styled("<5>", active("1h")),
                Span::styled(" 1h", desc_style),
            ]),
            Line::from(vec![
                Span::styled("<6>", active("all")),
                Span::styled(" all", desc_style),
            ]),
        ];
        let para = Paragraph::new(lines);
        f.render_widget(para, area);
    } else if state.is_history_focused() {
        // History focused mode: show history time filters
        let filter = state.get_history_time_filter();
        let active = |name: &str| if filter.as_str() == name { yellow } else { key_style };

        let lines = vec![
            Line::from(vec![Span::styled("Filters ", title_style), Span::raw("─".repeat(12))]),
            Line::from(vec![
                Span::styled("<0>", active("tail")),
                Span::styled(" tail  ", desc_style),
                Span::styled("<1>", active("1m")),
                Span::styled(" 1m", desc_style),
            ]),
            Line::from(vec![
                Span::styled("<2>", active("5m")),
                Span::styled(" 5m    ", desc_style),
                Span::styled("<3>", active("15m")),
                Span::styled(" 15m", desc_style),
            ]),
            Line::from(vec![
                Span::styled("<4>", active("30m")),
                Span::styled(" 30m   ", desc_style),
                Span::styled("<5>", active("1h")),
                Span::styled(" 1h", desc_style),
            ]),
            Line::from(vec![
                Span::styled("<6>", active("all")),
                Span::styled(" all", desc_style),
            ]),
        ];
        let para = Paragraph::new(lines);
        f.render_widget(para, area);
    } else {
        // Normal mode: show general shortcuts
        let lines = vec![
            Line::from(vec![Span::styled("Shortcuts ", title_style), Span::raw("─".repeat(10))]),
            Line::from(vec![
                Span::styled("<?>", key_style),
                Span::styled(" Help", desc_style),
            ]),
            Line::from(vec![
                Span::styled("<:>", key_style),
                Span::styled(" Command", desc_style),
            ]),
            Line::from(vec![
                Span::styled("<p>", key_style),
                Span::styled(" Pause", desc_style),
            ]),
            Line::from(vec![
                Span::styled("<l>", key_style),
                Span::styled(" Logs", desc_style),
            ]),
        ];
        let para = Paragraph::new(lines);
        f.render_widget(para, area);
    }
}

/// Draw Actions section (k9s style: blue title, no border)
#[cfg(not(tarpaulin_include))]
fn draw_header_actions(f: &mut Frame, area: Rect, state: &TuiState) {
    let key_style = Style::default().fg(Color::LightBlue);
    let desc_style = Style::default().fg(Color::DarkGray);

    // Title line with blue color (k9s style)
    let title_style = Style::default().fg(Color::Gray);
    let has_bitcoin = state.is_bitcoin_managed() || state.is_connected() || state.is_syncing() || state.is_downloading();

    if state.is_logs_focused() {
        // Log focused mode: show all log actions
        let scroll_status = if state.is_autoscroll() { "Scroll:On" } else { "Scroll:Off" };

        let mut lines = vec![
            Line::from(vec![Span::styled("Actions ", title_style), Span::raw("─".repeat(12))]),
            Line::from(vec![
                Span::styled("<esc>", key_style),
                Span::styled(" Unfocus", desc_style),
            ]),
            Line::from(vec![
                Span::styled("<s>", key_style),
                Span::styled(format!(" {}", scroll_status), desc_style),
            ]),
            Line::from(vec![
                Span::styled("<shift+c>", key_style),
                Span::styled(" Clear", desc_style),
            ]),
        ];

        if has_bitcoin {
            let log_label = if state.is_showing_bitcoin_logs() { " Miner" } else { " Core" };
            lines.push(Line::from(vec![
                Span::styled("<shift+l>", key_style),
                Span::styled(log_label, desc_style),
            ]));
        }

        let para = Paragraph::new(lines);
        f.render_widget(para, area);
    } else if state.is_history_focused() {
        // History focused mode: show history actions
        let lines = vec![
            Line::from(vec![Span::styled("Actions ", title_style), Span::raw("─".repeat(12))]),
            Line::from(vec![
                Span::styled("<esc>", key_style),
                Span::styled(" Unfocus", desc_style),
            ]),
            Line::from(vec![
                Span::styled("Total:", Style::default().fg(Color::White)),
                Span::styled("White", desc_style),
            ]),
            Line::from(vec![
                Span::styled("CPU:", Style::default().fg(Color::Cyan)),
                Span::styled("Cyan", desc_style),
                Span::raw(" "),
                Span::styled("GPU:", Style::default().fg(Color::Green)),
                Span::styled("Green", desc_style),
            ]),
            Line::from(vec![
                Span::styled("Blocks:", Style::default().fg(Color::Yellow)),
                Span::styled("Yellow", desc_style),
            ]),
        ];

        let para = Paragraph::new(lines);
        f.render_widget(para, area);
    } else {
        // Normal mode: show Bitcoin toggle if available
        let mut lines = vec![
            Line::from(vec![Span::styled("Actions ", title_style), Span::raw("─".repeat(12))]),
        ];

        if has_bitcoin {
            let log_label = if state.is_showing_bitcoin_logs() { " Miner Logs" } else { " Core Logs" };
            lines.push(Line::from(vec![
                Span::styled("<shift+l>", key_style),
                Span::styled(log_label, desc_style),
            ]));
        }

        // Always show hashrate toggle and history
        lines.push(Line::from(vec![
            Span::styled("<h>", key_style),
            Span::styled(" Toggle Hashrate", desc_style),
        ]));
        lines.push(Line::from(vec![
            Span::styled("<shift+h>", key_style),
            Span::styled(" History Chart", desc_style),
        ]));

        // Show overlay toggle if available
        if state.overlay_available() {
            let overlay_label = if state.is_overlay_running() { " Close Overlay" } else { " Open Overlay" };
            lines.push(Line::from(vec![
                Span::styled("<o>", key_style),
                Span::styled(overlay_label, desc_style),
            ]));
        }

        let para = Paragraph::new(lines);
        f.render_widget(para, area);
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
    let gpu_device_names = state.get_gpu_devices();
    let (primary_hashrate, mode_label): (f64, String) = match display_mode {
        crate::tui::state::HashrateDisplayMode::Total => (total_hashrate, "Total".to_string()),
        crate::tui::state::HashrateDisplayMode::CpuOnly => (cpu_hashrate, "CPU".to_string()),
        crate::tui::state::HashrateDisplayMode::GpuOnly => (gpu_hashrate, "GPU".to_string()),
        crate::tui::state::HashrateDisplayMode::GpuDevice(idx) => {
            // Show per-device hashrate with device name
            let device_rate = state.get_gpu_device_hashrate(idx as u32);
            let device_name = gpu_device_names.get(idx as usize)
                .map(|s| s.as_str())
                .unwrap_or("Unknown");
            // Truncate long device names
            let short_name = if device_name.len() > 15 {
                &device_name[..15]
            } else {
                device_name
            };
            (device_rate, format!("GPU#{} {}", idx, short_name))
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
        .map(format_duration)
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

    // Calculate available lines (accounting for border)
    let available_lines = area.height.saturating_sub(2) as usize;

    // Autoscroll: show most recent logs if enabled
    let scroll = if state.is_autoscroll() {
        logs.len().saturating_sub(available_lines)
    } else {
        state.get_log_scroll()
    };

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
            let elapsed_secs = total >> 16 ;
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
