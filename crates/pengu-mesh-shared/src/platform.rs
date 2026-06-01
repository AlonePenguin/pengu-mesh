use crate::types::{ExecutionChannel, HostAccessService, InterferenceLevel};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlatformOs {
    Macos,
    Linux,
    Windows,
    Unknown,
}

impl PlatformOs {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Unknown
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Macos => "macos",
            Self::Linux => "linux",
            Self::Windows => "windows",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlatformArch {
    Aarch64,
    X86_64,
    Unknown,
}

impl PlatformArch {
    pub fn current() -> Self {
        if cfg!(target_arch = "aarch64") {
            Self::Aarch64
        } else if cfg!(target_arch = "x86_64") {
            Self::X86_64
        } else {
            Self::Unknown
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Aarch64 => "aarch64",
            Self::X86_64 => "x86_64",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlatformInfo {
    pub os: PlatformOs,
    pub arch: PlatformArch,
}

impl PlatformInfo {
    pub fn new(os: PlatformOs, arch: PlatformArch) -> Self {
        Self { os, arch }
    }

    pub fn is_tier1(&self) -> bool {
        self.os == PlatformOs::Macos && self.arch == PlatformArch::Aarch64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccessibilitySupportState {
    Supported,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessibilityCapabilityContract {
    pub state: AccessibilitySupportState,
    pub platform_shim: Option<String>,
    pub execution_channel: Option<ExecutionChannel>,
    pub interference_level: Option<InterferenceLevel>,
    pub required_permissions: Vec<HostAccessService>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlatformAccessibilityContract {
    pub platform: PlatformInfo,
    pub tier1: bool,
    pub direct_accessibility: AccessibilityCapabilityContract,
    pub summary: String,
}

pub fn current_platform() -> PlatformInfo {
    PlatformInfo::new(PlatformOs::current(), PlatformArch::current())
}

pub fn is_tier1_platform() -> bool {
    current_platform().is_tier1()
}

pub fn platform_accessibility_contract(platform: PlatformInfo) -> PlatformAccessibilityContract {
    let tier1 = platform.is_tier1();

    match platform.os {
        PlatformOs::Macos => PlatformAccessibilityContract {
            platform,
            tier1,
            direct_accessibility: AccessibilityCapabilityContract {
                state: AccessibilitySupportState::Supported,
                platform_shim: Some("pengu-mesh-macos".to_string()),
                execution_channel: Some(ExecutionChannel::AxDirect),
                interference_level: Some(InterferenceLevel::BackgroundSafe),
                required_permissions: vec![HostAccessService::Accessibility],
                detail:
                    "The pengu-mesh-macos shim owns direct Accessibility discovery and action through ax_direct; runtime readiness still depends on host permission state."
                        .to_string(),
            },
            summary:
                "macOS-native accessibility is supported through the pengu-mesh-macos shim."
                    .to_string(),
        },
        PlatformOs::Linux => unsupported_platform_accessibility(
            platform,
            tier1,
            "Linux-native accessibility is unsupported on the current product surface.",
            "No Linux accessibility shim is shipped yet; keep the shared contract honest until a dedicated Linux platform crate lands.",
        ),
        PlatformOs::Windows => unsupported_platform_accessibility(
            platform,
            tier1,
            "Windows-native accessibility is unsupported on the current product surface.",
            "No Windows accessibility shim is shipped yet; keep the shared contract honest until a dedicated Windows platform crate lands.",
        ),
        PlatformOs::Unknown => unsupported_platform_accessibility(
            platform,
            tier1,
            "Native accessibility is unsupported on this platform.",
            "No platform shim is shipped for this operating system.",
        ),
    }
}

pub fn current_platform_accessibility() -> PlatformAccessibilityContract {
    platform_accessibility_contract(current_platform())
}

fn unsupported_platform_accessibility(
    platform: PlatformInfo,
    tier1: bool,
    summary: &str,
    detail: &str,
) -> PlatformAccessibilityContract {
    PlatformAccessibilityContract {
        platform,
        tier1,
        direct_accessibility: AccessibilityCapabilityContract {
            state: AccessibilitySupportState::Unsupported,
            platform_shim: None,
            execution_channel: None,
            interference_level: None,
            required_permissions: Vec::new(),
            detail: detail.to_string(),
        },
        summary: summary.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_platform_roundtrip_serializes() {
        let info = current_platform();
        let json = serde_json::to_string(&info).expect("serialize platform info");
        let back: PlatformInfo = serde_json::from_str(&json).expect("deserialize platform info");
        assert_eq!(back, info);
    }

    #[test]
    fn tier1_detection_matches_platform_info() {
        let tier1 = PlatformInfo::new(PlatformOs::Macos, PlatformArch::Aarch64);
        let non_tier1 = PlatformInfo::new(PlatformOs::Macos, PlatformArch::X86_64);
        assert!(tier1.is_tier1());
        assert!(!non_tier1.is_tier1());
    }

    #[test]
    fn macos_accessibility_contract_is_supported() {
        let contract = platform_accessibility_contract(PlatformInfo::new(
            PlatformOs::Macos,
            PlatformArch::Aarch64,
        ));
        assert!(contract.tier1);
        assert_eq!(
            contract.direct_accessibility.state,
            AccessibilitySupportState::Supported
        );
        assert_eq!(
            contract.direct_accessibility.execution_channel,
            Some(ExecutionChannel::AxDirect)
        );
        assert_eq!(
            contract.direct_accessibility.interference_level,
            Some(InterferenceLevel::BackgroundSafe)
        );
        assert_eq!(
            contract.direct_accessibility.required_permissions,
            vec![HostAccessService::Accessibility]
        );
        assert_eq!(
            contract.direct_accessibility.platform_shim.as_deref(),
            Some("pengu-mesh-macos")
        );
    }

    #[test]
    fn linux_accessibility_contract_is_honest_unsupported_stub() {
        let contract = platform_accessibility_contract(PlatformInfo::new(
            PlatformOs::Linux,
            PlatformArch::X86_64,
        ));
        assert!(!contract.tier1);
        assert_eq!(
            contract.direct_accessibility.state,
            AccessibilitySupportState::Unsupported
        );
        assert_eq!(contract.direct_accessibility.platform_shim, None);
        assert_eq!(contract.direct_accessibility.execution_channel, None);
        assert_eq!(
            contract.direct_accessibility.required_permissions,
            Vec::new()
        );
        assert!(
            contract
                .direct_accessibility
                .detail
                .contains("No Linux accessibility shim is shipped yet")
        );
    }

    #[test]
    fn windows_accessibility_contract_is_honest_unsupported_stub() {
        let contract = platform_accessibility_contract(PlatformInfo::new(
            PlatformOs::Windows,
            PlatformArch::X86_64,
        ));
        assert!(!contract.tier1);
        assert_eq!(
            contract.direct_accessibility.state,
            AccessibilitySupportState::Unsupported
        );
        assert_eq!(contract.direct_accessibility.platform_shim, None);
        assert_eq!(contract.direct_accessibility.execution_channel, None);
        assert_eq!(
            contract.direct_accessibility.required_permissions,
            Vec::new()
        );
        assert!(
            contract
                .direct_accessibility
                .detail
                .contains("No Windows accessibility shim is shipped yet")
        );
    }

    #[test]
    fn accessibility_contract_roundtrip_serializes() {
        let contract = current_platform_accessibility();
        let json = serde_json::to_string(&contract).expect("serialize accessibility contract");
        let back: PlatformAccessibilityContract =
            serde_json::from_str(&json).expect("deserialize accessibility contract");
        assert_eq!(back, contract);
    }
}
