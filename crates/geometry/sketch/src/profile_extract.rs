//! Extract closed profiles from a solved sketch for extrusion.

use glam::DVec2;
use crate::entity::SketchEntity;
use crate::sketch::Sketch;

/// A profile segment extracted from sketch entities.
#[derive(Clone, Debug)]
pub enum ExtractedSegment {
    Line { start: DVec2, end: DVec2 },
    Arc { center: DVec2, start: DVec2, end: DVec2, radius: f64 },
}

/// Extract closed profiles from a sketch.
/// Returns a list of closed loops, each being an ordered list of segments.
///
/// Currently supports simple cases: connected line segments forming closed loops.
pub fn extract_profiles(sketch: &Sketch) -> Vec<Vec<ExtractedSegment>> {
    // Collect all line segments with their start/end points
    let mut segments: Vec<(DVec2, DVec2, usize)> = Vec::new();
    for (i, entity) in sketch.entities.iter().enumerate() {
        match entity {
            SketchEntity::Line { start, end } => {
                segments.push((*start, *end, i));
            }
            _ => {}
        }
    }

    if segments.is_empty() { return Vec::new(); }

    let mut profiles = Vec::new();
    let mut used = vec![false; segments.len()];
    let eps = 1e-6;

    // Greedy loop extraction
    for start_idx in 0..segments.len() {
        if used[start_idx] { continue; }

        let mut loop_segs = Vec::new();
        let current = start_idx;
        let loop_start = segments[start_idx].0;
        used[current] = true;
        loop_segs.push(ExtractedSegment::Line {
            start: segments[current].0,
            end: segments[current].1,
        });

        let mut current_end = segments[current].1;

        // Try to find the next connected segment
        for _safety in 0..segments.len() {
            // Check if we've closed the loop
            if (current_end - loop_start).length() < eps && loop_segs.len() >= 3 {
                profiles.push(loop_segs);
                break;
            }

            // Find next unvisited segment starting at current_end
            let mut found = false;
            for j in 0..segments.len() {
                if used[j] { continue; }
                if (segments[j].0 - current_end).length() < eps {
                    used[j] = true;
                    loop_segs.push(ExtractedSegment::Line {
                        start: segments[j].0,
                        end: segments[j].1,
                    });
                    current_end = segments[j].1;
                    found = true;
                    break;
                }
                // Try reversed
                if (segments[j].1 - current_end).length() < eps {
                    used[j] = true;
                    loop_segs.push(ExtractedSegment::Line {
                        start: segments[j].1,
                        end: segments[j].0,
                    });
                    current_end = segments[j].0;
                    found = true;
                    break;
                }
            }
            if !found { break; }
        }
    }

    profiles
}

/// Convert extracted profile to a physical-brep Profile.
pub fn to_brep_profile(segments: &[ExtractedSegment]) -> physical_brep::Profile {
    let segs: Vec<physical_brep::ProfileSegment> = segments.iter().map(|s| {
        match s {
            ExtractedSegment::Line { start, end } => {
                physical_brep::ProfileSegment::line(*start, *end)
            }
            ExtractedSegment::Arc { center, radius, .. } => {
                let start_angle = 0.0; // simplified
                let end_angle = std::f64::consts::FRAC_PI_2;
                physical_brep::ProfileSegment::arc(*center, *radius, start_angle, end_angle)
            }
        }
    }).collect();
    physical_brep::Profile::new(segs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_rectangle_profile() {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::line(0.0, 0.0, 10.0, 0.0));
        s.add_entity(SketchEntity::line(10.0, 0.0, 10.0, 5.0));
        s.add_entity(SketchEntity::line(10.0, 5.0, 0.0, 5.0));
        s.add_entity(SketchEntity::line(0.0, 5.0, 0.0, 0.0));

        let profiles = extract_profiles(&s);
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].len(), 4);
    }

    #[test]
    fn empty_sketch_no_profiles() {
        let s = Sketch::new();
        let profiles = extract_profiles(&s);
        assert!(profiles.is_empty());
    }

    #[test]
    fn open_chain_not_profile() {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::line(0.0, 0.0, 10.0, 0.0));
        s.add_entity(SketchEntity::line(10.0, 0.0, 10.0, 5.0));
        // Not closed — only 2 segments
        let profiles = extract_profiles(&s);
        assert!(profiles.is_empty());
    }
}
