use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Declares a latency budget for a named metric.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PerformanceThreshold {
    pub name: String,
    pub metric: String,
    pub max_ms: u64,
    pub p50_ms: Option<u64>,
    pub p95_ms: Option<u64>,
    pub p99_ms: Option<u64>,
}

/// A single budget violation detected during evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThresholdViolation {
    pub threshold_name: String,
    pub metric: String,
    pub expected_ms: u64,
    pub actual_ms: u64,
    pub percentile: String,
}

/// Aggregated result of evaluating one threshold against its samples.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThresholdResult {
    pub threshold_name: String,
    pub metric: String,
    pub passed: bool,
    pub violations: Vec<ThresholdViolation>,
    pub samples_evaluated: usize,
}

/// Return the value at the given percentile from a **sorted** slice.
///
/// Uses nearest-rank: index = ceil(p * len) - 1, clamped to valid range.
fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((p * sorted.len() as f64).ceil() as usize).saturating_sub(1);
    let idx = idx.min(sorted.len() - 1);
    sorted[idx]
}

/// Evaluate a single [`PerformanceThreshold`] against raw latency samples.
pub fn evaluate_threshold(threshold: &PerformanceThreshold, samples: &[u64]) -> ThresholdResult {
    if samples.is_empty() {
        return ThresholdResult {
            threshold_name: threshold.name.clone(),
            metric: threshold.metric.clone(),
            passed: true,
            violations: Vec::new(),
            samples_evaluated: 0,
        };
    }

    let mut sorted = samples.to_vec();
    sorted.sort_unstable();

    let mut violations = Vec::new();

    let mut check = |label: &str, budget: u64, actual: u64| {
        if actual > budget {
            violations.push(ThresholdViolation {
                threshold_name: threshold.name.clone(),
                metric: threshold.metric.clone(),
                expected_ms: budget,
                actual_ms: actual,
                percentile: label.to_string(),
            });
        }
    };

    check("max", threshold.max_ms, sorted[sorted.len() - 1]);

    if let Some(budget) = threshold.p50_ms {
        check("p50", budget, percentile(&sorted, 0.50));
    }
    if let Some(budget) = threshold.p95_ms {
        check("p95", budget, percentile(&sorted, 0.95));
    }
    if let Some(budget) = threshold.p99_ms {
        check("p99", budget, percentile(&sorted, 0.99));
    }

    ThresholdResult {
        threshold_name: threshold.name.clone(),
        metric: threshold.metric.clone(),
        passed: violations.is_empty(),
        violations,
        samples_evaluated: samples.len(),
    }
}

/// Evaluate many thresholds, each matched to its metric key in `samples_by_metric`.
///
/// Thresholds whose metric has no entry in the map are evaluated with an empty
/// slice (and therefore pass with zero samples).
pub fn evaluate_thresholds(
    thresholds: &[PerformanceThreshold],
    samples_by_metric: &HashMap<String, Vec<u64>>,
) -> Vec<ThresholdResult> {
    let empty: Vec<u64> = Vec::new();
    thresholds
        .iter()
        .map(|t| {
            let samples = samples_by_metric.get(&t.metric).unwrap_or(&empty);
            evaluate_threshold(t, samples)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_threshold(
        name: &str,
        metric: &str,
        max_ms: u64,
        p50: Option<u64>,
        p95: Option<u64>,
        p99: Option<u64>,
    ) -> PerformanceThreshold {
        PerformanceThreshold {
            name: name.to_string(),
            metric: metric.to_string(),
            max_ms,
            p50_ms: p50,
            p95_ms: p95,
            p99_ms: p99,
        }
    }

    #[test]
    fn all_pass() {
        let t = make_threshold("fast", "load", 100, Some(50), Some(80), Some(90));
        let samples = vec![10, 20, 30, 40, 50];
        let result = evaluate_threshold(&t, &samples);
        assert_eq!(result.threshold_name, "fast");
        assert_eq!(result.metric, "load");
        assert!(result.passed);
        assert!(result.violations.is_empty());
        assert_eq!(result.samples_evaluated, 5);
    }

    #[test]
    fn max_violation() {
        let t = make_threshold("fast", "load", 100, None, None, None);
        let samples = vec![50, 60, 110];
        let result = evaluate_threshold(&t, &samples);
        assert!(!result.passed);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].percentile, "max");
        assert_eq!(result.violations[0].actual_ms, 110);
        assert_eq!(result.violations[0].expected_ms, 100);
    }

    #[test]
    fn p50_violation() {
        let t = make_threshold("median", "render", 1000, Some(30), None, None);
        // sorted: [10, 20, 50, 60, 80] -> p50 = 50
        let samples = vec![80, 10, 50, 20, 60];
        let result = evaluate_threshold(&t, &samples);
        assert!(!result.passed);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].percentile, "p50");
        assert_eq!(result.violations[0].actual_ms, 50);
    }

    #[test]
    fn multiple_violations() {
        let t = make_threshold("strict", "api", 50, Some(10), Some(30), Some(40));
        // sorted: [5, 15, 25, 35, 45, 55]
        // max=55 > 50, p50=25 > 10, p95=55 > 30, p99=55 > 40
        let samples = vec![55, 5, 35, 15, 45, 25];
        let result = evaluate_threshold(&t, &samples);
        assert!(!result.passed);
        assert_eq!(result.violations.len(), 4);
        let labels: Vec<&str> = result
            .violations
            .iter()
            .map(|v| v.percentile.as_str())
            .collect();
        assert_eq!(labels, vec!["max", "p50", "p95", "p99"]);
    }

    #[test]
    fn empty_samples() {
        let t = make_threshold("any", "metric", 100, Some(50), Some(80), Some(90));
        let result = evaluate_threshold(&t, &[]);
        assert_eq!(result.threshold_name, "any");
        assert_eq!(result.metric, "metric");
        assert!(result.passed);
        assert!(result.violations.is_empty());
        assert_eq!(result.samples_evaluated, 0);
    }

    #[test]
    fn single_sample_pass() {
        let t = make_threshold("one", "ping", 100, Some(100), Some(100), Some(100));
        let result = evaluate_threshold(&t, &[42]);
        assert!(result.passed);
        assert_eq!(result.samples_evaluated, 1);
    }

    #[test]
    fn single_sample_fail() {
        let t = make_threshold("one", "ping", 40, None, None, None);
        let result = evaluate_threshold(&t, &[42]);
        assert!(!result.passed);
        assert_eq!(result.violations[0].actual_ms, 42);
    }

    #[test]
    fn boundary_exact_max() {
        let t = make_threshold("edge", "x", 100, None, None, None);
        let result = evaluate_threshold(&t, &[100]);
        assert!(result.passed, "exact boundary should pass (not >)");
    }

    #[test]
    fn percentile_helper() {
        let sorted = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        assert_eq!(percentile(&sorted, 0.50), 50);
        assert_eq!(percentile(&sorted, 0.95), 100);
        assert_eq!(percentile(&sorted, 0.99), 100);
        assert_eq!(percentile(&sorted, 0.0), 10);
        assert_eq!(percentile(&sorted, 1.0), 100);
    }

    #[test]
    fn percentile_empty() {
        assert_eq!(percentile(&[], 0.50), 0);
    }

    #[test]
    fn evaluate_thresholds_multi_metric() {
        let thresholds = vec![
            make_threshold("load-budget", "load", 200, Some(100), None, None),
            make_threshold("api-budget", "api", 50, None, None, None),
            make_threshold("missing-metric", "nope", 100, None, None, None),
        ];
        let mut samples: HashMap<String, Vec<u64>> = HashMap::new();
        samples.insert("load".to_string(), vec![50, 80, 120, 150]);
        samples.insert("api".to_string(), vec![10, 20, 60]); // max 60 > 50

        let results = evaluate_thresholds(&thresholds, &samples);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].threshold_name, "load-budget");
        assert_eq!(results[0].metric, "load");
        assert!(results[0].passed); // load all within budget
        assert_eq!(results[1].threshold_name, "api-budget");
        assert_eq!(results[1].metric, "api");
        assert!(!results[1].passed); // api max violation
        assert_eq!(results[2].threshold_name, "missing-metric");
        assert_eq!(results[2].metric, "nope");
        assert!(results[2].passed); // missing metric -> 0 samples -> pass
        assert_eq!(results[2].samples_evaluated, 0);
    }
}
