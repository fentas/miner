mod v1;
mod v2;

pub use v1::{StratumV1Client, Notification};
pub use v2::StratumV2Client;

use crate::mining::Job;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StratumError {
    #[error("Connection failed: {0}")]
    ConnectionError(String),
    #[error("Authentication failed: {0}")]
    AuthError(String),
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Timeout")]
    Timeout,
}

/// Stratum client events
#[derive(Debug, Clone)]
pub enum StratumEvent {
    Connected,
    Disconnected,
    Authorized,
    JobReceived(Job),
    ShareAccepted { job_id: String },
    ShareRejected { job_id: String, reason: String },
    DifficultyChanged(f64),
    Error(String),
}

// Note: Each stratum client (V1, V2) has its own interface
// as the protocols differ significantly in structure

/// Parse pool address into host and port
pub fn parse_pool_address(address: &str) -> Result<(String, u16), StratumError> {
    let parts: Vec<&str> = address.split(':').collect();
    if parts.len() != 2 {
        return Err(StratumError::ConnectionError(
            "Invalid pool address format, expected host:port".to_string(),
        ));
    }

    let host = parts[0].to_string();
    let port = parts[1]
        .parse::<u16>()
        .map_err(|_| StratumError::ConnectionError("Invalid port number".to_string()))?;

    Ok((host, port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pool_address_valid() {
        let (host, port) = parse_pool_address("solo.ckpool.org:3333").unwrap();
        assert_eq!(host, "solo.ckpool.org");
        assert_eq!(port, 3333);
    }

    #[test]
    fn test_parse_pool_address_ip() {
        let (host, port) = parse_pool_address("127.0.0.1:8080").unwrap();
        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_parse_pool_address_invalid_no_port() {
        let result = parse_pool_address("solo.ckpool.org");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_pool_address_invalid_port() {
        let result = parse_pool_address("solo.ckpool.org:notaport");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_pool_address_empty() {
        let result = parse_pool_address("");
        assert!(result.is_err());
    }
}
