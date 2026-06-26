//! 2D profile — closed loop of segments for extrusion input.

use glam::{DVec2, DVec3};
use serde::{Serialize, Deserialize};

/// A segment in a 2D profile loop.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ProfileSegment {
    /// Straight line from start to end.
    Line { start: DVec2, end: DVec2 },
    /// Circular arc.
    Arc { center: DVec2, radius: f64, start_angle: f64, end_angle: f64 },
}

impl ProfileSegment {
    pub fn line(start: DVec2, end: DVec2) -> Self {
        Self::Line { start, end }
    }

    pub fn arc(center: DVec2, radius: f64, start: f64, end: f64) -> Self {
        Self::Arc { center, radius, start_angle: start, end_angle: end }
    }

    /// Start point of this segment.
    pub fn start_point(&self) -> DVec2 {
        match self {
            Self::Line { start, .. } => *start,
            Self::Arc { center, radius, start_angle, .. } => {
                *center + DVec2::new(radius * start_angle.cos(), radius * start_angle.sin())
            }
        }
    }

    /// End point of this segment.
    pub fn end_point(&self) -> DVec2 {
        match self {
            Self::Line { end, .. } => *end,
            Self::Arc { center, radius, end_angle, .. } => {
                *center + DVec2::new(radius * end_angle.cos(), radius * end_angle.sin())
            }
        }
    }

    /// Approximate length of segment.
    pub fn length(&self) -> f64 {
        match self {
            Self::Line { start, end } => (*end - *start).length(),
            Self::Arc { radius, start_angle, end_angle, .. } => {
                radius * (end_angle - start_angle).abs()
            }
        }
    }
}

/// A closed 2D profile — ordered loop of segments where each end connects to the next start.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub segments: Vec<ProfileSegment>,
}

impl Profile {
    pub fn new(segments: Vec<ProfileSegment>) -> Self {
        Self { segments }
    }

    /// Create a rectangular profile centered at origin.
    pub fn rectangle(width: f64, height: f64) -> Self {
        let hw = width / 2.0;
        let hh = height / 2.0;
        let p0 = DVec2::new(-hw, -hh);
        let p1 = DVec2::new( hw, -hh);
        let p2 = DVec2::new( hw,  hh);
        let p3 = DVec2::new(-hw,  hh);
        Self::new(vec![
            ProfileSegment::line(p0, p1),
            ProfileSegment::line(p1, p2),
            ProfileSegment::line(p2, p3),
            ProfileSegment::line(p3, p0),
        ])
    }

    /// Create a circular profile.
    pub fn circle(radius: f64) -> Self {
        Self::new(vec![
            ProfileSegment::arc(DVec2::ZERO, radius, 0.0, std::f64::consts::TAU),
        ])
    }

    /// Create an L-shaped profile.
    pub fn l_shape(w: f64, h: f64, t: f64) -> Self {
        let p0 = DVec2::new(0.0, 0.0);
        let p1 = DVec2::new(w, 0.0);
        let p2 = DVec2::new(w, t);
        let p3 = DVec2::new(t, t);
        let p4 = DVec2::new(t, h);
        let p5 = DVec2::new(0.0, h);
        Self::new(vec![
            ProfileSegment::line(p0, p1),
            ProfileSegment::line(p1, p2),
            ProfileSegment::line(p2, p3),
            ProfileSegment::line(p3, p4),
            ProfileSegment::line(p4, p5),
            ProfileSegment::line(p5, p0),
        ])
    }

    /// Check if profile is closed (last endpoint == first startpoint).
    pub fn is_closed(&self) -> bool {
        if self.segments.is_empty() { return false; }
        let first_start = self.segments[0].start_point();
        let last_end = self.segments.last().unwrap().end_point();
        (first_start - last_end).length() < 1e-8
    }

    /// Get ordered 2D vertices of the profile (for polygonal profiles).
    pub fn vertices_2d(&self) -> Vec<DVec2> {
        self.segments.iter().map(|s| s.start_point()).collect()
    }

    /// Lift 2D profile onto a 3D plane.
    /// `origin` is the plane center, `u` and `v` are the plane axes.
    pub fn to_3d(&self, origin: DVec3, u: DVec3, v: DVec3) -> Vec<DVec3> {
        self.vertices_2d().iter().map(|p| origin + u * p.x + v * p.y).collect()
    }

    /// Signed area (positive = CCW winding).
    pub fn signed_area(&self) -> f64 {
        let verts = self.vertices_2d();
        let n = verts.len();
        let mut area = 0.0;
        for i in 0..n {
            let j = (i + 1) % n;
            area += verts[i].x * verts[j].y;
            area -= verts[j].x * verts[i].y;
        }
        area * 0.5
    }

    /// Ensure CCW winding.
    pub fn ensure_ccw(&mut self) {
        if self.signed_area() < 0.0 {
            self.segments.reverse();
            // Swap start/end of each segment
            for seg in &mut self.segments {
                *seg = match seg {
                    ProfileSegment::Line { start, end } => ProfileSegment::Line { start: *end, end: *start },
                    ProfileSegment::Arc { center, radius, start_angle, end_angle } =>
                        ProfileSegment::Arc { center: *center, radius: *radius, start_angle: *end_angle, end_angle: *start_angle },
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_is_closed() {
        let p = Profile::rectangle(10.0, 5.0);
        assert!(p.is_closed());
        assert_eq!(p.segments.len(), 4);
    }

    #[test]
    fn circle_profile() {
        let p = Profile::circle(5.0);
        assert_eq!(p.segments.len(), 1);
    }

    #[test]
    fn rectangle_area() {
        let p = Profile::rectangle(10.0, 5.0);
        assert!((p.signed_area().abs() - 50.0).abs() < 1e-8);
    }

    #[test]
    fn l_shape_vertices() {
        let p = Profile::l_shape(20.0, 30.0, 5.0);
        assert_eq!(p.segments.len(), 6);
        assert!(p.is_closed());
    }
}
