//! `physical-emit-threemf` — 3MF (3D Manufacturing Format) export.
//!
//! Produces a minimal 3MF ZIP archive containing the model XML.
//! Uses inline ZIP writing (STORE method only) with no external dependencies.

use physical_tessellation::TessMesh;

/// Write a 3MF file from a tessellated mesh.
///
/// Returns the raw bytes of the ZIP archive. The archive contains:
/// - `[Content_Types].xml`
/// - `_rels/.rels`
/// - `3D/3dmodel.model`
pub fn write_3mf(mesh: &TessMesh, name: &str) -> Vec<u8> {
    let content_types = content_types_xml();
    let rels = rels_xml();
    let model = model_xml(mesh, name);

    let files = [
        ("[Content_Types].xml", content_types.as_bytes()),
        ("_rels/.rels", rels.as_bytes()),
        ("3D/3dmodel.model", model.as_bytes()),
    ];

    write_zip(&files)
}

// ---------------------------------------------------------------------------
// XML generation
// ---------------------------------------------------------------------------

fn content_types_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml" />
  <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml" />
</Types>"#.to_string()
}

fn rels_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Target="/3D/3dmodel.model" Id="rel0" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel" />
</Relationships>"#.to_string()
}

fn model_xml(mesh: &TessMesh, name: &str) -> String {
    let mut xml = String::with_capacity(mesh.vertices.len() * 60 + mesh.indices.len() * 20 + 512);

    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xml:lang="en-US"
       xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <resources>
    <object id="1" type="model" name=""#);
    xml.push_str(&xml_escape(name));
    xml.push_str(r#"">
      <mesh>
        <vertices>
"#);

    for v in &mesh.vertices {
        xml.push_str(&format!(
            "          <vertex x=\"{:.6}\" y=\"{:.6}\" z=\"{:.6}\" />\n",
            v.position[0], v.position[1], v.position[2]
        ));
    }

    xml.push_str("        </vertices>\n        <triangles>\n");

    for tri in mesh.indices.chunks(3) {
        if tri.len() == 3 {
            xml.push_str(&format!(
                "          <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\" />\n",
                tri[0], tri[1], tri[2]
            ));
        }
    }

    xml.push_str(r#"        </triangles>
      </mesh>
    </object>
  </resources>
  <build>
    <item objectid="1" />
  </build>
</model>"#);

    xml
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}

// ---------------------------------------------------------------------------
// Minimal ZIP writer (STORE method, no compression)
// ---------------------------------------------------------------------------

fn write_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4096);
    let mut central_entries: Vec<CentralEntry> = Vec::new();

    // Write local file headers + data
    for &(name, data) in files {
        let offset = buf.len() as u32;
        let crc = crc32(data);

        // Local file header
        buf.extend_from_slice(&0x04034b50_u32.to_le_bytes()); // signature
        buf.extend_from_slice(&20_u16.to_le_bytes());         // version needed
        buf.extend_from_slice(&0_u16.to_le_bytes());          // flags
        buf.extend_from_slice(&0_u16.to_le_bytes());          // method: STORE
        buf.extend_from_slice(&0_u16.to_le_bytes());          // mod time
        buf.extend_from_slice(&0_u16.to_le_bytes());          // mod date
        buf.extend_from_slice(&crc.to_le_bytes());            // CRC-32
        buf.extend_from_slice(&(data.len() as u32).to_le_bytes()); // compressed size
        buf.extend_from_slice(&(data.len() as u32).to_le_bytes()); // uncompressed size
        buf.extend_from_slice(&(name.len() as u16).to_le_bytes()); // file name length
        buf.extend_from_slice(&0_u16.to_le_bytes());          // extra field length

        buf.extend_from_slice(name.as_bytes());
        buf.extend_from_slice(data);

        central_entries.push(CentralEntry {
            name: name.to_string(),
            crc,
            size: data.len() as u32,
            offset,
        });
    }

    // Write central directory
    let cd_offset = buf.len() as u32;

    for entry in &central_entries {
        buf.extend_from_slice(&0x02014b50_u32.to_le_bytes()); // signature
        buf.extend_from_slice(&20_u16.to_le_bytes());         // version made by
        buf.extend_from_slice(&20_u16.to_le_bytes());         // version needed
        buf.extend_from_slice(&0_u16.to_le_bytes());          // flags
        buf.extend_from_slice(&0_u16.to_le_bytes());          // method: STORE
        buf.extend_from_slice(&0_u16.to_le_bytes());          // mod time
        buf.extend_from_slice(&0_u16.to_le_bytes());          // mod date
        buf.extend_from_slice(&entry.crc.to_le_bytes());      // CRC-32
        buf.extend_from_slice(&entry.size.to_le_bytes());     // compressed size
        buf.extend_from_slice(&entry.size.to_le_bytes());     // uncompressed size
        buf.extend_from_slice(&(entry.name.len() as u16).to_le_bytes()); // file name length
        buf.extend_from_slice(&0_u16.to_le_bytes());          // extra field length
        buf.extend_from_slice(&0_u16.to_le_bytes());          // file comment length
        buf.extend_from_slice(&0_u16.to_le_bytes());          // disk number start
        buf.extend_from_slice(&0_u16.to_le_bytes());          // internal file attrs
        buf.extend_from_slice(&0_u32.to_le_bytes());          // external file attrs
        buf.extend_from_slice(&entry.offset.to_le_bytes());   // local header offset

        buf.extend_from_slice(entry.name.as_bytes());
    }

    let cd_size = buf.len() as u32 - cd_offset;
    let entry_count = central_entries.len() as u16;

    // End of central directory record
    buf.extend_from_slice(&0x06054b50_u32.to_le_bytes()); // signature
    buf.extend_from_slice(&0_u16.to_le_bytes());          // disk number
    buf.extend_from_slice(&0_u16.to_le_bytes());          // disk with CD
    buf.extend_from_slice(&entry_count.to_le_bytes());    // entries on this disk
    buf.extend_from_slice(&entry_count.to_le_bytes());    // total entries
    buf.extend_from_slice(&cd_size.to_le_bytes());        // CD size
    buf.extend_from_slice(&cd_offset.to_le_bytes());      // CD offset
    buf.extend_from_slice(&0_u16.to_le_bytes());          // comment length

    buf
}

struct CentralEntry {
    name: String,
    crc: u32,
    size: u32,
    offset: u32,
}

/// CRC-32 (ISO 3309 / ITU-T V.42) — the same polynomial used by ZIP, PNG, etc.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use physical_tessellation::{TessMesh, TessVertex};

    fn test_mesh() -> TessMesh {
        // Simple triangle
        TessMesh {
            vertices: vec![
                TessVertex { position: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0] },
                TessVertex { position: [10.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0], uv: [1.0, 0.0] },
                TessVertex { position: [5.0, 10.0, 0.0], normal: [0.0, 0.0, 1.0], uv: [0.5, 1.0] },
            ],
            indices: vec![0, 1, 2],
        }
    }

    fn box_mesh() -> TessMesh {
        let solid = physical_brep::builder::make_box(10.0, 20.0, 30.0);
        physical_tessellation::tessellate(&solid, 1.0)
    }

    #[test]
    fn write_3mf_produces_valid_zip() {
        let mesh = test_mesh();
        let bytes = write_3mf(&mesh, "test_part");

        // ZIP files start with PK\x03\x04
        assert_eq!(&bytes[0..4], &[0x50, 0x4b, 0x03, 0x04], "should start with ZIP local header signature");

        // Should contain end-of-central-directory signature
        let eocd_sig = [0x50, 0x4b, 0x05, 0x06];
        assert!(
            bytes.windows(4).any(|w| w == eocd_sig),
            "should contain end-of-central-directory record"
        );
    }

    #[test]
    fn write_3mf_contains_required_files() {
        let mesh = test_mesh();
        let bytes = write_3mf(&mesh, "test");

        // File names appear in the ZIP
        let content = String::from_utf8_lossy(&bytes);
        assert!(content.contains("[Content_Types].xml"), "should contain Content_Types");
        assert!(content.contains("_rels/.rels"), "should contain rels");
        assert!(content.contains("3D/3dmodel.model"), "should contain model");
    }

    #[test]
    fn model_xml_contains_vertices_and_triangles() {
        let mesh = test_mesh();
        let xml = model_xml(&mesh, "part");

        assert!(xml.contains("<vertex"), "should contain vertex elements");
        assert!(xml.contains("<triangle"), "should contain triangle elements");
        assert!(xml.contains("unit=\"millimeter\""), "should specify millimeter units");
    }

    #[test]
    fn model_xml_vertex_count() {
        let mesh = test_mesh();
        let xml = model_xml(&mesh, "part");

        let vertex_count = xml.matches("<vertex x=").count();
        assert_eq!(vertex_count, 3, "triangle mesh should have 3 vertices");

        let tri_count = xml.matches("<triangle v1=").count();
        assert_eq!(tri_count, 1, "should have 1 triangle");
    }

    #[test]
    fn write_3mf_box_mesh() {
        let mesh = box_mesh();
        let bytes = write_3mf(&mesh, "box");

        // Valid ZIP
        assert_eq!(&bytes[0..4], &[0x50, 0x4b, 0x03, 0x04]);

        // Model should have real geometry from tessellator
        let xml = model_xml(&mesh, "box");
        assert!(xml.matches("<vertex x=").count() >= 8, "box needs >= 8 vertices");
        assert!(xml.matches("<triangle v1=").count() >= 12, "box needs >= 12 triangles");
    }

    #[test]
    fn crc32_known_values() {
        // CRC-32 of empty data
        assert_eq!(crc32(b""), 0x0000_0000);
        // CRC-32 of "123456789"
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn xml_escape_special_chars() {
        assert_eq!(xml_escape("a<b>c&d\"e"), "a&lt;b&gt;c&amp;d&quot;e");
    }

    #[test]
    fn write_3mf_name_escaping() {
        let mesh = test_mesh();
        let xml = model_xml(&mesh, "part<1>");
        assert!(xml.contains("part&lt;1&gt;"), "name should be XML-escaped");
    }

    #[test]
    fn write_3mf_nonzero_size() {
        let mesh = test_mesh();
        let bytes = write_3mf(&mesh, "part");
        assert!(bytes.len() > 100, "3MF should be non-trivial in size");
    }

    #[test]
    fn zip_has_three_entries() {
        let mesh = test_mesh();
        let bytes = write_3mf(&mesh, "part");

        // Find EOCD record and check entry count
        let eocd_sig = [0x50_u8, 0x4b, 0x05, 0x06];
        let eocd_pos = bytes.windows(4).position(|w| w == eocd_sig).unwrap();
        // Total entries is at offset +10 from EOCD signature
        let total_entries = u16::from_le_bytes([bytes[eocd_pos + 10], bytes[eocd_pos + 11]]);
        assert_eq!(total_entries, 3, "ZIP should contain 3 files");
    }
}
