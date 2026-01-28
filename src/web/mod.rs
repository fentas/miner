use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tower_http::services::ServeDir;

use crate::mining::StatsSnapshot;

/// Shared state for the web server
pub struct WebState {
    pub stats: RwLock<StatsSnapshot>,
    pub hashrate: RwLock<f64>,
    pub pool_address: String,
    pub worker_count: usize,
    pub start_time: std::time::Instant,
    pub mining_paused: RwLock<bool>,
    pub stats_tx: broadcast::Sender<StatsUpdate>,
}

/// Stats update for WebSocket broadcast
#[derive(Clone, serde::Serialize)]
pub struct StatsUpdate {
    pub hashrate: f64,
    pub hashrate_formatted: String,
    pub hashes: u64,
    pub shares_accepted: u64,
    pub shares_rejected: u64,
    pub blocks_found: u64,
    pub uptime_seconds: u64,
    pub uptime_formatted: String,
    pub mining_paused: bool,
}

impl WebState {
    pub fn new(pool_address: String, worker_count: usize) -> Self {
        let (stats_tx, _) = broadcast::channel(100);
        Self {
            stats: RwLock::new(StatsSnapshot::default()),
            hashrate: RwLock::new(0.0),
            pool_address,
            worker_count,
            start_time: std::time::Instant::now(),
            mining_paused: RwLock::new(false),
            stats_tx,
        }
    }

    pub async fn update_stats(&self, stats: StatsSnapshot, hashrate: f64) {
        *self.stats.write().await = stats.clone();
        *self.hashrate.write().await = hashrate;

        let uptime = self.start_time.elapsed().as_secs();
        let paused = *self.mining_paused.read().await;

        let update = StatsUpdate {
            hashrate,
            hashrate_formatted: format_hashrate_unit(hashrate),
            hashes: stats.hashes,
            shares_accepted: stats.shares_accepted,
            shares_rejected: stats.shares_rejected,
            blocks_found: stats.blocks_found,
            uptime_seconds: uptime,
            uptime_formatted: format_uptime(uptime),
            mining_paused: paused,
        };

        // Broadcast to all WebSocket clients (ignore errors if no receivers)
        let _ = self.stats_tx.send(update);
    }

    pub async fn set_paused(&self, paused: bool) {
        *self.mining_paused.write().await = paused;
    }

    pub async fn is_paused(&self) -> bool {
        *self.mining_paused.read().await
    }
}

/// Create the web server router
pub fn create_router(state: Arc<WebState>, static_dir: &str) -> Router {
    Router::new()
        .route("/api/stats", get(api_stats))
        .route("/api/info", get(api_info))
        .route("/api/control/pause", get(api_pause))
        .route("/api/control/resume", get(api_resume))
        .route("/ws", get(ws_handler))
        .nest_service("/", ServeDir::new(static_dir))
        .with_state(state)
}

/// WebSocket handler for real-time updates
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<WebState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws(socket, state))
}

async fn handle_ws(socket: WebSocket, state: Arc<WebState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut stats_rx = state.stats_tx.subscribe();

    // Spawn task to send stats updates
    let send_task = tokio::spawn(async move {
        while let Ok(update) = stats_rx.recv().await {
            if let Ok(json) = serde_json::to_string(&update) {
                if sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Handle incoming messages (for future control commands)
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                // Handle control commands
                if text == "ping" {
                    // Pong is handled automatically by axum
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    send_task.abort();
}

/// API endpoint for mining statistics
async fn api_stats(State(state): State<Arc<WebState>>) -> impl IntoResponse {
    let stats = state.stats.read().await.clone();
    let hashrate = *state.hashrate.read().await;
    let uptime = state.start_time.elapsed().as_secs();
    let paused = *state.mining_paused.read().await;

    Json(serde_json::json!({
        "hashrate": hashrate,
        "hashrate_unit": format_hashrate_unit(hashrate),
        "hashes": stats.hashes,
        "shares_accepted": stats.shares_accepted,
        "shares_rejected": stats.shares_rejected,
        "blocks_found": stats.blocks_found,
        "uptime_seconds": uptime,
        "uptime_formatted": format_uptime(uptime),
        "mining_paused": paused,
    }))
}

/// API endpoint for miner info
async fn api_info(State(state): State<Arc<WebState>>) -> impl IntoResponse {
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "pool": state.pool_address,
        "workers": state.worker_count,
    }))
}

/// API endpoint to pause mining
async fn api_pause(State(state): State<Arc<WebState>>) -> impl IntoResponse {
    state.set_paused(true).await;
    Json(serde_json::json!({ "status": "paused" }))
}

/// API endpoint to resume mining
async fn api_resume(State(state): State<Arc<WebState>>) -> impl IntoResponse {
    state.set_paused(false).await;
    Json(serde_json::json!({ "status": "resumed" }))
}

/// Format hashrate with appropriate unit
fn format_hashrate_unit(hashrate: f64) -> String {
    if hashrate >= 1e15 {
        format!("{:.2} PH/s", hashrate / 1e15)
    } else if hashrate >= 1e12 {
        format!("{:.2} TH/s", hashrate / 1e12)
    } else if hashrate >= 1e9 {
        format!("{:.2} GH/s", hashrate / 1e9)
    } else if hashrate >= 1e6 {
        format!("{:.2} MH/s", hashrate / 1e6)
    } else if hashrate >= 1e3 {
        format!("{:.2} KH/s", hashrate / 1e3)
    } else {
        format!("{:.2} H/s", hashrate)
    }
}

/// Format uptime in human-readable format
fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if days > 0 {
        format!("{}d {}h {}m {}s", days, hours, minutes, secs)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

/// Start the web server
pub async fn start_server(
    state: Arc<WebState>,
    static_dir: &str,
    port: u16,
) -> Result<(), std::io::Error> {
    let router = create_router(state, static_dir);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!("Web interface available at http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await
}
