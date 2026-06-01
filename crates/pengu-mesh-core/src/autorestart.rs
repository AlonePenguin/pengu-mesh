/// Auto-restart strategy with exponential backoff for managed Chrome instances.
///
/// Tracks crash frequency and decides whether to restart (with a computed delay)
/// or give up after too many rapid failures. If an instance stays alive longer
/// than the configured stable period the failure counter resets, treating the
/// next crash as a fresh first failure.
use std::time::Instant;

// ── Configuration ────────────────────────────────────────────────────

/// Tunables for the restart strategy.
#[derive(Debug, Clone)]
pub struct RestartConfig {
    /// Maximum consecutive restart attempts before giving up.
    pub max_restarts: u32,
    /// Delay before the first restart, in milliseconds.
    pub initial_delay_ms: u64,
    /// Upper-bound on the backoff delay, in milliseconds.
    pub max_delay_ms: u64,
    /// Multiplier applied to the delay after each consecutive failure.
    pub backoff_factor: f64,
    /// If the instance runs for at least this many milliseconds the failure
    /// counter resets on the next crash.
    pub stable_period_ms: u64,
}

const DEFAULT_MAX_RESTARTS: u32 = 3;
const DEFAULT_INITIAL_DELAY_MS: u64 = 2_000;
const DEFAULT_MAX_DELAY_MS: u64 = 60_000;
const DEFAULT_BACKOFF_FACTOR: f64 = 2.0;
const DEFAULT_STABLE_PERIOD_MS: u64 = 300_000;

impl Default for RestartConfig {
    fn default() -> Self {
        Self {
            max_restarts: DEFAULT_MAX_RESTARTS,
            initial_delay_ms: DEFAULT_INITIAL_DELAY_MS,
            max_delay_ms: DEFAULT_MAX_DELAY_MS,
            backoff_factor: DEFAULT_BACKOFF_FACTOR,
            stable_period_ms: DEFAULT_STABLE_PERIOD_MS,
        }
    }
}

// ── Decision ─────────────────────────────────────────────────────────

/// Outcome of a restart evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum RestartDecision {
    /// The instance should be restarted after waiting `delay_ms`.
    Restart { delay_ms: u64 },
    /// Too many failures — do not restart.
    GiveUp { reason: String },
}

// ── Tracker ──────────────────────────────────────────────────────────

/// Mutable state that tracks crash history and computes restart decisions.
#[derive(Debug)]
pub struct RestartTracker {
    attempts: u32,
    last_crash_at: Option<Instant>,
    last_start_at: Option<Instant>,
    config: RestartConfig,
}

impl RestartTracker {
    pub fn new(config: RestartConfig) -> Self {
        Self {
            attempts: 0,
            last_crash_at: None,
            last_start_at: None,
            config,
        }
    }

    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    pub fn last_crash_at(&self) -> Option<Instant> {
        self.last_crash_at
    }

    pub fn last_start_at(&self) -> Option<Instant> {
        self.last_start_at
    }

    pub fn config(&self) -> &RestartConfig {
        &self.config
    }

    /// Record that the managed instance has (re)started.
    pub fn record_start(&mut self) {
        self.last_start_at = Some(Instant::now());
    }

    /// Record a crash. If the instance was running longer than the stable
    /// period the attempt counter resets first.
    pub fn record_crash(&mut self) {
        let now = Instant::now();

        if let Some(start) = self.last_start_at {
            let uptime_ms = now.duration_since(start).as_millis() as u64;
            if uptime_ms >= self.config.stable_period_ms {
                self.attempts = 0;
            }
        }

        self.attempts += 1;
        self.last_crash_at = Some(now);
    }

    /// Decide whether to restart or give up based on the current state.
    ///
    /// Call this after `record_crash`, which advances the consecutive-failure
    /// counter used for the restart allowance and delay calculation.
    pub fn should_restart(&self) -> RestartDecision {
        if self.attempts > self.config.max_restarts {
            return RestartDecision::GiveUp {
                reason: format!(
                    "exceeded max restarts ({} of {})",
                    self.attempts, self.config.max_restarts
                ),
            };
        }

        // attempts is at least 1 after record_crash; exponent is 0-based.
        let exponent = self.attempts.saturating_sub(1);
        let delay = (self.config.initial_delay_ms as f64)
            * self.config.backoff_factor.powi(exponent as i32);
        let delay_ms = (delay as u64).min(self.config.max_delay_ms);

        RestartDecision::Restart { delay_ms }
    }

    /// Reset all tracking state (e.g. after a manual restart or
    /// re-configuration).
    pub fn reset(&mut self) {
        self.attempts = 0;
        self.last_crash_at = None;
        self.last_start_at = None;
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_tracker() -> RestartTracker {
        RestartTracker::new(RestartConfig::default())
    }

    #[test]
    fn defaults_match_upstream_autorestart_profile() {
        let config = RestartConfig::default();
        assert_eq!(config.max_restarts, DEFAULT_MAX_RESTARTS);
        assert_eq!(config.initial_delay_ms, DEFAULT_INITIAL_DELAY_MS);
        assert_eq!(config.max_delay_ms, DEFAULT_MAX_DELAY_MS);
        assert_eq!(config.backoff_factor, DEFAULT_BACKOFF_FACTOR);
        assert_eq!(config.stable_period_ms, DEFAULT_STABLE_PERIOD_MS);
    }

    #[test]
    fn normal_restart_after_first_crash() {
        let mut t = default_tracker();
        t.record_start();
        t.record_crash();

        match t.should_restart() {
            RestartDecision::Restart { delay_ms } => {
                assert_eq!(
                    delay_ms, DEFAULT_INITIAL_DELAY_MS,
                    "first restart uses initial delay"
                );
            }
            other => panic!("expected Restart, got {other:?}"),
        }
    }

    #[test]
    fn backoff_progression() {
        let mut t = RestartTracker::new(RestartConfig {
            max_restarts: 10,
            initial_delay_ms: 100,
            max_delay_ms: 50_000,
            backoff_factor: 2.0,
            stable_period_ms: 60_000,
        });

        let expected = [100, 200, 400, 800, 1_600];
        for (i, &want) in expected.iter().enumerate() {
            t.record_crash();
            match t.should_restart() {
                RestartDecision::Restart { delay_ms } => {
                    assert_eq!(delay_ms, want, "attempt {}", i + 1);
                }
                other => panic!("expected Restart at attempt {}, got {other:?}", i + 1),
            }
        }
    }

    #[test]
    fn max_delay_cap() {
        let mut t = RestartTracker::new(RestartConfig {
            max_restarts: 20,
            initial_delay_ms: 1_000,
            max_delay_ms: 5_000,
            backoff_factor: 10.0,
            stable_period_ms: 60_000,
        });

        // After two crashes: 1_000 * 10^1 = 10_000 → capped to 5_000
        t.record_crash();
        t.record_crash();

        match t.should_restart() {
            RestartDecision::Restart { delay_ms } => {
                assert_eq!(delay_ms, 5_000, "delay should be capped at max_delay_ms");
            }
            other => panic!("expected Restart, got {other:?}"),
        }
    }

    #[test]
    fn max_restarts_allows_exact_restart_budget() {
        let mut t = RestartTracker::new(RestartConfig {
            max_restarts: 3,
            ..RestartConfig::default()
        });

        for _ in 0..3 {
            t.record_crash();
            match t.should_restart() {
                RestartDecision::Restart { .. } => {}
                other => panic!("expected Restart within budget, got {other:?}"),
            }
        }

        t.record_crash();
        match t.should_restart() {
            RestartDecision::GiveUp { reason } => {
                assert!(
                    reason.contains("exceeded max restarts"),
                    "unexpected reason: {reason}"
                );
            }
            other => panic!("expected GiveUp, got {other:?}"),
        }
    }

    #[test]
    fn stable_period_resets_counter() {
        let mut t = RestartTracker::new(RestartConfig {
            max_restarts: 3,
            initial_delay_ms: 100,
            max_delay_ms: 50_000,
            backoff_factor: 2.0,
            // Use 0 ms so the stable period is trivially met.
            stable_period_ms: 0,
        });

        // Accumulate two crashes.
        t.record_crash();
        t.record_crash();
        assert_eq!(t.attempts, 2);

        // Start the instance and immediately crash — but the stable period
        // (0 ms) has elapsed, so the counter resets first.
        t.record_start();
        t.record_crash();
        assert_eq!(
            t.attempts, 1,
            "counter should have reset before incrementing"
        );

        // The delay should correspond to the first attempt again.
        match t.should_restart() {
            RestartDecision::Restart { delay_ms } => {
                assert_eq!(delay_ms, 100);
            }
            other => panic!("expected Restart, got {other:?}"),
        }
    }

    #[test]
    fn reset_clears_state() {
        let mut t = default_tracker();
        t.record_start();
        t.record_crash();
        t.record_crash();
        assert_eq!(t.attempts, 2);

        t.reset();
        assert_eq!(t.attempts, 0);
        assert!(t.last_crash_at.is_none());
        assert!(t.last_start_at.is_none());
    }
}
