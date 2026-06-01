use serde::{Deserialize, Serialize};

/// Risk tier for a capability, ordered from least to most dangerous.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityRiskTier {
    Safe,
    Elevated,
    Dangerous,
}

/// Describes a single capability with its risk classification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityDescriptor {
    pub name: String,
    pub risk_tier: CapabilityRiskTier,
    pub description: String,
    pub requires_explicit_grant: bool,
}

/// Policy that governs which capabilities are allowed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityGatePolicy {
    #[serde(default = "default_true")]
    pub allow_safe: bool,
    #[serde(default)]
    pub allow_elevated: bool,
    #[serde(default)]
    pub allow_dangerous: bool,
    #[serde(default)]
    pub explicit_grants: Vec<String>,
}

fn default_true() -> bool {
    true
}

impl Default for CapabilityGatePolicy {
    fn default() -> Self {
        Self {
            allow_safe: true,
            allow_elevated: false,
            allow_dangerous: false,
            explicit_grants: Vec::new(),
        }
    }
}

/// Result of evaluating a capability against a gate policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "decision")]
pub enum CapabilityDecision {
    Allowed,
    Denied { reason: String },
    RequiresGrant { capability: String },
}

impl CapabilityGatePolicy {
    /// Evaluate whether a capability is allowed under this policy.
    pub fn evaluate(&self, capability: &CapabilityDescriptor) -> CapabilityDecision {
        // Explicit grants override tier-based checks.
        if self.explicit_grants.contains(&capability.name) {
            return CapabilityDecision::Allowed;
        }

        // If the capability requires an explicit grant and none was found, signal that.
        if capability.requires_explicit_grant {
            return CapabilityDecision::RequiresGrant {
                capability: capability.name.clone(),
            };
        }

        let tier_allowed = match capability.risk_tier {
            CapabilityRiskTier::Safe => self.allow_safe,
            CapabilityRiskTier::Elevated => self.allow_elevated,
            CapabilityRiskTier::Dangerous => self.allow_dangerous,
        };

        if tier_allowed {
            CapabilityDecision::Allowed
        } else {
            CapabilityDecision::Denied {
                reason: format!(
                    "capability '{}' requires tier {:?} which is not allowed by policy",
                    capability.name, capability.risk_tier
                ),
            }
        }
    }
}

/// Build a structured JSON denial payload for logging or wire responses.
pub fn capability_denial_payload(
    operation: &str,
    capability: &str,
    tier: &str,
    reason: &str,
) -> serde_json::Value {
    serde_json::json!({
        "denied": true,
        "operation": operation,
        "capability": capability,
        "tier": tier,
        "reason": reason,
    })
}

/// Returns the built-in capability descriptors with their assigned risk tiers.
pub fn default_capabilities() -> Vec<CapabilityDescriptor> {
    let safe = &[
        ("navigate", "Navigate to a URL"),
        ("snapshot", "Capture a DOM snapshot"),
        ("text", "Extract text content"),
        ("screenshot", "Capture a screenshot"),
        ("pdf", "Generate a PDF"),
        ("artifact_list", "List artifacts"),
        ("health", "Health check"),
        ("diagnose", "Run diagnostics"),
    ];
    let elevated = &[
        ("evaluate", "Evaluate JavaScript in page"),
        ("click", "Click an element"),
        ("fill", "Fill an input element"),
        ("type", "Type text into an element"),
        ("press", "Press a key"),
        ("select", "Select an option"),
        ("instance_start", "Start a browser instance"),
        ("tab_open", "Open a new tab"),
    ];
    let dangerous: &[(&str, &str, bool)] = &[
        ("host_access_setup", "Apply-mode host access setup", true),
        (
            "browser_surface_action",
            "Global takeover surface action",
            true,
        ),
    ];

    let mut caps = Vec::new();

    for &(name, desc) in safe {
        caps.push(CapabilityDescriptor {
            name: name.to_owned(),
            risk_tier: CapabilityRiskTier::Safe,
            description: desc.to_owned(),
            requires_explicit_grant: false,
        });
    }
    for &(name, desc) in elevated {
        caps.push(CapabilityDescriptor {
            name: name.to_owned(),
            risk_tier: CapabilityRiskTier::Elevated,
            description: desc.to_owned(),
            requires_explicit_grant: false,
        });
    }
    for &(name, desc, requires_grant) in dangerous {
        caps.push(CapabilityDescriptor {
            name: name.to_owned(),
            risk_tier: CapabilityRiskTier::Dangerous,
            description: desc.to_owned(),
            requires_explicit_grant: requires_grant,
        });
    }

    caps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_ordering() {
        assert!(CapabilityRiskTier::Safe < CapabilityRiskTier::Elevated);
        assert!(CapabilityRiskTier::Elevated < CapabilityRiskTier::Dangerous);
    }

    #[test]
    fn default_policy_allows_safe_only() {
        let policy = CapabilityGatePolicy::default();
        assert!(policy.allow_safe);
        assert!(!policy.allow_elevated);
        assert!(!policy.allow_dangerous);
        assert!(policy.explicit_grants.is_empty());
    }

    #[test]
    fn evaluate_safe_allowed_by_default() {
        let policy = CapabilityGatePolicy::default();
        let cap = CapabilityDescriptor {
            name: "navigate".to_owned(),
            risk_tier: CapabilityRiskTier::Safe,
            description: "Navigate".to_owned(),
            requires_explicit_grant: false,
        };
        assert_eq!(policy.evaluate(&cap), CapabilityDecision::Allowed);
    }

    #[test]
    fn evaluate_elevated_denied_by_default() {
        let policy = CapabilityGatePolicy::default();
        let cap = CapabilityDescriptor {
            name: "click".to_owned(),
            risk_tier: CapabilityRiskTier::Elevated,
            description: "Click".to_owned(),
            requires_explicit_grant: false,
        };
        match policy.evaluate(&cap) {
            CapabilityDecision::Denied { reason } => {
                assert!(reason.contains("click"));
                assert!(reason.contains("Elevated"));
            }
            other => panic!("expected Denied, got {other:?}"),
        }
    }

    #[test]
    fn evaluate_dangerous_denied_by_default() {
        let policy = CapabilityGatePolicy::default();
        let cap = CapabilityDescriptor {
            name: "host_access_setup".to_owned(),
            risk_tier: CapabilityRiskTier::Dangerous,
            description: "Host access".to_owned(),
            requires_explicit_grant: true,
        };
        // requires_explicit_grant takes precedence, so we get RequiresGrant
        match policy.evaluate(&cap) {
            CapabilityDecision::RequiresGrant { capability } => {
                assert_eq!(capability, "host_access_setup");
            }
            other => panic!("expected RequiresGrant, got {other:?}"),
        }
    }

    #[test]
    fn explicit_grant_overrides_tier() {
        let policy = CapabilityGatePolicy {
            allow_dangerous: false,
            explicit_grants: vec!["host_access_setup".to_owned()],
            ..Default::default()
        };
        let cap = CapabilityDescriptor {
            name: "host_access_setup".to_owned(),
            risk_tier: CapabilityRiskTier::Dangerous,
            description: "Host access".to_owned(),
            requires_explicit_grant: true,
        };
        assert_eq!(policy.evaluate(&cap), CapabilityDecision::Allowed);
    }

    #[test]
    fn explicit_grant_overrides_requires_explicit_grant() {
        let policy = CapabilityGatePolicy {
            explicit_grants: vec!["browser_surface_action".to_owned()],
            ..Default::default()
        };
        let cap = CapabilityDescriptor {
            name: "browser_surface_action".to_owned(),
            risk_tier: CapabilityRiskTier::Dangerous,
            description: "Global takeover".to_owned(),
            requires_explicit_grant: true,
        };
        assert_eq!(policy.evaluate(&cap), CapabilityDecision::Allowed);
    }

    #[test]
    fn elevated_allowed_when_policy_permits() {
        let policy = CapabilityGatePolicy {
            allow_elevated: true,
            ..Default::default()
        };
        let cap = CapabilityDescriptor {
            name: "click".to_owned(),
            risk_tier: CapabilityRiskTier::Elevated,
            description: "Click".to_owned(),
            requires_explicit_grant: false,
        };
        assert_eq!(policy.evaluate(&cap), CapabilityDecision::Allowed);
    }

    #[test]
    fn dangerous_allowed_when_policy_permits_and_no_explicit_grant_required() {
        let policy = CapabilityGatePolicy {
            allow_dangerous: true,
            ..Default::default()
        };
        let cap = CapabilityDescriptor {
            name: "some_dangerous_op".to_owned(),
            risk_tier: CapabilityRiskTier::Dangerous,
            description: "A dangerous operation".to_owned(),
            requires_explicit_grant: false,
        };
        assert_eq!(policy.evaluate(&cap), CapabilityDecision::Allowed);
    }

    #[test]
    fn denial_payload_structure() {
        let payload =
            capability_denial_payload("run_script", "evaluate", "elevated", "not allowed");
        assert_eq!(payload["denied"], true);
        assert_eq!(payload["operation"], "run_script");
        assert_eq!(payload["capability"], "evaluate");
        assert_eq!(payload["tier"], "elevated");
        assert_eq!(payload["reason"], "not allowed");
    }

    #[test]
    fn default_capabilities_coverage() {
        let caps = default_capabilities();

        let safe: Vec<_> = caps
            .iter()
            .filter(|c| c.risk_tier == CapabilityRiskTier::Safe)
            .collect();
        let elevated: Vec<_> = caps
            .iter()
            .filter(|c| c.risk_tier == CapabilityRiskTier::Elevated)
            .collect();
        let dangerous: Vec<_> = caps
            .iter()
            .filter(|c| c.risk_tier == CapabilityRiskTier::Dangerous)
            .collect();

        assert_eq!(safe.len(), 8);
        assert_eq!(elevated.len(), 8);
        assert_eq!(dangerous.len(), 2);

        // All dangerous capabilities require explicit grants.
        for cap in &dangerous {
            assert!(
                cap.requires_explicit_grant,
                "{} should require grant",
                cap.name
            );
        }
        // Safe and elevated do not.
        for cap in safe.iter().chain(elevated.iter()) {
            assert!(
                !cap.requires_explicit_grant,
                "{} should not require grant",
                cap.name
            );
        }
    }

    #[test]
    fn tier_serde_roundtrip() {
        let tier = CapabilityRiskTier::Elevated;
        let json = serde_json::to_string(&tier).unwrap();
        assert_eq!(json, "\"elevated\"");
        let back: CapabilityRiskTier = serde_json::from_str(&json).unwrap();
        assert_eq!(back, tier);
    }

    #[test]
    fn policy_serde_defaults() {
        let json = "{}";
        let policy: CapabilityGatePolicy = serde_json::from_str(json).unwrap();
        assert!(policy.allow_safe);
        assert!(!policy.allow_elevated);
        assert!(!policy.allow_dangerous);
        assert!(policy.explicit_grants.is_empty());
    }

    #[test]
    fn decision_serde_roundtrip() {
        let decisions = vec![
            CapabilityDecision::Allowed,
            CapabilityDecision::Denied {
                reason: "nope".to_owned(),
            },
            CapabilityDecision::RequiresGrant {
                capability: "foo".to_owned(),
            },
        ];
        for d in &decisions {
            let json = serde_json::to_string(d).unwrap();
            let back: CapabilityDecision = serde_json::from_str(&json).unwrap();
            assert_eq!(&back, d);
        }
    }
}
