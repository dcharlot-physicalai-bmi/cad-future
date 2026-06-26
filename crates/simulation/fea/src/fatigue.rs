//! Fatigue life estimation — S-N curve analysis with Goodman diagram.
//!
//! Estimates fatigue life cycles from alternating and mean stresses
//! using material S-N data from the LUT layer. Supports Goodman,
//! Soderberg, and Gerber mean stress correction.

/// Mean stress correction method.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeanStressCorrection {
    /// Goodman: Sa/Se + Sm/Su = 1 (conservative for brittle failure).
    Goodman,
    /// Soderberg: Sa/Se + Sm/Sy = 1 (most conservative, uses yield).
    Soderberg,
    /// Gerber: Sa/Se + (Sm/Su)² = 1 (better for ductile materials).
    Gerber,
}

/// S-N curve data for a material.
#[derive(Debug, Clone)]
pub struct SnCurve {
    /// Material name.
    pub material: String,
    /// Ultimate tensile strength (MPa).
    pub ultimate_strength_mpa: f64,
    /// Yield strength (MPa).
    pub yield_strength_mpa: f64,
    /// Endurance limit (MPa) — stress below which infinite life (for steels).
    /// For aluminum, this is the fatigue strength at 5×10^8 cycles.
    pub endurance_limit_mpa: f64,
    /// S-N curve exponent b (Basquin): S = A · N^b.
    /// Typically b ≈ -0.08 to -0.12 for metals.
    pub basquin_exponent: f64,
    /// S-N curve coefficient A (MPa).
    pub basquin_coefficient: f64,
}

impl SnCurve {
    /// Create from basic material properties (estimates S-N parameters).
    /// Uses Marin approach: Se' ≈ 0.5 × Su for steels, 0.4 × Su for aluminum.
    pub fn from_properties(
        material: &str,
        ultimate_mpa: f64,
        yield_mpa: f64,
        is_steel: bool,
    ) -> Self {
        let se_prime = if is_steel {
            (0.5 * ultimate_mpa).min(700.0) // cap at 700 MPa for steels
        } else {
            0.4 * ultimate_mpa // aluminum, etc.
        };

        // Basquin parameters: S = A · N^b
        // At N=1000 cycles, S ≈ 0.9·Su. At N=1e6, S ≈ Se.
        // b = log(0.9·Su / Se) / log(1000 / 1e6)
        let s_1k = 0.9 * ultimate_mpa;
        let b = (s_1k / se_prime).ln() / (1000.0_f64 / 1e6).ln();
        let a = s_1k / 1000.0_f64.powf(b);

        Self {
            material: material.to_string(),
            ultimate_strength_mpa: ultimate_mpa,
            yield_strength_mpa: yield_mpa,
            endurance_limit_mpa: se_prime,
            basquin_exponent: b,
            basquin_coefficient: a,
        }
    }

    /// Compute fatigue life (cycles) for a given fully-reversed stress amplitude.
    pub fn life_cycles(&self, stress_amplitude_mpa: f64) -> f64 {
        if stress_amplitude_mpa <= self.endurance_limit_mpa {
            return f64::INFINITY; // infinite life
        }
        if stress_amplitude_mpa <= 0.0 {
            return f64::INFINITY;
        }
        // N = (S / A)^(1/b)
        (stress_amplitude_mpa / self.basquin_coefficient).powf(1.0 / self.basquin_exponent)
    }

    /// Compute equivalent fully-reversed stress amplitude using mean stress correction.
    pub fn equivalent_amplitude(
        &self,
        stress_amplitude_mpa: f64,
        mean_stress_mpa: f64,
        method: MeanStressCorrection,
    ) -> f64 {
        if mean_stress_mpa <= 0.0 {
            return stress_amplitude_mpa; // compressive mean = no correction needed
        }

        match method {
            MeanStressCorrection::Goodman => {
                // Sa_eq = Sa / (1 - Sm/Su)
                let denom = 1.0 - mean_stress_mpa / self.ultimate_strength_mpa;
                if denom <= 0.0 { return f64::INFINITY; }
                stress_amplitude_mpa / denom
            }
            MeanStressCorrection::Soderberg => {
                let denom = 1.0 - mean_stress_mpa / self.yield_strength_mpa;
                if denom <= 0.0 { return f64::INFINITY; }
                stress_amplitude_mpa / denom
            }
            MeanStressCorrection::Gerber => {
                let denom = 1.0 - (mean_stress_mpa / self.ultimate_strength_mpa).powi(2);
                if denom <= 0.0 { return f64::INFINITY; }
                stress_amplitude_mpa / denom
            }
        }
    }

    /// Compute fatigue life with mean stress correction.
    pub fn life_with_mean_stress(
        &self,
        stress_amplitude_mpa: f64,
        mean_stress_mpa: f64,
        method: MeanStressCorrection,
    ) -> f64 {
        let sa_eq = self.equivalent_amplitude(stress_amplitude_mpa, mean_stress_mpa, method);
        self.life_cycles(sa_eq)
    }

    /// Safety factor against fatigue failure for a target life.
    pub fn safety_factor(
        &self,
        stress_amplitude_mpa: f64,
        mean_stress_mpa: f64,
        method: MeanStressCorrection,
    ) -> f64 {
        let sa_eq = self.equivalent_amplitude(stress_amplitude_mpa, mean_stress_mpa, method);
        if sa_eq <= 0.0 { return f64::INFINITY; }
        self.endurance_limit_mpa / sa_eq
    }
}

/// Marin surface finish factor.
/// Accounts for surface roughness on fatigue life.
pub fn marin_surface_factor(ultimate_mpa: f64, finish: SurfaceFinish) -> f64 {
    let (a, b) = match finish {
        SurfaceFinish::Ground => (1.58, -0.085),
        SurfaceFinish::Machined => (4.51, -0.265),
        SurfaceFinish::HotRolled => (57.7, -0.718),
        SurfaceFinish::Forged => (272.0, -0.995),
        SurfaceFinish::AsBuiltAM => (100.0, -0.800), // additive manufacturing
    };
    a * ultimate_mpa.powf(b)
}

/// Marin size factor for rotating bending.
/// d in mm.
pub fn marin_size_factor(diameter_mm: f64) -> f64 {
    if diameter_mm <= 8.0 {
        1.0
    } else if diameter_mm <= 250.0 {
        1.189 * diameter_mm.powf(-0.097)
    } else {
        0.6
    }
}

/// Surface finish categories.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SurfaceFinish {
    Ground,
    Machined,
    HotRolled,
    Forged,
    AsBuiltAM,
}

/// Compute damage accumulation using Miner's rule.
/// Each entry is (stress_amplitude, mean_stress, n_cycles).
/// Returns damage ratio D (failure when D >= 1.0).
pub fn miners_rule_damage(
    sn: &SnCurve,
    load_blocks: &[(f64, f64, f64)],
    method: MeanStressCorrection,
) -> f64 {
    let mut damage = 0.0;
    for &(sa, sm, n) in load_blocks {
        let nf = sn.life_with_mean_stress(sa, sm, method);
        if nf.is_finite() && nf > 0.0 {
            damage += n / nf;
        }
    }
    damage
}

/// Common S-N curves from LUT data.
pub mod curves {
    use super::*;

    pub fn steel_1018() -> SnCurve {
        SnCurve::from_properties("Steel 1018", 440.0, 370.0, true)
    }

    pub fn steel_4140() -> SnCurve {
        SnCurve::from_properties("Steel 4140", 1020.0, 655.0, true)
    }

    pub fn aluminum_6061_t6() -> SnCurve {
        SnCurve::from_properties("Aluminum 6061-T6", 310.0, 276.0, false)
    }

    pub fn aluminum_7075_t6() -> SnCurve {
        SnCurve::from_properties("Aluminum 7075-T6", 572.0, 503.0, false)
    }

    pub fn titanium_6al4v() -> SnCurve {
        SnCurve::from_properties("Titanium Ti-6Al-4V", 950.0, 880.0, true)
    }

    pub fn stainless_304() -> SnCurve {
        SnCurve::from_properties("Stainless 304", 515.0, 205.0, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sn_curve_infinite_life() {
        let sn = curves::steel_4140();
        let life = sn.life_cycles(sn.endurance_limit_mpa * 0.9);
        assert!(life.is_infinite());
    }

    #[test]
    fn sn_curve_finite_life() {
        let sn = curves::steel_4140();
        let life = sn.life_cycles(sn.ultimate_strength_mpa * 0.8);
        assert!(life > 0.0);
        assert!(life < 1e7);
    }

    #[test]
    fn goodman_increases_equivalent_stress() {
        let sn = curves::steel_4140();
        let sa = 200.0;
        let sm = 100.0;
        let sa_eq = sn.equivalent_amplitude(sa, sm, MeanStressCorrection::Goodman);
        assert!(sa_eq > sa, "sa_eq={}", sa_eq);
    }

    #[test]
    fn soderberg_most_conservative() {
        let sn = curves::steel_4140();
        let sa = 200.0;
        let sm = 100.0;
        let g = sn.equivalent_amplitude(sa, sm, MeanStressCorrection::Goodman);
        let s = sn.equivalent_amplitude(sa, sm, MeanStressCorrection::Soderberg);
        let r = sn.equivalent_amplitude(sa, sm, MeanStressCorrection::Gerber);
        // Soderberg ≥ Goodman ≥ Gerber (most to least conservative)
        assert!(s >= g, "soderberg={} < goodman={}", s, g);
        assert!(g >= r, "goodman={} < gerber={}", g, r);
    }

    #[test]
    fn safety_factor() {
        let sn = curves::steel_4140();
        let sf = sn.safety_factor(100.0, 0.0, MeanStressCorrection::Goodman);
        assert!(sf > 1.0); // well below endurance limit
    }

    #[test]
    fn miners_rule() {
        let sn = curves::steel_1018();
        let blocks = vec![
            (sn.endurance_limit_mpa * 1.5, 0.0, 1000.0),
            (sn.endurance_limit_mpa * 1.2, 0.0, 5000.0),
        ];
        let damage = miners_rule_damage(&sn, &blocks, MeanStressCorrection::Goodman);
        assert!(damage > 0.0);
        assert!(damage < 10.0); // shouldn't be absurdly high for these few cycles
    }

    #[test]
    fn marin_factors() {
        let ka = marin_surface_factor(500.0, SurfaceFinish::Machined);
        assert!(ka > 0.0 && ka < 1.0);

        let kb = marin_size_factor(25.0);
        assert!(kb > 0.5 && kb < 1.0);
    }

    #[test]
    fn aluminum_no_true_endurance() {
        // Aluminum doesn't have a true endurance limit, but we estimate at 0.4×Su
        let sn = curves::aluminum_6061_t6();
        assert!((sn.endurance_limit_mpa - 0.4 * sn.ultimate_strength_mpa).abs() < 1.0);
    }

    #[test]
    fn compressive_mean_no_correction() {
        let sn = curves::steel_4140();
        let sa = 200.0;
        let sa_eq = sn.equivalent_amplitude(sa, -100.0, MeanStressCorrection::Goodman);
        assert!((sa_eq - sa).abs() < 0.01); // compressive mean → no correction
    }
}
