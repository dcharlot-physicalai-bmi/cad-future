//! IDF 3.0 reader — standard ECAD-MCAD board exchange format.
//!
//! IDF (Intermediate Data Format) is used to exchange PCB board geometry
//! and component placement data between electronic and mechanical CAD systems.
//! Defined in IPC-2581 predecessor standard.

use glam::DVec3;
use serde::{Serialize, Deserialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A parsed IDF board.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdfBoard {
    pub header: IdfHeader,
    pub board_outline: Vec<IdfLoop>,
    pub component_outlines: Vec<IdfComponentOutline>,
    pub drill_holes: Vec<IdfDrillHole>,
    pub placements: Vec<IdfPlacement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdfHeader {
    pub file_type: String, // BOARD_FILE or LIBRARY_FILE
    pub idf_version: f64,
    pub source_system: String,
    pub board_name: String,
    pub unit: IdfUnit,
    pub board_thickness: f64, // mm
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdfUnit { MM, THOU }

impl IdfUnit {
    pub fn to_mm(&self, val: f64) -> f64 {
        match self { Self::MM => val, Self::THOU => val * 0.0254 }
    }
}

/// A boundary loop (board outline or cutout).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdfLoop {
    pub points: Vec<IdfPoint>,
    pub is_cutout: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct IdfPoint {
    pub x: f64,
    pub y: f64,
    pub angle: f64, // 0 = line, nonzero = arc included angle
}

/// Component outline from the library file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdfComponentOutline {
    pub geometry_name: String,
    pub part_number: String,
    pub height: f64,
    pub outline: Vec<IdfPoint>,
}

/// A drilled hole.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdfDrillHole {
    pub diameter: f64,
    pub x: f64,
    pub y: f64,
    pub plating: HolePlating,
    pub associated_part: String,
    pub hole_type: HoleType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HolePlating { PTH, NPTH }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HoleType { Pin, Via, Mounting, Tooling, Other }

/// A component placement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdfPlacement {
    pub package_name: String,
    pub part_number: String,
    pub reference_designator: String,
    pub x: f64,
    pub y: f64,
    pub mounting_offset: f64,
    pub rotation: f64,
    pub side: BoardSide,
    pub status: PlacementStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoardSide { Top, Bottom }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlacementStatus { Placed, Unplaced, Ecad, Mcad }

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Read an IDF board file (.emn).
pub fn read_idf(text: &str) -> Option<IdfBoard> {
    let mut header = IdfHeader {
        file_type: "BOARD_FILE".into(),
        idf_version: 3.0,
        source_system: String::new(),
        board_name: String::new(),
        unit: IdfUnit::MM,
        board_thickness: 1.6,
    };
    let mut board_outline = Vec::new();
    let mut drill_holes = Vec::new();
    let mut placements = Vec::new();

    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        if line == ".HEADER" {
            i += 1;
            // Line 1: file_type idf_version
            if i < lines.len() {
                let parts: Vec<&str> = lines[i].trim().split_whitespace().collect();
                if parts.len() >= 2 {
                    header.file_type = parts[0].to_string();
                    header.idf_version = parts[1].parse().unwrap_or(3.0);
                }
            }
            i += 1;
            // Line 2: source_system date version
            if i < lines.len() { header.source_system = lines[i].trim().to_string(); }
            i += 1;
            // Line 3: board_name unit
            if i < lines.len() {
                let parts: Vec<&str> = lines[i].trim().split_whitespace().collect();
                if !parts.is_empty() { header.board_name = parts[0].to_string(); }
                if parts.len() >= 2 {
                    header.unit = if parts[1] == "THOU" { IdfUnit::THOU } else { IdfUnit::MM };
                }
            }
        }

        if line == ".BOARD_OUTLINE" {
            i += 1;
            // Next line: owner thickness
            let mut thickness = 1.6;
            if i < lines.len() {
                let parts: Vec<&str> = lines[i].trim().split_whitespace().collect();
                if parts.len() >= 2 {
                    thickness = parts[1].parse().unwrap_or(1.6);
                }
            }
            header.board_thickness = header.unit.to_mm(thickness);
            i += 1;

            let mut points = Vec::new();
            while i < lines.len() && lines[i].trim() != ".END_BOARD_OUTLINE" {
                if let Some(pt) = parse_idf_point(lines[i], header.unit) {
                    points.push(pt);
                }
                i += 1;
            }
            board_outline.push(IdfLoop { points, is_cutout: false });
        }

        if line == ".DRILLED_HOLES" {
            i += 1;
            while i < lines.len() && lines[i].trim() != ".END_DRILLED_HOLES" {
                if let Some(hole) = parse_drill_hole(lines[i], header.unit) {
                    drill_holes.push(hole);
                }
                i += 1;
            }
        }

        if line == ".PLACEMENT" {
            i += 1;
            while i < lines.len() && lines[i].trim() != ".END_PLACEMENT" {
                // Placement is 2 lines per component
                if i + 1 < lines.len() {
                    if let Some(pl) = parse_placement(lines[i], lines[i + 1], header.unit) {
                        placements.push(pl);
                        i += 1; // skip second line
                    }
                }
                i += 1;
            }
        }

        i += 1;
    }

    Some(IdfBoard {
        header,
        board_outline,
        component_outlines: Vec::new(),
        drill_holes,
        placements,
    })
}

fn parse_idf_point(line: &str, unit: IdfUnit) -> Option<IdfPoint> {
    let parts: Vec<&str> = line.trim().split_whitespace().collect();
    if parts.len() >= 3 {
        let x = unit.to_mm(parts[1].parse().ok()?);
        let y = unit.to_mm(parts[2].parse().ok()?);
        let angle = if parts.len() >= 4 { parts[3].parse().unwrap_or(0.0) } else { 0.0 };
        Some(IdfPoint { x, y, angle })
    } else {
        None
    }
}

fn parse_drill_hole(line: &str, unit: IdfUnit) -> Option<IdfDrillHole> {
    let parts: Vec<&str> = line.trim().split_whitespace().collect();
    if parts.len() >= 5 {
        Some(IdfDrillHole {
            diameter: unit.to_mm(parts[0].parse().ok()?),
            x: unit.to_mm(parts[1].parse().ok()?),
            y: unit.to_mm(parts[2].parse().ok()?),
            plating: if parts[3] == "PTH" { HolePlating::PTH } else { HolePlating::NPTH },
            associated_part: parts.get(4).unwrap_or(&"").to_string(),
            hole_type: match parts.get(5).unwrap_or(&"") {
                &"PIN" => HoleType::Pin, &"VIA" => HoleType::Via,
                &"MTG" => HoleType::Mounting, &"TOOL" => HoleType::Tooling,
                _ => HoleType::Other,
            },
        })
    } else {
        None
    }
}

fn parse_placement(line1: &str, line2: &str, unit: IdfUnit) -> Option<IdfPlacement> {
    let p1: Vec<&str> = line1.trim().split_whitespace().collect();
    let p2: Vec<&str> = line2.trim().split_whitespace().collect();
    if p1.len() >= 2 && p2.len() >= 4 {
        Some(IdfPlacement {
            package_name: p1[0].to_string(),
            part_number: p1.get(1).unwrap_or(&"").to_string(),
            reference_designator: p2[0].to_string(),
            x: unit.to_mm(p2[1].parse().ok()?),
            y: unit.to_mm(p2[2].parse().ok()?),
            mounting_offset: 0.0,
            rotation: p2[3].parse().unwrap_or(0.0),
            side: if p2.get(4) == Some(&"BOTTOM") { BoardSide::Bottom } else { BoardSide::Top },
            status: PlacementStatus::Placed,
        })
    } else {
        None
    }
}

/// Convert an IDF board outline to a B-Rep solid (extruded board).
pub fn idf_to_solid(board: &IdfBoard) -> physical_brep::Solid {
    if board.board_outline.is_empty() {
        return physical_brep::builder::make_box(100.0, 100.0, board.header.board_thickness);
    }
    let outline = &board.board_outline[0];
    if outline.points.len() < 3 {
        return physical_brep::builder::make_box(100.0, 100.0, board.header.board_thickness);
    }

    // Compute bounding box of outline
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for p in &outline.points {
        min_x = min_x.min(p.x); max_x = max_x.max(p.x);
        min_y = min_y.min(p.y); max_y = max_y.max(p.y);
    }
    let w = max_x - min_x;
    let h = max_y - min_y;
    physical_brep::builder::make_box(w.max(1.0), h.max(1.0), board.header.board_thickness)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_idf() -> &'static str {
        ".HEADER\n\
         BOARD_FILE 3.0\n\
         OpenIE 2026-04-03 1.0\n\
         test_board MM\n\
         .END_HEADER\n\
         .BOARD_OUTLINE\n\
         ECAD 1.6\n\
         0 0.0 0.0 0.0\n\
         0 100.0 0.0 0.0\n\
         0 100.0 80.0 0.0\n\
         0 0.0 80.0 0.0\n\
         0 0.0 0.0 0.0\n\
         .END_BOARD_OUTLINE\n\
         .DRILLED_HOLES\n\
         1.0 5.0 5.0 PTH REF1 PIN\n\
         1.0 95.0 5.0 PTH REF2 PIN\n\
         3.2 50.0 40.0 NPTH BOARD MTG\n\
         .END_DRILLED_HOLES\n\
         .PLACEMENT\n\
         SO8 LM358\n\
         U1 25.0 40.0 0.0 TOP\n\
         QFP48 STM32\n\
         U2 60.0 40.0 45.0 TOP\n\
         .END_PLACEMENT\n"
    }

    #[test]
    fn parse_header() {
        let board = read_idf(sample_idf()).unwrap();
        assert_eq!(board.header.board_name, "test_board");
        assert_eq!(board.header.unit, IdfUnit::MM);
        assert!((board.header.board_thickness - 1.6).abs() < 0.01);
    }

    #[test]
    fn parse_board_outline() {
        let board = read_idf(sample_idf()).unwrap();
        assert_eq!(board.board_outline.len(), 1);
        assert!(board.board_outline[0].points.len() >= 4);
    }

    #[test]
    fn parse_drill_holes() {
        let board = read_idf(sample_idf()).unwrap();
        assert_eq!(board.drill_holes.len(), 3);
        assert_eq!(board.drill_holes[0].plating, HolePlating::PTH);
        assert_eq!(board.drill_holes[2].plating, HolePlating::NPTH);
        assert_eq!(board.drill_holes[2].hole_type, HoleType::Mounting);
    }

    #[test]
    fn parse_placements() {
        let board = read_idf(sample_idf()).unwrap();
        assert_eq!(board.placements.len(), 2);
        assert_eq!(board.placements[0].reference_designator, "U1");
        assert_eq!(board.placements[1].reference_designator, "U2");
        assert!((board.placements[1].rotation - 45.0).abs() < 0.01);
    }

    #[test]
    fn idf_to_solid_produces_geometry() {
        let board = read_idf(sample_idf()).unwrap();
        let solid = idf_to_solid(&board);
        assert!(solid.face_count() >= 6);
    }

    #[test]
    fn unit_conversion() {
        assert!((IdfUnit::THOU.to_mm(1000.0) - 25.4).abs() < 0.01);
        assert!((IdfUnit::MM.to_mm(25.4) - 25.4).abs() < 0.001);
    }

    #[test]
    fn empty_idf() {
        let board = read_idf(".HEADER\nBOARD_FILE 3.0\nOpenIE\ntest MM\n.END_HEADER\n").unwrap();
        assert!(board.board_outline.is_empty());
        assert!(board.drill_holes.is_empty());
    }

    #[test]
    fn side_detection() {
        let board = read_idf(sample_idf()).unwrap();
        assert_eq!(board.placements[0].side, BoardSide::Top);
    }
}
