//! BTC Miner Overlay - Transparent floating stats window
//!
//! A minimal overlay that shows mining stats on top of other windows.
//! Connects to the btcminer web API to fetch real-time statistics.
//!
//! Usage: btcminer-overlay [--port PORT]
//!
//! The port can also be read from btcminer.toml if present.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use eframe::egui;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const DEFAULT_PORT: u16 = 8080;
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Web API configuration
struct ApiConfig {
    port: u16,
    auth: Option<(String, String)>, // (username, password)
}

fn main() -> eframe::Result<()> {
    let config = get_api_config();
    let api_base = format!("http://127.0.0.1:{}", config.port);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([180.0, 80.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_position([100.0, 100.0]),
        ..Default::default()
    };

    eframe::run_native(
        "btcminer-overlay",
        options,
        Box::new(move |cc| Ok(Box::new(OverlayApp::new(cc, api_base.clone(), config.auth.clone())))),
    )
}

/// Get API config from CLI args, environment, config file, or defaults
fn get_api_config() -> ApiConfig {
    let port = get_api_port();
    let auth = read_auth_from_config();
    ApiConfig { port, auth }
}

/// Get API port from CLI args, environment, config file, or default
fn get_api_port() -> u16 {
    // 1. Check CLI args: --port PORT
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--port" || args[i] == "-p" {
            if let Some(port_str) = args.get(i + 1) {
                if let Ok(port) = port_str.parse::<u16>() {
                    return port;
                }
            }
        }
    }

    // 2. Check environment variable
    if let Ok(port_str) = std::env::var("BTCMINER_WEB_PORT") {
        if let Ok(port) = port_str.parse::<u16>() {
            return port;
        }
    }

    // 3. Try to read from btcminer.toml in current directory or exe directory
    if let Some(port) = read_port_from_config() {
        return port;
    }

    DEFAULT_PORT
}

/// Try to read web port from btcminer.toml config file
fn read_port_from_config() -> Option<u16> {
    // Try current directory first
    let paths = [
        std::path::PathBuf::from("btcminer.toml"),
        std::env::current_exe()
            .ok()?
            .parent()?
            .join("btcminer.toml"),
    ];

    for path in &paths {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                // Simple TOML parsing for [web] port = N
                for line in content.lines() {
                    let line = line.trim();
                    if line.starts_with("port") && line.contains('=') {
                        if let Some(value) = line.split('=').nth(1) {
                            if let Ok(port) = value.trim().parse::<u16>() {
                                return Some(port);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Try to read web auth credentials from btcminer.toml config file
fn read_auth_from_config() -> Option<(String, String)> {
    let paths = [
        std::path::PathBuf::from("btcminer.toml"),
        std::env::current_exe()
            .ok()?
            .parent()?
            .join("btcminer.toml"),
    ];

    for path in &paths {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                let mut auth_enabled = false;
                let mut auth_user = None;
                let mut auth_password = None;

                for line in content.lines() {
                    let line = line.trim();
                    if line.starts_with("auth_enabled") && line.contains("true") {
                        auth_enabled = true;
                    } else if line.starts_with("auth_user") && line.contains('=') {
                        if let Some(value) = line.split('=').nth(1) {
                            let v = value.trim().trim_matches('"');
                            if !v.is_empty() {
                                auth_user = Some(v.to_string());
                            }
                        }
                    } else if line.starts_with("auth_password") && line.contains('=') {
                        if let Some(value) = line.split('=').nth(1) {
                            let v = value.trim().trim_matches('"');
                            if !v.is_empty() {
                                auth_password = Some(v.to_string());
                            }
                        }
                    }
                }

                if auth_enabled {
                    if let (Some(user), Some(pass)) = (auth_user, auth_password) {
                        return Some((user, pass));
                    }
                }
            }
        }
    }
    None
}

#[derive(Default, Clone)]
struct MinerStats {
    hashrate: f64,
    hashrate_formatted: String,
    mining_paused: bool,
    idle_mining: bool,
    connected: bool,
    shares_accepted: u64,
}

struct OverlayApp {
    stats: Arc<Mutex<MinerStats>>,
    last_fetch: Instant,
    dragging: bool,
    drag_start: egui::Pos2,
    api_base: String,
    auth: Option<(String, String)>,
}

impl OverlayApp {
    fn new(_cc: &eframe::CreationContext<'_>, api_base: String, auth: Option<(String, String)>) -> Self {
        Self {
            stats: Arc::new(Mutex::new(MinerStats::default())),
            last_fetch: Instant::now() - POLL_INTERVAL,
            dragging: false,
            drag_start: egui::Pos2::ZERO,
            api_base,
            auth,
        }
    }

    fn fetch_stats(&mut self) {
        if self.last_fetch.elapsed() < POLL_INTERVAL {
            return;
        }
        self.last_fetch = Instant::now();

        let stats = Arc::clone(&self.stats);
        let url = format!("{}/api/stats", self.api_base);
        let auth = self.auth.clone();

        // Non-blocking fetch using a thread
        std::thread::spawn(move || {
            let mut request = ureq::get(&url).timeout(Duration::from_secs(2));
            if let Some((user, pass)) = &auth {
                let credentials = BASE64.encode(format!("{}:{}", user, pass));
                request = request.set("Authorization", &format!("Basic {}", credentials));
            }
            if let Ok(response) = request.call() {
                if let Ok(json) = response.into_json::<serde_json::Value>() {
                    let mut s = stats.lock().unwrap();
                    s.hashrate = json["hashrate"].as_f64().unwrap_or(0.0);
                    s.hashrate_formatted = json["hashrate_formatted"]
                        .as_str()
                        .unwrap_or("0.00 H/s")
                        .to_string();
                    s.mining_paused = json["mining_paused"].as_bool().unwrap_or(false);
                    s.idle_mining = json["idle_mining"].as_bool().unwrap_or(false);
                    s.connected = true;
                    s.shares_accepted = json["shares_accepted"].as_u64().unwrap_or(0);
                }
            } else {
                stats.lock().unwrap().connected = false;
            }
        });
    }

    fn toggle_pause(&self) {
        let stats = self.stats.lock().unwrap();
        let is_paused = stats.mining_paused;
        drop(stats);

        let action = if is_paused { "resume" } else { "pause" };
        let url = format!("{}/api/control/{}", self.api_base, action);
        let auth = self.auth.clone();

        std::thread::spawn(move || {
            let mut request = ureq::get(&url).timeout(Duration::from_secs(2));
            if let Some((user, pass)) = &auth {
                let credentials = BASE64.encode(format!("{}:{}", user, pass));
                request = request.set("Authorization", &format!("Basic {}", credentials));
            }
            let _ = request.call();
        });
    }
}

impl eframe::App for OverlayApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0] // Fully transparent
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.fetch_stats();

        // Request repaint for continuous updates
        ctx.request_repaint_after(Duration::from_millis(100));

        let stats = self.stats.lock().unwrap().clone();

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_rgba_unmultiplied(20, 20, 20, 200)))
            .show(ctx, |ui| {
                // Handle dragging
                let response = ui.interact(
                    ui.max_rect(),
                    ui.id().with("drag"),
                    egui::Sense::drag(),
                );

                if response.dragged() {
                    if let Some(pointer_pos) = ctx.input(|i| i.pointer.interact_pos()) {
                        if !self.dragging {
                            self.dragging = true;
                            self.drag_start = pointer_pos;
                        }
                        let delta = pointer_pos - self.drag_start;
                        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(
                            ctx.input(|i| i.viewport().outer_rect)
                                .map(|r| r.min + delta)
                                .unwrap_or_default(),
                        ));
                    }
                } else {
                    self.dragging = false;
                }

                ui.vertical_centered(|ui| {
                    ui.add_space(4.0);

                    // Connection status indicator
                    let status_color = if !stats.connected {
                        egui::Color32::RED
                    } else if stats.mining_paused {
                        egui::Color32::YELLOW
                    } else {
                        egui::Color32::GREEN
                    };

                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        // Status dot
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(8.0, 8.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().circle_filled(rect.center(), 4.0, status_color);

                        // Hashrate
                        ui.label(
                            egui::RichText::new(&stats.hashrate_formatted)
                                .color(egui::Color32::WHITE)
                                .size(16.0)
                                .strong(),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(8.0);
                            // Close button
                            if ui.small_button("×").clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        });
                    });

                    ui.add_space(2.0);

                    // Status line
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        let status_text = if !stats.connected {
                            "Disconnected"
                        } else if stats.mining_paused {
                            "Paused"
                        } else if stats.idle_mining {
                            "Idle Mining"
                        } else {
                            "Mining"
                        };
                        ui.label(
                            egui::RichText::new(status_text)
                                .color(egui::Color32::GRAY)
                                .size(11.0),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(8.0);
                            // Shares count
                            ui.label(
                                egui::RichText::new(format!("[{}]", stats.shares_accepted))
                                    .color(egui::Color32::from_rgb(100, 200, 100))
                                    .size(11.0),
                            );
                        });
                    });

                    ui.add_space(4.0);

                    // Control buttons
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        let btn_text = if stats.mining_paused { "▶ Start" } else { "⏸ Pause" };
                        if ui.small_button(btn_text).clicked() {
                            self.toggle_pause();
                        }
                    });
                });
            });
    }
}
