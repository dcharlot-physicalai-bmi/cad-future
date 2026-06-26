//! Binary G-code emitter — PrusaSlicer .bgcode format.
//!
//! Binary G-code is a compressed format that reduces file size and upload time.
//! Used by PrusaLink and PrusaSlicer for Prusa MK4, MK3.9, XL.
//!
//! Format: magic header + blocks of deflate-compressed G-code lines.

use flate2::write::DeflateEncoder;
use flate2::Compression;
use physical_mfg_toolpath::gcode::GCodeProgram;
use physical_mfg_toolpath::post;
use std::io::Write;

/// Binary G-code magic bytes.
const BGCODE_MAGIC: &[u8] = b"BGCODE";
/// Version 1.
const BGCODE_VERSION: u32 = 1;

/// Block types in binary G-code.
#[repr(u16)]
enum BlockType {
    FileMetadata = 0,
    GCode = 1,
    // Thumbnail = 2,
    // PrinterMetadata = 3,
    // PrintMetadata = 4,
    // SlicerMetadata = 5,
}

/// Compression types.
#[repr(u16)]
enum CompressionType {
    None = 0,
    Deflate = 1,
}

/// Convert a GCodeProgram to binary G-code bytes.
pub fn gcode_to_bgcode(program: &GCodeProgram) -> Vec<u8> {
    let gcode_text = program.to_string(&post::marlin());
    text_to_bgcode(&gcode_text)
}

/// Convert G-code text to binary G-code bytes.
pub fn text_to_bgcode(gcode: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(gcode.len());

    // Magic header
    out.extend_from_slice(BGCODE_MAGIC);
    out.extend_from_slice(&BGCODE_VERSION.to_le_bytes());

    // File metadata block (minimal)
    write_block(
        &mut out,
        BlockType::FileMetadata,
        CompressionType::None,
        b"Producer=OpenIE CAD\n",
    );

    // G-code block (compressed)
    let compressed = deflate_compress(gcode.as_bytes());
    write_block_compressed(
        &mut out,
        BlockType::GCode,
        CompressionType::Deflate,
        gcode.len() as u32,
        &compressed,
    );

    out
}

fn write_block(out: &mut Vec<u8>, block_type: BlockType, compression: CompressionType, data: &[u8]) {
    out.extend_from_slice(&(block_type as u16).to_le_bytes());
    out.extend_from_slice(&(compression as u16).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes()); // uncompressed size
    out.extend_from_slice(&(data.len() as u32).to_le_bytes()); // compressed size (same if uncompressed)
    out.extend_from_slice(data);
}

fn write_block_compressed(
    out: &mut Vec<u8>,
    block_type: BlockType,
    compression: CompressionType,
    uncompressed_size: u32,
    compressed_data: &[u8],
) {
    out.extend_from_slice(&(block_type as u16).to_le_bytes());
    out.extend_from_slice(&(compression as u16).to_le_bytes());
    out.extend_from_slice(&uncompressed_size.to_le_bytes());
    out.extend_from_slice(&(compressed_data.len() as u32).to_le_bytes());
    out.extend_from_slice(compressed_data);
}

fn deflate_compress(data: &[u8]) -> Vec<u8> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_header() {
        let bgcode = text_to_bgcode("G28\nG1 X10 Y10 F1000\n");
        assert!(bgcode.starts_with(b"BGCODE"));
    }

    #[test]
    fn compressed_smaller() {
        let gcode = "G1 X10.000 Y10.000 Z0.200 E0.500 F1500\n".repeat(1000);
        let bgcode = text_to_bgcode(&gcode);
        assert!(
            bgcode.len() < gcode.len(),
            "bgcode ({}) should be smaller than gcode ({})",
            bgcode.len(),
            gcode.len()
        );
    }
}
