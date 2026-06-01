/// Human-like input timing helpers for browser automation.
///
/// Provides deterministic, reproducible delays and paths that mimic
/// natural mouse movement, typing cadence, and click placement.

const FALLBACK_SEED: u64 = 0x9E37_79B9_7F4A_7C15;
const MOUSE_PATH_SEED_SALT: u64 = 0xA076_1D64_78BD_642F;
const CLICK_POSITION_SEED_SALT: u64 = 0xE703_7ED1_A0B4_28DB;

/// Configuration for human-like input timing.
#[derive(Debug, Clone)]
pub struct HumanConfig {
    pub typing_delay_min_ms: u64,
    pub typing_delay_max_ms: u64,
    pub click_delay_ms: u64,
    pub mouse_steps: usize,
    pub seed: u64,
}

impl Default for HumanConfig {
    fn default() -> Self {
        Self {
            typing_delay_min_ms: 50,
            typing_delay_max_ms: 150,
            click_delay_ms: 100,
            mouse_steps: 10,
            seed: 42,
        }
    }
}

/// A 2D point used for mouse coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Deterministic xorshift64 PRNG. Mutates `state` in place and returns the
/// next pseudo-random `u64`.
pub(crate) fn pseudo_random(state: &mut u64) -> u64 {
    if *state == 0 {
        *state = FALLBACK_SEED;
    }
    let mut s = *state;
    s ^= s << 13;
    s ^= s >> 7;
    s ^= s << 17;
    *state = s;
    s
}

/// Returns a random delay in milliseconds between `min_ms` and `max_ms`
/// (inclusive) using the provided PRNG state.
pub(crate) fn random_delay(state: &mut u64, min_ms: u64, max_ms: u64) -> u64 {
    if min_ms >= max_ms {
        return min_ms;
    }
    let range = max_ms - min_ms + 1;
    min_ms + (pseudo_random(state) % range)
}

/// Returns a vector of per-character typing delays (in milliseconds) for `text`.
///
/// Each delay falls between `config.typing_delay_min_ms` and
/// `config.typing_delay_max_ms`. Repeated consecutive characters reduce the
/// previous delay's extra latency above the configured minimum to mimic
/// faster double-taps without leaving the configured bounds.
pub fn typing_delays(config: &HumanConfig, text: &str) -> Vec<u64> {
    let mut state = config.seed;
    let mut delays = Vec::with_capacity(text.chars().count());
    let mut previous: Option<(char, u64)> = None;

    for ch in text.chars() {
        let delay = match previous {
            Some((prev_ch, prev_delay)) if prev_ch == ch => {
                config.typing_delay_min_ms
                    + prev_delay.saturating_sub(config.typing_delay_min_ms) / 2
            }
            _ => random_delay(
                &mut state,
                config.typing_delay_min_ms,
                config.typing_delay_max_ms,
            ),
        };
        delays.push(delay);
        previous = Some((ch, delay));
    }

    delays
}

/// Returns the configured click delay in milliseconds.
pub fn click_delay_ms(config: &HumanConfig) -> u64 {
    config.click_delay_ms
}

/// Generates a quadratic Bezier path from `from` to `to` with `steps`
/// segments. The control point is deterministically offset using `seed` to
/// produce a natural-looking curve.
pub fn bezier_path(from: Point, to: Point, steps: usize, seed: u64) -> Vec<Point> {
    let steps = steps.max(1);
    let mut state = seed;

    // Randomised control point: midpoint + deterministic jitter.
    let rand_a = pseudo_random(&mut state);
    let rand_b = pseudo_random(&mut state);
    let jitter_x = (rand_a % 101) as f64 - 50.0; // -50..50
    let jitter_y = (rand_b % 101) as f64 - 50.0;
    let cp = Point {
        x: (from.x + to.x) / 2.0 + jitter_x,
        y: (from.y + to.y) / 2.0 + jitter_y,
    };

    (0..=steps)
        .map(|i| {
            let t = i as f64 / steps as f64;
            let inv = 1.0 - t;
            Point {
                x: inv * inv * from.x + 2.0 * inv * t * cp.x + t * t * to.x,
                y: inv * inv * from.y + 2.0 * inv * t * cp.y + t * t * to.y,
            }
        })
        .collect()
}

/// Generates a mouse path using the configured step count and a derived seed.
pub fn mouse_path(config: &HumanConfig, from: Point, to: Point) -> Vec<Point> {
    bezier_path(
        from,
        to,
        config.mouse_steps,
        config.seed ^ MOUSE_PATH_SEED_SALT,
    )
}

/// Returns a point near `center` with a small random offset. The offset
/// magnitude is at most `radius` pixels in each axis, determined by `seed`.
pub fn click_position_jitter(center: Point, radius: f64, seed: u64) -> Point {
    let mut state = seed;
    let rx = pseudo_random(&mut state);
    let ry = pseudo_random(&mut state);
    // Map to [-radius, +radius]
    let dx = (rx % 2001) as f64 / 1000.0 - 1.0; // -1.0..1.0
    let dy = (ry % 2001) as f64 / 1000.0 - 1.0;
    Point {
        x: center.x + dx * radius,
        y: center.y + dy * radius,
    }
}

/// Returns a jittered click position using the configured seed.
pub fn click_position(config: &HumanConfig, center: Point, radius: f64) -> Point {
    click_position_jitter(center, radius, config.seed ^ CLICK_POSITION_SEED_SALT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pseudo_random_is_deterministic() {
        let mut a = 123u64;
        let mut b = 123u64;
        let seq_a: Vec<u64> = (0..10).map(|_| pseudo_random(&mut a)).collect();
        let seq_b: Vec<u64> = (0..10).map(|_| pseudo_random(&mut b)).collect();
        assert_eq!(seq_a, seq_b);
    }

    #[test]
    fn pseudo_random_never_zero_stalls() {
        // Ensure the generator does not get stuck at zero.
        let mut state = 1u64;
        for _ in 0..1000 {
            let v = pseudo_random(&mut state);
            assert_ne!(v, 0, "PRNG should not produce zero from non-zero state");
        }
    }

    #[test]
    fn random_delay_within_range() {
        let mut state = 7u64;
        for _ in 0..200 {
            let d = random_delay(&mut state, 50, 150);
            assert!((50..=150).contains(&d), "delay {d} out of range");
        }
    }

    #[test]
    fn random_delay_equal_bounds() {
        let mut state = 99u64;
        assert_eq!(random_delay(&mut state, 100, 100), 100);
    }

    #[test]
    fn pseudo_random_recovers_from_zero_seed() {
        let mut state = 0u64;
        let first = pseudo_random(&mut state);
        let second = pseudo_random(&mut state);
        assert_ne!(first, 0);
        assert_ne!(second, 0);
        assert_ne!(first, second);
    }

    #[test]
    fn typing_delays_length_matches_text_char_count() {
        let config = HumanConfig::default();
        let text = "héllo🙂";
        let delays = typing_delays(&config, text);
        assert_eq!(delays.len(), text.chars().count());
    }

    #[test]
    fn typing_delays_are_deterministic() {
        let config = HumanConfig::default();
        let a = typing_delays(&config, "test string");
        let b = typing_delays(&config, "test string");
        assert_eq!(a, b);
    }

    #[test]
    fn typing_delays_repeated_char_is_faster() {
        let mut config = HumanConfig::default();
        config.seed = 1;
        // "aa" — the second 'a' should have half the delay of the first.
        let delays = typing_delays(&config, "aa");
        assert_eq!(delays.len(), 2);
        assert!(delays[1] <= delays[0], "repeated char should be faster");
        assert!(
            delays[1] >= config.typing_delay_min_ms,
            "repeated char should stay within configured minimum"
        );
    }

    #[test]
    fn typing_delays_with_zero_seed_still_vary() {
        let config = HumanConfig {
            seed: 0,
            ..HumanConfig::default()
        };
        let delays = typing_delays(&config, "human");
        assert!(delays.iter().all(|delay| {
            (config.typing_delay_min_ms..=config.typing_delay_max_ms).contains(delay)
        }));
        assert!(
            delays.windows(2).any(|window| window[0] != window[1]),
            "zero seed should not collapse to a constant delay stream"
        );
    }

    #[test]
    fn bezier_path_length() {
        let from = Point { x: 0.0, y: 0.0 };
        let to = Point { x: 100.0, y: 100.0 };
        let steps = 10;
        let path = bezier_path(from, to, steps, 42);
        // steps+1 points (inclusive of both endpoints).
        assert_eq!(path.len(), steps + 1);
    }

    #[test]
    fn bezier_path_zero_steps_still_returns_endpoints() {
        let from = Point { x: 10.0, y: 15.0 };
        let to = Point { x: 40.0, y: 45.0 };
        let path = bezier_path(from, to, 0, 3);
        assert_eq!(path.len(), 2);
        assert_eq!(path.first().copied(), Some(from));
        assert_eq!(path.last().copied(), Some(to));
    }

    #[test]
    fn click_delay_uses_configured_value() {
        let config = HumanConfig {
            click_delay_ms: 175,
            ..HumanConfig::default()
        };
        assert_eq!(click_delay_ms(&config), 175);
    }

    #[test]
    fn mouse_path_uses_configured_step_count() {
        let config = HumanConfig {
            mouse_steps: 4,
            ..HumanConfig::default()
        };
        let path = mouse_path(&config, Point { x: 0.0, y: 0.0 }, Point { x: 8.0, y: 6.0 });
        assert_eq!(path.len(), 5);
    }

    #[test]
    fn bezier_path_endpoints() {
        let from = Point { x: 10.0, y: 20.0 };
        let to = Point { x: 300.0, y: 400.0 };
        let path = bezier_path(from, to, 20, 99);
        let first = path.first().unwrap();
        let last = path.last().unwrap();
        assert!((first.x - from.x).abs() < 1e-9);
        assert!((first.y - from.y).abs() < 1e-9);
        assert!((last.x - to.x).abs() < 1e-9);
        assert!((last.y - to.y).abs() < 1e-9);
    }

    #[test]
    fn bezier_path_deterministic() {
        let from = Point { x: 0.0, y: 0.0 };
        let to = Point { x: 50.0, y: 50.0 };
        let a = bezier_path(from, to, 8, 77);
        let b = bezier_path(from, to, 8, 77);
        for (pa, pb) in a.iter().zip(b.iter()) {
            assert!((pa.x - pb.x).abs() < 1e-9);
            assert!((pa.y - pb.y).abs() < 1e-9);
        }
    }

    #[test]
    fn click_jitter_within_radius() {
        let center = Point { x: 500.0, y: 500.0 };
        let radius = 5.0;
        for seed in 1..200u64 {
            let jittered = click_position_jitter(center, radius, seed);
            let dx = (jittered.x - center.x).abs();
            let dy = (jittered.y - center.y).abs();
            assert!(dx <= radius + 1e-9, "x jitter {dx} exceeds radius {radius}");
            assert!(dy <= radius + 1e-9, "y jitter {dy} exceeds radius {radius}");
        }
    }

    #[test]
    fn click_jitter_deterministic() {
        let center = Point { x: 100.0, y: 200.0 };
        let a = click_position_jitter(center, 3.0, 42);
        let b = click_position_jitter(center, 3.0, 42);
        assert!((a.x - b.x).abs() < 1e-9);
        assert!((a.y - b.y).abs() < 1e-9);
    }

    #[test]
    fn click_position_uses_derived_config_seed() {
        let config = HumanConfig {
            seed: 9,
            ..HumanConfig::default()
        };
        let center = Point { x: 100.0, y: 200.0 };
        let a = click_position(&config, center, 3.0);
        let b = click_position(&config, center, 3.0);
        assert_eq!(a, b);
    }
}
