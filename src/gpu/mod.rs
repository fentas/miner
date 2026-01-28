//! GPU Mining Support
//!
//! Provides GPU acceleration via:
//! - OpenCL (AMD, Intel, NVIDIA)
//! - CUDA (NVIDIA) - future support via cuda-sys

mod manager;
mod opencl;

pub use manager::{GpuDevice, GpuManager};
// These types are part of the public API but only used internally when GPU features are enabled
#[allow(unused_imports)]
pub use manager::{GpuMiner, MiningResult, GpuBackend};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum GpuError {
    #[error("No GPU devices found")]
    NoDevices,
    #[error("Device not found: {0}")]
    DeviceNotFound(u32),
    #[error("OpenCL error: {0}")]
    OpenCL(String),
    #[error("CUDA error: {0}")]
    Cuda(String),
    #[error("Kernel compilation failed: {0}")]
    KernelCompile(String),
    #[error("Not supported on this platform")]
    NotSupported,
}
