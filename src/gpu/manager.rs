//! GPU Manager - handles device enumeration and miner creation


use super::GpuError;

/// GPU device information
#[derive(Debug, Clone)]
pub struct GpuDevice {
    pub id: u32,
    pub name: String,
    pub vendor: String,
    pub memory_total: u64,
    pub compute_units: u32,
    pub max_work_group_size: usize,
    pub backend: GpuBackend,
    pub available: bool,
}

/// GPU backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBackend {
    OpenCL,
    Cuda,
}

impl GpuBackend {
    pub fn name(&self) -> &'static str {
        match self {
            GpuBackend::OpenCL => "OpenCL",
            GpuBackend::Cuda => "CUDA",
        }
    }
}

/// Mining result from GPU
#[derive(Debug, Clone)]
pub struct MiningResult {
    pub nonce: u32,
    pub hash: [u8; 32],
}

/// GPU Miner trait
pub trait GpuMiner: Send {
    /// Set job parameters (midstate, tail, target)
    fn set_job(&mut self, midstate: &[u32; 8], tail: &[u8; 16], target: &[u8; 32]);

    /// Mine a range of nonces, returns found results
    fn mine(&mut self, nonce_start: u32, nonce_count: u32) -> Vec<MiningResult>;

    /// Get device info
    fn device(&self) -> &GpuDevice;

    /// Get hashrate
    fn hashrate(&self) -> f64;

    /// Stop mining
    fn stop(&mut self);
}

/// GPU Manager - handles device enumeration
pub struct GpuManager {
    devices: Vec<GpuDevice>,
}

impl GpuManager {
    /// Create a new GPU manager and enumerate devices
    // `devices` is only mutated when GPU features (opencl/cuda) are enabled
    #[allow(unused_mut)]
    pub fn new() -> Self {
        let mut devices = Vec::new();

        // Try to enumerate OpenCL devices
        #[cfg(feature = "opencl")]
        {
            if let Ok(ocl_devices) = super::opencl::enumerate_devices() {
                devices.extend(ocl_devices);
            }
        }

        // Try to enumerate CUDA devices
        #[cfg(feature = "cuda")]
        {
            // CUDA enumeration would go here
        }

        Self { devices }
    }

    /// Get all available devices
    pub fn devices(&self) -> &[GpuDevice] {
        &self.devices
    }

    /// Get device by ID
    pub fn get_device(&self, id: u32) -> Option<&GpuDevice> {
        self.devices.iter().find(|d| d.id == id)
    }

    /// Create a miner for a device
    // `intensity` is only used when GPU features are enabled
    #[allow(unused_variables)]
    pub fn create_miner(&self, device_id: u32, intensity: u8) -> Result<Box<dyn GpuMiner>, GpuError> {
        let device = self.get_device(device_id)
            .ok_or(GpuError::DeviceNotFound(device_id))?;

        match device.backend {
            #[cfg(feature = "opencl")]
            GpuBackend::OpenCL => {
                let miner = super::opencl::OpenCLMiner::new(device_id, intensity)?;
                Ok(Box::new(miner))
            }
            #[cfg(not(feature = "opencl"))]
            GpuBackend::OpenCL => Err(GpuError::NotSupported),
            GpuBackend::Cuda => Err(GpuError::NotSupported),
        }
    }

    /// Check if OpenCL is available
    pub fn has_opencl(&self) -> bool {
        self.devices.iter().any(|d| d.backend == GpuBackend::OpenCL)
    }

    /// Check if CUDA is available
    pub fn has_cuda(&self) -> bool {
        self.devices.iter().any(|d| d.backend == GpuBackend::Cuda)
    }

    /// Get total number of devices
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

impl Default for GpuManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Placeholder OpenCL miner when feature is disabled
#[cfg(not(feature = "opencl"))]
pub mod opencl {
    use super::*;

    pub fn enumerate_devices() -> Result<Vec<GpuDevice>, GpuError> {
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_backend_name() {
        assert_eq!(GpuBackend::OpenCL.name(), "OpenCL");
        assert_eq!(GpuBackend::Cuda.name(), "CUDA");
    }

    #[test]
    fn test_gpu_manager_new() {
        let manager = GpuManager::new();
        // Verify manager initializes (device_count depends on available hardware)
        let _ = manager.device_count();
    }

    #[test]
    fn test_gpu_manager_default() {
        let manager = GpuManager::default();
        assert!(manager.devices().len() == manager.device_count());
    }

    #[test]
    fn test_gpu_manager_get_device_not_found() {
        let manager = GpuManager::new();
        assert!(manager.get_device(9999).is_none());
    }

    #[test]
    fn test_gpu_manager_create_miner_device_not_found() {
        let manager = GpuManager::new();
        let result = manager.create_miner(9999, 50);
        assert!(matches!(result, Err(GpuError::DeviceNotFound(9999))));
    }

    #[test]
    fn test_gpu_manager_has_backends() {
        let manager = GpuManager::new();
        // Test backend detection methods
        let _has_opencl = manager.has_opencl();
        let _has_cuda = manager.has_cuda();
    }

    #[test]
    fn test_mining_result() {
        let result = MiningResult {
            nonce: 12345,
            hash: [0u8; 32],
        };
        assert_eq!(result.nonce, 12345);
        assert_eq!(result.hash.len(), 32);
    }
}
