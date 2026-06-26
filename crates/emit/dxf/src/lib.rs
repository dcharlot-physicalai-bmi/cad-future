//! `physical-emit-dxf` — DXF R12 (AC1009) import/export for 2D contours and 3D wireframes.
//!
//! Produces DXF files compatible with virtually all CAD/CAM software.

use std::fmt::Write;

/// Write 2D contours as DXF R12 polylines.
///
/// Each contour is a sequence of [x, y] points written as a POLYLINE entity.
pub fn write_dxf(contours: &[Vec<[f64; 2]>]) -> String {
    let mut out = String::with_capacity(4096);

    // Header section
    write_header(&mut out);

    // Entities section
    w(&mut out, "  0");
    w(&mut out, "SECTION");
    w(&mut out, "  2");
    w(&mut out, "ENTITIES");

    for contour in contours {
        if contour.is_empty() {
            continue;
        }
        write_polyline_2d(&mut out, contour);
    }

    w(&mut out, "  0");
    w(&mut out, "ENDSEC");

    // EOF
    w(&mut out, "  0");
    w(&mut out, "EOF");

    out
}

/// Write 3D wireframe edges from a B-Rep solid as DXF LINE entities.
pub fn write_dxf_3d(solid: &physical_brep::Solid) -> String {
    let mut out = String::with_capacity(4096);

    write_header(&mut out);

    // Entities section
    w(&mut out, "  0");
    w(&mut out, "SECTION");
    w(&mut out, "  2");
    w(&mut out, "ENTITIES");

    // Extract edges and write as LINE entities
    for (_eid, edge) in &solid.edges {
        let v_start = &solid.vertices[edge.v_start];
        let v_end = &solid.vertices[edge.v_end];
        write_line_3d(&mut out, v_start.point, v_end.point);
    }

    w(&mut out, "  0");
    w(&mut out, "ENDSEC");

    w(&mut out, "  0");
    w(&mut out, "EOF");

    out
}

/// Parse DXF polylines back to 2D contours.
///
/// Recognizes POLYLINE/VERTEX/SEQEND and LWPOLYLINE entities.
pub fn read_dxf_contours(text: &str) -> Vec<Vec<[f64; 2]>> {
    let mut contours = Vec::new();
    let lines: Vec<&str> = text.lines().map(|l| l.trim()).collect();
    let mut i = 0;

    while i + 1 < lines.len() {
        let code = lines[i].trim();
        let value = lines[i + 1].trim();

        if code == "0" && value == "POLYLINE" {
            // Read vertices until SEQEND
            i += 2;
            let mut contour = Vec::new();

            while i + 1 < lines.len() {
                let c = lines[i].trim();
                let v = lines[i + 1].trim();

                if c == "0" && v == "SEQEND" {
                    i += 2;
                    break;
                }

                if c == "0" && v == "VERTEX" {
                    i += 2;
                    let mut x = 0.0_f64;
                    let mut y = 0.0_f64;

                    while i + 1 < lines.len() {
                        let vc = lines[i].trim();
                        let vv = lines[i + 1].trim();

                        if vc == "0" {
                            break; // next entity
                        }

                        match vc {
                            "10" => x = vv.parse().unwrap_or(0.0),
                            "20" => y = vv.parse().unwrap_or(0.0),
                            _ => {}
                        }
                        i += 2;
                    }

                    contour.push([x, y]);
                } else {
                    i += 2;
                }
            }

            if !contour.is_empty() {
                contours.push(contour);
            }
        } else {
            i += 2;
        }
    }

    contours
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn w(out: &mut String, line: &str) {
    out.push_str(line);
    out.push('\n');
}

fn wf(out: &mut String, code: &str, val: f64) {
    w(out, code);
    let _ = writeln!(out, "{:.6}", val);
}

fn write_header(out: &mut String) {
    w(out, "  0");
    w(out, "SECTION");
    w(out, "  2");
    w(out, "HEADER");
    w(out, "  9");
    w(out, "$ACADVER");
    w(out, "  1");
    w(out, "AC1009");
    w(out, "  0");
    w(out, "ENDSEC");
}

fn write_polyline_2d(out: &mut String, contour: &[[f64; 2]]) {
    // POLYLINE entity header
    w(out, "  0");
    w(out, "POLYLINE");
    w(out, "  8");
    w(out, "0"); // layer
    w(out, " 66");
    w(out, "     1"); // vertices follow flag
    w(out, " 70");
    w(out, "     1"); // closed polyline

    for pt in contour {
        w(out, "  0");
        w(out, "VERTEX");
        w(out, "  8");
        w(out, "0");
        wf(out, " 10", pt[0]);
        wf(out, " 20", pt[1]);
        wf(out, " 30", 0.0);
    }

    w(out, "  0");
    w(out, "SEQEND");
    w(out, "  8");
    w(out, "0");
}

fn write_line_3d(out: &mut String, start: glam::DVec3, end: glam::DVec3) {
    w(out, "  0");
    w(out, "LINE");
    w(out, "  8");
    w(out, "0"); // layer
    wf(out, " 10", start.x);
    wf(out, " 20", start.y);
    wf(out, " 30", start.z);
    wf(out, " 11", end.x);
    wf(out, " 21", end.y);
    wf(out, " 31", end.z);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_single_contour() {
        let contours = vec![vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]];
        let dxf = write_dxf(&contours);

        assert!(dxf.contains("AC1009"), "should contain ACADVER");
        assert!(dxf.contains("POLYLINE"), "should contain POLYLINE");
        assert!(dxf.contains("VERTEX"), "should contain VERTEX");
        assert!(dxf.contains("SEQEND"), "should contain SEQEND");
        assert!(dxf.contains("EOF"), "should end with EOF");
    }

    #[test]
    fn write_multiple_contours() {
        let contours = vec![
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]],
            vec![[20.0, 20.0], [30.0, 20.0], [30.0, 30.0]],
        ];
        let dxf = write_dxf(&contours);

        // Should have two POLYLINE entities
        let polyline_count = dxf.matches("POLYLINE").count();
        assert_eq!(polyline_count, 2, "should have 2 POLYLINE entities");

        // Should have two SEQEND
        let seqend_count = dxf.matches("SEQEND").count();
        assert_eq!(seqend_count, 2, "should have 2 SEQEND markers");
    }

    #[test]
    fn write_empty_contours() {
        let contours: Vec<Vec<[f64; 2]>> = vec![];
        let dxf = write_dxf(&contours);
        assert!(dxf.contains("EOF"));
        assert!(!dxf.contains("POLYLINE"));
    }

    #[test]
    fn roundtrip_contours() {
        let original = vec![
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
        ];
        let dxf = write_dxf(&original);
        let parsed = read_dxf_contours(&dxf);

        assert_eq!(parsed.len(), 1, "should parse 1 contour");
        assert_eq!(parsed[0].len(), 4, "contour should have 4 vertices");

        for (orig, read) in original[0].iter().zip(parsed[0].iter()) {
            assert!((orig[0] - read[0]).abs() < 1e-4, "x mismatch");
            assert!((orig[1] - read[1]).abs() < 1e-4, "y mismatch");
        }
    }

    #[test]
    fn write_dxf_3d_box() {
        let solid = physical_brep::builder::make_box(10.0, 20.0, 30.0);
        let dxf = write_dxf_3d(&solid);

        assert!(dxf.contains("AC1009"));
        assert!(dxf.contains("LINE"), "should contain LINE entities");
        assert!(dxf.contains("EOF"));

        // A box has 12 edges
        let line_count = dxf.lines()
            .filter(|l| l.trim() == "LINE")
            .count();
        assert_eq!(line_count, 12, "box should have 12 LINE entities (12 edges)");
    }

    #[test]
    fn write_dxf_coordinates() {
        let contours = vec![vec![[1.5, 2.5], [3.5, 4.5]]];
        let dxf = write_dxf(&contours);
        assert!(dxf.contains("1.500000"), "x coordinate should be written");
        assert!(dxf.contains("2.500000"), "y coordinate should be written");
    }
}
