//! `physical-emit-kicad` -- KiCad PCB reader.
//!
//! Parses `.kicad_pcb` S-expression files and extracts board outline,
//! component positions, and drill locations. Can also extrude the board
//! outline into a B-Rep solid for ECAD-MCAD integration.

use glam::{DVec2, DVec3};
use physical_brep::profile::{Profile, ProfileSegment};
use physical_brep::extrude::extrude;
use physical_brep::solid::Solid;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A parsed KiCad PCB board.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KicadBoard {
    /// Board outline as an ordered list of 2D vertices (mm).
    pub board_outline: Vec<DVec2>,
    /// Components placed on the board.
    pub components: Vec<KicadComponent>,
    /// Through-holes and vias.
    pub drill_holes: Vec<DrillHole>,
    /// Board thickness in mm (default 1.6).
    pub board_thickness_mm: f64,
}

/// A component (module/footprint) placed on the board.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KicadComponent {
    /// Reference designator (e.g. "U1", "R3").
    pub reference: String,
    /// Footprint library name (e.g. "Package_SO:SOIC-8").
    pub footprint: String,
    /// 3D position (x, y in mm on the board plane; z = 0 for top, board_thickness for bottom).
    pub position: DVec3,
    /// Rotation in degrees.
    pub rotation: f64,
}

/// A drill hole (through-hole pad, via, or mounting hole).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DrillHole {
    /// Center position in mm.
    pub position: DVec2,
    /// Drill diameter in mm.
    pub diameter_mm: f64,
}

// ---------------------------------------------------------------------------
// S-expression tokenizer
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum SExpr {
    Atom(String),
    List(Vec<SExpr>),
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_string = false;

    for ch in text.chars() {
        if in_string {
            if ch == '"' {
                in_string = false;
                tokens.push(current.clone());
                current.clear();
            } else {
                current.push(ch);
            }
        } else {
            match ch {
                '"' => {
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                    in_string = true;
                }
                '(' => {
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                    tokens.push("(".to_string());
                }
                ')' => {
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                    tokens.push(")".to_string());
                }
                c if c.is_whitespace() => {
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                }
                c => current.push(c),
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn parse_sexpr(tokens: &[String], pos: &mut usize) -> Option<SExpr> {
    if *pos >= tokens.len() {
        return None;
    }
    if tokens[*pos] == "(" {
        *pos += 1;
        let mut children = Vec::new();
        while *pos < tokens.len() && tokens[*pos] != ")" {
            if let Some(child) = parse_sexpr(tokens, pos) {
                children.push(child);
            }
        }
        if *pos < tokens.len() {
            *pos += 1; // consume ')'
        }
        Some(SExpr::List(children))
    } else if tokens[*pos] == ")" {
        None
    } else {
        let atom = SExpr::Atom(tokens[*pos].clone());
        *pos += 1;
        Some(atom)
    }
}

// ---------------------------------------------------------------------------
// S-expression helpers
// ---------------------------------------------------------------------------

fn find_child<'a>(list: &'a [SExpr], tag: &str) -> Option<&'a [SExpr]> {
    for item in list {
        if let SExpr::List(children) = item {
            if let Some(SExpr::Atom(name)) = children.first() {
                if name == tag {
                    return Some(children);
                }
            }
        }
    }
    None
}

fn find_children<'a>(list: &'a [SExpr], tag: &str) -> Vec<&'a [SExpr]> {
    let mut result = Vec::new();
    for item in list {
        if let SExpr::List(children) = item {
            if let Some(SExpr::Atom(name)) = children.first() {
                if name == tag {
                    result.push(children.as_slice());
                }
            }
        }
    }
    result
}

fn atom_str(expr: &SExpr) -> Option<&str> {
    if let SExpr::Atom(s) = expr {
        Some(s.as_str())
    } else {
        None
    }
}

fn atom_f64(expr: &SExpr) -> Option<f64> {
    atom_str(expr)?.parse().ok()
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Parse a `.kicad_pcb` file text and return a `KicadBoard`.
///
/// Returns `None` if the text is not a valid KiCad PCB file.
pub fn read_kicad_pcb(text: &str) -> Option<KicadBoard> {
    let tokens = tokenize(text);
    let mut pos = 0;
    let root = parse_sexpr(&tokens, &mut pos)?;

    let root_children = match &root {
        SExpr::List(children) => children,
        _ => return None,
    };

    // Verify this is a kicad_pcb
    if atom_str(root_children.first()?)? != "kicad_pcb" {
        return None;
    }

    let mut board = KicadBoard {
        board_outline: Vec::new(),
        components: Vec::new(),
        drill_holes: Vec::new(),
        board_thickness_mm: 1.6,
    };

    // Extract board thickness from general section
    if let Some(general) = find_child(root_children, "general") {
        if let Some(thickness) = find_child(general, "thickness") {
            if let Some(val) = thickness.get(1).and_then(atom_f64) {
                board.board_thickness_mm = val;
            }
        }
    }

    // Extract board outline from gr_line elements on Edge.Cuts layer
    for gr_line in find_children(root_children, "gr_line") {
        let layer = find_child(gr_line, "layer");
        let is_edge_cuts = layer
            .and_then(|l| l.get(1))
            .and_then(atom_str)
            .is_some_and(|s| s == "Edge.Cuts");
        if !is_edge_cuts {
            continue;
        }
        if let Some(start) = find_child(gr_line, "start") {
            let x = start.get(1).and_then(atom_f64)?;
            let y = start.get(2).and_then(atom_f64)?;
            let pt = DVec2::new(x, y);
            if !board.board_outline.contains(&pt) {
                board.board_outline.push(pt);
            }
        }
        if let Some(end) = find_child(gr_line, "end") {
            let x = end.get(1).and_then(atom_f64)?;
            let y = end.get(2).and_then(atom_f64)?;
            let pt = DVec2::new(x, y);
            if !board.board_outline.contains(&pt) {
                board.board_outline.push(pt);
            }
        }
    }

    // Extract modules/footprints
    for tag in &["module", "footprint"] {
        for module in find_children(root_children, tag) {
            let footprint_name = module.get(1).and_then(atom_str).unwrap_or("").to_string();
            let mut reference = String::new();
            let mut pos_x = 0.0;
            let mut pos_y = 0.0;
            let mut rotation = 0.0;

            if let Some(at) = find_child(module, "at") {
                pos_x = at.get(1).and_then(atom_f64).unwrap_or(0.0);
                pos_y = at.get(2).and_then(atom_f64).unwrap_or(0.0);
                rotation = at.get(3).and_then(atom_f64).unwrap_or(0.0);
            }

            // Find reference designator in fp_text
            for fp_text in find_children(module, "fp_text") {
                if fp_text.get(1).and_then(atom_str) == Some("reference") {
                    reference = fp_text.get(2).and_then(atom_str).unwrap_or("").to_string();
                }
            }
            // Also check property nodes (KiCad 7+)
            for prop in find_children(module, "property") {
                if prop.get(1).and_then(atom_str) == Some("Reference") {
                    reference = prop.get(2).and_then(atom_str).unwrap_or("").to_string();
                }
            }

            board.components.push(KicadComponent {
                reference,
                footprint: footprint_name,
                position: DVec3::new(pos_x, pos_y, 0.0),
                rotation,
            });
        }
    }

    // Extract drill holes from pads with drill and vias
    for tag in &["module", "footprint"] {
        for module in find_children(root_children, tag) {
            let mod_at = find_child(module, "at");
            let mod_x = mod_at.and_then(|a| a.get(1)).and_then(atom_f64).unwrap_or(0.0);
            let mod_y = mod_at.and_then(|a| a.get(2)).and_then(atom_f64).unwrap_or(0.0);

            for pad in find_children(module, "pad") {
                if let Some(drill) = find_child(pad, "drill") {
                    let diameter = drill.get(1).and_then(atom_f64).unwrap_or(0.0);
                    if diameter > 0.0 {
                        let pad_at = find_child(pad, "at");
                        let px = pad_at.and_then(|a| a.get(1)).and_then(atom_f64).unwrap_or(0.0);
                        let py = pad_at.and_then(|a| a.get(2)).and_then(atom_f64).unwrap_or(0.0);
                        board.drill_holes.push(DrillHole {
                            position: DVec2::new(mod_x + px, mod_y + py),
                            diameter_mm: diameter,
                        });
                    }
                }
            }
        }
    }

    // Extract vias at top level
    for via in find_children(root_children, "via") {
        if let Some(at) = find_child(via, "at") {
            let x = at.get(1).and_then(atom_f64).unwrap_or(0.0);
            let y = at.get(2).and_then(atom_f64).unwrap_or(0.0);
            let drill_size = find_child(via, "drill")
                .and_then(|d| d.get(1))
                .and_then(atom_f64)
                .unwrap_or(0.0);
            // Fallback: try size
            let diameter = if drill_size > 0.0 {
                drill_size
            } else {
                find_child(via, "size")
                    .and_then(|s| s.get(1))
                    .and_then(atom_f64)
                    .unwrap_or(0.3)
            };
            board.drill_holes.push(DrillHole {
                position: DVec2::new(x, y),
                diameter_mm: diameter,
            });
        }
    }

    Some(board)
}

/// Extrude the board outline into a `Solid` with the given board thickness.
///
/// The board is extruded in +Z from z=0 to z=board_thickness_mm.
pub fn board_to_solid(board: &KicadBoard) -> Solid {
    if board.board_outline.len() < 3 {
        return Solid::new();
    }

    let segments: Vec<ProfileSegment> = board
        .board_outline
        .windows(2)
        .map(|w| ProfileSegment::Line {
            start: w[0],
            end: w[1],
        })
        .chain(std::iter::once(ProfileSegment::Line {
            start: *board.board_outline.last().unwrap(),
            end: board.board_outline[0],
        }))
        .collect();

    let profile = Profile::new(segments);

    extrude(
        &profile,
        DVec3::ZERO,
        DVec3::X,
        DVec3::Y,
        DVec3::Z,
        board.board_thickness_mm,
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PCB: &str = r#"
(kicad_pcb (version 4) (host pcbnew "5.1.0")
  (general
    (thickness 1.6)
  )
  (gr_line (start 0 0) (end 50 0) (layer Edge.Cuts) (width 0.05))
  (gr_line (start 50 0) (end 50 40) (layer Edge.Cuts) (width 0.05))
  (gr_line (start 50 40) (end 0 40) (layer Edge.Cuts) (width 0.05))
  (gr_line (start 0 40) (end 0 0) (layer Edge.Cuts) (width 0.05))
  (module Package_SO:SOIC-8 (layer F.Cu) (at 25 20 90)
    (fp_text reference "U1" (at 0 0) (layer F.SilkS))
    (pad 1 thru_hole rect (at -1.27 0) (size 1 1) (drill 0.6) (layers *.Cu))
    (pad 2 thru_hole rect (at 1.27 0) (size 1 1) (drill 0.6) (layers *.Cu))
  )
  (module Resistor_SMD:R_0402 (layer F.Cu) (at 10 10)
    (fp_text reference "R1" (at 0 0) (layer F.SilkS))
  )
  (via (at 30 15) (size 0.8) (drill 0.4) (layers F.Cu B.Cu))
)
"#;

    #[test]
    fn parse_board_outline() {
        let board = read_kicad_pcb(SAMPLE_PCB).unwrap();
        assert_eq!(board.board_outline.len(), 4);
        assert_eq!(board.board_outline[0], DVec2::new(0.0, 0.0));
    }

    #[test]
    fn parse_board_thickness() {
        let board = read_kicad_pcb(SAMPLE_PCB).unwrap();
        assert!((board.board_thickness_mm - 1.6).abs() < 1e-6);
    }

    #[test]
    fn parse_components() {
        let board = read_kicad_pcb(SAMPLE_PCB).unwrap();
        assert_eq!(board.components.len(), 2);
        assert_eq!(board.components[0].reference, "U1");
        assert_eq!(board.components[0].footprint, "Package_SO:SOIC-8");
        assert!((board.components[0].rotation - 90.0).abs() < 1e-6);
    }

    #[test]
    fn parse_component_position() {
        let board = read_kicad_pcb(SAMPLE_PCB).unwrap();
        let u1 = &board.components[0];
        assert!((u1.position.x - 25.0).abs() < 1e-6);
        assert!((u1.position.y - 20.0).abs() < 1e-6);
    }

    #[test]
    fn parse_drill_holes() {
        let board = read_kicad_pcb(SAMPLE_PCB).unwrap();
        // 2 pads with drill + 1 via = 3
        assert_eq!(board.drill_holes.len(), 3);
    }

    #[test]
    fn parse_via_drill() {
        let board = read_kicad_pcb(SAMPLE_PCB).unwrap();
        let via = board.drill_holes.iter().find(|d| (d.position.x - 30.0).abs() < 1e-6).unwrap();
        assert!((via.diameter_mm - 0.4).abs() < 1e-6);
    }

    #[test]
    fn board_extrude() {
        let board = read_kicad_pcb(SAMPLE_PCB).unwrap();
        let solid = board_to_solid(&board);
        assert!(solid.faces.len() > 0);
    }

    #[test]
    fn reject_invalid_input() {
        assert!(read_kicad_pcb("not a kicad file").is_none());
        assert!(read_kicad_pcb("").is_none());
        assert!(read_kicad_pcb("(step_file)").is_none());
    }

    #[test]
    fn parse_kicad7_property_reference() {
        let pcb = r#"
(kicad_pcb (version 20221018) (generator pcbnew)
  (general (thickness 1.2))
  (footprint "Resistor:R_0603" (layer "F.Cu") (at 5 5)
    (property "Reference" "R10")
  )
)
"#;
        let board = read_kicad_pcb(pcb).unwrap();
        assert_eq!(board.components.len(), 1);
        assert_eq!(board.components[0].reference, "R10");
        assert!((board.board_thickness_mm - 1.2).abs() < 1e-6);
    }
}
