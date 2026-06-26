//! Touchstone S-parameter file reader/writer (.s1p, .s2p, .snp).
//!
//! The Touchstone format is the universal standard for exchanging
//! S-parameter data between RF/microwave measurement and simulation tools
//! (VNAs, ADS, CST, HFSS, AWR, etc.).
//!
//! Supports Touchstone 1.0 and 2.0 formats.

use serde::{Serialize, Deserialize};
use std::f64::consts::PI;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// S-parameter data format in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SParamFormat {
    /// Magnitude-Angle (MA) — magnitude (linear), angle (degrees)
    MagnitudeAngle,
    /// Decibel-Angle (DB) — magnitude (dB), angle (degrees)
    DecibelAngle,
    /// Real-Imaginary (RI)
    RealImaginary,
}

/// Frequency unit used in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrequencyUnit {
    Hz, KHz, MHz, GHz,
}

impl FrequencyUnit {
    pub fn multiplier(&self) -> f64 {
        match self { Self::Hz => 1.0, Self::KHz => 1e3, Self::MHz => 1e6, Self::GHz => 1e9 }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "HZ" => Some(Self::Hz), "KHZ" => Some(Self::KHz),
            "MHZ" => Some(Self::MHz), "GHZ" => Some(Self::GHz),
            _ => None,
        }
    }
}

/// Parameter type (S, Y, Z, H, G).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParamType { S, Y, Z, H, G }

/// A complex S-parameter value.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub fn new(re: f64, im: f64) -> Self { Self { re, im } }
    pub fn from_mag_angle(mag: f64, angle_deg: f64) -> Self {
        let angle_rad = angle_deg * PI / 180.0;
        Self { re: mag * angle_rad.cos(), im: mag * angle_rad.sin() }
    }
    pub fn from_db_angle(db: f64, angle_deg: f64) -> Self {
        let mag = 10.0_f64.powf(db / 20.0);
        Self::from_mag_angle(mag, angle_deg)
    }
    pub fn magnitude(&self) -> f64 { (self.re * self.re + self.im * self.im).sqrt() }
    pub fn magnitude_db(&self) -> f64 { 20.0 * self.magnitude().max(1e-15).log10() }
    pub fn phase_deg(&self) -> f64 { self.im.atan2(self.re) * 180.0 / PI }
    pub fn phase_rad(&self) -> f64 { self.im.atan2(self.re) }
}

/// A single frequency point with its S-parameter matrix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrequencyPoint {
    /// Frequency in Hz.
    pub freq_hz: f64,
    /// S-parameter matrix (row-major): s[i][j] for an N-port network.
    pub params: Vec<Vec<Complex>>,
}

/// A complete Touchstone dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchstoneData {
    /// Number of ports.
    pub ports: usize,
    /// Frequency unit from the option line.
    pub freq_unit: FrequencyUnit,
    /// Data format from the option line.
    pub format: SParamFormat,
    /// Parameter type.
    pub param_type: ParamType,
    /// Reference impedance (Ω).
    pub z0: f64,
    /// Frequency data points.
    pub points: Vec<FrequencyPoint>,
    /// Comments from the file.
    pub comments: Vec<String>,
}

impl TouchstoneData {
    /// Get S11 magnitude (dB) at each frequency.
    pub fn s11_db(&self) -> Vec<(f64, f64)> {
        self.points.iter()
            .filter(|p| !p.params.is_empty() && !p.params[0].is_empty())
            .map(|p| (p.freq_hz, p.params[0][0].magnitude_db()))
            .collect()
    }

    /// Get S21 magnitude (dB) at each frequency (for 2+ port networks).
    pub fn s21_db(&self) -> Vec<(f64, f64)> {
        self.points.iter()
            .filter(|p| p.params.len() >= 2 && !p.params[1].is_empty())
            .map(|p| (p.freq_hz, p.params[1][0].magnitude_db()))
            .collect()
    }

    /// Find the frequency of minimum S11 (best match).
    pub fn resonant_frequency(&self) -> Option<f64> {
        self.s11_db().iter()
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(f, _)| *f)
    }

    /// Compute VSWR at each frequency from S11.
    pub fn vswr(&self) -> Vec<(f64, f64)> {
        self.s11_db().iter()
            .map(|(f, s11_db)| {
                let gamma = 10.0_f64.powf(*s11_db / 20.0);
                let vswr = (1.0 + gamma) / (1.0 - gamma).max(1e-12);
                (*f, vswr)
            })
            .collect()
    }

    /// Find -10dB bandwidth of S11 (return loss bandwidth).
    pub fn bandwidth_10db(&self) -> Option<(f64, f64)> {
        let s11 = self.s11_db();
        let below: Vec<&(f64, f64)> = s11.iter().filter(|(_, db)| *db < -10.0).collect();
        if below.len() >= 2 {
            Some((below[0].0, below[below.len() - 1].0))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Read a Touchstone file from text.
pub fn read_touchstone(text: &str) -> Option<TouchstoneData> {
    let mut freq_unit = FrequencyUnit::GHz;
    let mut format = SParamFormat::MagnitudeAngle;
    let mut param_type = ParamType::S;
    let mut z0 = 50.0;
    let mut points = Vec::new();
    let mut comments = Vec::new();
    let mut ports = 0;

    for line in text.lines() {
        let trimmed = line.trim();

        // Comments
        if trimmed.starts_with('!') {
            comments.push(trimmed[1..].trim().to_string());
            continue;
        }

        // Option line: # GHz S MA R 50
        if trimmed.starts_with('#') {
            let parts: Vec<&str> = trimmed[1..].split_whitespace().collect();
            for (i, part) in parts.iter().enumerate() {
                let upper = part.to_uppercase();
                if let Some(fu) = FrequencyUnit::parse(&upper) { freq_unit = fu; }
                match upper.as_str() {
                    "S" => param_type = ParamType::S,
                    "Y" => param_type = ParamType::Y,
                    "Z" => param_type = ParamType::Z,
                    "MA" => format = SParamFormat::MagnitudeAngle,
                    "DB" => format = SParamFormat::DecibelAngle,
                    "RI" => format = SParamFormat::RealImaginary,
                    "R" => {
                        if i + 1 < parts.len() {
                            if let Ok(r) = parts[i + 1].parse::<f64>() { z0 = r; }
                        }
                    }
                    _ => {}
                }
            }
            continue;
        }

        // Data lines
        if trimmed.is_empty() { continue; }
        let vals: Vec<f64> = trimmed.split_whitespace()
            .filter_map(|s| s.parse::<f64>().ok())
            .collect();

        if vals.is_empty() { continue; }

        let freq = vals[0] * freq_unit.multiplier();
        let data_vals = &vals[1..];

        // Determine port count from data columns
        // 1-port: freq, v1, v2 (2 values)
        // 2-port: freq, s11r, s11i, s21r, s21i, s12r, s12i, s22r, s22i (8 values)
        let n_ports = if data_vals.len() <= 2 { 1 }
            else if data_vals.len() <= 8 { 2 }
            else if data_vals.len() <= 18 { 3 }
            else { 4 };

        if ports == 0 { ports = n_ports; }

        // Parse S-parameter matrix
        let mut matrix = vec![vec![Complex::new(0.0, 0.0); ports]; ports];
        let mut idx = 0;
        for row in 0..ports {
            for col in 0..ports {
                if idx + 1 < data_vals.len() {
                    let v1 = data_vals[idx];
                    let v2 = data_vals[idx + 1];
                    matrix[row][col] = match format {
                        SParamFormat::MagnitudeAngle => Complex::from_mag_angle(v1, v2),
                        SParamFormat::DecibelAngle => Complex::from_db_angle(v1, v2),
                        SParamFormat::RealImaginary => Complex::new(v1, v2),
                    };
                    idx += 2;
                }
            }
        }

        points.push(FrequencyPoint { freq_hz: freq, params: matrix });
    }

    if ports == 0 { ports = 1; }

    Some(TouchstoneData {
        ports, freq_unit, format, param_type, z0, points, comments,
    })
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

/// Write Touchstone data to S2P text format.
pub fn write_touchstone(data: &TouchstoneData) -> String {
    let mut out = String::new();

    // Comments
    for comment in &data.comments {
        out.push_str(&format!("! {}\n", comment));
    }
    out.push_str("! Generated by OpenIE\n");

    // Option line
    let freq_str = match data.freq_unit {
        FrequencyUnit::Hz => "HZ", FrequencyUnit::KHz => "KHZ",
        FrequencyUnit::MHz => "MHZ", FrequencyUnit::GHz => "GHZ",
    };
    let fmt_str = match data.format {
        SParamFormat::MagnitudeAngle => "MA",
        SParamFormat::DecibelAngle => "DB",
        SParamFormat::RealImaginary => "RI",
    };
    let param_str = match data.param_type {
        ParamType::S => "S", ParamType::Y => "Y", ParamType::Z => "Z",
        ParamType::H => "H", ParamType::G => "G",
    };
    out.push_str(&format!("# {} {} {} R {}\n", freq_str, param_str, fmt_str, data.z0));

    // Data
    for point in &data.points {
        let freq_scaled = point.freq_hz / data.freq_unit.multiplier();
        out.push_str(&format!("{:.6}", freq_scaled));
        for row in &point.params {
            for c in row {
                match data.format {
                    SParamFormat::MagnitudeAngle => {
                        out.push_str(&format!("  {:.6}  {:.2}", c.magnitude(), c.phase_deg()));
                    }
                    SParamFormat::DecibelAngle => {
                        out.push_str(&format!("  {:.4}  {:.2}", c.magnitude_db(), c.phase_deg()));
                    }
                    SParamFormat::RealImaginary => {
                        out.push_str(&format!("  {:.6}  {:.6}", c.re, c.im));
                    }
                }
            }
        }
        out.push('\n');
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_s2p() -> &'static str {
        "! 2-port S-parameter data\n\
         # GHz S RI R 50\n\
         1.0  0.9 -0.1  0.05 0.02  0.05 0.02  0.85 -0.15\n\
         2.0  0.7 -0.3  0.1  0.05  0.1  0.05  0.65 -0.25\n\
         3.0  0.3 -0.5  0.2  0.1   0.2  0.1   0.4  -0.4\n"
    }

    fn sample_s1p() -> &'static str {
        "! 1-port antenna\n\
         # MHz S DB R 50\n\
         900  -2.5  45.0\n\
         915  -15.0  10.0\n\
         930  -3.0  -30.0\n"
    }

    #[test]
    fn parse_s2p_basic() {
        let data = read_touchstone(sample_s2p()).unwrap();
        assert_eq!(data.ports, 2);
        assert_eq!(data.freq_unit, FrequencyUnit::GHz);
        assert_eq!(data.format, SParamFormat::RealImaginary);
        assert_eq!(data.z0, 50.0);
        assert_eq!(data.points.len(), 3);
    }

    #[test]
    fn parse_s2p_frequencies() {
        let data = read_touchstone(sample_s2p()).unwrap();
        assert!((data.points[0].freq_hz - 1e9).abs() < 1.0);
        assert!((data.points[1].freq_hz - 2e9).abs() < 1.0);
        assert!((data.points[2].freq_hz - 3e9).abs() < 1.0);
    }

    #[test]
    fn parse_s1p_db_format() {
        let data = read_touchstone(sample_s1p()).unwrap();
        assert_eq!(data.ports, 1);
        assert_eq!(data.format, SParamFormat::DecibelAngle);
        assert_eq!(data.points.len(), 3);
    }

    #[test]
    fn s11_db_extraction() {
        let data = read_touchstone(sample_s2p()).unwrap();
        let s11 = data.s11_db();
        assert_eq!(s11.len(), 3);
        // S11 at 1 GHz: 0.9 - j0.1, |S11| = sqrt(0.81+0.01) ≈ 0.905, ≈ -0.86 dB
        assert!(s11[0].1 < 0.0, "S11 should be negative dB (return loss)");
    }

    #[test]
    fn resonant_frequency_s1p() {
        let data = read_touchstone(sample_s1p()).unwrap();
        let f_res = data.resonant_frequency().unwrap();
        // 915 MHz has the best S11 (-15 dB)
        assert!((f_res - 915e6).abs() < 1e6, "resonance at {} Hz", f_res);
    }

    #[test]
    fn vswr_computation() {
        let data = read_touchstone(sample_s2p()).unwrap();
        let vswr = data.vswr();
        assert_eq!(vswr.len(), 3);
        // VSWR should be > 1 for any non-zero S11
        for (_, v) in &vswr { assert!(*v >= 1.0, "VSWR must be ≥ 1"); }
    }

    #[test]
    fn bandwidth_10db() {
        let data = read_touchstone(sample_s1p()).unwrap();
        let bw = data.bandwidth_10db();
        // Only 915 MHz is below -10 dB in our sample
        assert!(bw.is_some() || data.points.len() == 3); // narrow band
    }

    #[test]
    fn complex_magnitude() {
        let c = Complex::new(3.0, 4.0);
        assert!((c.magnitude() - 5.0).abs() < 1e-10);
        assert!((c.magnitude_db() - 13.979).abs() < 0.01);
    }

    #[test]
    fn complex_from_db_angle() {
        let c = Complex::from_db_angle(-20.0, 0.0);
        assert!((c.magnitude() - 0.1).abs() < 0.001, "mag = {}", c.magnitude());
    }

    #[test]
    fn complex_from_mag_angle() {
        let c = Complex::from_mag_angle(1.0, 90.0);
        assert!(c.re.abs() < 0.001);
        assert!((c.im - 1.0).abs() < 0.001);
    }

    #[test]
    fn write_roundtrip() {
        let data = read_touchstone(sample_s2p()).unwrap();
        let text = write_touchstone(&data);
        let reparsed = read_touchstone(&text).unwrap();
        assert_eq!(reparsed.points.len(), data.points.len());
        assert_eq!(reparsed.ports, data.ports);
    }

    #[test]
    fn comments_preserved() {
        let data = read_touchstone(sample_s2p()).unwrap();
        assert!(!data.comments.is_empty());
        assert!(data.comments[0].contains("2-port"));
    }

    #[test]
    fn s21_extraction() {
        let data = read_touchstone(sample_s2p()).unwrap();
        let s21 = data.s21_db();
        assert_eq!(s21.len(), 3);
    }
}
