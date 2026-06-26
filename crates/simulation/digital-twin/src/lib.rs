//! `physical-digital-twin` — Manufacturing feedback loop.
//!
//! Connects manufactured part data back to design by tracking manufactured
//! instances, sensor readings, process capability (Cpk), trend analysis,
//! maintenance prediction, and design-feedback suggestions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Quality status of a manufactured instance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityStatus {
    Pass,
    Fail,
    Rework,
}

/// A single dimensional measurement taken on a manufactured part.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Measurement {
    pub feature_name: String,
    pub nominal: f64,
    pub actual: f64,
    pub tolerance: f64,
    pub in_spec: bool,
}

impl Measurement {
    pub fn new(feature_name: &str, nominal: f64, actual: f64, tolerance: f64) -> Self {
        let deviation = (actual - nominal).abs();
        Self {
            feature_name: feature_name.to_string(),
            nominal,
            actual,
            tolerance,
            in_spec: deviation <= tolerance,
        }
    }
}

/// Defect type classification.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DefectType {
    Dimensional,
    Surface,
    Material,
    Assembly,
}

/// A defect found during inspection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Defect {
    pub defect_type: DefectType,
    pub description: String,
    /// Severity 1 (cosmetic) to 5 (critical/safety).
    pub severity: u8,
}

/// A single manufactured part instance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManufacturedInstance {
    pub serial_number: String,
    pub timestamp: f64,
    pub machine_id: String,
    pub process_params: HashMap<String, f64>,
    pub measured_dimensions: Vec<Measurement>,
    pub defects: Vec<Defect>,
    pub status: QualityStatus,
}

/// A sensor reading from the field (vibration, temperature, etc.).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SensorReading {
    pub sensor_id: String,
    pub timestamp: f64,
    pub value: f64,
    pub unit: String,
}

/// Process capability metrics.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cpk {
    pub cp: f64,
    pub cpk: f64,
    pub mean: f64,
    pub std_dev: f64,
    pub usl: f64,
    pub lsl: f64,
}

/// Trend direction for a measured feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TrendDirection {
    Stable,
    DriftingHigh,
    DriftingLow,
    Erratic,
}

/// Trend analysis result for a feature dimension.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trend {
    pub feature_name: String,
    pub direction: TrendDirection,
    /// Slope of linear regression (units per instance).
    pub slope: f64,
    /// R-squared of the linear fit.
    pub r_squared: f64,
    /// Predicted number of instances until out-of-spec (None if stable).
    pub instances_until_oos: Option<usize>,
}

/// Severity of a maintenance alert.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// A predictive maintenance alert.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaintenanceAlert {
    pub sensor_id: String,
    pub message: String,
    pub severity: AlertSeverity,
    pub estimated_remaining_life_hours: Option<f64>,
}

/// A suggestion to feed back to the design.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DesignSuggestion {
    pub feature_name: String,
    pub suggestion: String,
    pub priority: u8,
}

// ---------------------------------------------------------------------------
// DigitalTwin
// ---------------------------------------------------------------------------

/// The manufacturing digital twin — links design data to production reality.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DigitalTwin {
    pub design_id: String,
    pub part_number: String,
    pub manufactured_instances: Vec<ManufacturedInstance>,
    pub sensor_readings: Vec<SensorReading>,
}

impl DigitalTwin {
    pub fn new(design_id: &str, part_number: &str) -> Self {
        Self {
            design_id: design_id.to_string(),
            part_number: part_number.to_string(),
            manufactured_instances: Vec::new(),
            sensor_readings: Vec::new(),
        }
    }

    /// Add a manufactured instance to the twin.
    pub fn add_instance(&mut self, instance: ManufacturedInstance) {
        self.manufactured_instances.push(instance);
    }

    /// Add a sensor reading for field monitoring.
    pub fn add_sensor_reading(&mut self, reading: SensorReading) {
        self.sensor_readings.push(reading);
    }

    /// Compute process capability (Cpk) for a named feature.
    ///
    /// Collects all measured actuals for `feature_name`, computes mean and
    /// standard deviation, then derives Cp and Cpk using the nominal +/-
    /// tolerance as USL/LSL.
    pub fn process_capability(&self, feature_name: &str) -> Option<Cpk> {
        // Gather all measurements for this feature.
        let measurements: Vec<&Measurement> = self
            .manufactured_instances
            .iter()
            .flat_map(|inst| inst.measured_dimensions.iter())
            .filter(|m| m.feature_name == feature_name)
            .collect();

        if measurements.len() < 2 {
            return None;
        }

        let n = measurements.len() as f64;
        let nominal = measurements[0].nominal;
        let tol = measurements[0].tolerance;
        let usl = nominal + tol;
        let lsl = nominal - tol;

        let values: Vec<f64> = measurements.iter().map(|m| m.actual).collect();
        let mean = values.iter().sum::<f64>() / n;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
        let std_dev = variance.sqrt();

        if std_dev < 1e-15 {
            return None;
        }

        let cp = (usl - lsl) / (6.0 * std_dev);
        let cpu = (usl - mean) / (3.0 * std_dev);
        let cpl = (mean - lsl) / (3.0 * std_dev);
        let cpk = cpu.min(cpl);

        Some(Cpk {
            cp,
            cpk,
            mean,
            std_dev,
            usl,
            lsl,
        })
    }

    /// Trend analysis: is a feature dimension drifting over time?
    ///
    /// Performs simple linear regression on the sequential actual values
    /// and checks if the slope indicates drift toward a spec limit.
    pub fn trend_analysis(&self, feature_name: &str) -> Option<Trend> {
        let measurements: Vec<&Measurement> = self
            .manufactured_instances
            .iter()
            .flat_map(|inst| inst.measured_dimensions.iter())
            .filter(|m| m.feature_name == feature_name)
            .collect();

        if measurements.len() < 3 {
            return None;
        }

        let n = measurements.len() as f64;
        let nominal = measurements[0].nominal;
        let tol = measurements[0].tolerance;
        let usl = nominal + tol;
        let lsl = nominal - tol;

        // x = 0, 1, 2, ... (sequential index)
        let ys: Vec<f64> = measurements.iter().map(|m| m.actual).collect();
        let x_mean = (n - 1.0) / 2.0;
        let y_mean = ys.iter().sum::<f64>() / n;

        let mut ss_xy = 0.0;
        let mut ss_xx = 0.0;
        let mut ss_yy = 0.0;
        for (i, &y) in ys.iter().enumerate() {
            let dx = i as f64 - x_mean;
            let dy = y - y_mean;
            ss_xy += dx * dy;
            ss_xx += dx * dx;
            ss_yy += dy * dy;
        }

        let slope = if ss_xx.abs() > 1e-15 {
            ss_xy / ss_xx
        } else {
            0.0
        };

        let r_squared = if ss_xx.abs() > 1e-15 && ss_yy.abs() > 1e-15 {
            (ss_xy * ss_xy) / (ss_xx * ss_yy)
        } else {
            0.0
        };

        // Determine direction.
        let threshold = tol * 0.01; // 1% of tolerance per sample is significant.
        let direction = if r_squared < 0.3 && slope.abs() > threshold {
            TrendDirection::Erratic
        } else if slope > threshold {
            TrendDirection::DriftingHigh
        } else if slope < -threshold {
            TrendDirection::DriftingLow
        } else {
            TrendDirection::Stable
        };

        // Predict instances until out-of-spec.
        let last_y = ys[ys.len() - 1];
        let instances_until_oos = if slope.abs() > 1e-15 {
            let to_usl = if slope > 0.0 {
                ((usl - last_y) / slope).ceil() as usize
            } else {
                usize::MAX
            };
            let to_lsl = if slope < 0.0 {
                ((lsl - last_y) / slope).ceil() as usize
            } else {
                usize::MAX
            };
            let remaining = to_usl.min(to_lsl);
            if remaining < 100_000 {
                Some(remaining)
            } else {
                None
            }
        } else {
            None
        };

        Some(Trend {
            feature_name: feature_name.to_string(),
            direction,
            slope,
            r_squared,
            instances_until_oos,
        })
    }
}

// ---------------------------------------------------------------------------
// Standalone analysis functions
// ---------------------------------------------------------------------------

/// Predict maintenance needs based on sensor data trends.
///
/// Groups sensor readings by sensor_id, fits a linear trend, and flags
/// sensors that are approaching known thresholds.
pub fn predict_maintenance(twin: &DigitalTwin) -> Vec<MaintenanceAlert> {
    let mut alerts = Vec::new();

    // Group readings by sensor_id.
    let mut by_sensor: HashMap<String, Vec<&SensorReading>> = HashMap::new();
    for r in &twin.sensor_readings {
        by_sensor.entry(r.sensor_id.clone()).or_default().push(r);
    }

    for (sensor_id, readings) in &by_sensor {
        if readings.len() < 3 {
            continue;
        }

        // Sort by timestamp.
        let mut sorted: Vec<&&SensorReading> = readings.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap());

        let n = sorted.len() as f64;
        let ts: Vec<f64> = sorted.iter().map(|r| r.timestamp).collect();
        let vs: Vec<f64> = sorted.iter().map(|r| r.value).collect();

        let t_mean = ts.iter().sum::<f64>() / n;
        let v_mean = vs.iter().sum::<f64>() / n;

        let mut ss_tv = 0.0;
        let mut ss_tt = 0.0;
        for i in 0..sorted.len() {
            let dt = ts[i] - t_mean;
            let dv = vs[i] - v_mean;
            ss_tv += dt * dv;
            ss_tt += dt * dt;
        }

        let slope = if ss_tt.abs() > 1e-15 {
            ss_tv / ss_tt
        } else {
            0.0
        };

        // Heuristic: if values are increasing significantly, warn.
        let last_val = vs[vs.len() - 1];
        let first_val = vs[0];
        let range = (last_val - first_val).abs();
        let baseline = first_val.abs().max(1.0);

        if range / baseline > 0.2 {
            // More than 20% change from baseline.
            let hours_left = if slope.abs() > 1e-15 {
                let threshold = first_val + first_val.abs() * 0.5 * slope.signum();
                let remaining = (threshold - last_val) / slope;
                if remaining > 0.0 {
                    Some(remaining / 3600.0)
                } else {
                    Some(0.0)
                }
            } else {
                None
            };

            let severity = if range / baseline > 0.4 {
                AlertSeverity::Critical
            } else {
                AlertSeverity::Warning
            };

            alerts.push(MaintenanceAlert {
                sensor_id: sensor_id.clone(),
                message: format!(
                    "Sensor {} trending {} (slope={:.4}/s, range={:.2}% of baseline)",
                    sensor_id,
                    if slope > 0.0 { "upward" } else { "downward" },
                    slope,
                    range / baseline * 100.0,
                ),
                severity,
                estimated_remaining_life_hours: hours_left,
            });
        }
    }

    alerts
}

/// Generate design feedback suggestions based on manufacturing data.
///
/// Analyzes process capability and defect patterns to suggest tolerance,
/// material, or process changes.
pub fn feedback_to_design(twin: &DigitalTwin) -> Vec<DesignSuggestion> {
    let mut suggestions = Vec::new();

    // Collect all unique feature names.
    let mut features: Vec<String> = twin
        .manufactured_instances
        .iter()
        .flat_map(|inst| inst.measured_dimensions.iter())
        .map(|m| m.feature_name.clone())
        .collect();
    features.sort();
    features.dedup();

    for feature in &features {
        if let Some(cpk) = twin.process_capability(feature) {
            if cpk.cpk < 1.0 {
                suggestions.push(DesignSuggestion {
                    feature_name: feature.clone(),
                    suggestion: format!(
                        "Cpk={:.2} < 1.0 — widen tolerance on '{}' (current ±{:.4}) or improve process",
                        cpk.cpk,
                        feature,
                        (cpk.usl - cpk.lsl) / 2.0,
                    ),
                    priority: 4,
                });
            } else if cpk.cpk < 1.33 {
                suggestions.push(DesignSuggestion {
                    feature_name: feature.clone(),
                    suggestion: format!(
                        "Cpk={:.2} marginal on '{}' — consider widening tolerance or tightening process controls",
                        cpk.cpk, feature,
                    ),
                    priority: 2,
                });
            } else if cpk.cpk > 3.0 {
                suggestions.push(DesignSuggestion {
                    feature_name: feature.clone(),
                    suggestion: format!(
                        "Cpk={:.2} on '{}' is very high — tolerance could be tightened for better fit/function",
                        cpk.cpk, feature,
                    ),
                    priority: 1,
                });
            }
        }

        if let Some(trend) = twin.trend_analysis(feature) {
            if trend.direction == TrendDirection::DriftingHigh
                || trend.direction == TrendDirection::DriftingLow
            {
                suggestions.push(DesignSuggestion {
                    feature_name: feature.clone(),
                    suggestion: format!(
                        "'{}' is {:?} (slope={:.6}/part, R²={:.2}) — investigate tool wear or material lot variation",
                        feature, trend.direction, trend.slope, trend.r_squared,
                    ),
                    priority: 3,
                });
            }
        }
    }

    // Aggregate defect patterns.
    let mut defect_counts: HashMap<String, usize> = HashMap::new();
    let total_instances = twin.manufactured_instances.len();
    for inst in &twin.manufactured_instances {
        for defect in &inst.defects {
            *defect_counts.entry(defect.description.clone()).or_insert(0) += 1;
        }
    }
    for (desc, count) in &defect_counts {
        let rate = *count as f64 / total_instances.max(1) as f64;
        if rate > 0.1 {
            suggestions.push(DesignSuggestion {
                feature_name: desc.clone(),
                suggestion: format!(
                    "Defect '{}' occurs in {:.0}% of parts — review design for manufacturability",
                    desc,
                    rate * 100.0,
                ),
                priority: 4,
            });
        }
    }

    suggestions.sort_by(|a, b| b.priority.cmp(&a.priority));
    suggestions
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_instance(serial: &str, feature: &str, actual: f64) -> ManufacturedInstance {
        ManufacturedInstance {
            serial_number: serial.to_string(),
            timestamp: 1000.0,
            machine_id: "CNC-01".to_string(),
            process_params: HashMap::from([("spindle_rpm".to_string(), 3000.0)]),
            measured_dimensions: vec![Measurement::new(feature, 10.0, actual, 0.05)],
            defects: Vec::new(),
            status: QualityStatus::Pass,
        }
    }

    fn make_twin_with_data() -> DigitalTwin {
        let mut twin = DigitalTwin::new("DES-001", "PN-1234");
        // Values centered around 10.0 with small spread.
        let actuals = [10.01, 9.99, 10.02, 9.98, 10.00, 10.01, 9.99, 10.02, 9.98, 10.00];
        for (i, &v) in actuals.iter().enumerate() {
            twin.add_instance(make_instance(&format!("SN-{:03}", i), "bore_dia", v));
        }
        twin
    }

    #[test]
    fn test_measurement_in_spec() {
        let m = Measurement::new("bore", 10.0, 10.03, 0.05);
        assert!(m.in_spec);
        let m2 = Measurement::new("bore", 10.0, 10.08, 0.05);
        assert!(!m2.in_spec);
    }

    #[test]
    fn test_add_instance() {
        let mut twin = DigitalTwin::new("D1", "P1");
        assert_eq!(twin.manufactured_instances.len(), 0);
        twin.add_instance(make_instance("SN-001", "bore", 10.01));
        assert_eq!(twin.manufactured_instances.len(), 1);
    }

    #[test]
    fn test_add_sensor_reading() {
        let mut twin = DigitalTwin::new("D1", "P1");
        twin.add_sensor_reading(SensorReading {
            sensor_id: "TEMP-01".to_string(),
            timestamp: 100.0,
            value: 22.5,
            unit: "C".to_string(),
        });
        assert_eq!(twin.sensor_readings.len(), 1);
    }

    #[test]
    fn test_cpk_centered_process() {
        let twin = make_twin_with_data();
        let cpk = twin.process_capability("bore_dia").unwrap();
        // Tolerance = ±0.05, so USL-LSL = 0.1. Spread is small.
        assert!(cpk.cp > 1.0, "Cp should be > 1.0, got {}", cpk.cp);
        assert!(cpk.cpk > 0.5, "Cpk should be > 0.5, got {}", cpk.cpk);
        assert!((cpk.mean - 10.0).abs() < 0.03);
    }

    #[test]
    fn test_cpk_none_for_missing_feature() {
        let twin = make_twin_with_data();
        assert!(twin.process_capability("nonexistent").is_none());
    }

    #[test]
    fn test_cpk_none_for_single_measurement() {
        let mut twin = DigitalTwin::new("D1", "P1");
        twin.add_instance(make_instance("SN-001", "bore", 10.01));
        assert!(twin.process_capability("bore").is_none());
    }

    #[test]
    fn test_trend_stable() {
        let twin = make_twin_with_data();
        let trend = twin.trend_analysis("bore_dia").unwrap();
        // Small random spread, should be roughly stable.
        assert!(
            trend.slope.abs() < 0.01,
            "Slope should be small, got {}",
            trend.slope
        );
    }

    #[test]
    fn test_trend_drifting() {
        let mut twin = DigitalTwin::new("D1", "P1");
        // Monotonically increasing measurements.
        for i in 0..20 {
            let actual = 10.0 + 0.003 * i as f64;
            twin.add_instance(make_instance(&format!("SN-{:03}", i), "width", actual));
        }
        let trend = twin.trend_analysis("width").unwrap();
        assert_eq!(trend.direction, TrendDirection::DriftingHigh);
        assert!(trend.slope > 0.002);
        assert!(trend.r_squared > 0.9);
    }

    #[test]
    fn test_predict_maintenance_warning() {
        let mut twin = DigitalTwin::new("D1", "P1");
        // Simulate vibration sensor with increasing values.
        for i in 0..20 {
            twin.add_sensor_reading(SensorReading {
                sensor_id: "VIB-01".to_string(),
                timestamp: i as f64 * 3600.0,
                value: 1.0 + 0.05 * i as f64,
                unit: "mm/s".to_string(),
            });
        }
        let alerts = predict_maintenance(&twin);
        assert!(!alerts.is_empty(), "Should generate at least one alert");
        assert_eq!(alerts[0].sensor_id, "VIB-01");
    }

    #[test]
    fn test_predict_maintenance_no_alert_for_stable() {
        let mut twin = DigitalTwin::new("D1", "P1");
        for i in 0..20 {
            twin.add_sensor_reading(SensorReading {
                sensor_id: "TEMP-01".to_string(),
                timestamp: i as f64 * 3600.0,
                value: 22.0,
                unit: "C".to_string(),
            });
        }
        let alerts = predict_maintenance(&twin);
        assert!(alerts.is_empty());
    }

    #[test]
    fn test_feedback_low_cpk() {
        let mut twin = DigitalTwin::new("D1", "P1");
        // Wide spread relative to tight tolerance.
        let actuals = [10.04, 9.96, 10.05, 9.95, 10.03, 9.97, 10.04, 9.96, 10.05, 9.95];
        for (i, &v) in actuals.iter().enumerate() {
            twin.add_instance(make_instance(&format!("SN-{:03}", i), "pin_dia", v));
        }
        let suggestions = feedback_to_design(&twin);
        assert!(
            !suggestions.is_empty(),
            "Should suggest tolerance change for low Cpk"
        );
        let relevant: Vec<_> = suggestions.iter().filter(|s| s.feature_name == "pin_dia").collect();
        assert!(!relevant.is_empty());
    }

    #[test]
    fn test_feedback_high_defect_rate() {
        let mut twin = DigitalTwin::new("D1", "P1");
        for i in 0..10 {
            let mut inst = make_instance(&format!("SN-{:03}", i), "bore", 10.0);
            if i < 3 {
                inst.defects.push(Defect {
                    defect_type: DefectType::Surface,
                    description: "burr on edge".to_string(),
                    severity: 2,
                });
            }
            twin.add_instance(inst);
        }
        let suggestions = feedback_to_design(&twin);
        let defect_suggestions: Vec<_> = suggestions
            .iter()
            .filter(|s| s.suggestion.contains("burr on edge"))
            .collect();
        assert!(
            !defect_suggestions.is_empty(),
            "Should flag high defect rate"
        );
    }

    #[test]
    fn test_serialization_roundtrip() {
        let twin = make_twin_with_data();
        let json = serde_json::to_string(&twin).unwrap();
        let restored: DigitalTwin = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.design_id, twin.design_id);
        assert_eq!(
            restored.manufactured_instances.len(),
            twin.manufactured_instances.len()
        );
    }
}
