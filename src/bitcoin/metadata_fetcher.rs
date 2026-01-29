//! Metadata fetcher for magnet links
//!
//! Note: With the fix in gosh-dl (verify_existing() called after metadata received),
//! magnet links now properly verify existing files on resume.
//! This module is kept for potential future use.

use std::path::Path;

use super::BitcoinError;

/// Fetch torrent metadata from a magnet link and save as .torrent file
///
/// Note: This is no longer needed for resume functionality since gosh-dl
/// now properly verifies existing files when metadata is received for magnets.
/// Kept for cases where you want to cache a .torrent file separately.
#[allow(dead_code)]
pub async fn fetch_and_save_metadata(
    _magnet_uri: &str,
    _output_path: &Path,
    _timeout_secs: u64,
) -> Result<(), BitcoinError> {
    Err(BitcoinError::DownloadError(
        "Use gosh-dl directly - magnet resume is now fixed".into()
    ))
}
