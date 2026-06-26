//! Ruida .rd binary emitter for CO2 laser controllers.
//!
//! Ruida controllers (RDC6442, RDC6445) use a proprietary binary format
//! for laser cutting jobs. Bytes are XOR-scrambled with 0x88.
//!
//! The .rd format encodes:
//! - Machine settings (speed, power, layer assignments)
//! - Vector cutting paths (move-to, cut-to coordinates)
//! - Layer parameters (color, speed, min/max power)

use physical_mfg_toolpath::Contour;

/// Ruida coordinate scaling: 1000 steps per mm.
const STEPS_PER_MM: f64 = 1000.0;

/// XOR scramble byte used by Ruida protocol.
const SCRAMBLE_BYTE: u8 = 0x88;

/// Convert contours to Ruida .rd binary format.
///
/// Each contour becomes a vector cutting path on layer 0.
pub fn contours_to_rd(contours: &[Contour], speed_mm_s: f64, power_pct: f64) -> Vec<u8> {
    let mut data = Vec::with_capacity(contours.len() * 256);

    // File header
    emit_header(&mut data);

    // Layer 0 settings
    emit_layer_settings(&mut data, 0, speed_mm_s, power_pct);

    // Vector data for each contour
    for contour in contours {
        if contour.points.len() < 2 {
            continue;
        }

        // Move to start
        let start = contour.points[0];
        emit_move_abs(&mut data, start.x, start.y);

        // Cut to each subsequent point
        for pt in &contour.points[1..] {
            emit_cut_abs(&mut data, pt.x, pt.y);
        }

        // Close if needed
        if contour.is_closed && contour.points.len() >= 3 {
            emit_cut_abs(&mut data, start.x, start.y);
        }
    }

    // End of file
    emit_eof(&mut data);

    // Scramble all bytes
    for byte in &mut data {
        *byte ^= SCRAMBLE_BYTE;
    }

    data
}

fn emit_header(data: &mut Vec<u8>) {
    // Ruida header magic
    data.push(0xD8); // File start
    data.push(0x12); // Version indicator
}

fn emit_layer_settings(data: &mut Vec<u8>, layer: u8, speed_mm_s: f64, power_pct: f64) {
    // Layer speed command (0xC9 0x04)
    data.push(0xC9);
    data.push(0x04);
    data.push(layer);
    let speed_steps = (speed_mm_s * STEPS_PER_MM) as u32;
    emit_u32(data, speed_steps);

    // Layer min power (0xC6 0x31)
    data.push(0xC6);
    data.push(0x31);
    data.push(layer);
    let power_val = (power_pct * 163.83).min(16383.0) as u16; // 0-16383 range
    emit_u16(data, power_val);

    // Layer max power (0xC6 0x32)
    data.push(0xC6);
    data.push(0x32);
    data.push(layer);
    emit_u16(data, power_val);
}

fn emit_move_abs(data: &mut Vec<u8>, x_mm: f64, y_mm: f64) {
    data.push(0x88); // Absolute move command
    let x = (x_mm * STEPS_PER_MM) as i32;
    let y = (y_mm * STEPS_PER_MM) as i32;
    emit_coord(data, x, y);
}

fn emit_cut_abs(data: &mut Vec<u8>, x_mm: f64, y_mm: f64) {
    data.push(0xA8); // Absolute cut command
    let x = (x_mm * STEPS_PER_MM) as i32;
    let y = (y_mm * STEPS_PER_MM) as i32;
    emit_coord(data, x, y);
}

fn emit_eof(data: &mut Vec<u8>) {
    data.push(0xE7); // End of file
    data.push(0x00);
}

fn emit_coord(data: &mut Vec<u8>, x: i32, y: i32) {
    // Ruida uses 5-byte coordinate encoding for each axis
    emit_i32_5byte(data, x);
    emit_i32_5byte(data, y);
}

fn emit_i32_5byte(data: &mut Vec<u8>, val: i32) {
    // Sign bit in MSB, then 4 bytes of magnitude
    let abs_val = val.unsigned_abs();
    let sign = if val < 0 { 0x80 } else { 0x00 };
    data.push(sign | ((abs_val >> 24) as u8 & 0x7F));
    data.push((abs_val >> 16) as u8);
    data.push((abs_val >> 8) as u8);
    data.push(abs_val as u8);
}

fn emit_u32(data: &mut Vec<u8>, val: u32) {
    data.extend_from_slice(&val.to_be_bytes());
}

fn emit_u16(data: &mut Vec<u8>, val: u16) {
    data.extend_from_slice(&val.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;

    fn square() -> Contour {
        Contour::closed(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(50.0, 0.0),
            DVec2::new(50.0, 50.0),
            DVec2::new(0.0, 50.0),
        ])
    }

    #[test]
    fn produces_bytes() {
        let rd = contours_to_rd(&[square()], 20.0, 80.0);
        assert!(!rd.is_empty());
    }

    #[test]
    fn scrambled() {
        let rd = contours_to_rd(&[square()], 20.0, 80.0);
        // First byte should be header XOR scrambled
        assert_eq!(rd[0], 0xD8 ^ SCRAMBLE_BYTE);
    }

    #[test]
    fn multiple_contours() {
        let c1 = square();
        let c2 = Contour::closed(vec![
            DVec2::new(10.0, 10.0),
            DVec2::new(40.0, 10.0),
            DVec2::new(40.0, 40.0),
            DVec2::new(10.0, 40.0),
        ]);
        let rd = contours_to_rd(&[c1, c2], 15.0, 60.0);
        assert!(!rd.is_empty());
    }
}
