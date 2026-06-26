//! Gerber RS-274X reader/writer — universal PCB fabrication format.
//!
//! Parses Gerber commands (D01 draw, D02 move, D03 flash, G01/G02/G03 interpolation)
//! and aperture definitions (%ADD...). Extracts traces, flashes, and board outlines.

use serde::{Serialize, Deserialize};

/// A 2D point in Gerber coordinates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GPoint {
    pub x: f64,
    pub y: f64,
}

/// Aperture shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApertureShape { Circle, Rectangle, Obround, Polygon }

/// An aperture definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aperture {
    pub code: u32,
    pub shape: ApertureShape,
    pub params: Vec<f64>, // diameter, width, height, etc.
}

/// A trace (drawn line segment).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GerberTrace {
    pub start: GPoint,
    pub end: GPoint,
    pub aperture_code: u32,
    pub interpolation: Interpolation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Interpolation { Linear, ClockwiseArc, CounterClockwiseArc }

/// A flash (pad placement).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GerberFlash {
    pub position: GPoint,
    pub aperture_code: u32,
}

/// Polarity of the layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Polarity { Dark, Clear }

/// A parsed Gerber layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GerberLayer {
    pub apertures: Vec<Aperture>,
    pub traces: Vec<GerberTrace>,
    pub flashes: Vec<GerberFlash>,
    pub polarity: Polarity,
    pub unit: GerberUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GerberUnit { Inch, Millimeter }

/// Parse a Gerber RS-274X file.
pub fn read_gerber(text: &str) -> Option<GerberLayer> {
    let mut apertures = Vec::new();
    let mut traces = Vec::new();
    let mut flashes = Vec::new();
    let mut polarity = Polarity::Dark;
    let mut unit = GerberUnit::Millimeter;
    let mut current_x = 0.0_f64;
    let mut current_y = 0.0_f64;
    let mut current_aperture = 10u32;
    let mut interp = Interpolation::Linear;
    let mut coord_format = (2, 4); // integer, decimal digits

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }

        // Unit mode
        if line.contains("%MOIN*%") { unit = GerberUnit::Inch; }
        if line.contains("%MOMM*%") { unit = GerberUnit::Millimeter; }

        // Coordinate format
        if line.starts_with("%FSLAX") || line.starts_with("%FSLA") {
            // %FSLAX24Y24*%
            if let Some(rest) = line.strip_prefix("%FSLAX") {
                let digits: Vec<char> = rest.chars().take(2).collect();
                if digits.len() == 2 {
                    coord_format = (
                        digits[0].to_digit(10).unwrap_or(2) as usize,
                        digits[1].to_digit(10).unwrap_or(4) as usize,
                    );
                }
            }
        }

        // Aperture definition: %ADD10C,0.1*%
        if line.starts_with("%ADD") {
            if let Some(ap) = parse_aperture_def(line) {
                apertures.push(ap);
            }
        }

        // Polarity
        if line.contains("%LPD*%") { polarity = Polarity::Dark; }
        if line.contains("%LPC*%") { polarity = Polarity::Clear; }

        // Interpolation mode
        if line.starts_with("G01") || line.contains("G01") { interp = Interpolation::Linear; }
        if line.starts_with("G02") || line.contains("G02") { interp = Interpolation::ClockwiseArc; }
        if line.starts_with("G03") || line.contains("G03") { interp = Interpolation::CounterClockwiseArc; }

        // Tool select: D10, D11, etc. (but not D01/D02/D03)
        if let Some(d) = extract_d_code(line) {
            if d >= 10 { current_aperture = d; }
        }

        // Coordinate + operation
        let (new_x, new_y) = parse_coordinates(line, current_x, current_y, coord_format);

        // D01 = draw (interpolate)
        if line.contains("D01") {
            traces.push(GerberTrace {
                start: GPoint { x: current_x, y: current_y },
                end: GPoint { x: new_x, y: new_y },
                aperture_code: current_aperture,
                interpolation: interp,
            });
            current_x = new_x;
            current_y = new_y;
        }
        // D02 = move
        else if line.contains("D02") {
            current_x = new_x;
            current_y = new_y;
        }
        // D03 = flash
        else if line.contains("D03") {
            current_x = new_x;
            current_y = new_y;
            flashes.push(GerberFlash {
                position: GPoint { x: current_x, y: current_y },
                aperture_code: current_aperture,
            });
        }
    }

    Some(GerberLayer { apertures, traces, flashes, polarity, unit })
}

fn parse_aperture_def(line: &str) -> Option<Aperture> {
    // %ADD10C,0.1*% or %ADD11R,0.5X0.3*%
    let inner = line.trim_start_matches('%').trim_end_matches("*%").trim_end_matches('*');
    let inner = inner.strip_prefix("ADD")?;
    // Code + shape + params
    let code_end = inner.find(|c: char| !c.is_ascii_digit())?;
    let code: u32 = inner[..code_end].parse().ok()?;
    let rest = &inner[code_end..];
    let shape_char = rest.chars().next()?;
    let shape = match shape_char {
        'C' => ApertureShape::Circle,
        'R' => ApertureShape::Rectangle,
        'O' => ApertureShape::Obround,
        'P' => ApertureShape::Polygon,
        _ => return None,
    };
    let params_str = rest.get(2..)?.trim_start_matches(',');
    let params: Vec<f64> = params_str.split('X')
        .filter_map(|s| s.parse().ok())
        .collect();
    Some(Aperture { code, shape, params })
}

fn extract_d_code(line: &str) -> Option<u32> {
    // Find Dnn where nn >= 10 (tool select, not operation)
    if let Some(pos) = line.rfind('D') {
        let after = &line[pos + 1..];
        let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(d) = num_str.parse::<u32>() {
            if d >= 10 { return Some(d); }
        }
    }
    None
}

fn parse_coordinates(line: &str, cur_x: f64, cur_y: f64, fmt: (usize, usize)) -> (f64, f64) {
    let mut x = cur_x;
    let mut y = cur_y;
    let divisor = 10.0_f64.powi(fmt.1 as i32);

    if let Some(xpos) = line.find('X') {
        let after = &line[xpos + 1..];
        let num_str: String = after.chars().take_while(|c| c.is_ascii_digit() || *c == '-' || *c == '+').collect();
        if let Ok(v) = num_str.parse::<f64>() { x = v / divisor; }
    }
    if let Some(ypos) = line.find('Y') {
        let after = &line[ypos + 1..];
        let num_str: String = after.chars().take_while(|c| c.is_ascii_digit() || *c == '-' || *c == '+').collect();
        if let Ok(v) = num_str.parse::<f64>() { y = v / divisor; }
    }
    (x, y)
}

/// Compute bounding box of a Gerber layer.
pub fn gerber_bounding_box(layer: &GerberLayer) -> (GPoint, GPoint) {
    let mut min = GPoint { x: f64::INFINITY, y: f64::INFINITY };
    let mut max = GPoint { x: f64::NEG_INFINITY, y: f64::NEG_INFINITY };
    for t in &layer.traces {
        for p in [&t.start, &t.end] {
            min.x = min.x.min(p.x); min.y = min.y.min(p.y);
            max.x = max.x.max(p.x); max.y = max.y.max(p.y);
        }
    }
    for f in &layer.flashes {
        min.x = min.x.min(f.position.x); min.y = min.y.min(f.position.y);
        max.x = max.x.max(f.position.x); max.y = max.y.max(f.position.y);
    }
    (min, max)
}

/// Write a Gerber layer to RS-274X text.
pub fn write_gerber(layer: &GerberLayer) -> String {
    let mut out = String::new();
    out.push_str("G04 Generated by OpenIE*\n");
    out.push_str("%MOMM*%\n");
    out.push_str("%FSLAX24Y24*%\n");

    for ap in &layer.apertures {
        let shape = match ap.shape {
            ApertureShape::Circle => 'C',
            ApertureShape::Rectangle => 'R',
            ApertureShape::Obround => 'O',
            ApertureShape::Polygon => 'P',
        };
        let params_str = ap.params.iter().map(|p| format!("{p}")).collect::<Vec<_>>().join("X");
        out.push_str(&format!("%ADD{}{},{params_str}*%\n", ap.code, shape));
    }

    if !layer.traces.is_empty() || !layer.flashes.is_empty() {
        out.push_str(&format!("D{}*\n", layer.apertures.first().map(|a| a.code).unwrap_or(10)));
    }

    for trace in &layer.traces {
        let sx = (trace.start.x * 10000.0) as i64;
        let sy = (trace.start.y * 10000.0) as i64;
        let ex = (trace.end.x * 10000.0) as i64;
        let ey = (trace.end.y * 10000.0) as i64;
        out.push_str(&format!("X{sx}Y{sy}D02*\n"));
        out.push_str(&format!("X{ex}Y{ey}D01*\n"));
    }

    for flash in &layer.flashes {
        let fx = (flash.position.x * 10000.0) as i64;
        let fy = (flash.position.y * 10000.0) as i64;
        out.push_str(&format!("X{fx}Y{fy}D03*\n"));
    }

    out.push_str("M02*\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_gerber() -> &'static str {
        "G04 Test*\n%MOMM*%\n%FSLAX24Y24*%\n%ADD10C,0.1*%\nD10*\nX10000Y20000D02*\nX30000Y20000D01*\nX30000Y40000D01*\nX20000Y30000D03*\nM02*\n"
    }

    #[test]
    fn parse_basic_gerber() {
        let layer = read_gerber(sample_gerber()).unwrap();
        assert!(!layer.traces.is_empty());
        assert!(!layer.flashes.is_empty());
    }

    #[test]
    fn parse_aperture() {
        let layer = read_gerber(sample_gerber()).unwrap();
        assert_eq!(layer.apertures.len(), 1);
        assert_eq!(layer.apertures[0].code, 10);
        assert_eq!(layer.apertures[0].shape, ApertureShape::Circle);
    }

    #[test]
    fn parse_traces() {
        let layer = read_gerber(sample_gerber()).unwrap();
        assert_eq!(layer.traces.len(), 2);
    }

    #[test]
    fn parse_flashes() {
        let layer = read_gerber(sample_gerber()).unwrap();
        assert_eq!(layer.flashes.len(), 1);
        assert!((layer.flashes[0].position.x - 2.0).abs() < 0.01);
    }

    #[test]
    fn bounding_box() {
        let layer = read_gerber(sample_gerber()).unwrap();
        let (min, max) = gerber_bounding_box(&layer);
        assert!(min.x < max.x);
        assert!(min.y < max.y);
    }

    #[test]
    fn unit_detection() {
        let layer = read_gerber(sample_gerber()).unwrap();
        assert_eq!(layer.unit, GerberUnit::Millimeter);
    }

    #[test]
    fn write_gerber_roundtrip() {
        let layer = read_gerber(sample_gerber()).unwrap();
        let text = write_gerber(&layer);
        assert!(text.contains("D01"));
        assert!(text.contains("D03"));
        assert!(text.contains("M02"));
    }

    #[test]
    fn empty_gerber() {
        let layer = read_gerber("G04 empty*\nM02*\n").unwrap();
        assert!(layer.traces.is_empty());
        assert!(layer.flashes.is_empty());
    }
}
