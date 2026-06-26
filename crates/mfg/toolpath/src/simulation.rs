//! Machine simulation for collision detection and toolpath verification.

use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::path::ToolpathSegment;

/// Axis-aligned bounding box for collision checks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Aabb {
    pub min: DVec3,
    pub max: DVec3,
}

impl Aabb {
    pub fn new(min: DVec3, max: DVec3) -> Self {
        Self { min, max }
    }

    pub fn intersects(&self, other: &Aabb) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x
            && self.min.y <= other.max.y && self.max.y >= other.min.y
            && self.min.z <= other.max.z && self.max.z >= other.min.z
    }

    pub fn contains_point(&self, p: DVec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x
            && p.y >= self.min.y && p.y <= self.max.y
            && p.z >= self.min.z && p.z <= self.max.z
    }

    pub fn expand(&self, margin: f64) -> Self {
        Self {
            min: self.min - DVec3::splat(margin),
            max: self.max + DVec3::splat(margin),
        }
    }
}

/// A fixture or clamp that might collide with the tool/holder.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fixture {
    pub name: String,
    pub bounds: Aabb,
}

/// Machine simulation configuration.
#[derive(Clone, Debug)]
pub struct MachineSimulation {
    /// Tool holder bounding box (relative to tool tip).
    pub holder_bounds: Aabb,
    /// Fixtures in the work envelope.
    pub fixtures: Vec<Fixture>,
    /// Work piece bounding box.
    pub workpiece: Aabb,
    /// Machine travel limits.
    pub travel_limits: Option<Aabb>,
}

/// Result of a collision check.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollisionResult {
    pub has_collision: bool,
    pub collisions: Vec<CollisionEvent>,
    pub out_of_bounds: Vec<OutOfBoundsEvent>,
    pub total_segments: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollisionEvent {
    pub segment_index: usize,
    pub point_index: usize,
    pub position: [f64; 3],
    pub fixture_name: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutOfBoundsEvent {
    pub segment_index: usize,
    pub point_index: usize,
    pub position: [f64; 3],
    pub axis: String,
}

impl MachineSimulation {
    pub fn new(workpiece: Aabb) -> Self {
        Self {
            holder_bounds: Aabb::new(
                DVec3::new(-25.0, -25.0, 0.0),
                DVec3::new(25.0, 25.0, 100.0),
            ),
            fixtures: Vec::new(),
            workpiece,
            travel_limits: None,
        }
    }

    /// Check a toolpath for collisions and travel limit violations.
    pub fn check_toolpath(&self, segments: &[ToolpathSegment]) -> CollisionResult {
        let mut collisions = Vec::new();
        let mut out_of_bounds = Vec::new();

        for (seg_idx, segment) in segments.iter().enumerate() {
            for (pt_idx, &point) in segment.path.iter().enumerate() {
                // Check holder against fixtures
                let holder_at_point = Aabb::new(
                    point + self.holder_bounds.min,
                    point + self.holder_bounds.max,
                );

                for fixture in &self.fixtures {
                    if holder_at_point.intersects(&fixture.bounds) {
                        collisions.push(CollisionEvent {
                            segment_index: seg_idx,
                            point_index: pt_idx,
                            position: [point.x, point.y, point.z],
                            fixture_name: fixture.name.clone(),
                            description: format!(
                                "Tool holder collision with '{}' at ({:.1}, {:.1}, {:.1})",
                                fixture.name, point.x, point.y, point.z
                            ),
                        });
                    }
                }

                // Check travel limits
                if let Some(ref limits) = self.travel_limits {
                    if !limits.contains_point(point) {
                        let axis = if point.x < limits.min.x || point.x > limits.max.x { "X" }
                            else if point.y < limits.min.y || point.y > limits.max.y { "Y" }
                            else { "Z" };
                        out_of_bounds.push(OutOfBoundsEvent {
                            segment_index: seg_idx,
                            point_index: pt_idx,
                            position: [point.x, point.y, point.z],
                            axis: axis.to_string(),
                        });
                    }
                }
            }
        }

        CollisionResult {
            has_collision: !collisions.is_empty() || !out_of_bounds.is_empty(),
            collisions,
            out_of_bounds,
            total_segments: segments.len(),
        }
    }

    /// Estimate machining time from a toolpath (seconds).
    pub fn estimate_time(&self, segments: &[ToolpathSegment], rapid_feed: f64) -> f64 {
        segments.iter().map(|s| s.estimated_time_s(rapid_feed)).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aabb_intersection() {
        let a = Aabb::new(DVec3::ZERO, DVec3::splat(10.0));
        let b = Aabb::new(DVec3::splat(5.0), DVec3::splat(15.0));
        let c = Aabb::new(DVec3::splat(20.0), DVec3::splat(30.0));
        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }

    #[test]
    fn no_collision_simple_path() {
        let sim = MachineSimulation::new(
            Aabb::new(DVec3::ZERO, DVec3::new(100.0, 100.0, 50.0)),
        );
        let segments = vec![
            ToolpathSegment::rapid(DVec3::new(0.0, 0.0, 50.0), DVec3::new(50.0, 50.0, 50.0)),
        ];
        let result = sim.check_toolpath(&segments);
        assert!(!result.has_collision);
    }

    #[test]
    fn collision_with_fixture() {
        let mut sim = MachineSimulation::new(
            Aabb::new(DVec3::ZERO, DVec3::new(100.0, 100.0, 50.0)),
        );
        sim.fixtures.push(Fixture {
            name: "Vise jaw".into(),
            bounds: Aabb::new(DVec3::new(40.0, 40.0, 0.0), DVec3::new(60.0, 60.0, 80.0)),
        });

        let segments = vec![
            ToolpathSegment::rapid(
                DVec3::new(50.0, 50.0, 50.0),
                DVec3::new(50.0, 50.0, 10.0),
            ),
        ];
        let result = sim.check_toolpath(&segments);
        assert!(result.has_collision);
        assert!(!result.collisions.is_empty());
    }

    #[test]
    fn out_of_bounds_detection() {
        let mut sim = MachineSimulation::new(
            Aabb::new(DVec3::ZERO, DVec3::new(100.0, 100.0, 50.0)),
        );
        sim.travel_limits = Some(Aabb::new(DVec3::ZERO, DVec3::new(200.0, 200.0, 200.0)));

        let segments = vec![
            ToolpathSegment::rapid(
                DVec3::new(0.0, 0.0, 50.0),
                DVec3::new(250.0, 0.0, 50.0), // Beyond X limit
            ),
        ];
        let result = sim.check_toolpath(&segments);
        assert!(result.has_collision);
        assert!(!result.out_of_bounds.is_empty());
        assert_eq!(result.out_of_bounds[0].axis, "X");
    }

    #[test]
    fn time_estimation() {
        let sim = MachineSimulation::new(
            Aabb::new(DVec3::ZERO, DVec3::new(100.0, 100.0, 50.0)),
        );
        let segments = vec![
            ToolpathSegment::cut(
                vec![DVec3::ZERO, DVec3::new(60.0, 0.0, 0.0)], // 60mm
                600.0, // 600 mm/min = 10 mm/s
            ),
        ];
        let time = sim.estimate_time(&segments, 5000.0);
        assert!((time - 6.0).abs() < 0.1, "Expected ~6s for 60mm at 600mm/min, got {time}");
    }
}
