//! Bitmap font system with Unicode support.
//!
//! Includes a built-in 5x7 ASCII bitmap font for characters 32..=126 and a
//! [`FontAtlas`] that can register glyph bitmaps for arbitrary Unicode codepoints.

use std::collections::HashMap;

use crate::draw::UiVertex;

/// Number of columns per built-in ASCII glyph.
const GLYPH_W: u32 = 5;
/// Number of rows per built-in ASCII glyph.
const GLYPH_H: u32 = 7;

/// Bitmap font data for ASCII 32..=126 (95 glyphs).
/// Each `u64` packs 35 bits (5 columns x 7 rows), row-major from MSB.
const FONT_DATA: &[u64] = &[
    // 32 ' '
    0b00000_00000_00000_00000_00000_00000_00000,
    // 33 '!'
    0b00100_00100_00100_00100_00100_00000_00100,
    // 34 '"'
    0b01010_01010_01010_00000_00000_00000_00000,
    // 35 '#'
    0b01010_01010_11111_01010_11111_01010_01010,
    // 36 '$'
    0b00100_01111_10100_01110_00101_11110_00100,
    // 37 '%'
    0b11000_11001_00010_00100_01000_10011_00011,
    // 38 '&'
    0b01100_10010_10100_01000_10101_10010_01101,
    // 39 '\''
    0b00100_00100_00100_00000_00000_00000_00000,
    // 40 '('
    0b00010_00100_01000_01000_01000_00100_00010,
    // 41 ')'
    0b01000_00100_00010_00010_00010_00100_01000,
    // 42 '*'
    0b00000_00100_10101_01110_10101_00100_00000,
    // 43 '+'
    0b00000_00100_00100_11111_00100_00100_00000,
    // 44 ','
    0b00000_00000_00000_00000_00100_00100_01000,
    // 45 '-'
    0b00000_00000_00000_11111_00000_00000_00000,
    // 46 '.'
    0b00000_00000_00000_00000_00000_00000_00100,
    // 47 '/'
    0b00001_00010_00010_00100_01000_01000_10000,
    // 48 '0'
    0b01110_10001_10011_10101_11001_10001_01110,
    // 49 '1'
    0b00100_01100_00100_00100_00100_00100_01110,
    // 50 '2'
    0b01110_10001_00001_00010_00100_01000_11111,
    // 51 '3'
    0b01110_10001_00001_00110_00001_10001_01110,
    // 52 '4'
    0b00010_00110_01010_10010_11111_00010_00010,
    // 53 '5'
    0b11111_10000_11110_00001_00001_10001_01110,
    // 54 '6'
    0b00110_01000_10000_11110_10001_10001_01110,
    // 55 '7'
    0b11111_00001_00010_00100_01000_01000_01000,
    // 56 '8'
    0b01110_10001_10001_01110_10001_10001_01110,
    // 57 '9'
    0b01110_10001_10001_01111_00001_00010_01100,
    // 58 ':'
    0b00000_00000_00100_00000_00100_00000_00000,
    // 59 ';'
    0b00000_00000_00100_00000_00100_00100_01000,
    // 60 '<'
    0b00010_00100_01000_10000_01000_00100_00010,
    // 61 '='
    0b00000_00000_11111_00000_11111_00000_00000,
    // 62 '>'
    0b10000_01000_00100_00010_00100_01000_10000,
    // 63 '?'
    0b01110_10001_00001_00010_00100_00000_00100,
    // 64 '@'
    0b01110_10001_10111_10101_10110_10000_01110,
    // 65 'A'
    0b01110_10001_10001_11111_10001_10001_10001,
    // 66 'B'
    0b11110_10001_10001_11110_10001_10001_11110,
    // 67 'C'
    0b01110_10001_10000_10000_10000_10001_01110,
    // 68 'D'
    0b11100_10010_10001_10001_10001_10010_11100,
    // 69 'E'
    0b11111_10000_10000_11110_10000_10000_11111,
    // 70 'F'
    0b11111_10000_10000_11110_10000_10000_10000,
    // 71 'G'
    0b01110_10001_10000_10111_10001_10001_01110,
    // 72 'H'
    0b10001_10001_10001_11111_10001_10001_10001,
    // 73 'I'
    0b01110_00100_00100_00100_00100_00100_01110,
    // 74 'J'
    0b00111_00010_00010_00010_00010_10010_01100,
    // 75 'K'
    0b10001_10010_10100_11000_10100_10010_10001,
    // 76 'L'
    0b10000_10000_10000_10000_10000_10000_11111,
    // 77 'M'
    0b10001_11011_10101_10101_10001_10001_10001,
    // 78 'N'
    0b10001_11001_10101_10011_10001_10001_10001,
    // 79 'O'
    0b01110_10001_10001_10001_10001_10001_01110,
    // 80 'P'
    0b11110_10001_10001_11110_10000_10000_10000,
    // 81 'Q'
    0b01110_10001_10001_10001_10101_10010_01101,
    // 82 'R'
    0b11110_10001_10001_11110_10100_10010_10001,
    // 83 'S'
    0b01110_10001_10000_01110_00001_10001_01110,
    // 84 'T'
    0b11111_00100_00100_00100_00100_00100_00100,
    // 85 'U'
    0b10001_10001_10001_10001_10001_10001_01110,
    // 86 'V'
    0b10001_10001_10001_10001_01010_01010_00100,
    // 87 'W'
    0b10001_10001_10001_10101_10101_10101_01010,
    // 88 'X'
    0b10001_10001_01010_00100_01010_10001_10001,
    // 89 'Y'
    0b10001_10001_01010_00100_00100_00100_00100,
    // 90 'Z'
    0b11111_00001_00010_00100_01000_10000_11111,
    // 91 '['
    0b01110_01000_01000_01000_01000_01000_01110,
    // 92 '\\'
    0b10000_01000_01000_00100_00010_00010_00001,
    // 93 ']'
    0b01110_00010_00010_00010_00010_00010_01110,
    // 94 '^'
    0b00100_01010_10001_00000_00000_00000_00000,
    // 95 '_'
    0b00000_00000_00000_00000_00000_00000_11111,
    // 96 '`'
    0b01000_00100_00010_00000_00000_00000_00000,
    // 97 'a'
    0b00000_00000_01110_00001_01111_10001_01111,
    // 98 'b'
    0b10000_10000_10110_11001_10001_10001_11110,
    // 99 'c'
    0b00000_00000_01110_10000_10000_10001_01110,
    // 100 'd'
    0b00001_00001_01101_10011_10001_10001_01111,
    // 101 'e'
    0b00000_00000_01110_10001_11111_10000_01110,
    // 102 'f'
    0b00110_01001_01000_11100_01000_01000_01000,
    // 103 'g'
    0b00000_00000_01111_10001_01111_00001_01110,
    // 104 'h'
    0b10000_10000_10110_11001_10001_10001_10001,
    // 105 'i'
    0b00100_00000_01100_00100_00100_00100_01110,
    // 106 'j'
    0b00010_00000_00110_00010_00010_10010_01100,
    // 107 'k'
    0b10000_10000_10010_10100_11000_10100_10010,
    // 108 'l'
    0b01100_00100_00100_00100_00100_00100_01110,
    // 109 'm'
    0b00000_00000_11010_10101_10101_10001_10001,
    // 110 'n'
    0b00000_00000_10110_11001_10001_10001_10001,
    // 111 'o'
    0b00000_00000_01110_10001_10001_10001_01110,
    // 112 'p'
    0b00000_00000_11110_10001_11110_10000_10000,
    // 113 'q'
    0b00000_00000_01111_10001_01111_00001_00001,
    // 114 'r'
    0b00000_00000_10110_11001_10000_10000_10000,
    // 115 's'
    0b00000_00000_01110_10000_01110_00001_11110,
    // 116 't'
    0b01000_01000_11100_01000_01000_01001_00110,
    // 117 'u'
    0b00000_00000_10001_10001_10001_10011_01101,
    // 118 'v'
    0b00000_00000_10001_10001_10001_01010_00100,
    // 119 'w'
    0b00000_00000_10001_10001_10101_10101_01010,
    // 120 'x'
    0b00000_00000_10001_01010_00100_01010_10001,
    // 121 'y'
    0b00000_00000_10001_10001_01111_00001_01110,
    // 122 'z'
    0b00000_00000_11111_00010_00100_01000_11111,
    // 123 '{'
    0b00010_00100_00100_01000_00100_00100_00010,
    // 124 '|'
    0b00100_00100_00100_00100_00100_00100_00100,
    // 125 '}'
    0b01000_00100_00100_00010_00100_00100_01000,
    // 126 '~'
    0b00000_00000_01000_10101_00010_00000_00000,
];

/// A registered glyph in the font atlas.
struct AtlasGlyph {
    width: u8,
    height: u8,
    bitmap: Vec<u8>,
}

/// A font atlas that can hold glyph bitmaps for arbitrary Unicode codepoints.
///
/// ASCII glyphs (32..=126) are pre-loaded from the built-in 5x7 bitmap font.
/// Additional codepoints can be registered via [`FontAtlas::register_glyph`].
pub struct FontAtlas {
    glyphs: HashMap<char, AtlasGlyph>,
}

impl FontAtlas {
    /// Create a new font atlas with built-in ASCII glyphs pre-loaded.
    pub fn new() -> Self {
        let mut glyphs = HashMap::new();

        for code in 32u32..=126 {
            let c = char::from_u32(code).unwrap();
            let bitmap_bits = FONT_DATA[(code - 32) as usize];
            let mut bitmap = Vec::with_capacity((GLYPH_W * GLYPH_H) as usize);
            for row in 0..GLYPH_H {
                for col in 0..GLYPH_W {
                    let bit_index = (GLYPH_H - 1 - row) * GLYPH_W + (GLYPH_W - 1 - col);
                    let pixel = if (bitmap_bits >> bit_index) & 1 == 1 {
                        255
                    } else {
                        0
                    };
                    bitmap.push(pixel);
                }
            }
            glyphs.insert(c, AtlasGlyph {
                width: GLYPH_W as u8,
                height: GLYPH_H as u8,
                bitmap,
            });
        }

        Self { glyphs }
    }

    /// Register a glyph bitmap for a Unicode codepoint.
    pub fn register_glyph(&mut self, codepoint: char, width: u8, height: u8, bitmap: Vec<u8>) {
        self.glyphs.insert(codepoint, AtlasGlyph {
            width,
            height,
            bitmap,
        });
    }

    /// Check whether a glyph is available for the given character.
    pub fn has_glyph(&self, c: char) -> bool {
        self.glyphs.contains_key(&c)
    }
}

impl Default for FontAtlas {
    fn default() -> Self {
        Self::new()
    }
}

/// Look up the 5x7 bitmap for an ASCII character. Returns 0 (blank) for unsupported chars.
fn ascii_char_bitmap(c: char) -> u64 {
    let idx = c as u32;
    if (32..=126).contains(&idx) {
        FONT_DATA[(idx - 32) as usize]
    } else {
        0
    }
}

/// Emit quads for a single character using the built-in ASCII 5x7 font.
/// Returns the advance width.
fn emit_ascii_char_quads(
    c: char,
    x: f32,
    y: f32,
    size: f32,
    color: [f32; 4],
    vertices: &mut Vec<UiVertex>,
    indices: &mut Vec<u32>,
) -> f32 {
    let bitmap = ascii_char_bitmap(c);
    let cell = size / GLYPH_H as f32;
    for row in 0..GLYPH_H {
        for col in 0..GLYPH_W {
            let bit_index = (GLYPH_H - 1 - row) * GLYPH_W + (GLYPH_W - 1 - col);
            if (bitmap >> bit_index) & 1 == 1 {
                let px = x + col as f32 * cell;
                let py = y + row as f32 * cell;
                let base = vertices.len() as u32;
                vertices.push(UiVertex { pos: [px, py], uv: [0.0, 0.0], color });
                vertices.push(UiVertex { pos: [px + cell, py], uv: [1.0, 0.0], color });
                vertices.push(UiVertex { pos: [px + cell, py + cell], uv: [1.0, 1.0], color });
                vertices.push(UiVertex { pos: [px, py + cell], uv: [0.0, 1.0], color });
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
    }
    GLYPH_W as f32 * cell + cell
}

/// Parameters for emitting character quads.
pub struct CharQuadParams<'a> {
    /// The character to render.
    pub c: char,
    /// X position in pixels.
    pub x: f32,
    /// Y position in pixels.
    pub y: f32,
    /// Pixel height of the rendered glyph.
    pub size: f32,
    /// RGBA color.
    pub color: [f32; 4],
    /// Optional font atlas for Unicode glyph lookup.
    pub atlas: Option<&'a FontAtlas>,
}

/// Emit quads for a single character using the font atlas, falling back to
/// the built-in ASCII 5x7 bitmap for characters 32..=126 when no atlas glyph
/// is registered. Returns the advance width.
pub fn emit_char_quads(
    params: &CharQuadParams<'_>,
    vertices: &mut Vec<UiVertex>,
    indices: &mut Vec<u32>,
) -> f32 {
    let CharQuadParams { c, x, y, size, color, atlas } = *params;
    // Try atlas glyph first (for non-ASCII or overridden glyphs)
    if let Some(glyph) = atlas.and_then(|a| a.glyphs.get(&c)) {
        let cell = size / glyph.height as f32;
        let w = glyph.width as u32;
        let h = glyph.height as u32;
        for row in 0..h {
            for col in 0..w {
                let idx = (row * w + col) as usize;
                if idx < glyph.bitmap.len() && glyph.bitmap[idx] > 0 {
                    let px = x + col as f32 * cell;
                    let py = y + row as f32 * cell;
                    let base = vertices.len() as u32;
                    vertices.push(UiVertex { pos: [px, py], uv: [0.0, 0.0], color });
                    vertices.push(UiVertex { pos: [px + cell, py], uv: [1.0, 0.0], color });
                    vertices.push(UiVertex { pos: [px + cell, py + cell], uv: [1.0, 1.0], color });
                    vertices.push(UiVertex { pos: [px, py + cell], uv: [0.0, 1.0], color });
                    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                }
            }
        }
        return glyph.width as f32 * cell + cell;
    }

    // Fall back to built-in ASCII bitmap
    emit_ascii_char_quads(c, x, y, size, color, vertices, indices)
}

/// Measure the pixel width of a string rendered at the given size.
pub fn measure_text(text: &str, size: f32, atlas: Option<&FontAtlas>) -> f32 {
    let count = text.chars().count();
    if count == 0 {
        return 0.0;
    }

    if let Some(atlas) = atlas {
        let mut total = 0.0f32;
        let mut char_count = 0usize;
        for c in text.chars() {
            let (w, h) = if let Some(glyph) = atlas.glyphs.get(&c) {
                (glyph.width as f32, glyph.height as f32)
            } else {
                (GLYPH_W as f32, GLYPH_H as f32)
            };
            let cell = size / h;
            total += w * cell + cell;
            char_count += 1;
        }
        if char_count > 0 {
            let last_char = text.chars().last().unwrap();
            let h = if let Some(glyph) = atlas.glyphs.get(&last_char) {
                glyph.height as f32
            } else {
                GLYPH_H as f32
            };
            total -= size / h;
        }
        total
    } else {
        let cell = size / GLYPH_H as f32;
        let char_advance = GLYPH_W as f32 * cell + cell;
        char_advance * count as f32 - cell
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_bitmap_lookup() {
        assert_ne!(ascii_char_bitmap('A'), 0);
        assert_eq!(ascii_char_bitmap('\u{200}'), 0);
    }

    #[test]
    fn font_atlas_has_ascii() {
        let atlas = FontAtlas::new();
        assert!(atlas.has_glyph('A'));
        assert!(atlas.has_glyph(' '));
        assert!(atlas.has_glyph('~'));
        assert!(!atlas.has_glyph('\u{4e00}'));
    }

    #[test]
    fn font_atlas_register_custom_glyph() {
        let mut atlas = FontAtlas::new();
        let bitmap = vec![255; 8 * 8];
        atlas.register_glyph('\u{4e00}', 8, 8, bitmap);
        assert!(atlas.has_glyph('\u{4e00}'));
    }

    #[test]
    fn measure_text_empty() {
        assert_eq!(measure_text("", 14.0, None), 0.0);
    }

    #[test]
    fn measure_text_ascii_no_atlas() {
        let w = measure_text("Hi", 14.0, None);
        assert!(w > 0.0);
    }

    #[test]
    fn measure_text_with_atlas() {
        let atlas = FontAtlas::new();
        let w = measure_text("Hi", 14.0, Some(&atlas));
        assert!(w > 0.0);
    }

    #[test]
    fn emit_char_quads_ascii_no_atlas() {
        let mut verts = Vec::new();
        let mut idxs = Vec::new();
        let params = CharQuadParams {
            c: 'A', x: 0.0, y: 0.0, size: 14.0, color: [1.0; 4], atlas: None,
        };
        let advance = emit_char_quads(&params, &mut verts, &mut idxs);
        assert!(advance > 0.0);
        assert!(!verts.is_empty());
    }

    #[test]
    fn emit_char_quads_with_atlas_custom_glyph() {
        let mut atlas = FontAtlas::new();
        atlas.register_glyph('\u{4e00}', 2, 2, vec![255, 0, 0, 255]);
        let mut verts = Vec::new();
        let mut idxs = Vec::new();
        let params = CharQuadParams {
            c: '\u{4e00}', x: 0.0, y: 0.0, size: 14.0, color: [1.0; 4], atlas: Some(&atlas),
        };
        let advance = emit_char_quads(&params, &mut verts, &mut idxs);
        assert!(advance > 0.0);
        assert_eq!(verts.len(), 8);
    }
}
