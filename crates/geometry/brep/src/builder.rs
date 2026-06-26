//! Primitive solid builders.

use glam::DVec3;
use crate::solid::Solid;
use crate::surface::Surface;

/// Build a box solid with given dimensions, centered at origin.
pub fn make_box(width: f64, height: f64, depth: f64) -> Solid {
    let mut s = Solid::new();
    let hw = width / 2.0;
    let hh = height / 2.0;
    let hd = depth / 2.0;

    // 8 vertices of the box
    let v0 = s.add_vertex(DVec3::new(-hw, -hh, -hd)); // 0: left-bottom-back
    let v1 = s.add_vertex(DVec3::new( hw, -hh, -hd)); // 1: right-bottom-back
    let v2 = s.add_vertex(DVec3::new( hw,  hh, -hd)); // 2: right-top-back
    let v3 = s.add_vertex(DVec3::new(-hw,  hh, -hd)); // 3: left-top-back
    let v4 = s.add_vertex(DVec3::new(-hw, -hh,  hd)); // 4: left-bottom-front
    let v5 = s.add_vertex(DVec3::new( hw, -hh,  hd)); // 5: right-bottom-front
    let v6 = s.add_vertex(DVec3::new( hw,  hh,  hd)); // 6: right-top-front
    let v7 = s.add_vertex(DVec3::new(-hw,  hh,  hd)); // 7: left-top-front

    // 6 faces with outward normals (CCW winding when viewed from outside)
    // Front face (+Z)
    s.add_face_from_vertices(
        Surface::plane(DVec3::new(0.0, 0.0, hd), DVec3::Z),
        &[v4, v5, v6, v7], true,
    );
    // Back face (-Z)
    s.add_face_from_vertices(
        Surface::plane(DVec3::new(0.0, 0.0, -hd), -DVec3::Z),
        &[v1, v0, v3, v2], true,
    );
    // Right face (+X)
    s.add_face_from_vertices(
        Surface::plane(DVec3::new(hw, 0.0, 0.0), DVec3::X),
        &[v5, v1, v2, v6], true,
    );
    // Left face (-X)
    s.add_face_from_vertices(
        Surface::plane(DVec3::new(-hw, 0.0, 0.0), -DVec3::X),
        &[v0, v4, v7, v3], true,
    );
    // Top face (+Y)
    s.add_face_from_vertices(
        Surface::plane(DVec3::new(0.0, hh, 0.0), DVec3::Y),
        &[v3, v7, v6, v2], true,
    );
    // Bottom face (-Y)
    s.add_face_from_vertices(
        Surface::plane(DVec3::new(0.0, -hh, 0.0), -DVec3::Y),
        &[v0, v1, v5, v4], true,
    );

    s.link_twins();
    s
}

/// Build a cylinder solid with given radius and height, centered at origin, axis along Y.
pub fn make_cylinder(radius: f64, height: f64, segments: usize) -> Solid {
    let mut s = Solid::new();
    let hh = height / 2.0;
    let seg = segments.max(8);

    // Generate vertices: bottom ring, top ring, plus center vertices
    let mut bottom_verts = Vec::with_capacity(seg);
    let mut top_verts = Vec::with_capacity(seg);

    for i in 0..seg {
        let angle = std::f64::consts::TAU * (i as f64 / seg as f64);
        let x = radius * angle.cos();
        let z = radius * angle.sin();
        bottom_verts.push(s.add_vertex(DVec3::new(x, -hh, z)));
        top_verts.push(s.add_vertex(DVec3::new(x, hh, z)));
    }

    // Bottom face (normal -Y)
    let bottom_reversed: Vec<_> = bottom_verts.iter().rev().copied().collect();
    s.add_face_from_vertices(
        Surface::plane(DVec3::new(0.0, -hh, 0.0), -DVec3::Y),
        &bottom_reversed, true,
    );

    // Top face (normal +Y)
    s.add_face_from_vertices(
        Surface::plane(DVec3::new(0.0, hh, 0.0), DVec3::Y),
        &top_verts, true,
    );

    // Side faces (quads as individual faces)
    for i in 0..seg {
        let j = (i + 1) % seg;
        s.add_face_from_vertices(
            Surface::cylinder(DVec3::ZERO, DVec3::Y, radius),
            &[bottom_verts[i], bottom_verts[j], top_verts[j], top_verts[i]], true,
        );
    }

    s.link_twins();
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_euler() {
        let b = make_box(10.0, 20.0, 30.0);
        assert_eq!(b.vertex_count(), 8);
        assert_eq!(b.face_count(), 6);
        assert_eq!(b.edge_count(), 12);
        assert!(b.is_valid_shell(), "Euler: V-E+F = {}", b.euler_characteristic());
    }

    #[test]
    fn box_bounding_box() {
        let b = make_box(10.0, 20.0, 30.0);
        let (min, max) = b.bounding_box();
        assert!((min.x + 5.0).abs() < 1e-10);
        assert!((max.y - 10.0).abs() < 1e-10);
    }

    #[test]
    fn cylinder_topology() {
        let c = make_cylinder(5.0, 10.0, 16);
        // 32 vertices (16 bottom + 16 top)
        assert_eq!(c.vertex_count(), 32);
        // 2 caps + 16 sides = 18 faces
        assert_eq!(c.face_count(), 18);
        // Euler: V-E+F = 2
        assert!(c.is_valid_shell(), "Euler: V-E+F = {}", c.euler_characteristic());
    }

    #[test]
    fn box_edge_queries() {
        let b = make_box(10.0, 10.0, 10.0);
        let edges = b.edge_ids();
        for eid in &edges {
            let faces = b.faces_of_edge(*eid);
            assert_eq!(faces.len(), 2, "Every edge of a box should border 2 faces");
        }
    }

    #[test]
    fn box_face_queries() {
        let b = make_box(10.0, 10.0, 10.0);
        for fid in b.face_ids() {
            let edges = b.edges_of_face(fid);
            assert_eq!(edges.len(), 4, "Every face of a box should have 4 edges");
        }
    }
}
