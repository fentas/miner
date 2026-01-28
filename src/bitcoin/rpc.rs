use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{BitcoinError, BlockTemplate, SyncStatus};

/// Bitcoin Core JSON-RPC client
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

        rpc_response.result.ok_or_else(|| BitcoinError::RpcError("Empty result".into()))
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

        // Parse coinbase transaction
        let coinbasevalue = result["coinbasevalue"].as_u64().unwrap_or(0);

        Ok(BlockTemplate {
            version: result["version"].as_u64().unwrap_or(0x20000000) as u32,
            prev_block_hash: result["previousblockhash"].as_str().unwrap_or("").to_string(),
            merkle_root: String::new(), // Computed from transactions
            timestamp: result["curtime"].as_u64().unwrap_or(0),
            bits: result["bits"].as_str().unwrap_or("").to_string(),
            height: result["height"].as_u64().unwrap_or(0),
            coinbase_value: coinbasevalue,
            transactions: result["transactions"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|tx| tx["data"].as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            target: result["target"].as_str().unwrap_or("").to_string(),
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
}
