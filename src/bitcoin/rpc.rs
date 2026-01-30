use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{BitcoinError, BlockTemplate, SyncStatus};

/// Bitcoin Core JSON-RPC client
#[derive(Clone)]
pub struct BitcoinRpc {
    client: Client,
    url: String,
    auth: Option<(String, String)>,
}

#[derive(Serialize)]
struct RpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    params: Value,
}

#[derive(Deserialize)]
struct RpcResponse {
    result: Option<Value>,
    error: Option<RpcError>,
}

#[derive(Deserialize)]
struct RpcError {
    code: i32,
    message: String,
}

impl BitcoinRpc {
    /// Create a new RPC client
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            client: Client::new(),
            url: format!("http://{}:{}", host, port),
            auth: None,
        }
    }

    /// Set RPC authentication
    pub fn with_auth(mut self, user: &str, password: &str) -> Self {
        self.auth = Some((user.to_string(), password.to_string()));
        self
    }

    /// Execute an RPC call
    async fn call(&self, method: &str, params: Value) -> Result<Value, BitcoinError> {
        let request = RpcRequest {
            jsonrpc: "1.0",
            id: 1,
            method: method.to_string(),
            params,
        };

        let mut req = self.client.post(&self.url)
            .header("Content-Type", "application/json")
            .json(&request);

        if let Some((ref user, ref pass)) = self.auth {
            req = req.basic_auth(user, Some(pass));
        }

        let response = req
            .send()
            .await
            .map_err(|e| BitcoinError::ConnectionError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| BitcoinError::RpcError(format!("Failed to read response: {}", e)))?;

        // Check for HTTP errors
        if !status.is_success() {
            // Bitcoin Core returns specific errors during warmup
            if body.contains("Loading") || body.contains("Verifying") || body.contains("Starting") {
                return Err(BitcoinError::RpcError(format!("Bitcoin Core starting: {}", body.trim())));
            }
            return Err(BitcoinError::RpcError(format!("HTTP {}: {}", status, body.chars().take(200).collect::<String>())));
        }

        // Handle empty response (can happen during startup)
        if body.is_empty() {
            return Err(BitcoinError::RpcError("Empty response from Bitcoin Core".into()));
        }

        // Parse JSON response
        let rpc_response: RpcResponse = serde_json::from_str(&body)
            .map_err(|e| BitcoinError::RpcError(format!("Invalid JSON: {} (body: {})", e, body.chars().take(100).collect::<String>())))?;

        if let Some(err) = rpc_response.error {
            // Handle warmup errors specially
            if err.code == -28 {
                return Err(BitcoinError::RpcError(format!("Bitcoin Core warming up: {}", err.message)));
            }
            return Err(BitcoinError::RpcError(format!("{}: {}", err.code, err.message)));
        }

        // Return Null for null results (like submitblock success)
        Ok(rpc_response.result.unwrap_or(Value::Null))
    }

    /// Get blockchain info
    pub async fn get_blockchain_info(&self) -> Result<SyncStatus, BitcoinError> {
        let result = self.call("getblockchaininfo", json!([])).await?;

        Ok(SyncStatus {
            synced: result["initialblockdownload"].as_bool() == Some(false),
            progress: result["verificationprogress"].as_f64().unwrap_or(0.0) * 100.0,
            headers: result["headers"].as_u64().unwrap_or(0),
            blocks: result["blocks"].as_u64().unwrap_or(0),
            best_block_hash: result["bestblockhash"].as_str().unwrap_or("").to_string(),
            difficulty: result["difficulty"].as_f64().unwrap_or(0.0),
        })
    }

    /// Get a block template for mining
    pub async fn get_block_template(&self, _address: &str) -> Result<BlockTemplate, BitcoinError> {
        // First check sync status
        let status = self.get_blockchain_info().await?;
        if !status.synced {
            return Err(BitcoinError::NotSynced { progress: status.progress });
        }

        // Get block template
        let rules = json!({"rules": ["segwit"]});
        let result = self.call("getblocktemplate", json!([rules])).await?;

        // Validate required fields
        let prev_block_hash = result["previousblockhash"]
            .as_str()
            .ok_or_else(|| BitcoinError::RpcError("Missing previousblockhash in template".into()))?
            .to_string();

        let bits = result["bits"]
            .as_str()
            .ok_or_else(|| BitcoinError::RpcError("Missing bits in template".into()))?
            .to_string();

        let height = result["height"]
            .as_u64()
            .ok_or_else(|| BitcoinError::RpcError("Missing height in template".into()))?;

        let timestamp = result["curtime"]
            .as_u64()
            .ok_or_else(|| BitcoinError::RpcError("Missing curtime in template".into()))?;

        let coinbase_value = result["coinbasevalue"]
            .as_u64()
            .ok_or_else(|| BitcoinError::RpcError("Missing coinbasevalue in template".into()))?;

        // Target is required for difficulty calculation
        let target = result["target"]
            .as_str()
            .ok_or_else(|| BitcoinError::RpcError("Missing target in template".into()))?
            .to_string();

        Ok(BlockTemplate {
            version: result["version"].as_u64().unwrap_or(0x20000000) as u32,
            prev_block_hash,
            merkle_root: String::new(), // Computed from transactions
            timestamp,
            bits,
            height,
            coinbase_value,
            transactions: result["transactions"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|tx| tx["data"].as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            target,
        })
    }

    /// Submit a solved block
    pub async fn submit_block(&self, block_hex: &str) -> Result<(), BitcoinError> {
        let result = self.call("submitblock", json!([block_hex])).await?;

        // submitblock returns null on success
        if result.is_null() {
            Ok(())
        } else {
            Err(BitcoinError::RpcError(result.to_string()))
        }
    }

    /// Get network difficulty
    pub async fn get_difficulty(&self) -> Result<f64, BitcoinError> {
        let result = self.call("getdifficulty", json!([])).await?;
        result.as_f64().ok_or_else(|| BitcoinError::RpcError("Invalid difficulty".into()))
    }

    /// Get network hash rate estimate
    pub async fn get_network_hashrate(&self) -> Result<f64, BitcoinError> {
        let result = self.call("getnetworkhashps", json!([])).await?;
        result.as_f64().ok_or_else(|| BitcoinError::RpcError("Invalid hashrate".into()))
    }

    /// Test connection
    pub async fn ping(&self) -> Result<(), BitcoinError> {
        self.call("getbestblockhash", json!([])).await?;
        Ok(())
    }

    /// Get the number of connected peers
    pub async fn get_connection_count(&self) -> Result<u64, BitcoinError> {
        let result = self.call("getconnectioncount", json!([])).await?;
        result.as_u64().ok_or_else(|| BitcoinError::RpcError("Invalid connection count".into()))
    }

    /// Get network info including peer count and warnings
    pub async fn get_network_info(&self) -> Result<Value, BitcoinError> {
        self.call("getnetworkinfo", json!([])).await
    }

    /// Load UTXO snapshot for AssumeUTXO fast sync
    pub async fn load_utxo_snapshot(&self, path: &std::path::Path) -> Result<(), BitcoinError> {
        let path_str = path.to_string_lossy();
        let result = self.call("loadtxoutset", json!([path_str])).await?;

        // Check for errors in result
        if let Some(error) = result.get("error") {
            if !error.is_null() {
                return Err(BitcoinError::RpcError(error.to_string()));
            }
        }

        Ok(())
    }

    /// Request graceful shutdown of Bitcoin Core
    pub async fn stop(&self) -> Result<(), BitcoinError> {
        // The stop RPC returns a message like "Bitcoin Core stopping"
        let _ = self.call("stop", json!([])).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitcoin_rpc_new() {
        let rpc = BitcoinRpc::new("127.0.0.1", 8332);
        assert_eq!(rpc.url, "http://127.0.0.1:8332");
        assert!(rpc.auth.is_none());
    }

    #[test]
    fn test_bitcoin_rpc_with_auth() {
        let rpc = BitcoinRpc::new("localhost", 8332)
            .with_auth("user", "password");
        assert!(rpc.auth.is_some());
        let (user, pass) = rpc.auth.unwrap();
        assert_eq!(user, "user");
        assert_eq!(pass, "password");
    }

    #[test]
    fn test_bitcoin_rpc_custom_port() {
        let rpc = BitcoinRpc::new("192.168.1.100", 18332);
        assert_eq!(rpc.url, "http://192.168.1.100:18332");
    }

    #[test]
    fn test_bitcoin_rpc_ipv6() {
        let rpc = BitcoinRpc::new("::1", 8332);
        assert_eq!(rpc.url, "http://::1:8332");
    }

    #[test]
    fn test_bitcoin_rpc_auth_chaining() {
        let rpc = BitcoinRpc::new("localhost", 8332)
            .with_auth("rpcuser", "rpcpass");
        assert!(rpc.auth.is_some());
    }

    #[test]
    fn test_rpc_request_serialization() {
        let request = RpcRequest {
            jsonrpc: "1.0",
            id: 42,
            method: "getblockchaininfo".to_string(),
            params: json!([]),
        };

        let serialized = serde_json::to_string(&request).unwrap();
        assert!(serialized.contains("\"jsonrpc\":\"1.0\""));
        assert!(serialized.contains("\"id\":42"));
        assert!(serialized.contains("\"method\":\"getblockchaininfo\""));
    }

    #[test]
    fn test_rpc_response_deserialization_success() {
        let json = r#"{"result": {"blocks": 100}, "error": null}"#;
        let response: RpcResponse = serde_json::from_str(json).unwrap();
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_rpc_response_deserialization_error() {
        let json = r#"{"result": null, "error": {"code": -28, "message": "Warming up"}}"#;
        let response: RpcResponse = serde_json::from_str(json).unwrap();
        assert!(response.result.is_none());
        assert!(response.error.is_some());
        let err = response.error.unwrap();
        assert_eq!(err.code, -28);
        assert_eq!(err.message, "Warming up");
    }

    #[test]
    fn test_rpc_error_deserialization() {
        let json = r#"{"code": -32601, "message": "Method not found"}"#;
        let error: RpcError = serde_json::from_str(json).unwrap();
        assert_eq!(error.code, -32601);
        assert_eq!(error.message, "Method not found");
    }

    #[test]
    fn test_sync_status_from_json() {
        // Test parsing of blockchain info JSON
        let result = json!({
            "initialblockdownload": false,
            "verificationprogress": 0.999999,
            "headers": 800000,
            "blocks": 800000,
            "bestblockhash": "0000000000000000000abc",
            "difficulty": 55621444139429.57
        });

        let status = SyncStatus {
            synced: result["initialblockdownload"].as_bool() == Some(false),
            progress: result["verificationprogress"].as_f64().unwrap_or(0.0) * 100.0,
            headers: result["headers"].as_u64().unwrap_or(0),
            blocks: result["blocks"].as_u64().unwrap_or(0),
            best_block_hash: result["bestblockhash"].as_str().unwrap_or("").to_string(),
            difficulty: result["difficulty"].as_f64().unwrap_or(0.0),
        };

        assert!(status.synced);
        assert!(status.progress > 99.0);
        assert_eq!(status.headers, 800000);
        assert_eq!(status.blocks, 800000);
    }

    #[test]
    fn test_block_template_parsing() {
        let result = json!({
            "version": 0x20000000_u64,
            "previousblockhash": "00000000000000000001abc",
            "curtime": 1700000000_u64,
            "bits": "1703a8b4",
            "height": 800001_u64,
            "coinbasevalue": 312500000_u64,
            "transactions": [
                {"data": "tx1hex"},
                {"data": "tx2hex"}
            ],
            "target": "0000000000000000000abc"
        });

        let template = BlockTemplate {
            version: result["version"].as_u64().unwrap_or(0x20000000) as u32,
            prev_block_hash: result["previousblockhash"].as_str().unwrap_or("").to_string(),
            merkle_root: String::new(),
            timestamp: result["curtime"].as_u64().unwrap_or(0),
            bits: result["bits"].as_str().unwrap_or("").to_string(),
            height: result["height"].as_u64().unwrap_or(0),
            coinbase_value: result["coinbasevalue"].as_u64().unwrap_or(0),
            transactions: result["transactions"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|tx| tx["data"].as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            target: result["target"].as_str().unwrap_or("").to_string(),
        };

        assert_eq!(template.version, 0x20000000);
        assert_eq!(template.height, 800001);
        assert_eq!(template.coinbase_value, 312500000);
        assert_eq!(template.transactions.len(), 2);
    }
}
