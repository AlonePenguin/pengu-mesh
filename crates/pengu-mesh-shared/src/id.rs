use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdKind {
    Profile,
    Instance,
    Tab,
    Run,
    ScenarioRun,
    ScenarioStep,
    ScenarioAssertion,
    LatencySample,
    EnvironmentFingerprint,
    Lease,
    Event,
    Artifact,
}

impl IdKind {
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Profile => "prof",
            Self::Instance => "inst",
            Self::Tab => "tab",
            Self::Run => "run",
            Self::ScenarioRun => "scenario_run",
            Self::ScenarioStep => "scenario_step",
            Self::ScenarioAssertion => "scenario_assertion",
            Self::LatencySample => "latency_sample",
            Self::EnvironmentFingerprint => "environment_fingerprint",
            Self::Lease => "lease",
            Self::Event => "event",
            Self::Artifact => "artifact",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StableId(String);

impl StableId {
    pub fn new(kind: IdKind, seed: impl AsRef<str>) -> Self {
        let cleaned = seed
            .as_ref()
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
            .collect::<String>()
            .trim_matches('_')
            .to_ascii_lowercase();
        Self(format!("{}_{}", kind.prefix(), cleaned))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{IdKind, StableId};

    #[test]
    fn builds_expected_prefixes() {
        let id = StableId::new(IdKind::Profile, "My Browser");
        assert_eq!(id.as_str(), "prof_my_browser");
    }
}
