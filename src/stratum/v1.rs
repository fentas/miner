use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use super::{parse_pool_address, StratumError};
use crate::mining::Job;

/// Stratum V1 JSON-RPC client
pub struct StratumV1Client {
    stream: Option<TcpStream>,
    reader: Option<BufReader<TcpStream>>,
    extranonce1: Vec<u8>,
    extranonce2_size: usize,
    difficulty: f64,
    connected: AtomicBool,
    request_id: AtomicU64,
}

impl StratumV1Client {
    pub fn new() -> Self {
        Self {
            stream: None,
            reader: None,
            extranonce1: Vec::new(),
            extranonce2_size: 4,
            difficulty: 1.0,
            connected: AtomicBool::new(false),
            request_id: AtomicU64::new(1),
        }
    }

    /// Connect to pool
    pub fn connect(&mut self, address: &str) -> Result<(), StratumError> {
        let (host, port) = parse_pool_address(address)?;

        let socket_addr = format!("{}:{}", host, port)
            .parse()
            .map_err(|e| StratumError::ConnectionError(format!("Invalid address: {}", e)))?;

        let stream = TcpStream::connect_timeout(
            &socket_addr,
            Duration::from_secs(30),
        )
        .map_err(|e| StratumError::ConnectionError(e.to_string()))?;

        stream.set_read_timeout(Some(Duration::from_secs(60)))?;
        stream.set_write_timeout(Some(Duration::from_secs(30)))?;

        let reader = BufReader::new(stream.try_clone()?);

        self.stream = Some(stream);
        self.reader = Some(reader);
        self.connected.store(true, Ordering::SeqCst);

        Ok(())
    }

    /// Connect to pool with exponential backoff retry
    /// Retries up to `max_retries` times with exponentially increasing delay
    pub fn connect_with_retry(&mut self, address: &str, max_retries: u32) -> Result<(), StratumError> {
        let mut delay = Duration::from_secs(1);
        let max_delay = Duration::from_secs(60);

        for attempt in 0..=max_retries {
            match self.connect(address) {
                Ok(()) => return Ok(()),
                Err(e) if attempt < max_retries => {
                    // Add jitter (±25% of delay)
                    let jitter_range = delay.as_millis() as u64 / 4;
                    let jitter = if jitter_range > 0 {
                        (std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos() as u64) % (jitter_range * 2)
                    } else {
                        0
                    };
                    let actual_delay = Duration::from_millis(
                        delay.as_millis() as u64 - jitter_range + jitter
                    );

                    std::thread::sleep(actual_delay);

                    // Exponential backoff (double each time, capped at max_delay)
                    delay = std::cmp::min(delay * 2, max_delay);
                }
                Err(e) => return Err(e),
            }
        }

        Err(StratumError::ConnectionError("Max retries exceeded".into()))
    }

    /// Disconnect from pool
    pub fn disconnect(&mut self) {
        self.connected.store(false, Ordering::SeqCst);
        self.stream = None;
        self.reader = None;
    }

    /// Subscribe to mining notifications
    pub fn subscribe(&mut self) -> Result<(), StratumError> {
        let request = json!({
            "id": self.next_id(),
            "method": "mining.subscribe",
            "params": ["btcminer/0.1.0"]
        });

        self.send(&request)?;
        let response = self.recv()?;

        // Parse subscription response
        if let Some(result) = response.get("result") {
            if let Some(arr) = result.as_array() {
                // extranonce1 is at index 1
                if let Some(en1) = arr.get(1).and_then(|v| v.as_str()) {
                    self.extranonce1 = hex::decode(en1)
                        .map_err(|_| StratumError::ProtocolError("Invalid extranonce1".into()))?;
                }
                // extranonce2_size is at index 2 (must be 1-8 bytes)
                if let Some(en2_size) = arr.get(2).and_then(|v| v.as_u64()) {
                    if !(1..=8).contains(&en2_size) {
                        return Err(StratumError::ProtocolError(
                            format!("Invalid extranonce2_size: expected 1-8, got {}", en2_size),
                        ));
                    }
                    self.extranonce2_size = en2_size as usize;
                }
            }
        }

        Ok(())
    }

    /// Authorize with the pool
    pub fn authorize(&mut self, username: &str, password: &str) -> Result<(), StratumError> {
        let request = json!({
            "id": self.next_id(),
            "method": "mining.authorize",
            "params": [username, password]
        });

        self.send(&request)?;
        let response = self.recv()?;

        // Check if authorized
        if response.get("result") == Some(&Value::Bool(true)) {
            Ok(())
        } else {
            let error = response
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("Unknown error");
            Err(StratumError::AuthError(error.to_string()))
        }
    }

    /// Submit a share
    pub fn submit(
        &mut self,
        username: &str,
        job_id: &str,
        extranonce2: &str,
        ntime: &str,
        nonce: &str,
    ) -> Result<bool, StratumError> {
        let request = json!({
            "id": self.next_id(),
            "method": "mining.submit",
            "params": [username, job_id, extranonce2, ntime, nonce]
        });

        self.send(&request)?;
        let response = self.recv()?;

        Ok(response.get("result") == Some(&Value::Bool(true)))
    }

    /// Receive and parse a notification (non-blocking check) with size limit
    pub fn poll_notification(&mut self) -> Result<Option<Notification>, StratumError> {
        let reader = self.reader.as_mut().ok_or_else(|| {
            StratumError::ConnectionError("Not connected".to_string())
        })?;

        // Limit line size to 64KB to prevent memory exhaustion
        const MAX_LINE_SIZE: usize = 64 * 1024;
        let mut line = String::new();
        let mut total_read = 0;

        loop {
            let buf = match reader.fill_buf() {
                Ok(b) => b,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(None),
                Err(e) => return Err(e.into()),
            };

            if buf.is_empty() {
                return Err(StratumError::ConnectionError("Connection closed".to_string()));
            }

            let newline_pos = buf.iter().position(|&b| b == b'\n');
            let bytes_to_consume = newline_pos.map(|p| p + 1).unwrap_or(buf.len());

            if total_read + bytes_to_consume > MAX_LINE_SIZE {
                return Err(StratumError::ProtocolError(
                    format!("Response too large (>{} bytes)", MAX_LINE_SIZE)
                ));
            }

            let end = if newline_pos.is_some() { bytes_to_consume - 1 } else { bytes_to_consume };
            line.push_str(
                std::str::from_utf8(&buf[..end])
                    .map_err(|_| StratumError::ProtocolError("Invalid UTF-8".to_string()))?
            );
            total_read += bytes_to_consume;
            reader.consume(bytes_to_consume);

            if newline_pos.is_some() {
                break;
            }
        }

        let msg: Value = serde_json::from_str(&line)
            .map_err(|e| StratumError::ProtocolError(e.to_string()))?;

        self.parse_notification(&msg)
    }

    /// Parse a stratum notification
    pub fn parse_notification(&mut self, msg: &Value) -> Result<Option<Notification>, StratumError> {
        let method = msg.get("method").and_then(|m| m.as_str());

        match method {
            Some("mining.notify") => {
                let params = msg.get("params").and_then(|p| p.as_array())
                    .ok_or_else(|| StratumError::ProtocolError("Invalid notify params".into()))?;

                let job = self.parse_job(params)?;
                Ok(Some(Notification::Job(Box::new(job))))
            }
            Some("mining.set_difficulty") => {
                if let Some(diff) = msg.get("params")
                    .and_then(|p| p.as_array())
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_f64())
                {
                    self.difficulty = diff;
                    Ok(Some(Notification::Difficulty(diff)))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    /// Parse job from mining.notify params
    fn parse_job(&self, params: &[Value]) -> Result<Job, StratumError> {
        let get_str = |idx: usize| -> Result<&str, StratumError> {
            params.get(idx)
                .and_then(|v| v.as_str())
                .ok_or_else(|| StratumError::ProtocolError(format!("Missing param at index {}", idx)))
        };

        let job_id = get_str(0)?;
        let prev_hash = get_str(1)?;
        let coinbase1 = get_str(2)?;
        let coinbase2 = get_str(3)?;
        let merkle_branches = params.get(4)
            .and_then(|v| v.as_array())
            .ok_or_else(|| StratumError::ProtocolError("Missing merkle branches".into()))?;
        // Limit merkle branches to prevent DoS (32 branches = 2^32 txs max)
        if merkle_branches.len() > 32 {
            return Err(StratumError::ProtocolError(
                format!("Too many merkle branches: {} (max 32)", merkle_branches.len()),
            ));
        }
        let version = get_str(5)?;
        let nbits = get_str(6)?;
        let ntime = get_str(7)?;
        let clean_jobs = params.get(8)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut job = Job::new(job_id.to_string());

        // Parse previous hash (must be exactly 32 bytes)
        let prev_hash_bytes = hex::decode(prev_hash)
            .map_err(|_| StratumError::ProtocolError("Invalid prev_hash hex".into()))?;
        if prev_hash_bytes.len() != 32 {
            return Err(StratumError::ProtocolError(
                format!("Invalid prev_hash length: expected 32 bytes, got {}", prev_hash_bytes.len())
            ));
        }
        job.prev_hash.copy_from_slice(&prev_hash_bytes);

        // Parse coinbase parts
        job.coinbase1 = hex::decode(coinbase1)
            .map_err(|_| StratumError::ProtocolError("Invalid coinbase1".into()))?;
        job.coinbase2 = hex::decode(coinbase2)
            .map_err(|_| StratumError::ProtocolError("Invalid coinbase2".into()))?;

        // Parse merkle branches (each must be exactly 32 bytes)
        for (i, branch) in merkle_branches.iter().enumerate() {
            let s = branch.as_str().ok_or_else(|| {
                StratumError::ProtocolError(format!("Merkle branch {} is not a string", i))
            })?;
            let bytes = hex::decode(s)
                .map_err(|_| StratumError::ProtocolError(format!("Invalid merkle branch {} hex", i)))?;
            if bytes.len() != 32 {
                return Err(StratumError::ProtocolError(
                    format!("Invalid merkle branch {} length: expected 32 bytes, got {}", i, bytes.len())
                ));
            }
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            job.merkle_branches.push(arr);
        }

        // Parse version, nbits, ntime
        job.version = u32::from_str_radix(version, 16)
            .map_err(|_| StratumError::ProtocolError("Invalid version".into()))?;
        job.nbits = u32::from_str_radix(nbits, 16)
            .map_err(|_| StratumError::ProtocolError("Invalid nbits".into()))?;
        job.ntime = u32::from_str_radix(ntime, 16)
            .map_err(|_| StratumError::ProtocolError("Invalid ntime".into()))?;

        // Set target from difficulty
        job.target = crate::mining::difficulty_to_target(self.difficulty);
        job.clean_jobs = clean_jobs;

        Ok(job)
    }

    /// Send a JSON-RPC request
    fn send(&mut self, request: &Value) -> Result<(), StratumError> {
        let stream = self.stream.as_mut().ok_or_else(|| {
            StratumError::ConnectionError("Not connected".to_string())
        })?;

        let data = serde_json::to_string(request)
            .map_err(|e| StratumError::ProtocolError(format!("JSON serialization failed: {}", e)))?
            + "\n";
        stream.write_all(data.as_bytes())?;
        stream.flush()?;

        Ok(())
    }

    /// Receive a JSON-RPC response with size limit to prevent DoS
    fn recv(&mut self) -> Result<Value, StratumError> {
        let reader = self.reader.as_mut().ok_or_else(|| {
            StratumError::ConnectionError("Not connected".to_string())
        })?;

        // Limit line size to 64KB to prevent memory exhaustion from malicious servers
        const MAX_LINE_SIZE: usize = 64 * 1024;
        let mut line = String::new();
        let mut total_read = 0;

        loop {
            let buf = reader.fill_buf()?;
            if buf.is_empty() {
                return Err(StratumError::ConnectionError("Connection closed".to_string()));
            }

            // Find newline or end of buffer
            let newline_pos = buf.iter().position(|&b| b == b'\n');
            let bytes_to_consume = newline_pos.map(|p| p + 1).unwrap_or(buf.len());

            // Check size limit
            if total_read + bytes_to_consume > MAX_LINE_SIZE {
                return Err(StratumError::ProtocolError(
                    format!("Response too large (>{} bytes)", MAX_LINE_SIZE)
                ));
            }

            // Append to line (strip newline if present)
            let end = if newline_pos.is_some() { bytes_to_consume - 1 } else { bytes_to_consume };
            line.push_str(
                std::str::from_utf8(&buf[..end])
                    .map_err(|_| StratumError::ProtocolError("Invalid UTF-8".to_string()))?
            );
            total_read += bytes_to_consume;
            reader.consume(bytes_to_consume);

            if newline_pos.is_some() {
                break;
            }
        }

        serde_json::from_str(&line)
            .map_err(|e| StratumError::ProtocolError(e.to_string()))
    }

    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    pub fn get_difficulty(&self) -> f64 {
        self.difficulty
    }

    pub fn get_extranonce1(&self) -> &[u8] {
        &self.extranonce1
    }

    pub fn get_extranonce2_size(&self) -> usize {
        self.extranonce2_size
    }
}

impl Default for StratumV1Client {
    fn default() -> Self {
        Self::new()
    }
}

/// Stratum notification types
#[derive(Debug)]
pub enum Notification {
    Job(Box<Job>),
    Difficulty(f64),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stratum_v1_client_new() {
        let client = StratumV1Client::new();
        assert!(!client.is_connected());
        assert_eq!(client.get_difficulty(), 1.0);
        assert!(client.get_extranonce1().is_empty());
        assert_eq!(client.get_extranonce2_size(), 4);
    }

    #[test]
    fn test_stratum_v1_client_default() {
        let client = StratumV1Client::default();
        assert!(!client.is_connected());
    }

    #[test]
    fn test_parse_notification_difficulty() {
        let mut client = StratumV1Client::new();

        let msg = serde_json::json!({
            "method": "mining.set_difficulty",
            "params": [8.0]
        });

        let result = client.parse_notification(&msg).unwrap();
        assert!(matches!(result, Some(Notification::Difficulty(d)) if (d - 8.0).abs() < 0.001));
        assert!((client.get_difficulty() - 8.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_notification_difficulty_invalid() {
        let mut client = StratumV1Client::new();

        let msg = serde_json::json!({
            "method": "mining.set_difficulty",
            "params": []  // Missing difficulty value
        });

        let result = client.parse_notification(&msg).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_notification_unknown_method() {
        let mut client = StratumV1Client::new();

        let msg = serde_json::json!({
            "method": "mining.unknown_method",
            "params": []
        });

        let result = client.parse_notification(&msg).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_notification_no_method() {
        let mut client = StratumV1Client::new();

        let msg = serde_json::json!({
            "result": true
        });

        let result = client.parse_notification(&msg).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_job_valid() {
        let client = StratumV1Client::new();

        let params = vec![
            serde_json::json!("job123"),                    // job_id
            serde_json::json!("0".repeat(64)),              // prev_hash (32 bytes hex)
            serde_json::json!("01020304"),                  // coinbase1
            serde_json::json!("05060708"),                  // coinbase2
            serde_json::json!(["a".repeat(64)]),            // merkle_branches
            serde_json::json!("20000000"),                  // version
            serde_json::json!("1d00ffff"),                  // nbits
            serde_json::json!("60000000"),                  // ntime
            serde_json::json!(true),                        // clean_jobs
        ];

        let params_array: Vec<serde_json::Value> = params;
        let job = client.parse_job(&params_array).unwrap();

        assert_eq!(job.id, "job123");
        assert_eq!(job.version, 0x20000000);
        assert_eq!(job.nbits, 0x1d00ffff);
        assert_eq!(job.ntime, 0x60000000);
        assert!(job.clean_jobs);
        assert_eq!(job.coinbase1, vec![0x01, 0x02, 0x03, 0x04]);
        assert_eq!(job.coinbase2, vec![0x05, 0x06, 0x07, 0x08]);
    }

    #[test]
    fn test_parse_job_missing_param() {
        let client = StratumV1Client::new();

        let params = vec![
            serde_json::json!("job123"),
            // Missing the rest of the params
        ];

        let result = client.parse_job(&params);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_job_invalid_hex() {
        let client = StratumV1Client::new();

        let params = vec![
            serde_json::json!("job123"),
            serde_json::json!("not_valid_hex"),             // Invalid prev_hash
            serde_json::json!("01020304"),
            serde_json::json!("05060708"),
            serde_json::json!([]),
            serde_json::json!("20000000"),
            serde_json::json!("1d00ffff"),
            serde_json::json!("60000000"),
            serde_json::json!(true),
        ];

        let result = client.parse_job(&params);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_job_clean_jobs_default() {
        let client = StratumV1Client::new();

        let params = vec![
            serde_json::json!("job123"),
            serde_json::json!("0".repeat(64)),
            serde_json::json!(""),
            serde_json::json!(""),
            serde_json::json!([]),
            serde_json::json!("20000000"),
            serde_json::json!("1d00ffff"),
            serde_json::json!("60000000"),
            // No clean_jobs - should default to false
        ];

        let job = client.parse_job(&params).unwrap();
        assert!(!job.clean_jobs);
    }

    #[test]
    fn test_parse_notification_job() {
        let mut client = StratumV1Client::new();

        let msg = serde_json::json!({
            "method": "mining.notify",
            "params": [
                "job456",
                "0".repeat(64),
                "",
                "",
                [],
                "20000000",
                "1d00ffff",
                "60000000",
                false
            ]
        });

        let result = client.parse_notification(&msg).unwrap();
        assert!(matches!(result, Some(Notification::Job(j)) if j.id == "job456"));
    }

    #[test]
    fn test_parse_notification_job_invalid_params() {
        let mut client = StratumV1Client::new();

        let msg = serde_json::json!({
            "method": "mining.notify",
            "params": "not an array"
        });

        let result = client.parse_notification(&msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_next_id() {
        let client = StratumV1Client::new();
        let id1 = client.next_id();
        let id2 = client.next_id();
        let id3 = client.next_id();

        assert_eq!(id2, id1 + 1);
        assert_eq!(id3, id2 + 1);
    }

    #[test]
    fn test_parse_job_invalid_prev_hash_length() {
        let client = StratumV1Client::new();

        // prev_hash too short (only 16 bytes instead of 32)
        let params = vec![
            serde_json::json!("job123"),
            serde_json::json!("0".repeat(32)),  // 16 bytes = 32 hex chars
            serde_json::json!(""),
            serde_json::json!(""),
            serde_json::json!([]),
            serde_json::json!("20000000"),
            serde_json::json!("1d00ffff"),
            serde_json::json!("60000000"),
            serde_json::json!(false),
        ];

        let result = client.parse_job(&params);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(format!("{:?}", err).contains("prev_hash length"));
    }

    #[test]
    fn test_parse_job_invalid_merkle_branch_length() {
        let client = StratumV1Client::new();

        // merkle branch too short (16 bytes instead of 32)
        let params = vec![
            serde_json::json!("job123"),
            serde_json::json!("0".repeat(64)),  // Valid 32 bytes
            serde_json::json!(""),
            serde_json::json!(""),
            serde_json::json!(["a".repeat(32)]),  // 16 bytes = invalid
            serde_json::json!("20000000"),
            serde_json::json!("1d00ffff"),
            serde_json::json!("60000000"),
            serde_json::json!(false),
        ];

        let result = client.parse_job(&params);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(format!("{:?}", err).contains("merkle branch"));
    }

    #[test]
    fn test_parse_job_merkle_branch_not_string() {
        let client = StratumV1Client::new();

        // merkle branch is a number, not a string
        let params = vec![
            serde_json::json!("job123"),
            serde_json::json!("0".repeat(64)),
            serde_json::json!(""),
            serde_json::json!(""),
            serde_json::json!([12345]),  // Number instead of string
            serde_json::json!("20000000"),
            serde_json::json!("1d00ffff"),
            serde_json::json!("60000000"),
            serde_json::json!(false),
        ];

        let result = client.parse_job(&params);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(format!("{:?}", err).contains("not a string"));
    }

    #[test]
    fn test_parse_job_multiple_valid_merkle_branches() {
        let client = StratumV1Client::new();

        let params = vec![
            serde_json::json!("job123"),
            serde_json::json!("0".repeat(64)),
            serde_json::json!(""),
            serde_json::json!(""),
            serde_json::json!(["a".repeat(64), "b".repeat(64), "c".repeat(64)]),
            serde_json::json!("20000000"),
            serde_json::json!("1d00ffff"),
            serde_json::json!("60000000"),
            serde_json::json!(false),
        ];

        let job = client.parse_job(&params).unwrap();
        assert_eq!(job.merkle_branches.len(), 3);
    }

    #[test]
    fn test_disconnect() {
        let mut client = StratumV1Client::new();
        // Client is not connected initially
        assert!(!client.is_connected());

        // Disconnect should work even when not connected
        client.disconnect();
        assert!(!client.is_connected());
    }

    #[test]
    fn test_get_extranonce1_empty() {
        let client = StratumV1Client::new();
        assert!(client.get_extranonce1().is_empty());
    }

    #[test]
    fn test_get_extranonce2_size_default() {
        let client = StratumV1Client::new();
        assert_eq!(client.get_extranonce2_size(), 4);
    }

    #[test]
    fn test_parse_job_too_many_merkle_branches() {
        let client = StratumV1Client::new();

        // Create 33 merkle branches (exceeds limit of 32)
        let branches: Vec<_> = (0..33).map(|_| serde_json::json!("a".repeat(64))).collect();

        let params = vec![
            serde_json::json!("job123"),
            serde_json::json!("0".repeat(64)),
            serde_json::json!(""),
            serde_json::json!(""),
            serde_json::json!(branches),
            serde_json::json!("20000000"),
            serde_json::json!("1d00ffff"),
            serde_json::json!("60000000"),
            serde_json::json!(false),
        ];

        let result = client.parse_job(&params);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(format!("{:?}", err).contains("Too many merkle branches"));
    }

    #[test]
    fn test_parse_job_max_merkle_branches_allowed() {
        let client = StratumV1Client::new();

        // Create 32 merkle branches (exactly at limit)
        let branches: Vec<_> = (0..32).map(|_| serde_json::json!("a".repeat(64))).collect();

        let params = vec![
            serde_json::json!("job123"),
            serde_json::json!("0".repeat(64)),
            serde_json::json!(""),
            serde_json::json!(""),
            serde_json::json!(branches),
            serde_json::json!("20000000"),
            serde_json::json!("1d00ffff"),
            serde_json::json!("60000000"),
            serde_json::json!(false),
        ];

        let result = client.parse_job(&params);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().merkle_branches.len(), 32);
    }
}
