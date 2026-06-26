//! UFP emitter — UltiMaker File Package format.
//!
//! UFP is a ZIP archive containing G-code and optional thumbnails,
//! used by UltiMaker S-series and Method printers.
//!
//! Structure:
//! ```text
//! /3D/model.gcode
//! /Metadata/thumbnail.png (optional)
//! ```

use physical_mfg_toolpath::gcode::GCodeProgram;
use physical_mfg_toolpath::post;
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// Convert a GCodeProgram to UFP ZIP bytes.
pub fn gcode_to_ufp(program: &GCodeProgram) -> Vec<u8> {
    gcode_to_ufp_with_dialect(program, &post::marlin())
}

/// Convert a GCodeProgram to UFP with a specific G-code dialect.
pub fn gcode_to_ufp_with_dialect(
    program: &GCodeProgram,
    dialect: &physical_mfg_toolpath::post::GCodeDialect,
) -> Vec<u8> {
    let gcode = program.to_string(dialect);
    text_to_ufp(&gcode)
}

/// Convert raw G-code text to UFP ZIP bytes.
pub fn text_to_ufp(gcode: &str) -> Vec<u8> {
    let buf = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(buf);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // G-code file
    zip.start_file("/3D/model.gcode", options).unwrap();
    zip.write_all(gcode.as_bytes()).unwrap();

    // Metadata (minimal)
    zip.start_file("/Metadata/UFP_Global.json", options).unwrap();
    write!(
        zip,
        r#"{{"application":"OpenIE CAD","version":"0.1.0"}}"#
    )
    .unwrap();

    let cursor = zip.finish().unwrap();
    cursor.into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_valid_zip() {
        let ufp = text_to_ufp("G28\nG1 X10 Y10\n");
        let cursor = std::io::Cursor::new(&ufp);
        let mut archive = zip::ZipArchive::new(cursor).expect("valid ZIP");
        assert!(archive.by_name("/3D/model.gcode").is_ok());
    }

    #[test]
    fn contains_gcode() {
        let ufp = text_to_ufp("G28\nG1 X10 Y10\n");
        let cursor = std::io::Cursor::new(&ufp);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        let mut gcode_file = archive.by_name("/3D/model.gcode").unwrap();
        let mut contents = String::new();
        std::io::Read::read_to_string(&mut gcode_file, &mut contents).unwrap();
        assert!(contents.contains("G28"));
        assert!(contents.contains("G1 X10"));
    }
}
