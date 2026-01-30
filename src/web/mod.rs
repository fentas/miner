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

use crate::config::Config;
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

/// Idle mining state
#[derive(Clone, Default, serde::Serialize)]
pub struct IdleState {
    pub enabled: bool,
    pub timeout: u64,  // seconds
    pub is_idle: bool,
    pub idle_time: u64,  // current idle time in seconds
    pub mining_due_to_idle: bool,
}

/// Mining context with block/pool info
#[derive(Clone, Default, serde::Serialize)]
pub struct MiningContext {
    pub block_height: u64,
    pub difficulty: f64,
    pub pool_difficulty: f64,
    pub est_time_to_share: f64,  // seconds
    pub gpu_enabled: bool,
    pub gpu_hashrate: f64,
    pub gpu_device: String,
    pub peers: u32,  // Number of connected peers (for Bitcoin Core mode)
    pub network_hashrate: f64,  // Network hashrate estimate
}

/// Configuration snapshot for API
#[derive(Clone, Default, serde::Serialize)]
pub struct ConfigSnapshot {
    pub pool_preset: String,
    pub pool_address: String,
    pub payout_address: String,
    pub enable_cpu: bool,
    pub cpu_threads: usize,
    pub enable_gpu: bool,
    pub idle_enabled: bool,
    pub idle_timeout: u64,
    pub bitcoind_enabled: bool,
    pub bitcoind_managed: bool,
}

impl ConfigSnapshot {
    /// Create a ConfigSnapshot for solo mining mode
    pub fn for_solo(cfg: &Config, thread_count: usize) -> Self {
        Self {
            pool_preset: "solo".to_string(),
            pool_address: "Bitcoin Network (direct)".to_string(),
            payout_address: cfg.pool.payout_address.clone(),
            enable_cpu: cfg.mining.enable_cpu,
            cpu_threads: thread_count,
            enable_gpu: cfg.mining.enable_gpu,
            idle_enabled: cfg.idle.enabled,
            idle_timeout: cfg.idle.timeout as u64,
            bitcoind_enabled: true,
            bitcoind_managed: cfg.bitcoind.managed,
        }
    }

    /// Create a ConfigSnapshot for pool mining mode
    pub fn for_pool(cfg: &Config, pool_address: &str, thread_count: usize) -> Self {
        Self {
            pool_preset: format!("{:?}", cfg.pool.preset),
            pool_address: pool_address.to_string(),
            payout_address: cfg.pool.payout_address.clone(),
            enable_cpu: cfg.mining.enable_cpu,
            cpu_threads: thread_count,
            enable_gpu: cfg.mining.enable_gpu,
            idle_enabled: cfg.idle.enabled,
            idle_timeout: cfg.idle.timeout as u64,
            bitcoind_enabled: false,
            bitcoind_managed: false,
        }
    }
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
    pub idle_state: RwLock<IdleState>,
    pub logs: RwLock<Vec<LogEntry>>,
    pub mining_context: RwLock<MiningContext>,
    pub config: RwLock<ConfigSnapshot>,
}

/// Log entry for web dashboard
#[derive(Clone, serde::Serialize)]
pub struct LogEntry {
    pub timestamp: u64,
    pub message: String,
    pub level: String,  // "info", "success", "warning", "error"
}

/// Stats update for WebSocket broadcast
#[derive(Clone, serde::Serialize)]
pub struct StatsUpdate {
    pub hashrate: f64,
    pub hashrate_formatted: String,
    pub hashes: u64,
    pub total_hashes: u64,  // Alias for web dashboard
    pub shares_accepted: u64,
    pub shares_rejected: u64,
    pub blocks_found: u64,
    pub uptime_seconds: u64,
    pub uptime: u64,  // Alias for web dashboard
    pub uptime_formatted: String,
    pub mining_paused: bool,
    pub state: String,
    pub threads: usize,
    // Mining context
    pub block_height: u64,
    pub difficulty: f64,
    pub est_time_to_share: f64,
    pub gpu_enabled: bool,
    pub gpu_hashrate: f64,
}

impl WebState {
    pub fn new(pool_address: String, worker_count: usize) -> Self {
        let (stats_tx, _) = broadcast::channel(100);
        Self {
            stats: RwLock::new(StatsSnapshot::default()),
            hashrate: RwLock::new(0.0),
            pool_address: pool_address.clone(),
            worker_count,
            start_time: std::time::Instant::now(),
            mining_paused: RwLock::new(false),
            stats_tx,
            idle_state: RwLock::new(IdleState::default()),
            logs: RwLock::new(Vec::new()),
            mining_context: RwLock::new(MiningContext::default()),
            config: RwLock::new(ConfigSnapshot {
                pool_address,
                cpu_threads: worker_count,
                enable_cpu: true,
                ..Default::default()
            }),
        }
    }

    /// Initialize idle state from config (call before starting tokio runtime)
    pub fn with_idle_config(mut self, enabled: bool, timeout: u64) -> Self {
        // Replace the idle state entirely during construction
        self.idle_state = RwLock::new(IdleState {
            enabled,
            timeout,
            ..Default::default()
        });
        self
    }

    /// Add a log entry
    pub async fn add_log(&self, message: String, level: &str) {
        let entry = LogEntry {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            message,
            level: level.to_string(),
        };

        let mut logs = self.logs.write().await;
        logs.push(entry);
        // Keep only last 500 entries
        if logs.len() > 500 {
            logs.remove(0);
        }
    }

    /// Update idle state
    pub async fn update_idle(&self, is_idle: bool, idle_time: u64, mining_due_to_idle: bool) {
        let mut state = self.idle_state.write().await;
        state.is_idle = is_idle;
        state.idle_time = idle_time;
        state.mining_due_to_idle = mining_due_to_idle;
    }

    /// Set idle enabled
    pub async fn set_idle_enabled(&self, enabled: bool, timeout: Option<u64>) {
        let mut state = self.idle_state.write().await;
        state.enabled = enabled;
        if let Some(t) = timeout {
            state.timeout = t;
        }
    }

    /// Update mining context (block height, difficulty, etc.)
    pub async fn update_mining_context(&self, ctx: MiningContext) {
        *self.mining_context.write().await = ctx;
    }

    /// Update block height
    pub async fn set_block_height(&self, height: u64) {
        self.mining_context.write().await.block_height = height;
    }

    /// Update difficulty
    pub async fn set_difficulty(&self, difficulty: f64, pool_difficulty: f64) {
        let mut ctx = self.mining_context.write().await;
        ctx.difficulty = difficulty;
        ctx.pool_difficulty = pool_difficulty;
    }

    /// Update GPU stats
    pub async fn set_gpu_stats(&self, enabled: bool, hashrate: f64, device: String) {
        let mut ctx = self.mining_context.write().await;
        ctx.gpu_enabled = enabled;
        ctx.gpu_hashrate = hashrate;
        ctx.gpu_device = device;
    }

    /// Update Bitcoin Core network info
    pub async fn set_network_info(&self, peers: u32, network_hashrate: f64) {
        let mut ctx = self.mining_context.write().await;
        ctx.peers = peers;
        ctx.network_hashrate = network_hashrate;
    }

    /// Update config snapshot
    pub async fn set_config(&self, config: ConfigSnapshot) {
        *self.config.write().await = config;
    }

    /// Initialize config from parsed TOML (call before starting tokio runtime)
    pub fn with_config(mut self, config: ConfigSnapshot) -> Self {
        self.config = RwLock::new(config);
        self
    }

    pub async fn update_stats(&self, stats: StatsSnapshot, hashrate: f64) {
        *self.stats.write().await = stats.clone();
        *self.hashrate.write().await = hashrate;

        let uptime = self.start_time.elapsed().as_secs();
        let paused = *self.mining_paused.read().await;
        let ctx = self.mining_context.read().await.clone();

        // Determine miner state
        let state = if paused {
            "paused"
        } else if hashrate > 0.0 {
            "running"
        } else {
            "stopped"
        };

        // Calculate estimated time to share based on pool difficulty
        let est_time = if hashrate > 0.0 && ctx.pool_difficulty > 0.0 {
            // Time = (2^32 * difficulty) / hashrate
            (4_294_967_296.0 * ctx.pool_difficulty) / hashrate
        } else {
            0.0
        };

        let update = StatsUpdate {
            hashrate,
            hashrate_formatted: format_hashrate_unit(hashrate),
            hashes: stats.hashes,
            total_hashes: stats.hashes,
            shares_accepted: stats.shares_accepted,
            shares_rejected: stats.shares_rejected,
            blocks_found: stats.blocks_found,
            uptime_seconds: uptime,
            uptime,
            uptime_formatted: format_uptime(uptime),
            mining_paused: paused,
            state: state.to_string(),
            threads: self.worker_count,
            block_height: ctx.block_height,
            difficulty: ctx.pool_difficulty,
            est_time_to_share: est_time,
            gpu_enabled: ctx.gpu_enabled,
            gpu_hashrate: ctx.gpu_hashrate,
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
        // Stats and info
        .route("/api/stats", get(api_stats))
        .route("/api/info", get(api_info))
        .route("/api/settings", get(api_settings))
        .route("/api/logs", get(api_logs))
        // Mining control
        .route("/api/control/pause", get(api_pause))
        .route("/api/control/resume", get(api_resume))
        .route("/api/control/idle", get(api_toggle_idle))
        // Idle mining
        .route("/api/idle", get(api_idle_status))
        .route("/api/idle/enable", axum::routing::post(api_idle_enable))
        .route("/api/idle/disable", axum::routing::post(api_idle_disable))
        .route("/api/idle/timeout", axum::routing::post(api_idle_timeout))
        // OpenAPI spec
        .route("/api/openapi.json", get(api_openapi))
        // WebSocket
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
    let ctx = state.mining_context.read().await.clone();
    let config = state.config.read().await.clone();

    // Determine miner state
    let miner_state = if paused {
        "paused"
    } else if hashrate > 0.0 {
        "running"
    } else {
        "stopped"
    };

    // Calculate estimated time to share
    let est_time = if hashrate > 0.0 && ctx.pool_difficulty > 0.0 {
        (4_294_967_296.0 * ctx.pool_difficulty) / hashrate
    } else {
        0.0
    };

    let idle = state.idle_state.read().await.clone();
    let hashrate_str = format_hashrate_unit(hashrate);
    Json(serde_json::json!({
        "hashrate": hashrate,
        "hashrate_unit": hashrate_str,
        "hashrate_formatted": hashrate_str,
        "hashes": stats.hashes,
        "total_hashes": stats.hashes,
        "shares_accepted": stats.shares_accepted,
        "shares_rejected": stats.shares_rejected,
        "blocks_found": stats.blocks_found,
        "uptime_seconds": uptime,
        "uptime": uptime,
        "uptime_formatted": format_uptime(uptime),
        "mining_paused": paused,
        "state": miner_state,
        "threads": state.worker_count,
        "block_height": ctx.block_height,
        "difficulty": ctx.pool_difficulty,
        "est_time_to_share": est_time,
        "gpu_enabled": ctx.gpu_enabled,
        "gpu_hashrate": ctx.gpu_hashrate,
        "gpu_device": ctx.gpu_device,
        "peers": ctx.peers,
        "network_hashrate": ctx.network_hashrate,
        "idle_mining_enabled": idle.enabled,
        "idle_mining_active": idle.mining_due_to_idle,
        "user_idle": idle.is_idle,
        "mode": if config.bitcoind_enabled { "solo" } else { "pool" },
        "pool_preset": config.pool_preset,
    }))
}

/// API endpoint for miner info
async fn api_info(State(state): State<Arc<WebState>>) -> impl IntoResponse {
    let config = state.config.read().await.clone();
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "pool": state.pool_address,
        "workers": state.worker_count,
        "pool_preset": config.pool_preset,
        "payout_address": config.payout_address,
        "enable_cpu": config.enable_cpu,
        "enable_gpu": config.enable_gpu,
    }))
}

/// API endpoint for settings/configuration
async fn api_settings(State(state): State<Arc<WebState>>) -> impl IntoResponse {
    let config = state.config.read().await.clone();
    let idle = state.idle_state.read().await.clone();
    let ctx = state.mining_context.read().await.clone();
    Json(serde_json::json!({
        "pool": {
            "preset": config.pool_preset,
            "address": config.pool_address,
            "payout_address": config.payout_address,
        },
        "mining": {
            "enable_cpu": config.enable_cpu,
            "cpu_threads": config.cpu_threads,
            "enable_gpu": config.enable_gpu,
        },
        "idle": {
            "enabled": idle.enabled,
            "timeout": idle.timeout,
        },
        "bitcoind": {
            "enabled": config.bitcoind_enabled,
            "managed": config.bitcoind_managed,
            "peers": ctx.peers,
            "network_hashrate": ctx.network_hashrate,
        },
        "version": env!("CARGO_PKG_VERSION"),
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

/// API endpoint to toggle idle mode
async fn api_toggle_idle(State(state): State<Arc<WebState>>) -> impl IntoResponse {
    let mut idle = state.idle_state.write().await;
    idle.enabled = !idle.enabled;
    let enabled = idle.enabled;
    let timeout = idle.timeout;
    drop(idle);
    Json(serde_json::json!({ "status": if enabled { "enabled" } else { "disabled" }, "enabled": enabled, "timeout": timeout }))
}

/// API endpoint for logs
async fn api_logs(State(state): State<Arc<WebState>>) -> impl IntoResponse {
    let logs = state.logs.read().await;
    Json(serde_json::json!({ "logs": *logs }))
}

/// API endpoint for idle mining status
async fn api_idle_status(State(state): State<Arc<WebState>>) -> impl IntoResponse {
    let idle = state.idle_state.read().await.clone();
    Json(serde_json::json!({
        "enabled": idle.enabled,
        "timeout": idle.timeout,
        "is_idle": idle.is_idle,
        "idle_time": idle.idle_time,
        "mining_due_to_idle": idle.mining_due_to_idle,
    }))
}

/// API endpoint to enable idle mining
async fn api_idle_enable(
    State(state): State<Arc<WebState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let timeout = params.get("timeout")
        .and_then(|t| t.parse::<u64>().ok())
        .unwrap_or(900);
    state.set_idle_enabled(true, Some(timeout)).await;
    Json(serde_json::json!({ "success": true, "enabled": true, "timeout": timeout }))
}

/// API endpoint to disable idle mining
async fn api_idle_disable(State(state): State<Arc<WebState>>) -> impl IntoResponse {
    state.set_idle_enabled(false, None).await;
    Json(serde_json::json!({ "success": true, "enabled": false }))
}

/// API endpoint to update idle timeout
async fn api_idle_timeout(
    State(state): State<Arc<WebState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let timeout = params.get("timeout")
        .and_then(|t| t.parse::<u64>().ok())
        .unwrap_or(900);
    let mut idle = state.idle_state.write().await;
    idle.timeout = timeout;
    Json(serde_json::json!({ "success": true, "timeout": timeout }))
}

/// OpenAPI specification
async fn api_openapi() -> impl IntoResponse {
    let spec = serde_json::json!({
        "openapi": "3.0.3",
        "info": {
            "title": "BTC Miner API",
            "description": "REST API for BTC Miner - High-performance Bitcoin solo miner",
            "version": env!("CARGO_PKG_VERSION"),
            "license": { "name": "MIT" }
        },
        "servers": [{ "url": "/" }],
        "paths": {
            "/api/stats": {
                "get": {
                    "summary": "Get mining statistics",
                    "tags": ["Stats"],
                    "responses": {
                        "200": {
                            "description": "Mining statistics including hashrate, shares, block info",
                            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Stats" } } }
                        }
                    }
                }
            },
            "/api/info": {
                "get": {
                    "summary": "Get miner information",
                    "tags": ["Info"],
                    "responses": { "200": { "description": "Miner version, pool, and basic config" } }
                }
            },
            "/api/settings": {
                "get": {
                    "summary": "Get full configuration",
                    "tags": ["Settings"],
                    "responses": { "200": { "description": "Complete miner configuration" } }
                }
            },
            "/api/logs": {
                "get": {
                    "summary": "Get recent log entries",
                    "tags": ["Logs"],
                    "responses": { "200": { "description": "Array of log entries with timestamp and level" } }
                }
            },
            "/api/control/pause": {
                "get": {
                    "summary": "Pause mining",
                    "tags": ["Control"],
                    "responses": { "200": { "description": "Mining paused" } }
                }
            },
            "/api/control/resume": {
                "get": {
                    "summary": "Resume mining",
                    "tags": ["Control"],
                    "responses": { "200": { "description": "Mining resumed" } }
                }
            },
            "/api/idle": {
                "get": {
                    "summary": "Get idle mining status",
                    "tags": ["Idle"],
                    "responses": { "200": { "description": "Idle mining state including timeout and current idle time" } }
                }
            },
            "/api/idle/enable": {
                "post": {
                    "summary": "Enable idle mining",
                    "tags": ["Idle"],
                    "parameters": [{
                        "name": "timeout",
                        "in": "query",
                        "description": "Idle timeout in seconds",
                        "schema": { "type": "integer", "default": 900 }
                    }],
                    "responses": { "200": { "description": "Idle mining enabled" } }
                }
            },
            "/api/idle/disable": {
                "post": {
                    "summary": "Disable idle mining",
                    "tags": ["Idle"],
                    "responses": { "200": { "description": "Idle mining disabled" } }
                }
            },
            "/api/idle/timeout": {
                "post": {
                    "summary": "Update idle timeout",
                    "tags": ["Idle"],
                    "parameters": [{
                        "name": "timeout",
                        "in": "query",
                        "description": "Idle timeout in seconds",
                        "schema": { "type": "integer" }
                    }],
                    "responses": { "200": { "description": "Timeout updated" } }
                }
            }
        },
        "components": {
            "schemas": {
                "Stats": {
                    "type": "object",
                    "properties": {
                        "hashrate": { "type": "number", "description": "Current hashrate in H/s" },
                        "hashrate_formatted": { "type": "string", "description": "Human-readable hashrate" },
                        "hashes": { "type": "integer", "description": "Total hashes computed" },
                        "total_hashes": { "type": "integer", "description": "Alias for hashes" },
                        "shares_accepted": { "type": "integer" },
                        "shares_rejected": { "type": "integer" },
                        "blocks_found": { "type": "integer" },
                        "uptime_seconds": { "type": "integer" },
                        "uptime": { "type": "integer", "description": "Alias for uptime_seconds" },
                        "mining_paused": { "type": "boolean" },
                        "state": { "type": "string", "enum": ["running", "paused", "stopped"] },
                        "threads": { "type": "integer" },
                        "block_height": { "type": "integer", "description": "Current network block height" },
                        "difficulty": { "type": "number", "description": "Pool/share difficulty" },
                        "est_time_to_share": { "type": "number", "description": "Estimated seconds to find share" },
                        "gpu_enabled": { "type": "boolean" },
                        "gpu_hashrate": { "type": "number" },
                        "gpu_device": { "type": "string" },
                        "peers": { "type": "integer", "description": "Connected Bitcoin Core peers" },
                        "mode": { "type": "string", "enum": ["pool", "solo"] },
                        "pool_preset": { "type": "string" },
                        "idle_mining_enabled": { "type": "boolean" },
                        "idle_mining_active": { "type": "boolean" },
                        "user_idle": { "type": "boolean" }
                    }
                }
            }
        }
    });
    Json(spec)
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
            total_hashes: 50000,
            shares_accepted: 10,
            shares_rejected: 1,
            blocks_found: 0,
            uptime_seconds: 3600,
            uptime: 3600,
            uptime_formatted: "1h 0m 0s".to_string(),
            mining_paused: false,
            state: "running".to_string(),
            threads: 8,
            block_height: 800000,
            difficulty: 1.0,
            est_time_to_share: 100.0,
            gpu_enabled: false,
            gpu_hashrate: 0.0,
        };

        let cloned = update.clone();
        assert_eq!(cloned.hashrate, 1000.0);
        assert_eq!(cloned.shares_accepted, 10);
        assert!(!cloned.mining_paused);
        assert_eq!(cloned.state, "running");
        assert_eq!(cloned.threads, 8);
        assert_eq!(cloned.block_height, 800000);
    }

    #[test]
    fn test_stats_update_serialize() {
        let update = StatsUpdate {
            hashrate: 5000.0,
            hashrate_formatted: "5.00 KH/s".to_string(),
            hashes: 100000,
            total_hashes: 100000,
            shares_accepted: 5,
            shares_rejected: 0,
            blocks_found: 0,
            uptime_seconds: 600,
            uptime: 600,
            uptime_formatted: "10m 0s".to_string(),
            mining_paused: true,
            state: "paused".to_string(),
            threads: 4,
            block_height: 850000,
            difficulty: 2.5,
            est_time_to_share: 50.0,
            gpu_enabled: true,
            gpu_hashrate: 1000.0,
        };

        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("\"hashrate\":5000.0"));
        assert!(json.contains("\"mining_paused\":true"));
        assert!(json.contains("\"shares_accepted\":5"));
        assert!(json.contains("\"state\":\"paused\""));
        assert!(json.contains("\"threads\":4"));
        assert!(json.contains("\"block_height\":850000"));
        assert!(json.contains("\"gpu_enabled\":true"));
    }

    #[test]
    fn test_mining_context_default() {
        let ctx = MiningContext::default();
        assert_eq!(ctx.block_height, 0);
        assert_eq!(ctx.difficulty, 0.0);
        assert_eq!(ctx.peers, 0);
        assert!(!ctx.gpu_enabled);
    }

    #[test]
    fn test_config_snapshot_default() {
        let config = ConfigSnapshot::default();
        assert_eq!(config.pool_preset, "");
        assert!(!config.enable_cpu);
        assert!(!config.bitcoind_enabled);
    }

    #[tokio::test]
    async fn test_web_state_mining_context() {
        let state = WebState::new("pool".to_string(), 4);

        // Update mining context
        state.set_block_height(800000).await;
        state.set_difficulty(1.5, 2.0).await;
        state.set_network_info(8, 500e18).await;

        let ctx = state.mining_context.read().await;
        assert_eq!(ctx.block_height, 800000);
        assert_eq!(ctx.pool_difficulty, 2.0);
        assert_eq!(ctx.peers, 8);
    }

    #[tokio::test]
    async fn test_web_state_idle_config() {
        let state = WebState::new("pool".to_string(), 4)
            .with_idle_config(true, 900);

        let idle = state.idle_state.read().await;
        assert!(idle.enabled);
        assert_eq!(idle.timeout, 900);
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
