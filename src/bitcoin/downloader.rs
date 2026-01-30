//! Bitcoin Core binary download and installation
//!
//! This module handles downloading and extracting Bitcoin Core binaries
//! for managed mode operation.

use std::path::Path;
use std::process::Command;
use tracing::info;

use super::BitcoinError;

/// Bitcoin Core version to download
pub const BITCOIN_CORE_VERSION: &str = "30.2";

/// Base URL for Bitcoin Core downloads
const DOWNLOAD_BASE_URL: &str = "https://bitcoincore.org/bin";

/// Download and install Bitcoin Core to the specified directory
pub async fn download_and_install(install_dir: &Path) -> Result<(), BitcoinError> {
    info!("Downloading Bitcoin Core v{}...", BITCOIN_CORE_VERSION);

    let platform = get_platform();

    // Windows uses .zip, others use .tar.gz
    #[cfg(target_os = "windows")]
    let filename = format!("bitcoin-{}-{}.zip", BITCOIN_CORE_VERSION, platform);
    #[cfg(not(target_os = "windows"))]
    let filename = format!("bitcoin-{}-{}.tar.gz", BITCOIN_CORE_VERSION, platform);

    let url = format!(
        "{}/bitcoin-core-{}/{}",
        DOWNLOAD_BASE_URL, BITCOIN_CORE_VERSION, filename
    );

    // Create install directory
    std::fs::create_dir_all(install_dir)?;

    // Download
    let response = reqwest::get(&url)
        .await
        .map_err(|e| BitcoinError::DownloadError(e.to_string()))?;

    if !response.status().is_success() {
        return Err(BitcoinError::DownloadError(format!(
            "HTTP {}: {}",
            response.status(),
            url
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| BitcoinError::DownloadError(e.to_string()))?;

    // Save to temp file
    let temp_path = install_dir.join(&filename);
    std::fs::write(&temp_path, &bytes)?;

    // Extract - different method per platform
    info!("Extracting Bitcoin Core...");

    extract_archive(&temp_path, install_dir)?;

    // Cleanup
    std::fs::remove_file(&temp_path)?;

    info!(
        "Bitcoin Core v{} installed successfully",
        BITCOIN_CORE_VERSION
    );
    Ok(())
}

/// Extract the downloaded archive to the install directory
fn extract_archive(archive_path: &Path, install_dir: &Path) -> Result<(), BitcoinError> {
    #[cfg(target_os = "windows")]
    {
        // Use PowerShell to extract zip on Windows
        let status = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!(
                    "Expand-Archive -Path '{}' -DestinationPath '{}' -Force; \
                     Get-ChildItem -Path '{}' -Directory | ForEach-Object {{ \
                         Get-ChildItem -Path $_.FullName | Move-Item -Destination '{}' -Force \
                     }}",
                    archive_path.display(),
                    install_dir.display(),
                    install_dir.display(),
                    install_dir.display()
                ),
            ])
            .status()?;

        if !status.success() {
            return Err(BitcoinError::DownloadError(
                "Failed to extract archive".into(),
            ));
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let filename = archive_path
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or_else(|| BitcoinError::DownloadError("Invalid archive path".into()))?;

        let status = Command::new("tar")
            .args(["-xzf", filename, "--strip-components=1"])
            .current_dir(install_dir)
            .status()?;

        if !status.success() {
            return Err(BitcoinError::DownloadError(
                "Failed to extract archive".into(),
            ));
        }
    }

    Ok(())
}

/// Get platform string for Bitcoin Core download
pub fn get_platform() -> &'static str {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "x86_64-linux-gnu";

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "aarch64-linux-gnu";

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "x86_64-apple-darwin";

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "arm64-apple-darwin";

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "win64";

    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64"),
    )))]
    compile_error!("Unsupported platform for Bitcoin Core");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_platform() {
        let platform = get_platform();
        assert!(!platform.is_empty());

        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        assert_eq!(platform, "x86_64-linux-gnu");

        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        assert_eq!(platform, "aarch64-linux-gnu");

        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        assert_eq!(platform, "x86_64-apple-darwin");

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        assert_eq!(platform, "arm64-apple-darwin");

        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        assert_eq!(platform, "win64");
    }

    #[test]
    fn test_bitcoin_core_version_format() {
        // Version should be in X.Y format
        let parts: Vec<&str> = BITCOIN_CORE_VERSION.split('.').collect();
        assert_eq!(parts.len(), 2);
        assert!(parts[0].parse::<u32>().is_ok());
        assert!(parts[1].parse::<u32>().is_ok());
    }
}
