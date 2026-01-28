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

        let stream = TcpStream::connect_timeout(
            &format!("{}:{}", host, port).parse().unwrap(),
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
                // extranonce2_size is at index 2
                if let Some(en2_size) = arr.get(2).and_then(|v| v.as_u64()) {
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

    /// Receive and parse a notification (non-blocking check)
    pub fn poll_notification(&mut self) -> Result<Option<Notification>, StratumError> {
        let reader = self.reader.as_mut().ok_or_else(|| {
            StratumError::ConnectionError("Not connected".to_string())
        })?;

        // Check if data is available (this is a simplification)
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => return Err(StratumError::ConnectionError("Connection closed".to_string())),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(None),
            Err(e) => return Err(e.into()),
        }

        let msg: Value = serde_json::from_str(&line)
            .map_err(|e| StratumError::ProtocolError(e.to_string()))?;

        self.parse_notification(&msg)
    }

    /// Parse a stratum notification
    fn parse_notification(&mut self, msg: &Value) -> Result<Option<Notification>, StratumError> {
        let method = msg.get("method").and_then(|m| m.as_str());

        match method {
            Some("mining.notify") => {
                let params = msg.get("params").and_then(|p| p.as_array())
                    .ok_or_else(|| StratumError::ProtocolError("Invalid notify params".into()))?;

                let job = self.parse_job(params)?;
                Ok(Some(Notification::Job(job)))
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
        let version = get_str(5)?;
        let nbits = get_str(6)?;
        let ntime = get_str(7)?;
        let clean_jobs = params.get(8)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut job = Job::new(job_id.to_string());

        // Parse previous hash
        let prev_hash_bytes = hex::decode(prev_hash)
            .map_err(|_| StratumError::ProtocolError("Invalid prev_hash".into()))?;
        if prev_hash_bytes.len() == 32 {
            job.prev_hash.copy_from_slice(&prev_hash_bytes);
        }

        // Parse coinbase parts
        job.coinbase1 = hex::decode(coinbase1)
            .map_err(|_| StratumError::ProtocolError("Invalid coinbase1".into()))?;
        job.coinbase2 = hex::decode(coinbase2)
            .map_err(|_| StratumError::ProtocolError("Invalid coinbase2".into()))?;

        // Parse merkle branches
        for branch in merkle_branches {
            if let Some(s) = branch.as_str() {
                let bytes = hex::decode(s)
                    .map_err(|_| StratumError::ProtocolError("Invalid merkle branch".into()))?;
                if bytes.len() == 32 {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    job.merkle_branches.push(arr);
                }
            }
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

        let data = serde_json::to_string(request).unwrap() + "\n";
        stream.write_all(data.as_bytes())?;
        stream.flush()?;

        Ok(())
    }

    /// Receive a JSON-RPC response
    fn recv(&mut self) -> Result<Value, StratumError> {
        let reader = self.reader.as_mut().ok_or_else(|| {
            StratumError::ConnectionError("Not connected".to_string())
        })?;

        let mut line = String::new();
        reader.read_line(&mut line)?;

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
    Job(Job),
    Difficulty(f64),
}
