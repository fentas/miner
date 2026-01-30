//! Integration tests for Web API endpoints
//!
//! Tests the web dashboard API:
//! - GET /api/stats - Mining statistics
//! - GET /api/info - Miner information
//! - GET /api/control/pause - Pause mining
//! - GET /api/control/resume - Resume mining
//! - WebSocket /ws - Real-time updates

use std::sync::Arc;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use btcminer::web::{WebState, create_router, StatsUpdate};
use btcminer::mining::StatsSnapshot;

/// Create a test web state
fn create_test_state() -> Arc<WebState> {
    Arc::new(WebState::new("test-pool:3333".to_string(), 8))
}

/// Test GET /api/stats endpoint
#[tokio::test]
async fn test_api_stats_endpoint() {
    let state = create_test_state();
    let app = create_router(state.clone(), "web", None);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify response structure
    assert!(json.get("hashrate").is_some());
    assert!(json.get("hashrate_unit").is_some());
    assert!(json.get("hashrate_formatted").is_some()); // Overlay compatibility
    assert!(json.get("hashes").is_some());
    assert!(json.get("total_hashes").is_some()); // Web dashboard alias
    assert!(json.get("shares_accepted").is_some());
    assert!(json.get("shares_rejected").is_some());
    assert!(json.get("blocks_found").is_some());
    assert!(json.get("uptime_seconds").is_some());
    assert!(json.get("uptime").is_some()); // Web dashboard alias
    assert!(json.get("uptime_formatted").is_some());
    assert!(json.get("mining_paused").is_some());
    assert!(json.get("state").is_some()); // Miner state
    assert!(json.get("threads").is_some()); // Worker count

    // Verify hashrate_unit and hashrate_formatted are the same
    assert_eq!(
        json["hashrate_unit"].as_str(),
        json["hashrate_formatted"].as_str()
    );

    // Verify hashes and total_hashes are the same
    assert_eq!(json["hashes"], json["total_hashes"]);

    // Verify uptime_seconds and uptime are the same
    assert_eq!(json["uptime_seconds"], json["uptime"]);

    // Verify state is a valid value
    let state = json["state"].as_str().unwrap();
    assert!(["running", "paused", "stopped"].contains(&state));
}

/// Test GET /api/info endpoint
#[tokio::test]
async fn test_api_info_endpoint() {
    let state = create_test_state();
    let app = create_router(state.clone(), "web", None);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/info")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify response structure
    assert!(json.get("version").is_some());
    assert!(json.get("pool").is_some());
    assert!(json.get("workers").is_some());

    // Verify values
    assert_eq!(json["pool"].as_str().unwrap(), "test-pool:3333");
    assert_eq!(json["workers"].as_u64().unwrap(), 8);
}

/// Test GET /api/control/pause endpoint
#[tokio::test]
async fn test_api_pause_endpoint() {
    let state = create_test_state();
    let app = create_router(state.clone(), "web", None);

    // Initially not paused
    assert!(!state.is_paused().await);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/control/pause")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"].as_str().unwrap(), "paused");

    // State should now be paused
    assert!(state.is_paused().await);
}

/// Test GET /api/control/resume endpoint
#[tokio::test]
async fn test_api_resume_endpoint() {
    let state = create_test_state();

    // Set paused first
    state.set_paused(true).await;
    assert!(state.is_paused().await);

    let app = create_router(state.clone(), "web", None);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/control/resume")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"].as_str().unwrap(), "resumed");

    // State should now be resumed
    assert!(!state.is_paused().await);
}

/// Test stats update and broadcast
#[tokio::test]
async fn test_stats_update() {
    let state = create_test_state();

    let stats = StatsSnapshot {
        hashes: 1_000_000,
        gpu_hashes: 0,
        shares_accepted: 5,
        shares_rejected: 1,
        blocks_found: 0,
    };

    state.update_stats(stats, 50_000.0).await;

    // Verify stats were stored
    let stored_stats = state.stats.read().await;
    assert_eq!(stored_stats.hashes, 1_000_000);
    assert_eq!(stored_stats.shares_accepted, 5);
    assert_eq!(stored_stats.shares_rejected, 1);

    let hashrate = *state.hashrate.read().await;
    assert!((hashrate - 50_000.0).abs() < 0.1);
}

/// Test StatsUpdate serialization
#[test]
fn test_stats_update_serialization() {
    let update = StatsUpdate {
        hashrate: 1_500_000.0,
        hashrate_formatted: "1.50 MH/s".to_string(),
        hashes: 50_000_000,
        total_hashes: 50_000_000,
        shares_accepted: 10,
        shares_rejected: 2,
        blocks_found: 0,
        uptime_seconds: 3600,
        uptime: 3600,
        uptime_formatted: "1h 0m 0s".to_string(),
        mining_paused: false,
        state: "running".to_string(),
        threads: 8,
    };

    let json = serde_json::to_string(&update).unwrap();

    // Verify all fields are present
    assert!(json.contains("\"hashrate\":1500000"));
    assert!(json.contains("\"hashrate_formatted\":\"1.50 MH/s\""));
    assert!(json.contains("\"hashes\":50000000"));
    assert!(json.contains("\"total_hashes\":50000000"));
    assert!(json.contains("\"shares_accepted\":10"));
    assert!(json.contains("\"shares_rejected\":2"));
    assert!(json.contains("\"blocks_found\":0"));
    assert!(json.contains("\"uptime_seconds\":3600"));
    assert!(json.contains("\"uptime\":3600"));
    assert!(json.contains("\"uptime_formatted\":\"1h 0m 0s\""));
    assert!(json.contains("\"mining_paused\":false"));
    assert!(json.contains("\"state\":\"running\""));
    assert!(json.contains("\"threads\":8"));
}

/// Test StatsUpdate clone
#[test]
fn test_stats_update_clone() {
    let update = StatsUpdate {
        hashrate: 1000.0,
        hashrate_formatted: "1.00 KH/s".to_string(),
        hashes: 100000,
        total_hashes: 100000,
        shares_accepted: 5,
        shares_rejected: 0,
        blocks_found: 1,
        uptime_seconds: 600,
        uptime: 600,
        uptime_formatted: "10m 0s".to_string(),
        mining_paused: true,
        state: "paused".to_string(),
        threads: 4,
    };

    let cloned = update.clone();

    assert_eq!(cloned.hashrate, update.hashrate);
    assert_eq!(cloned.hashrate_formatted, update.hashrate_formatted);
    assert_eq!(cloned.hashes, update.hashes);
    assert_eq!(cloned.shares_accepted, update.shares_accepted);
    assert_eq!(cloned.blocks_found, update.blocks_found);
    assert_eq!(cloned.mining_paused, update.mining_paused);
}

/// Test WebState creation
#[test]
fn test_web_state_creation() {
    let state = WebState::new("pool.example.com:3333".to_string(), 16);

    assert_eq!(state.pool_address, "pool.example.com:3333");
    assert_eq!(state.worker_count, 16);
}

/// Test paused state toggle
#[tokio::test]
async fn test_paused_toggle() {
    let state = create_test_state();

    assert!(!state.is_paused().await);

    state.set_paused(true).await;
    assert!(state.is_paused().await);

    state.set_paused(false).await;
    assert!(!state.is_paused().await);
}

/// Test stats endpoint returns updated values
#[tokio::test]
async fn test_api_stats_returns_updated_values() {
    let state = create_test_state();

    // Update stats
    let stats = StatsSnapshot {
        hashes: 99_999_999,
        gpu_hashes: 0,
        shares_accepted: 42,
        shares_rejected: 3,
        blocks_found: 1, // Found a block!
    };
    state.update_stats(stats, 2_500_000.0).await;

    let app = create_router(state, "web", None);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["hashes"].as_u64().unwrap(), 99_999_999);
    assert_eq!(json["shares_accepted"].as_u64().unwrap(), 42);
    assert_eq!(json["shares_rejected"].as_u64().unwrap(), 3);
    assert_eq!(json["blocks_found"].as_u64().unwrap(), 1);
    assert!((json["hashrate"].as_f64().unwrap() - 2_500_000.0).abs() < 0.1);
}

/// Test 404 for unknown routes
#[tokio::test]
async fn test_unknown_route_404() {
    let state = create_test_state();
    let app = create_router(state, "web", None);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/unknown")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Test stats formatting
#[test]
fn test_hashrate_formatting() {
    // Test that hashrate is formatted correctly
    let cases = vec![
        (1000.0, "1.00 KH/s"),
        (1_000_000.0, "1.00 MH/s"),
        (1_000_000_000.0, "1.00 GH/s"),
        (1_000_000_000_000.0, "1.00 TH/s"),
        (1_000_000_000_000_000.0, "1.00 PH/s"),
    ];

    for (hashrate, expected_suffix) in cases {
        let update = StatsUpdate {
            hashrate,
            hashrate_formatted: btcminer::stats::format_hashrate(hashrate),
            hashes: 0,
            total_hashes: 0,
            shares_accepted: 0,
            shares_rejected: 0,
            blocks_found: 0,
            uptime_seconds: 0,
            uptime: 0,
            uptime_formatted: String::new(),
            mining_paused: false,
            state: "stopped".to_string(),
            threads: 0,
        };

        assert_eq!(update.hashrate_formatted, expected_suffix,
            "Hashrate {} should format to {}", hashrate, expected_suffix);
    }
}

/// Test uptime formatting
#[test]
fn test_uptime_formatting() {
    let cases = vec![
        (0, "0s"),
        (59, "59s"),
        (60, "1m 0s"),
        (3599, "59m 59s"),
        (3600, "1h 0m 0s"),
        (86399, "23h 59m 59s"),
        (86400, "1d 0h 0m 0s"),
    ];

    for (seconds, expected) in cases {
        let formatted = btcminer::web::format_uptime(seconds);
        assert_eq!(formatted, expected, "Uptime {} seconds should format to {}", seconds, expected);
    }
}

/// Test GET /api/idle endpoint
#[tokio::test]
async fn test_api_idle_status() {
    let state = create_test_state();
    let app = create_router(state.clone(), "web", None);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/idle")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify response structure
    assert!(json.get("enabled").is_some());
    assert!(json.get("timeout").is_some());
    assert!(json.get("is_idle").is_some());
    assert!(json.get("idle_time").is_some());
    assert!(json.get("mining_due_to_idle").is_some());
}

/// Test POST /api/idle/enable endpoint
#[tokio::test]
async fn test_api_idle_enable() {
    let state = create_test_state();
    let app = create_router(state.clone(), "web", None);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/idle/enable?timeout=600")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["success"].as_bool().unwrap(), true);
    assert_eq!(json["enabled"].as_bool().unwrap(), true);
    assert_eq!(json["timeout"].as_u64().unwrap(), 600);
}

/// Test POST /api/idle/disable endpoint
#[tokio::test]
async fn test_api_idle_disable() {
    let state = create_test_state();

    // Enable first
    state.set_idle_enabled(true, Some(900)).await;

    let app = create_router(state.clone(), "web", None);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/idle/disable")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["success"].as_bool().unwrap(), true);
    assert_eq!(json["enabled"].as_bool().unwrap(), false);
}

/// Test GET /api/logs endpoint
#[tokio::test]
async fn test_api_logs() {
    let state = create_test_state();

    // Add some logs
    state.add_log("Test log 1".to_string(), "info").await;
    state.add_log("Test log 2".to_string(), "success").await;

    let app = create_router(state.clone(), "web", None);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/logs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let logs = json["logs"].as_array().unwrap();
    assert_eq!(logs.len(), 2);
    assert_eq!(logs[0]["message"].as_str().unwrap(), "Test log 1");
    assert_eq!(logs[1]["level"].as_str().unwrap(), "success");
}

/// Test GET /api/openapi.json endpoint
#[tokio::test]
async fn test_api_openapi() {
    let state = create_test_state();
    let app = create_router(state.clone(), "web", None);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify OpenAPI structure
    assert_eq!(json["openapi"].as_str().unwrap(), "3.0.3");
    assert!(json.get("info").is_some());
    assert!(json.get("paths").is_some());
    assert!(json.get("components").is_some());
    assert!(json["paths"].get("/api/stats").is_some());
    assert!(json["paths"].get("/api/idle").is_some());
}
