//! `physical-tolerance` — Tolerance stack-up analysis.
//!
//! Computes how manufacturing tolerances propagate through an assembly.
//! Supports worst-case (arithmetic), RSS (statistical), and Monte Carlo methods.

use serde::{Serialize, Deserialize};

// ---------------------------------------------------------------------------
// Tolerance Types
// ---------------------------------------------------------------------------

/// A single dimension in a tolerance chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TolDimension {
    pub name: String,
    pub nominal: f64,
    pub plus_tol: f64,  // positive tolerance (e.g., +0.1)
    pub minus_tol: f64, // negative tolerance (e.g., -0.05), stored as negative
    pub direction: TolDirection,
    pub distribution: Distribution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TolDirection {
    /// Adds to the gap (positive contributor).
    Positive,
    /// Subtracts from the gap (negative contributor).
    Negative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Distribution {
    /// Uniform distribution (worst case).
    Uniform,
    /// Normal distribution (typical for machined parts).
    Normal,
    /// Skewed (typical for formed parts).
    Skewed,
}

impl TolDimension {
    pub fn symmetric(name: &str, nominal: f64, tol: f64, direction: TolDirection) -> Self {
        Self {
            name: name.into(), nominal, plus_tol: tol, minus_tol: -tol,
            direction, distribution: Distribution::Normal,
        }
    }

    pub fn bilateral(name: &str, nominal: f64, plus: f64, minus: f64, direction: TolDirection) -> Self {
        Self {
            name: name.into(), nominal, plus_tol: plus, minus_tol: minus,
            direction, distribution: Distribution::Normal,
        }
    }

    /// Total tolerance range.
    pub fn range(&self) -> f64 { self.plus_tol - self.minus_tol }

    /// Midpoint of the tolerance band.
    pub fn midpoint(&self) -> f64 { self.nominal + (self.plus_tol + self.minus_tol) / 2.0 }
}

// ---------------------------------------------------------------------------
// Stack-up Result
// ---------------------------------------------------------------------------

/// Result of a tolerance stack-up analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackUpResult {
    /// Nominal gap/dimension.
    pub nominal_gap: f64,
    /// Worst-case maximum gap.
    pub worst_case_max: f64,
    /// Worst-case minimum gap.
    pub worst_case_min: f64,
    /// RSS (root sum of squares) maximum gap at given sigma.
    pub rss_max: f64,
    /// RSS minimum gap.
    pub rss_min: f64,
    /// Sigma level used for RSS (typically 3.0).
    pub sigma: f64,
    /// Number of dimensions in the chain.
    pub dimension_count: usize,
    /// Per-dimension contribution to variance (sensitivity).
    pub contributions: Vec<DimensionContribution>,
    /// Probability of interference (gap < 0) using RSS model.
    pub interference_probability: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionContribution {
    pub name: String,
    pub nominal: f64,
    pub tolerance_range: f64,
    /// Percentage of total variance attributed to this dimension.
    pub variance_contribution_pct: f64,
}

// ---------------------------------------------------------------------------
// Worst-Case Analysis
// ---------------------------------------------------------------------------

/// Perform worst-case (arithmetic) tolerance stack-up.
pub fn worst_case(dimensions: &[TolDimension]) -> StackUpResult {
    let mut nominal = 0.0;
    let mut max_gap = 0.0;
    let mut min_gap = 0.0;

    for dim in dimensions {
        let sign = match dim.direction {
            TolDirection::Positive => 1.0,
            TolDirection::Negative => -1.0,
        };
        nominal += sign * dim.nominal;
        max_gap += sign * (dim.nominal + if sign > 0.0 { dim.plus_tol } else { dim.minus_tol });
        min_gap += sign * (dim.nominal + if sign > 0.0 { dim.minus_tol } else { dim.plus_tol });
    }

    let contributions = compute_contributions(dimensions);

    StackUpResult {
        nominal_gap: nominal,
        worst_case_max: max_gap,
        worst_case_min: min_gap,
        rss_max: max_gap, // worst case = RSS for this method
        rss_min: min_gap,
        sigma: 0.0,
        dimension_count: dimensions.len(),
        contributions,
        interference_probability: if min_gap < 0.0 { 1.0 } else { 0.0 },
    }
}

// ---------------------------------------------------------------------------
// RSS (Statistical) Analysis
// ---------------------------------------------------------------------------

/// Perform RSS tolerance stack-up at the given sigma level.
pub fn rss(dimensions: &[TolDimension], sigma: f64) -> StackUpResult {
    let mut nominal = 0.0;
    let mut sum_var = 0.0;

    for dim in dimensions {
        let sign = match dim.direction {
            TolDirection::Positive => 1.0,
            TolDirection::Negative => -1.0,
        };
        nominal += sign * dim.nominal;

        // Assume tolerance = 3σ for each dimension (99.73%)
        let half_tol = dim.range() / 2.0;
        let std_dev = half_tol / 3.0; // each dimension at 3σ
        sum_var += std_dev * std_dev;
    }

    let rss_std = sum_var.sqrt();
    let rss_range = sigma * rss_std;

    let contributions = compute_contributions(dimensions);

    // Interference probability: P(gap < 0) = Φ(-nominal/rss_std)
    let z = if rss_std > 1e-12 { nominal / rss_std } else { f64::INFINITY };
    let interference_prob = normal_cdf(-z);

    StackUpResult {
        nominal_gap: nominal,
        worst_case_max: nominal + dimensions.iter().map(|d| d.plus_tol.abs()).sum::<f64>(),
        worst_case_min: nominal - dimensions.iter().map(|d| d.minus_tol.abs()).sum::<f64>(),
        rss_max: nominal + rss_range,
        rss_min: nominal - rss_range,
        sigma,
        dimension_count: dimensions.len(),
        contributions,
        interference_probability: interference_prob,
    }
}

// ---------------------------------------------------------------------------
// Monte Carlo Analysis
// ---------------------------------------------------------------------------

/// Perform Monte Carlo tolerance stack-up with N random samples.
pub fn monte_carlo(dimensions: &[TolDimension], samples: usize, seed: u64) -> StackUpResult {
    let mut rng = seed;
    let mut results = Vec::with_capacity(samples);

    for _ in 0..samples {
        let mut gap = 0.0;
        for dim in dimensions {
            let sign = match dim.direction {
                TolDirection::Positive => 1.0,
                TolDirection::Negative => -1.0,
            };
            // Random value within tolerance range (uniform or normal)
            let variation = match dim.distribution {
                Distribution::Uniform => {
                    let u = lcg_f64(&mut rng);
                    dim.minus_tol + u * dim.range()
                }
                Distribution::Normal | Distribution::Skewed => {
                    // Box-Muller for normal distribution
                    let u1 = lcg_f64(&mut rng).max(1e-15);
                    let u2 = lcg_f64(&mut rng);
                    let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
                    let std_dev = dim.range() / 6.0; // 3σ per side
                    let v = z * std_dev;
                    v.clamp(dim.minus_tol, dim.plus_tol)
                }
            };
            gap += sign * (dim.nominal + variation);
        }
        results.push(gap);
    }

    results.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let nominal: f64 = dimensions.iter().map(|d| {
        let sign = match d.direction { TolDirection::Positive => 1.0, TolDirection::Negative => -1.0 };
        sign * d.nominal
    }).sum();

    let mean: f64 = results.iter().sum::<f64>() / samples as f64;
    let variance: f64 = results.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / samples as f64;
    let std_dev = variance.sqrt();

    let interference_count = results.iter().filter(|&&g| g < 0.0).count();

    let contributions = compute_contributions(dimensions);

    StackUpResult {
        nominal_gap: nominal,
        worst_case_max: *results.last().unwrap_or(&0.0),
        worst_case_min: *results.first().unwrap_or(&0.0),
        rss_max: mean + 3.0 * std_dev,
        rss_min: mean - 3.0 * std_dev,
        sigma: 3.0,
        dimension_count: dimensions.len(),
        contributions,
        interference_probability: interference_count as f64 / samples as f64,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn compute_contributions(dimensions: &[TolDimension]) -> Vec<DimensionContribution> {
    let total_var: f64 = dimensions.iter().map(|d| {
        let half = d.range() / 2.0;
        (half / 3.0).powi(2)
    }).sum();

    dimensions.iter().map(|d| {
        let half = d.range() / 2.0;
        let var = (half / 3.0).powi(2);
        let pct = if total_var > 1e-15 { var / total_var * 100.0 } else { 0.0 };
        DimensionContribution {
            name: d.name.clone(),
            nominal: d.nominal,
            tolerance_range: d.range(),
            variance_contribution_pct: pct,
        }
    }).collect()
}

/// Standard normal CDF approximation (Abramowitz & Stegun).
fn normal_cdf(x: f64) -> f64 {
    if x < -8.0 { return 0.0; }
    if x > 8.0 { return 1.0; }
    let t = 1.0 / (1.0 + 0.2316419 * x.abs());
    let d = 0.3989422804014327; // 1/sqrt(2π)
    let p = d * (-x * x / 2.0).exp();
    let c = t * (0.319381530 + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))));
    if x >= 0.0 { 1.0 - p * c } else { p * c }
}

fn lcg_f64(state: &mut u64) -> f64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*state >> 33) as f64 / (1u64 << 31) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_stack() -> Vec<TolDimension> {
        vec![
            TolDimension::symmetric("Housing", 50.0, 0.1, TolDirection::Positive),
            TolDimension::symmetric("Shaft", 49.9, 0.05, TolDirection::Negative),
        ]
    }

    fn three_part_stack() -> Vec<TolDimension> {
        vec![
            TolDimension::symmetric("Outer", 100.0, 0.2, TolDirection::Positive),
            TolDimension::symmetric("Spacer", 30.0, 0.1, TolDirection::Negative),
            TolDimension::symmetric("Inner", 30.0, 0.1, TolDirection::Negative),
            TolDimension::symmetric("Ring", 30.0, 0.15, TolDirection::Negative),
        ]
    }

    #[test]
    fn worst_case_simple_gap() {
        let result = worst_case(&simple_stack());
        assert!((result.nominal_gap - 0.1).abs() < 0.01, "nominal gap = {}", result.nominal_gap);
        assert!(result.worst_case_max > result.nominal_gap);
        assert!(result.worst_case_min < result.nominal_gap);
    }

    #[test]
    fn worst_case_three_parts() {
        let result = worst_case(&three_part_stack());
        assert!((result.nominal_gap - 10.0).abs() < 0.01);
    }

    #[test]
    fn rss_tighter_than_worst_case() {
        let dims = three_part_stack();
        let wc = worst_case(&dims);
        let rss_result = rss(&dims, 3.0);
        let wc_range = wc.worst_case_max - wc.worst_case_min;
        let rss_range = rss_result.rss_max - rss_result.rss_min;
        assert!(rss_range < wc_range, "RSS range ({rss_range:.4}) should be tighter than WC ({wc_range:.4})");
    }

    #[test]
    fn rss_nominal_matches_worst_case() {
        let dims = simple_stack();
        let wc = worst_case(&dims);
        let rss_result = rss(&dims, 3.0);
        assert!((wc.nominal_gap - rss_result.nominal_gap).abs() < 0.001);
    }

    #[test]
    fn monte_carlo_within_bounds() {
        let dims = simple_stack();
        let mc = monte_carlo(&dims, 10000, 42);
        let wc = worst_case(&dims);
        assert!(mc.worst_case_max <= wc.worst_case_max + 0.01);
        assert!(mc.worst_case_min >= wc.worst_case_min - 0.01);
    }

    #[test]
    fn monte_carlo_nominal_close() {
        let dims = three_part_stack();
        let mc = monte_carlo(&dims, 50000, 123);
        assert!((mc.nominal_gap - 10.0).abs() < 0.5, "MC nominal = {}", mc.nominal_gap);
    }

    #[test]
    fn contributions_sum_to_100() {
        let result = rss(&three_part_stack(), 3.0);
        let total: f64 = result.contributions.iter().map(|c| c.variance_contribution_pct).sum();
        assert!((total - 100.0).abs() < 0.1, "contributions sum to {total}%");
    }

    #[test]
    fn no_interference_with_large_gap() {
        let dims = vec![
            TolDimension::symmetric("A", 100.0, 0.1, TolDirection::Positive),
            TolDimension::symmetric("B", 50.0, 0.1, TolDirection::Negative),
        ];
        let result = rss(&dims, 3.0);
        assert!(result.interference_probability < 0.001, "large gap should have ~0 interference");
    }

    #[test]
    fn interference_with_tight_gap() {
        let dims = vec![
            TolDimension::symmetric("Hole", 10.0, 0.1, TolDirection::Positive),
            TolDimension::symmetric("Pin", 9.99, 0.1, TolDirection::Negative),
        ];
        let result = rss(&dims, 3.0);
        // Very tight gap (0.01mm nominal with ±0.1mm tolerance) — high interference
        assert!(result.interference_probability > 0.1, "tight gap should have interference risk");
    }

    #[test]
    fn bilateral_tolerance() {
        let dim = TolDimension::bilateral("Bore", 25.0, 0.021, -0.0, TolDirection::Positive);
        assert!((dim.range() - 0.021).abs() < 0.001);
    }

    #[test]
    fn normal_cdf_values() {
        assert!((normal_cdf(0.0) - 0.5).abs() < 0.001);
        assert!(normal_cdf(3.0) > 0.998);
        assert!(normal_cdf(-3.0) < 0.002);
    }
}
