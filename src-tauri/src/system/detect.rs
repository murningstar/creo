use serde::Serialize;
use sysinfo::System;

/// Complete system info for hardware detection and model recommendation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub cpu: CpuInfo,
    pub ram: RamInfo,
    pub gpu: Option<GpuInfo>,
    pub display_server: DisplayServer,
    pub os: OsInfo,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuInfo {
    pub brand: String,
    pub vendor: String,
    pub physical_cores: Option<usize>,
    pub logical_cores: usize,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RamInfo {
    pub total_mb: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    pub vendor: GpuVendor,
    pub vram_mb: u64,
    pub device_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum GpuVendor {
    #[serde(rename = "nvidia")]
    Nvidia,
    #[serde(rename = "amd")]
    Amd,
    #[serde(rename = "intel")]
    Intel,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum DisplayServer {
    #[serde(rename = "x11")]
    X11,
    #[serde(rename = "wayland")]
    Wayland,
    #[serde(rename = "windows")]
    Windows,
    #[serde(rename = "macos")]
    MacOS,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsInfo {
    pub name: String,
    pub version: String,
}

/// Detect system hardware. GPU detection via Vulkan (ash) — graceful fallback if unavailable.
pub fn detect_system() -> SystemInfo {
    let cpu = detect_cpu();
    let ram = detect_ram();
    let gpu = detect_gpu();
    let display_server = detect_display_server();
    let os = detect_os();

    log::info!(
        "System: CPU={}, RAM={}MB, GPU={}, display={}",
        cpu.brand,
        ram.total_mb,
        gpu.as_ref()
            .map(|g| format!("{} ({}MB VRAM)", g.name, g.vram_mb))
            .unwrap_or_else(|| "none".to_string()),
        serde_json::to_string(&display_server).unwrap_or_default(),
    );

    SystemInfo {
        cpu,
        ram,
        gpu,
        display_server,
        os,
    }
}

fn detect_cpu() -> CpuInfo {
    let mut sys = System::new();
    sys.refresh_cpu_all();

    let (brand, vendor) = if let Some(cpu) = sys.cpus().first() {
        (cpu.brand().to_string(), cpu.vendor_id().to_string())
    } else {
        ("Unknown".to_string(), "Unknown".to_string())
    };

    CpuInfo {
        brand,
        vendor,
        physical_cores: System::physical_core_count(),
        logical_cores: sys.cpus().len(),
        arch: System::cpu_arch(),
    }
}

fn detect_ram() -> RamInfo {
    let mut sys = System::new();
    sys.refresh_memory();
    RamInfo {
        total_mb: sys.total_memory() / 1024 / 1024,
    }
}

/// Detect GPU via Vulkan (ash with runtime loading).
/// Returns None if Vulkan is not available (no GPU or no drivers).
///
/// TODO: Current coverage gaps:
/// - macOS: Vulkan not preinstalled (needs MoltenVK). Need Metal/IOKit fallback
///   for Apple Silicon (M1+) and Intel Mac dGPU detection.
/// - Intel/AMD iGPU: VRAM reported as DEVICE_LOCAL may be misleading (shared system RAM).
///   Need to detect integrated vs discrete and adjust recommendation accordingly.
/// - Linux without vulkan-loader: GPU physically present but undetectable.
///   Could fall back to parsing /sys/class/drm/ or lspci output.
/// Drop guard for Vulkan instance — ensures `destroy_instance` even on early return.
struct VkInstanceGuard {
    instance: ash::Instance,
}

impl Drop for VkInstanceGuard {
    fn drop(&mut self) {
        unsafe {
            self.instance.destroy_instance(None);
        }
    }
}

fn detect_gpu() -> Option<GpuInfo> {
    // ash with "loaded" feature: runtime load, no link-time dependency.
    // Fails gracefully if Vulkan runtime is not installed.
    let entry = unsafe { ash::Entry::load().ok()? };

    let app_info = ash::vk::ApplicationInfo {
        api_version: ash::vk::make_api_version(0, 1, 0, 0),
        ..Default::default()
    };

    let create_info = ash::vk::InstanceCreateInfo {
        p_application_info: &app_info,
        ..Default::default()
    };

    let instance = unsafe { entry.create_instance(&create_info, None).ok()? };
    let guard = VkInstanceGuard { instance };

    let devices = unsafe { guard.instance.enumerate_physical_devices().ok()? };

    // Pick the best GPU (prefer discrete over integrated)
    let mut best: Option<GpuInfo> = None;
    for &device in &devices {
        let props = unsafe { guard.instance.get_physical_device_properties(device) };
        let name = props
            .device_name_as_c_str()
            .ok()
            .and_then(|s| s.to_str().ok())
            .unwrap_or("Unknown")
            .to_string();

        let vendor = match props.vendor_id {
            0x10DE => GpuVendor::Nvidia,
            0x1002 => GpuVendor::Amd,
            0x8086 => GpuVendor::Intel,
            _ => GpuVendor::Unknown,
        };

        let device_type = match props.device_type {
            ash::vk::PhysicalDeviceType::DISCRETE_GPU => "discrete",
            ash::vk::PhysicalDeviceType::INTEGRATED_GPU => "integrated",
            ash::vk::PhysicalDeviceType::VIRTUAL_GPU => "virtual",
            ash::vk::PhysicalDeviceType::CPU => "cpu",
            _ => "other",
        };

        // Sum DEVICE_LOCAL memory heaps for VRAM
        let mem_props = unsafe { guard.instance.get_physical_device_memory_properties(device) };
        let vram_bytes: u64 = mem_props.memory_heaps
            [..mem_props.memory_heap_count as usize]
            .iter()
            .filter(|h| h.flags.contains(ash::vk::MemoryHeapFlags::DEVICE_LOCAL))
            .map(|h| h.size)
            .sum();

        let info = GpuInfo {
            name,
            vendor,
            vram_mb: vram_bytes / 1024 / 1024,
            device_type: device_type.to_string(),
        };

        // Prefer discrete GPU over integrated
        let dominated = match (&best, device_type) {
            (None, _) => false,
            (Some(prev), "discrete") if prev.device_type != "discrete" => false,
            _ => true,
        };

        if !dominated {
            best = Some(info);
        }
    }

    // Guard dropped here — destroy_instance called automatically
    best
}

pub fn detect_display_server() -> DisplayServer {
    if cfg!(target_os = "windows") {
        DisplayServer::Windows
    } else if cfg!(target_os = "macos") {
        DisplayServer::MacOS
    } else {
        // Linux: check environment variables
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            DisplayServer::Wayland
        } else if std::env::var("XDG_SESSION_TYPE")
            .map(|v| v == "wayland")
            .unwrap_or(false)
        {
            DisplayServer::Wayland
        } else if std::env::var("DISPLAY").is_ok() {
            DisplayServer::X11
        } else {
            DisplayServer::Unknown
        }
    }
}

fn detect_os() -> OsInfo {
    OsInfo {
        name: System::name().unwrap_or_else(|| "Unknown".to_string()),
        version: System::os_version().unwrap_or_else(|| "Unknown".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pinned wire format for GpuVendor — if this fails, you changed a serialized value.
    #[test]
    fn gpu_vendor_serialization_stability() {
        assert_eq!(serde_json::to_string(&GpuVendor::Nvidia).unwrap(), "\"nvidia\"");
        assert_eq!(serde_json::to_string(&GpuVendor::Amd).unwrap(), "\"amd\"");
        assert_eq!(serde_json::to_string(&GpuVendor::Intel).unwrap(), "\"intel\"");
        assert_eq!(serde_json::to_string(&GpuVendor::Unknown).unwrap(), "\"unknown\"");
    }

    /// Pinned wire format for DisplayServer — if this fails, you changed a serialized value.
    #[test]
    fn display_server_serialization_stability() {
        assert_eq!(serde_json::to_string(&DisplayServer::X11).unwrap(), "\"x11\"");
        assert_eq!(serde_json::to_string(&DisplayServer::Wayland).unwrap(), "\"wayland\"");
        assert_eq!(serde_json::to_string(&DisplayServer::Windows).unwrap(), "\"windows\"");
        assert_eq!(serde_json::to_string(&DisplayServer::MacOS).unwrap(), "\"macos\"");
        assert_eq!(serde_json::to_string(&DisplayServer::Unknown).unwrap(), "\"unknown\"");
    }
}
