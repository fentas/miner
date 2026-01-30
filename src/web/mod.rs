use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{Request, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use subtle::ConstantTimeEq;
use tokio::sync::{broadcast, RwLock};
use tower_http::services::ServeDir;

use crate::mining::StatsSnapshot;

/// Basic authentication credentials
#[derive(Clone)]
pub struct BasicAuthConfig {
    pub username: String,
    pub password: String,
}

/// Basic authentication middleware
async fn basic_auth_middleware(
    State(auth): State<BasicAuthConfig>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Check for Authorization header
    if let Some(auth_header) = request.headers().get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(encoded) = auth_str.strip_prefix("Basic ") {
                if let Ok(decoded) = BASE64.decode(encoded) {
                    if let Ok(credentials) = String::from_utf8(decoded) {
                        if let Some((user, pass)) = credentials.split_once(':') {
                            // Use constant-time comparison to prevent timing attacks
                            let user_match = user.as_bytes().ct_eq(auth.username.as_bytes());
                            let pass_match = pass.as_bytes().ct_eq(auth.password.as_bytes());
                            if (user_match & pass_match).into() {
                                return next.run(request).await;
                            }
                        }
                    }
                }
            }
        }
    }

    // Return 401 Unauthorized with WWW-Authenticate header
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::WWW_AUTHENTICATE, "Basic realm=\"btcminer\"")
        .body(Body::from("Unauthorized"))
        .expect("static response should always build")
}

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
pub fn create_router(state: Arc<WebState>, static_dir: &str, auth: Option<BasicAuthConfig>) -> Router {
    let router = Router::new()
        .route("/api/stats", get(api_stats))
        .route("/api/info", get(api_info))
        .route("/api/control/pause", get(api_pause))
        .route("/api/control/resume", get(api_resume))
        .route("/ws", get(ws_handler))
        .fallback_service(ServeDir::new(static_dir))
        .with_state(state);

    // Apply basic auth middleware if credentials are provided
    if let Some(auth_config) = auth {
        router.layer(middleware::from_fn_with_state(auth_config, basic_auth_middleware))
    } else {
        router
    }
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

    let hashrate_str = format_hashrate_unit(hashrate);
    Json(serde_json::json!({
        "hashrate": hashrate,
        "hashrate_unit": hashrate_str,
        "hashrate_formatted": hashrate_str,  // Alias for overlay compatibility
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
pub fn format_uptime(seconds: u64) -> String {
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
    auth: Option<BasicAuthConfig>,
) -> Result<(), std::io::Error> {
    let router = create_router(state, static_dir, auth);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!("Web interface available at http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_hashrate_unit_hash() {
        assert_eq!(format_hashrate_unit(0.0), "0.00 H/s");
        assert_eq!(format_hashrate_unit(100.0), "100.00 H/s");
        assert_eq!(format_hashrate_unit(999.99), "999.99 H/s");
    }

    #[test]
    fn test_format_hashrate_unit_kilohash() {
        assert_eq!(format_hashrate_unit(1_000.0), "1.00 KH/s");
        assert_eq!(format_hashrate_unit(5_500.0), "5.50 KH/s");
        assert_eq!(format_hashrate_unit(999_999.0), "1000.00 KH/s");
    }

    #[test]
    fn test_format_hashrate_unit_megahash() {
        assert_eq!(format_hashrate_unit(1_000_000.0), "1.00 MH/s");
        assert_eq!(format_hashrate_unit(2_500_000.0), "2.50 MH/s");
    }

    #[test]
    fn test_format_hashrate_unit_gigahash() {
        assert_eq!(format_hashrate_unit(1e9), "1.00 GH/s");
        assert_eq!(format_hashrate_unit(15.5e9), "15.50 GH/s");
    }

    #[test]
    fn test_format_hashrate_unit_terahash() {
        assert_eq!(format_hashrate_unit(1e12), "1.00 TH/s");
        assert_eq!(format_hashrate_unit(100e12), "100.00 TH/s");
    }

    #[test]
    fn test_format_hashrate_unit_petahash() {
        assert_eq!(format_hashrate_unit(1e15), "1.00 PH/s");
        assert_eq!(format_hashrate_unit(2.5e15), "2.50 PH/s");
    }

    #[test]
    fn test_format_uptime_seconds() {
        assert_eq!(format_uptime(0), "0s");
        assert_eq!(format_uptime(30), "30s");
        assert_eq!(format_uptime(59), "59s");
    }

    #[test]
    fn test_format_uptime_minutes() {
        assert_eq!(format_uptime(60), "1m 0s");
        assert_eq!(format_uptime(90), "1m 30s");
        assert_eq!(format_uptime(3599), "59m 59s");
    }

    #[test]
    fn test_format_uptime_hours() {
        assert_eq!(format_uptime(3600), "1h 0m 0s");
        assert_eq!(format_uptime(7200), "2h 0m 0s");
        assert_eq!(format_uptime(3661), "1h 1m 1s");
        assert_eq!(format_uptime(86399), "23h 59m 59s");
    }

    #[test]
    fn test_format_uptime_days() {
        assert_eq!(format_uptime(86400), "1d 0h 0m 0s");
        assert_eq!(format_uptime(172800), "2d 0h 0m 0s");
        assert_eq!(format_uptime(90061), "1d 1h 1m 1s");
    }

    #[test]
    fn test_web_state_new() {
        let state = WebState::new("stratum+tcp://pool:3333".to_string(), 8);
        assert_eq!(state.pool_address, "stratum+tcp://pool:3333");
        assert_eq!(state.worker_count, 8);
    }

    #[test]
    fn test_stats_update_clone() {
        let update = StatsUpdate {
            hashrate: 1000.0,
            hashrate_formatted: "1.00 KH/s".to_string(),
            hashes: 50000,
            shares_accepted: 10,
            shares_rejected: 1,
            blocks_found: 0,
            uptime_seconds: 3600,
            uptime_formatted: "1h 0m 0s".to_string(),
            mining_paused: false,
        };

        let cloned = update.clone();
        assert_eq!(cloned.hashrate, 1000.0);
        assert_eq!(cloned.shares_accepted, 10);
        assert!(!cloned.mining_paused);
    }

    #[test]
    fn test_stats_update_serialize() {
        let update = StatsUpdate {
            hashrate: 5000.0,
            hashrate_formatted: "5.00 KH/s".to_string(),
            hashes: 100000,
            shares_accepted: 5,
            shares_rejected: 0,
            blocks_found: 0,
            uptime_seconds: 600,
            uptime_formatted: "10m 0s".to_string(),
            mining_paused: true,
        };

        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("\"hashrate\":5000.0"));
        assert!(json.contains("\"mining_paused\":true"));
        assert!(json.contains("\"shares_accepted\":5"));
    }

    #[tokio::test]
    async fn test_web_state_paused() {
        let state = WebState::new("pool".to_string(), 4);

        // Initially not paused
        assert!(!state.is_paused().await);

        // Pause mining
        state.set_paused(true).await;
        assert!(state.is_paused().await);

        // Resume mining
        state.set_paused(false).await;
        assert!(!state.is_paused().await);
    }

    #[tokio::test]
    async fn test_web_state_update_stats() {
        let state = WebState::new("pool".to_string(), 2);

        let stats = StatsSnapshot {
            hashes: 1_000_000,
            gpu_hashes: 0,
            shares_accepted: 10,
            shares_rejected: 1,
            blocks_found: 0,
        };

        state.update_stats(stats.clone(), 50_000.0).await;

        let stored_stats = state.stats.read().await;
        assert_eq!(stored_stats.hashes, 1_000_000);
        assert_eq!(stored_stats.shares_accepted, 10);

        let stored_hashrate = *state.hashrate.read().await;
        assert!((stored_hashrate - 50_000.0).abs() < 0.1);
    }
}
