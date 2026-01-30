use std::sync::atomic::{AtomicBool, Ordering};
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::StratumError;
use crate::mining::Job;

/// Stratum V2 binary protocol client
///
/// Implements the Stratum V2 Mining Protocol as per:
/// https://stratumprotocol.org/specification/
///
/// Connects through SRI (Stratum Reference Implementation) proxy
/// which handles Bitcoin Core IPC Mining Interface communication.
pub struct StratumV2Client {
    stream: Option<TcpStream>,
    extranonce_prefix: Vec<u8>,
    difficulty: f64,
    connected: AtomicBool,
    channel_id: u32,
}

impl StratumV2Client {
    pub fn new() -> Self {
        Self {
            stream: None,
            extranonce_prefix: Vec::new(),
            difficulty: 1.0,
            connected: AtomicBool::new(false),
            channel_id: 0,
        }
    }

    /// Connect to SRI proxy
    pub async fn connect(&mut self, host: &str, port: u16) -> Result<(), StratumError> {
        let addr = format!("{}:{}", host, port);
        let stream = TcpStream::connect(&addr)
            .await
            .map_err(|e| StratumError::ConnectionError(e.to_string()))?;

        self.stream = Some(stream);
        self.connected.store(true, Ordering::SeqCst);

        // Perform SV2 handshake
        self.handshake().await?;

        Ok(())
    }

    /// Disconnect from proxy
    pub async fn disconnect(&mut self) {
        self.connected.store(false, Ordering::SeqCst);
        self.stream = None;
    }

    /// Perform SV2 connection handshake
    async fn handshake(&mut self) -> Result<(), StratumError> {
        // SV2 uses a binary protocol with Noise encryption
        // The handshake establishes a secure channel

        // For now, this is a simplified implementation
        // Full SV2 would use the roles_logic_sv2 crate

        // 1. Send SetupConnection message
        let setup_msg = self.build_setup_connection();
        self.send_frame(&setup_msg).await?;

        // 2. Receive SetupConnection.Success
        let response = self.recv_frame().await?;
        self.parse_setup_response(&response)?;

        // 3. Open mining channel
        let open_channel = self.build_open_channel();
        self.send_frame(&open_channel).await?;

        // 4. Receive OpenMiningChannel.Success
        let response = self.recv_frame().await?;
        self.parse_channel_response(&response)?;

        Ok(())
    }

    /// Build SetupConnection message
    fn build_setup_connection(&self) -> Vec<u8> {
        // SV2 binary format:
        // - Protocol version (2 bytes)
        // - Min protocol version (2 bytes)
        // - Flags (4 bytes)
        // - Endpoint host length + host
        // - Endpoint port (2 bytes)
        // - Vendor length + vendor
        // - Hardware version length + version
        // - Firmware length + firmware
        // - Device ID length + id

        let mut msg = Vec::new();

        // Protocol: Mining Protocol (0x0000)
        msg.extend_from_slice(&0u16.to_le_bytes());
        // Version
        msg.extend_from_slice(&2u16.to_le_bytes());
        msg.extend_from_slice(&2u16.to_le_bytes());
        // Flags
        msg.extend_from_slice(&0u32.to_le_bytes());
        // Endpoint (empty for client)
        msg.push(0);
        msg.extend_from_slice(&0u16.to_le_bytes());
        // Vendor: "btcminer"
        msg.push(8);
        msg.extend_from_slice(b"btcminer");
        // Hardware version
        msg.push(5);
        msg.extend_from_slice(b"0.1.0");
        // Firmware
        msg.push(0);
        // Device ID
        msg.push(0);

        msg
    }

    /// Parse SetupConnection.Success response
    fn parse_setup_response(&mut self, _data: &[u8]) -> Result<(), StratumError> {
        // Parse flags and version info
        // For simplicity, assume success
        Ok(())
    }

    /// Build OpenStandardMiningChannel message
    fn build_open_channel(&self) -> Vec<u8> {
        let mut msg = Vec::new();

        // Request ID (4 bytes)
        msg.extend_from_slice(&1u32.to_le_bytes());
        // User identity (length-prefixed string)
        msg.push(0);
        // Nominal hash rate (4 bytes float)
        msg.extend_from_slice(&1000000f32.to_le_bytes());
        // Max target (32 bytes)
        msg.extend_from_slice(&[0xff; 32]);

        msg
    }

    /// Parse OpenStandardMiningChannel.Success response
    fn parse_channel_response(&mut self, data: &[u8]) -> Result<(), StratumError> {
        if data.len() < 4 {
            return Err(StratumError::ProtocolError("Invalid channel response".into()));
        }

        // Parse channel ID
        self.channel_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

        // Parse extranonce prefix if present
        if data.len() > 4 {
            let prefix_len = data[4] as usize;
            if data.len() >= 5 + prefix_len {
                self.extranonce_prefix = data[5..5 + prefix_len].to_vec();
            }
        }

        Ok(())
    }

    /// Send a binary frame
    async fn send_frame(&mut self, data: &[u8]) -> Result<(), StratumError> {
        let stream = self.stream.as_mut()
            .ok_or_else(|| StratumError::ConnectionError("Not connected".into()))?;

        // SV2 frame: extension type (2) + message type (1) + length (3) + payload
        let len = data.len();
        let mut frame = Vec::with_capacity(6 + len);
        frame.extend_from_slice(&0u16.to_le_bytes()); // Extension type
        frame.push(0); // Message type
        frame.push((len & 0xff) as u8);
        frame.push(((len >> 8) & 0xff) as u8);
        frame.push(((len >> 16) & 0xff) as u8);
        frame.extend_from_slice(data);

        stream.write_all(&frame).await
            .map_err(StratumError::IoError)?;

        Ok(())
    }

    /// Receive a binary frame
    async fn recv_frame(&mut self) -> Result<Vec<u8>, StratumError> {
        let stream = self.stream.as_mut()
            .ok_or_else(|| StratumError::ConnectionError("Not connected".into()))?;

        // Read frame header
        let mut header = [0u8; 6];
        stream.read_exact(&mut header).await
            .map_err(StratumError::IoError)?;

        // Parse length
        let len = (header[3] as usize)
            | ((header[4] as usize) << 8)
            | ((header[5] as usize) << 16);

        // Read payload
        let mut payload = vec![0u8; len];
        stream.read_exact(&mut payload).await
            .map_err(StratumError::IoError)?;

        Ok(payload)
    }

    /// Receive a mining job notification
    pub async fn recv_job(&mut self) -> Result<Job, StratumError> {
        let data = self.recv_frame().await?;
        self.parse_new_mining_job(&data)
    }

    /// Parse NewMiningJob message
    fn parse_new_mining_job(&self, data: &[u8]) -> Result<Job, StratumError> {
        if data.len() < 72 {
            return Err(StratumError::ProtocolError("Invalid job message".into()));
        }

        let mut job = Job::new(format!("{}", u32::from_le_bytes([data[0], data[1], data[2], data[3]])));

        // Parse job fields from binary format
        let mut offset = 4;

        // Version (4 bytes)
        job.version = u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
        offset += 4;

        // Previous block hash (32 bytes)
        job.prev_hash.copy_from_slice(&data[offset..offset + 32]);
        offset += 32;

        // Merkle root (we get it directly in SV2, not branches)
        // For simplicity, skip merkle handling here
        offset += 32;

        // nBits (4 bytes)
        job.nbits = u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);

        // Set target from difficulty
        job.target = crate::mining::difficulty_to_target(self.difficulty);

        Ok(job)
    }

    /// Submit a share
    pub async fn submit_share(
        &mut self,
        job_id: u32,
        nonce: u32,
        ntime: u32,
        version: u32,
    ) -> Result<bool, StratumError> {
        let mut msg = Vec::new();

        // Channel ID
        msg.extend_from_slice(&self.channel_id.to_le_bytes());
        // Job ID
        msg.extend_from_slice(&job_id.to_le_bytes());
        // Nonce
        msg.extend_from_slice(&nonce.to_le_bytes());
        // nTime
        msg.extend_from_slice(&ntime.to_le_bytes());
        // Version
        msg.extend_from_slice(&version.to_le_bytes());

        self.send_frame(&msg).await?;

        // Wait for response
        let response = self.recv_frame().await?;

        // Check first byte for success/failure
        Ok(!response.is_empty() && response[0] == 0)
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    pub fn get_difficulty(&self) -> f64 {
        self.difficulty
    }

    pub fn get_extranonce_prefix(&self) -> &[u8] {
        &self.extranonce_prefix
    }

    pub fn set_difficulty(&mut self, difficulty: f64) {
        self.difficulty = difficulty;
    }
}

impl Default for StratumV2Client {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stratum_v2_client_new() {
        let client = StratumV2Client::new();
        assert!(!client.is_connected());
        assert_eq!(client.get_difficulty(), 1.0);
        assert!(client.get_extranonce_prefix().is_empty());
    }

    #[test]
    fn test_stratum_v2_client_default() {
        let client = StratumV2Client::default();
        assert!(!client.is_connected());
        assert_eq!(client.get_difficulty(), 1.0);
    }

    #[test]
    fn test_set_difficulty() {
        let mut client = StratumV2Client::new();
        client.set_difficulty(16.0);
        assert_eq!(client.get_difficulty(), 16.0);
    }

    #[test]
    fn test_build_setup_connection() {
        let client = StratumV2Client::new();
        let msg = client.build_setup_connection();

        // Should have protocol, version, flags, and vendor info
        assert!(msg.len() > 10);

        // Check protocol version (Mining Protocol = 0x0000)
        assert_eq!(msg[0], 0);
        assert_eq!(msg[1], 0);

        // Check version numbers
        assert_eq!(msg[2], 2); // min version
        assert_eq!(msg[3], 0);
        assert_eq!(msg[4], 2); // version
        assert_eq!(msg[5], 0);
    }

    #[test]
    fn test_build_open_channel() {
        let client = StratumV2Client::new();
        let msg = client.build_open_channel();

        // Should have request ID (4), user identity len (1), hash rate (4), max target (32)
        assert!(msg.len() >= 41);

        // Request ID should be 1
        assert_eq!(u32::from_le_bytes([msg[0], msg[1], msg[2], msg[3]]), 1);
    }

    #[test]
    fn test_parse_setup_response() {
        let mut client = StratumV2Client::new();
        // Empty response should succeed (simplified implementation)
        assert!(client.parse_setup_response(&[]).is_ok());
    }

    #[test]
    fn test_parse_channel_response_valid() {
        let mut client = StratumV2Client::new();

        // Channel ID = 42, no extranonce prefix
        let data = vec![42, 0, 0, 0];
        let result = client.parse_channel_response(&data);

        assert!(result.is_ok());
        assert_eq!(client.channel_id, 42);
    }

    #[test]
    fn test_parse_channel_response_with_extranonce() {
        let mut client = StratumV2Client::new();

        // Channel ID = 1, extranonce prefix length = 3, prefix = [0xAA, 0xBB, 0xCC]
        let data = vec![1, 0, 0, 0, 3, 0xAA, 0xBB, 0xCC];
        let result = client.parse_channel_response(&data);

        assert!(result.is_ok());
        assert_eq!(client.channel_id, 1);
        assert_eq!(client.extranonce_prefix, vec![0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn test_parse_channel_response_invalid() {
        let mut client = StratumV2Client::new();

        // Too short data
        let data = vec![1, 0];
        let result = client.parse_channel_response(&data);

        assert!(result.is_err());
    }

    #[test]
    fn test_parse_new_mining_job_valid() {
        let client = StratumV2Client::new();

        // Build a minimal valid job message (72+ bytes)
        let mut data = vec![0u8; 76];
        // Job ID = 123
        data[0..4].copy_from_slice(&123u32.to_le_bytes());
        // Version = 0x20000000
        data[4..8].copy_from_slice(&0x20000000u32.to_le_bytes());
        // prev_hash (32 bytes starting at offset 8)
        data[8..40].copy_from_slice(&[0xAB; 32]);
        // merkle root (32 bytes starting at offset 40)
        data[40..72].copy_from_slice(&[0xCD; 32]);
        // nbits = 0x1d00ffff
        data[72..76].copy_from_slice(&0x1d00ffffu32.to_le_bytes());

        let result = client.parse_new_mining_job(&data);
        assert!(result.is_ok());

        let job = result.unwrap();
        assert_eq!(job.id, "123");
        assert_eq!(job.version, 0x20000000);
        assert_eq!(job.nbits, 0x1d00ffff);
        assert_eq!(job.prev_hash, [0xAB; 32]);
    }

    #[test]
    fn test_parse_new_mining_job_too_short() {
        let client = StratumV2Client::new();

        // Data too short
        let data = vec![0u8; 50];
        let result = client.parse_new_mining_job(&data);

        assert!(result.is_err());
    }

    #[test]
    fn test_difficulty_range() {
        let mut client = StratumV2Client::new();

        // Test various difficulty values
        client.set_difficulty(0.0);
        assert_eq!(client.get_difficulty(), 0.0);

        client.set_difficulty(1e15);
        assert_eq!(client.get_difficulty(), 1e15);
    }

    #[test]
    fn test_parse_channel_response_truncated_extranonce() {
        let mut client = StratumV2Client::new();

        // Channel ID = 1, extranonce length = 5, but only 2 bytes of data
        let data = vec![1, 0, 0, 0, 5, 0xAA, 0xBB];
        let result = client.parse_channel_response(&data);

        // Should succeed but not set extranonce (graceful handling)
        assert!(result.is_ok());
        assert!(client.extranonce_prefix.is_empty()); // Not enough data to parse
    }

    #[test]
    fn test_parse_new_mining_job_boundary() {
        let client = StratumV2Client::new();

        // Exactly minimum valid size (76 bytes)
        let mut data = vec![0u8; 76];
        data[0..4].copy_from_slice(&1u32.to_le_bytes());
        data[4..8].copy_from_slice(&0x20000000u32.to_le_bytes());
        data[72..76].copy_from_slice(&0x1d00ffffu32.to_le_bytes());

        let result = client.parse_new_mining_job(&data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extranonce_prefix_empty() {
        let client = StratumV2Client::new();
        assert!(client.get_extranonce_prefix().is_empty());
    }
}
