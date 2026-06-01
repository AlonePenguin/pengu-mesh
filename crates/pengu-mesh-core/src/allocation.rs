/// Allocation policy abstraction and built-in implementations.
///
/// An `AllocationPolicy` decides which running instance should handle the next
/// request. Three concrete policies are provided: FCFS, round-robin, and random.
/// This mirrors PinchTab's `internal/allocation/` package.
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Candidate
// ---------------------------------------------------------------------------

/// A running instance that may be selected by an allocation policy.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// Unique identifier for the instance.
    pub instance_id: String,
    /// Current load factor in the range `0.0..=1.0`.
    pub load: f64,
    /// Whether the instance is currently accepting work.
    pub available: bool,
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors returned by [`AllocationPolicy::select`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationError {
    /// The candidate list was empty.
    NoCandidates,
    /// Candidates were present but none were available.
    NoneAvailable,
}

impl fmt::Display for AllocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCandidates => write!(f, "no candidate instances available"),
            Self::NoneAvailable => write!(f, "no candidate instances are currently available"),
        }
    }
}

impl std::error::Error for AllocationError {}

/// Errors returned by [`create_policy`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationFactoryError {
    /// The requested policy name does not match a built-in implementation.
    UnknownPolicy { name: String },
}

impl fmt::Display for AllocationFactoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPolicy { name } => write!(
                f,
                "unknown allocation policy {name:?} (available: {})",
                supported_policy_names().join(", ")
            ),
        }
    }
}

impl std::error::Error for AllocationFactoryError {}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Selects an instance from a list of running candidates.
pub trait AllocationPolicy {
    /// Pick the best instance from `candidates`, returning its `instance_id`.
    fn select(&mut self, candidates: &[Candidate]) -> Result<String, AllocationError>;
}

// ---------------------------------------------------------------------------
// FCFS
// ---------------------------------------------------------------------------

/// First-come-first-served: returns the first available candidate.
pub struct FcfsPolicy;

impl AllocationPolicy for FcfsPolicy {
    fn select(&mut self, candidates: &[Candidate]) -> Result<String, AllocationError> {
        if candidates.is_empty() {
            return Err(AllocationError::NoCandidates);
        }
        candidates
            .iter()
            .find(|c| c.available)
            .map(|c| c.instance_id.clone())
            .ok_or(AllocationError::NoneAvailable)
    }
}

// ---------------------------------------------------------------------------
// Round-robin
// ---------------------------------------------------------------------------

/// Cycles through available candidates in order.
pub struct RoundRobinPolicy {
    last_index: usize,
}

impl RoundRobinPolicy {
    pub fn new() -> Self {
        Self { last_index: 0 }
    }
}

impl Default for RoundRobinPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl AllocationPolicy for RoundRobinPolicy {
    fn select(&mut self, candidates: &[Candidate]) -> Result<String, AllocationError> {
        if candidates.is_empty() {
            return Err(AllocationError::NoCandidates);
        }
        let available: Vec<_> = candidates.iter().filter(|c| c.available).collect();
        if available.is_empty() {
            return Err(AllocationError::NoneAvailable);
        }
        let idx = self.last_index % available.len();
        self.last_index = self.last_index.wrapping_add(1);
        Ok(available[idx].instance_id.clone())
    }
}

// ---------------------------------------------------------------------------
// Random (deterministic LCG — no external dependency)
// ---------------------------------------------------------------------------

/// Selects a random available candidate using a simple linear congruential
/// generator, avoiding the need for the `rand` crate.
pub struct RandomPolicy {
    state: u64,
}

impl RandomPolicy {
    pub fn new() -> Self {
        Self {
            state: default_random_seed(),
        }
    }

    /// Create with an explicit seed for reproducible tests.
    pub fn with_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        // LCG parameters from Numerical Recipes.
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }
}

impl Default for RandomPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl AllocationPolicy for RandomPolicy {
    fn select(&mut self, candidates: &[Candidate]) -> Result<String, AllocationError> {
        if candidates.is_empty() {
            return Err(AllocationError::NoCandidates);
        }
        let available: Vec<_> = candidates.iter().filter(|c| c.available).collect();
        if available.is_empty() {
            return Err(AllocationError::NoneAvailable);
        }
        let idx = (self.next_u64() as usize) % available.len();
        Ok(available[idx].instance_id.clone())
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

const SUPPORTED_POLICY_NAMES: &[&str] = &["fcfs", "round_robin", "random"];

fn supported_policy_names() -> &'static [&'static str] {
    SUPPORTED_POLICY_NAMES
}

fn default_random_seed() -> u64 {
    static SEED_COUNTER: AtomicU64 = AtomicU64::new(0);

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    let counter = SEED_COUNTER.fetch_add(1, Ordering::Relaxed);
    nanos ^ counter.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// Create a boxed allocation policy by name.
///
/// Recognised names: `"fcfs"`, `"round_robin"`, `"random"`.
pub fn create_policy(name: &str) -> Result<Box<dyn AllocationPolicy>, AllocationFactoryError> {
    match name {
        "fcfs" => Ok(Box::new(FcfsPolicy)),
        "round_robin" => Ok(Box::new(RoundRobinPolicy::new())),
        "random" => Ok(Box::new(RandomPolicy::new())),
        other => Err(AllocationFactoryError::UnknownPolicy {
            name: other.to_owned(),
        }),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_candidates() -> Vec<Candidate> {
        vec![
            Candidate {
                instance_id: "a".into(),
                load: 0.1,
                available: true,
            },
            Candidate {
                instance_id: "b".into(),
                load: 0.5,
                available: true,
            },
            Candidate {
                instance_id: "c".into(),
                load: 0.9,
                available: false,
            },
        ]
    }

    // -- FCFS ---------------------------------------------------------------

    #[test]
    fn fcfs_selects_first_available() {
        let mut p = FcfsPolicy;
        let result = p.select(&sample_candidates()).unwrap();
        assert_eq!(result, "a");
    }

    #[test]
    fn fcfs_skips_unavailable() {
        let candidates = vec![
            Candidate {
                instance_id: "x".into(),
                load: 0.0,
                available: false,
            },
            Candidate {
                instance_id: "y".into(),
                load: 0.0,
                available: true,
            },
        ];
        let mut p = FcfsPolicy;
        assert_eq!(p.select(&candidates).unwrap(), "y");
    }

    #[test]
    fn fcfs_empty_returns_no_candidates() {
        let mut p = FcfsPolicy;
        assert_eq!(p.select(&[]), Err(AllocationError::NoCandidates));
    }

    #[test]
    fn fcfs_all_unavailable_returns_none_available() {
        let candidates = vec![Candidate {
            instance_id: "z".into(),
            load: 0.0,
            available: false,
        }];
        let mut p = FcfsPolicy;
        assert_eq!(p.select(&candidates), Err(AllocationError::NoneAvailable));
    }

    // -- Round-robin --------------------------------------------------------

    #[test]
    fn round_robin_cycles() {
        let mut p = RoundRobinPolicy::new();
        let candidates = sample_candidates(); // a(avail), b(avail), c(not)
        let r1 = p.select(&candidates).unwrap();
        let r2 = p.select(&candidates).unwrap();
        let r3 = p.select(&candidates).unwrap();
        // Should cycle through the two available candidates: a, b, a
        assert_eq!(r1, "a");
        assert_eq!(r2, "b");
        assert_eq!(r3, "a");
    }

    #[test]
    fn round_robin_empty() {
        let mut p = RoundRobinPolicy::new();
        assert_eq!(p.select(&[]), Err(AllocationError::NoCandidates));
    }

    #[test]
    fn round_robin_none_available() {
        let mut p = RoundRobinPolicy::new();
        let candidates = vec![Candidate {
            instance_id: "z".into(),
            load: 0.0,
            available: false,
        }];
        assert_eq!(p.select(&candidates), Err(AllocationError::NoneAvailable));
    }

    // -- Random -------------------------------------------------------------

    #[test]
    fn random_selects_available() {
        let mut p = RandomPolicy::with_seed(42);
        let candidates = sample_candidates();
        // Should always return an available candidate (a or b, never c).
        for _ in 0..20 {
            let id = p.select(&candidates).unwrap();
            assert!(id == "a" || id == "b", "unexpected id: {id}");
        }
    }

    #[test]
    fn random_empty() {
        let mut p = RandomPolicy::new();
        assert_eq!(p.select(&[]), Err(AllocationError::NoCandidates));
    }

    #[test]
    fn random_none_available() {
        let mut p = RandomPolicy::new();
        let candidates = vec![Candidate {
            instance_id: "z".into(),
            load: 0.0,
            available: false,
        }];
        assert_eq!(p.select(&candidates), Err(AllocationError::NoneAvailable));
    }

    #[test]
    fn random_with_seed_is_deterministic() {
        let candidates = sample_candidates();
        let mut p1 = RandomPolicy::with_seed(99);
        let mut p2 = RandomPolicy::with_seed(99);
        for _ in 0..10 {
            assert_eq!(
                p1.select(&candidates).unwrap(),
                p2.select(&candidates).unwrap()
            );
        }
    }

    #[test]
    fn random_new_uses_distinct_seeds() {
        let p1 = RandomPolicy::new();
        let p2 = RandomPolicy::new();
        assert_ne!(p1.state, p2.state);
    }

    // -- Factory ------------------------------------------------------------

    #[test]
    fn factory_creates_fcfs() {
        let mut p = create_policy("fcfs").unwrap();
        let candidates = sample_candidates();
        assert_eq!(p.select(&candidates).unwrap(), "a");
    }

    #[test]
    fn factory_creates_round_robin() {
        let mut p = create_policy("round_robin").unwrap();
        let candidates = sample_candidates();
        let _ = p.select(&candidates).unwrap();
        // Second call should differ from first if more than one available.
        let r2 = p.select(&candidates).unwrap();
        assert_eq!(r2, "b");
    }

    #[test]
    fn factory_creates_random() {
        let mut p = create_policy("random").unwrap();
        let candidates = sample_candidates();
        let id = p.select(&candidates).unwrap();
        assert!(id == "a" || id == "b");
    }

    #[test]
    fn factory_unknown_returns_error() {
        match create_policy("nope") {
            Ok(_) => panic!("expected unknown allocation policy error"),
            Err(error) => assert_eq!(
                error,
                AllocationFactoryError::UnknownPolicy {
                    name: "nope".into(),
                }
            ),
        }
    }

    // -- Error display ------------------------------------------------------

    #[test]
    fn error_display() {
        assert_eq!(
            AllocationError::NoCandidates.to_string(),
            "no candidate instances available"
        );
        assert_eq!(
            AllocationError::NoneAvailable.to_string(),
            "no candidate instances are currently available"
        );
        assert_eq!(
            AllocationFactoryError::UnknownPolicy {
                name: "nope".into()
            }
            .to_string(),
            "unknown allocation policy \"nope\" (available: fcfs, round_robin, random)"
        );
    }
}
