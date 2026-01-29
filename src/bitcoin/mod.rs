mod rpc;
mod manager;
mod metadata_fetcher;

pub use rpc::BitcoinRpc;
pub use manager::BitcoindManager;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum BitcoinError {
    #[error("RPC error: {0}")]
    RpcError(String),
    #[error("Connection error: {0}")]
    ConnectionError(String),
    #[error("Not synced: {progress}% complete")]
    NotSynced { progress: f64 },
    #[error("Download error: {0}")]
    DownloadError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Block template from Bitcoin Core
#[derive(Debug, Clone)]
pub struct BlockTemplate {
    pub version: u32,
    pub prev_block_hash: String,
    pub merkle_root: String,
    pub timestamp: u64,
    pub bits: String,
    pub height: u64,
    pub coinbase_value: u64,
    pub transactions: Vec<String>,
    pub target: String,
}

/// Bitcoin Core sync status
#[derive(Debug, Clone)]
pub struct SyncStatus {
    pub synced: bool,
    pub progress: f64,
    pub headers: u64,
    pub blocks: u64,
    pub best_block_hash: String,
    pub difficulty: f64,
}

/// Progress status for AssumeUTXO loading
#[derive(Debug, Clone)]
pub enum LoadProgress {
    /// Syncing block headers (current, target)
    SyncingHeaders { current: u64, target: u64 },
    /// Loading the UTXO snapshot into Bitcoin Core
    /// - elapsed_secs: how long we've been loading
    /// - progress_pct: optional loading percentage from debug.log (0.0-100.0)
    LoadingSnapshot { elapsed_secs: u64, progress_pct: Option<f64> },
    /// Snapshot already not needed (blocks already synced past snapshot height)
    AlreadySynced,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_status_creation() {
        let status = SyncStatus {
            synced: true,
            progress: 100.0,
            headers: 850000,
            blocks: 850000,
            best_block_hash: "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f".to_string(),
            difficulty: 83148355189239.77,
        };
        assert!(status.synced);
        assert_eq!(status.progress, 100.0);
        assert_eq!(status.headers, 850000);
    }

    #[test]
    fn test_block_template_creation() {
        let template = BlockTemplate {
            version: 0x20000000,
            prev_block_hash: "0000000000000000000123456789abcdef".to_string(),
            merkle_root: String::new(),
            timestamp: 1700000000,
            bits: "1d00ffff".to_string(),
            height: 850000,
            coinbase_value: 312500000,
            transactions: vec!["tx1".to_string(), "tx2".to_string()],
            target: "00000000ffff0000000000000000000000000000000000000000000000000000".to_string(),
        };
        assert_eq!(template.version, 0x20000000);
        assert_eq!(template.height, 850000);
        assert_eq!(template.transactions.len(), 2);
    }

    #[test]
    fn test_bitcoin_error_display() {
        let err = BitcoinError::NotSynced { progress: 95.5 };
        assert!(err.to_string().contains("95.5%"));

        let err = BitcoinError::RpcError("test error".to_string());
        assert!(err.to_string().contains("test error"));
    }
}
