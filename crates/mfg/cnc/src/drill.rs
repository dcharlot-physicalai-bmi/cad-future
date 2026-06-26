//! Drill cycle generation — G81 (simple), G83 (peck drilling).

use glam::{DVec2, DVec3};
use physical_mfg_toolpath::path::{MoveType, ToolpathSegment};

/// Drill cycle type.
#[derive(Clone, Copy, Debug)]
pub enum DrillCycle {
    /// G81: Simple drill — plunge to depth in one pass.
    Simple,
    /// G83: Peck drilling — drill in increments, retracting to clear chips.
    Peck { peck_depth: f64 },
}

/// A drill hole location.
#[derive(Clone, Debug)]
pub struct DrillHole {
    /// XY position.
    pub position: DVec2,
    /// Target depth (Z, typically negative relative to stock top).
    pub depth: f64,
}

/// Generate drill cycle toolpath for a set of holes.
pub fn generate_drill_cycle(
    holes: &[DrillHole],
    cycle: DrillCycle,
    top_z: f64,
    feed_rate: f64,
    safe_z: f64,
) -> Vec<ToolpathSegment> {
    let mut segments = Vec::new();

    for hole in holes {
        let target_z = top_z - hole.depth;
        let pos = hole.position;

        match cycle {
            DrillCycle::Simple => {
                // Rapid to position
                segments.push(ToolpathSegment::rapid(
                    DVec3::new(pos.x, pos.y, safe_z),
                    DVec3::new(pos.x, pos.y, safe_z),
                ));
                // Plunge to depth
                segments.push(ToolpathSegment {
                    path: vec![
                        DVec3::new(pos.x, pos.y, safe_z),
                        DVec3::new(pos.x, pos.y, target_z),
                    ],
                    feed_rate,
                    move_type: MoveType::Plunge,
                });
                // Retract
                segments.push(ToolpathSegment {
                    path: vec![
                        DVec3::new(pos.x, pos.y, target_z),
                        DVec3::new(pos.x, pos.y, safe_z),
                    ],
                    feed_rate: 0.0,
                    move_type: MoveType::Retract,
                });
            }
            DrillCycle::Peck { peck_depth } => {
                // Rapid to position
                segments.push(ToolpathSegment::rapid(
                    DVec3::new(pos.x, pos.y, safe_z),
                    DVec3::new(pos.x, pos.y, safe_z),
                ));

                let mut current_z = top_z;
                while current_z > target_z + 1e-6 {
                    let next_z = (current_z - peck_depth).max(target_z);

                    // Plunge (rapid to previous depth, then feed to new depth)
                    segments.push(ToolpathSegment {
                        path: vec![
                            DVec3::new(pos.x, pos.y, current_z),
                            DVec3::new(pos.x, pos.y, next_z),
                        ],
                        feed_rate,
                        move_type: MoveType::Plunge,
                    });

                    // Retract to clear chips
                    segments.push(ToolpathSegment {
                        path: vec![
                            DVec3::new(pos.x, pos.y, next_z),
                            DVec3::new(pos.x, pos.y, safe_z),
                        ],
                        feed_rate: 0.0,
                        move_type: MoveType::Retract,
                    });

                    current_z = next_z;
                }
            }
        }
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_drill() {
        let holes = vec![
            DrillHole { position: DVec2::new(10.0, 10.0), depth: 15.0 },
            DrillHole { position: DVec2::new(30.0, 10.0), depth: 15.0 },
        ];
        let segments = generate_drill_cycle(&holes, DrillCycle::Simple, 10.0, 200.0, 15.0);
        assert!(!segments.is_empty());
        // 2 holes × 3 segments (rapid + plunge + retract) = 6
        assert_eq!(segments.len(), 6);
    }

    #[test]
    fn peck_drill() {
        let holes = vec![DrillHole {
            position: DVec2::new(10.0, 10.0),
            depth: 20.0,
        }];
        let segments = generate_drill_cycle(
            &holes,
            DrillCycle::Peck { peck_depth: 5.0 },
            10.0,
            200.0,
            15.0,
        );
        assert!(!segments.is_empty());
        // 20mm depth / 5mm peck = 4 pecks × 2 segments (plunge + retract) + 1 rapid = 9
        let plunges: Vec<_> = segments.iter().filter(|s| s.move_type == MoveType::Plunge).collect();
        assert_eq!(plunges.len(), 4, "Should have 4 peck plunges");
    }
}
