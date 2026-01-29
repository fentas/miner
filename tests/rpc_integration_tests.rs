//! Integration tests for Bitcoin RPC communication
//!
//! Tests the RPC client against mock responses to validate:
//! - Request/response serialization
//! - Error handling
//! - Block template parsing
//! - Block submission flow

use serde_json::json;
use wiremock::{Mock, MockServer, ResponseTemplate};
use wiremock::matchers::method;

use btcminer::bitcoin::BitcoinRpc;

/// Test RPC client creation
#[test]
fn test_rpc_client_creation() {
    let _rpc = BitcoinRpc::new("127.0.0.1", 8332);
    // Client should be created successfully
}

/// Test RPC client with authentication
#[test]
fn test_rpc_client_with_auth() {
    let _rpc = BitcoinRpc::new("localhost", 8332)
        .with_auth("rpcuser", "rpcpassword");
    // Should not panic
}

/// Test getblockchaininfo response parsing
#[tokio::test]
async fn test_mock_getblockchaininfo() {
    let mock_server = MockServer::start().await;

    // Set up mock response
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": {
                "chain": "main",
                "blocks": 850000,
                "headers": 850000,
                "bestblockhash": "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f",
                "difficulty": 83148355189239.77,
                "verificationprogress": 0.9999999,
                "initialblockdownload": false
            },
            "error": null,
            "id": 1
        })))
        .mount(&mock_server)
        .await;

    // Parse the URI to get host and port
    let uri = mock_server.uri();
    let parts: Vec<&str> = uri.trim_start_matches("http://").split(':').collect();
    let host = parts[0];
    let port: u16 = parts[1].parse().unwrap();

    let rpc = BitcoinRpc::new(host, port);
    let result = rpc.get_blockchain_info().await;

    assert!(result.is_ok());
    let status = result.unwrap();
    assert!(status.synced);
    assert_eq!(status.blocks, 850000);
    assert_eq!(status.headers, 850000);
}

/// Test getblockchaininfo during sync
#[tokio::test]
async fn test_mock_getblockchaininfo_syncing() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": {
                "chain": "main",
                "blocks": 400000,
                "headers": 850000,
                "bestblockhash": "000000000000000abc",
                "difficulty": 1000000.0,
                "verificationprogress": 0.47,
                "initialblockdownload": true
            },
            "error": null,
            "id": 1
        })))
        .mount(&mock_server)
        .await;

    let uri = mock_server.uri();
    let parts: Vec<&str> = uri.trim_start_matches("http://").split(':').collect();
    let rpc = BitcoinRpc::new(parts[0], parts[1].parse().unwrap());

    let result = rpc.get_blockchain_info().await;
    assert!(result.is_ok());

    let status = result.unwrap();
    assert!(!status.synced); // Still syncing
    assert!(status.progress < 100.0);
    assert!(status.blocks < status.headers);
}

/// Test submitblock success response
#[tokio::test]
async fn test_mock_submitblock_success() {
    let mock_server = MockServer::start().await;

    // submitblock returns null on success
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": null,
            "error": null,
            "id": 1
        })))
        .mount(&mock_server)
        .await;

    let uri = mock_server.uri();
    let parts: Vec<&str> = uri.trim_start_matches("http://").split(':').collect();
    let rpc = BitcoinRpc::new(parts[0], parts[1].parse().unwrap());

    // Create a dummy block hex
    let block_hex = hex::encode([0u8; 100]);
    let result = rpc.submit_block(&block_hex).await;

    assert!(result.is_ok(), "Expected success but got: {:?}", result.err());
}

/// Test submitblock rejection
#[tokio::test]
async fn test_mock_submitblock_rejection() {
    let mock_server = MockServer::start().await;

    // submitblock returns error string on rejection
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": "duplicate",
            "error": null,
            "id": 1
        })))
        .mount(&mock_server)
        .await;

    let uri = mock_server.uri();
    let parts: Vec<&str> = uri.trim_start_matches("http://").split(':').collect();
    let rpc = BitcoinRpc::new(parts[0], parts[1].parse().unwrap());

    let block_hex = hex::encode([0u8; 100]);
    let result = rpc.submit_block(&block_hex).await;

    // Should be an error because result is not null
    assert!(result.is_err());
}

/// Test RPC error handling (warmup)
#[tokio::test]
async fn test_mock_rpc_warmup_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": null,
            "error": {
                "code": -28,
                "message": "Loading block index..."
            },
            "id": 1
        })))
        .mount(&mock_server)
        .await;

    let uri = mock_server.uri();
    let parts: Vec<&str> = uri.trim_start_matches("http://").split(':').collect();
    let rpc = BitcoinRpc::new(parts[0], parts[1].parse().unwrap());

    let result = rpc.get_blockchain_info().await;
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(err.to_string().contains("warming up") || err.to_string().contains("-28"));
}

/// Test getdifficulty response
#[tokio::test]
async fn test_mock_getdifficulty() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": 83148355189239.77,
            "error": null,
            "id": 1
        })))
        .mount(&mock_server)
        .await;

    let uri = mock_server.uri();
    let parts: Vec<&str> = uri.trim_start_matches("http://").split(':').collect();
    let rpc = BitcoinRpc::new(parts[0], parts[1].parse().unwrap());

    let result = rpc.get_difficulty().await;
    assert!(result.is_ok());

    let difficulty = result.unwrap();
    assert!(difficulty > 0.0);
    assert!((difficulty - 83148355189239.77).abs() < 1.0);
}

/// Test getnetworkhashps response
#[tokio::test]
async fn test_mock_getnetworkhashps() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": 5.5e20, // 550 EH/s
            "error": null,
            "id": 1
        })))
        .mount(&mock_server)
        .await;

    let uri = mock_server.uri();
    let parts: Vec<&str> = uri.trim_start_matches("http://").split(':').collect();
    let rpc = BitcoinRpc::new(parts[0], parts[1].parse().unwrap());

    let result = rpc.get_network_hashrate().await;
    assert!(result.is_ok());

    let hashrate = result.unwrap();
    assert!(hashrate > 0.0);
}

/// Test connection failure handling
#[tokio::test]
async fn test_rpc_connection_failure() {
    // Connect to a port that's definitely not listening
    let rpc = BitcoinRpc::new("127.0.0.1", 59999);

    let result = rpc.ping().await;
    assert!(result.is_err());

    let err = result.unwrap_err();
    // Should be a connection error
    assert!(err.to_string().to_lowercase().contains("connection") ||
            err.to_string().to_lowercase().contains("error"));
}

/// Test ping (getbestblockhash) response
#[tokio::test]
async fn test_mock_ping() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f",
            "error": null,
            "id": 1
        })))
        .mount(&mock_server)
        .await;

    let uri = mock_server.uri();
    let parts: Vec<&str> = uri.trim_start_matches("http://").split(':').collect();
    let rpc = BitcoinRpc::new(parts[0], parts[1].parse().unwrap());

    let result = rpc.ping().await;
    assert!(result.is_ok());
}

/// Test HTTP error handling
#[tokio::test]
async fn test_mock_http_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&mock_server)
        .await;

    let uri = mock_server.uri();
    let parts: Vec<&str> = uri.trim_start_matches("http://").split(':').collect();
    let rpc = BitcoinRpc::new(parts[0], parts[1].parse().unwrap());

    let result = rpc.ping().await;
    assert!(result.is_err());
}

/// Test invalid JSON response handling
#[tokio::test]
async fn test_mock_invalid_json() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not valid json"))
        .mount(&mock_server)
        .await;

    let uri = mock_server.uri();
    let parts: Vec<&str> = uri.trim_start_matches("http://").split(':').collect();
    let rpc = BitcoinRpc::new(parts[0], parts[1].parse().unwrap());

    let result = rpc.ping().await;
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(err.to_string().to_lowercase().contains("json") ||
            err.to_string().to_lowercase().contains("invalid"));
}
