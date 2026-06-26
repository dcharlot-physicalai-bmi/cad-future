//! Annotation tools — text callouts, arrows, and revision clouds for markup.
//!
//! Inspired by SolidWorks annotations, Fusion 360 markup tools,
//! and PDF annotation conventions. Used for design review, notes,
//! and non-geometric documentation overlays.

use crate::draw::DrawList;
use crate::font;
use glam::{Mat4, Vec4};

/// Type of annotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnotationType {
    /// Text callout with optional leader line.
    Callout,
    /// Arrow pointing from start to end.
    Arrow,
    /// Revision cloud (rectangle outline with wavy edges).
    RevisionCloud,
    /// Simple text note (no leader).
    Note,
    /// Balloon (circle with number).
    Balloon,
}

/// A single annotation.
#[derive(Clone, Debug)]
pub struct Annotation {
    /// Annotation type.
    pub kind: AnnotationType,
    /// Text content.
    pub text: String,
    /// Start point (world coords for 3D, screen coords for 2D).
    pub start: [f32; 3],
    /// End point (for arrows, leader line target).
    pub end: [f32; 3],
    /// Color.
    pub color: [f32; 4],
    /// Whether this annotation is in 3D space or screen space.
    pub world_space: bool,
    /// Whether selected.
    pub selected: bool,
    /// Author name (for review annotations).
    pub author: String,
    /// Balloon number (for Balloon type).
    pub balloon_number: u32,
}

impl Annotation {
    pub fn callout(text: &str, anchor: [f32; 3], leader_target: [f32; 3]) -> Self {
        Self {
            kind: AnnotationType::Callout,
            text: text.to_string(),
            start: anchor,
            end: leader_target,
            color: [1.0, 0.85, 0.2, 0.95],
            world_space: true,
            selected: false,
            author: String::new(),
            balloon_number: 0,
        }
    }

    pub fn arrow(start: [f32; 3], end: [f32; 3]) -> Self {
        Self {
            kind: AnnotationType::Arrow,
            text: String::new(),
            start,
            end,
            color: [1.0, 0.3, 0.3, 0.9],
            world_space: true,
            selected: false,
            author: String::new(),
            balloon_number: 0,
        }
    }

    pub fn revision_cloud(min: [f32; 3], max: [f32; 3], text: &str) -> Self {
        Self {
            kind: AnnotationType::RevisionCloud,
            text: text.to_string(),
            start: min,
            end: max,
            color: [0.8, 0.2, 0.2, 0.8],
            world_space: true,
            selected: false,
            author: String::new(),
            balloon_number: 0,
        }
    }

    pub fn note(position: [f32; 3], text: &str) -> Self {
        Self {
            kind: AnnotationType::Note,
            text: text.to_string(),
            start: position,
            end: position,
            color: [0.3, 0.8, 1.0, 0.9],
            world_space: true,
            selected: false,
            author: String::new(),
            balloon_number: 0,
        }
    }

    pub fn balloon(position: [f32; 3], number: u32) -> Self {
        Self {
            kind: AnnotationType::Balloon,
            text: number.to_string(),
            start: position,
            end: position,
            color: [0.2, 0.7, 0.2, 0.9],
            world_space: true,
            selected: false,
            author: String::new(),
            balloon_number: number,
        }
    }

    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }

    pub fn with_author(mut self, author: &str) -> Self {
        self.author = author.to_string();
        self
    }

    pub fn screen_space(mut self) -> Self {
        self.world_space = false;
        self
    }
}

/// The annotation overlay system.
pub struct AnnotationTools {
    /// All annotations.
    pub annotations: Vec<Annotation>,
    /// Whether annotations are visible.
    pub visible: bool,
    /// Hovered annotation index.
    pub hovered: Option<usize>,
    /// Selected annotation index.
    pub selected: Option<usize>,
    /// Current drawing tool (None = not drawing).
    pub active_tool: Option<AnnotationType>,
    /// Drawing in progress: start point set, waiting for end.
    pub draw_start: Option<[f32; 3]>,
}

impl AnnotationTools {
    pub fn new() -> Self {
        Self {
            annotations: Vec::new(),
            visible: true,
            hovered: None,
            selected: None,
            active_tool: None,
            draw_start: None,
        }
    }

    /// Add an annotation.
    pub fn add(&mut self, annotation: Annotation) {
        self.annotations.push(annotation);
    }

    /// Remove an annotation by index.
    pub fn remove(&mut self, idx: usize) {
        if idx < self.annotations.len() {
            self.annotations.remove(idx);
        }
    }

    /// Clear all annotations.
    pub fn clear(&mut self) {
        self.annotations.clear();
    }

    /// Toggle visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Begin drawing with a tool.
    pub fn begin_tool(&mut self, tool: AnnotationType) {
        self.active_tool = Some(tool);
        self.draw_start = None;
    }

    /// Cancel current drawing operation.
    pub fn cancel_tool(&mut self) {
        self.active_tool = None;
        self.draw_start = None;
    }

    /// Count annotations.
    pub fn count(&self) -> usize {
        self.annotations.len()
    }

    /// Project a 3D point to screen.
    fn project(pos: [f32; 3], vp: Mat4, sw: f32, sh: f32) -> Option<(f32, f32)> {
        let clip = vp * Vec4::new(pos[0], pos[1], pos[2], 1.0);
        if clip.w <= 0.0 { return None; }
        let ndc = clip.truncate() / clip.w;
        Some((
            (ndc.x * 0.5 + 0.5) * sw,
            (1.0 - (ndc.y * 0.5 + 0.5)) * sh,
        ))
    }

    /// Hit test: returns annotation index if mouse is near.
    pub fn hit_test(
        &self, mx: f32, my: f32,
        vp: Mat4, sw: f32, sh: f32,
    ) -> Option<usize> {
        if !self.visible { return None; }

        for (i, ann) in self.annotations.iter().enumerate() {
            let (sx, sy) = if ann.world_space {
                match Self::project(ann.start, vp, sw, sh) {
                    Some(p) => p,
                    None => continue,
                }
            } else {
                (ann.start[0], ann.start[1])
            };

            let hit_size = 24.0;
            if mx >= sx - hit_size && mx <= sx + hit_size
                && my >= sy - hit_size && my <= sy + hit_size
            {
                return Some(i);
            }
        }
        None
    }

    /// Draw all annotations.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        vp: Mat4,
        sw: f32,
        sh: f32,
    ) {
        if !self.visible { return; }

        for (i, ann) in self.annotations.iter().enumerate() {
            let color = if ann.selected || self.selected == Some(i) {
                [1.0, 1.0, 1.0, 1.0]
            } else if self.hovered == Some(i) {
                [ann.color[0] + 0.2, ann.color[1] + 0.2, ann.color[2] + 0.2, 1.0]
            } else {
                ann.color
            };

            match ann.kind {
                AnnotationType::Callout => self.draw_callout(dl, ann, vp, sw, sh, color),
                AnnotationType::Arrow => self.draw_arrow(dl, ann, vp, sw, sh, color),
                AnnotationType::RevisionCloud => self.draw_cloud(dl, ann, vp, sw, sh, color),
                AnnotationType::Note => self.draw_note(dl, ann, vp, sw, sh, color),
                AnnotationType::Balloon => self.draw_balloon(dl, ann, vp, sw, sh, color),
            }
        }
    }

    fn resolve_screen(&self, pos: [f32; 3], world: bool, vp: Mat4, sw: f32, sh: f32) -> Option<(f32, f32)> {
        if world {
            Self::project(pos, vp, sw, sh)
        } else {
            Some((pos[0], pos[1]))
        }
    }

    fn draw_callout(&self, dl: &mut DrawList, ann: &Annotation, vp: Mat4, sw: f32, sh: f32, color: [f32; 4]) {
        let Some((sx, sy)) = self.resolve_screen(ann.start, ann.world_space, vp, sw, sh) else { return };
        let Some((ex, ey)) = self.resolve_screen(ann.end, ann.world_space, vp, sw, sh) else { return };

        // Leader line
        let dx = (ex - sx).abs();
        let dy = (ey - sy).abs();
        if dx > dy {
            let min_x = sx.min(ex);
            dl.push_quad(min_x, sy, dx, 1.0, color);
        } else {
            let min_y = sy.min(ey);
            dl.push_quad(sx, min_y, 1.0, dy, color);
        }

        // Arrowhead at end
        dl.push_quad(ex - 3.0, ey - 3.0, 6.0, 6.0, color);

        // Text box at start
        let tw = font::measure_text(&ann.text, 11.0, None);
        let pad = 4.0;
        dl.push_quad(sx - pad, sy - 14.0, tw + pad * 2.0, 18.0, [0.0, 0.0, 0.0, 0.75]);
        dl.push_quad(sx - pad, sy - 14.0, tw + pad * 2.0, 2.0, color);
        emit_text(dl, &ann.text, sx, sy - 12.0, 11.0, color);
    }

    fn draw_arrow(&self, dl: &mut DrawList, ann: &Annotation, vp: Mat4, sw: f32, sh: f32, color: [f32; 4]) {
        let Some((sx, sy)) = self.resolve_screen(ann.start, ann.world_space, vp, sw, sh) else { return };
        let Some((ex, ey)) = self.resolve_screen(ann.end, ann.world_space, vp, sw, sh) else { return };

        // Shaft
        let dx = (ex - sx).abs();
        let dy = (ey - sy).abs();
        if dx > dy {
            let min_x = sx.min(ex);
            dl.push_quad(min_x, sy - 1.0, dx, 2.0, color);
        } else {
            let min_y = sy.min(ey);
            dl.push_quad(sx - 1.0, min_y, 2.0, dy, color);
        }

        // Arrowhead
        dl.push_quad(ex - 4.0, ey - 4.0, 8.0, 8.0, color);
    }

    fn draw_cloud(&self, dl: &mut DrawList, ann: &Annotation, vp: Mat4, sw: f32, sh: f32, color: [f32; 4]) {
        let Some((sx, sy)) = self.resolve_screen(ann.start, ann.world_space, vp, sw, sh) else { return };
        let Some((ex, ey)) = self.resolve_screen(ann.end, ann.world_space, vp, sw, sh) else { return };

        let min_x = sx.min(ex);
        let min_y = sy.min(ey);
        let w = (ex - sx).abs();
        let h = (ey - sy).abs();

        // Cloud border (wavy approximation: dashed outline)
        let dash_len: f32 = 8.0;
        let cloud_color = [color[0], color[1], color[2], color[3] * 0.6];

        // Top and bottom edges (dashed)
        let mut cx = min_x;
        while cx < min_x + w {
            let seg = dash_len.min(min_x + w - cx);
            dl.push_quad(cx, min_y, seg, 2.0, cloud_color);
            dl.push_quad(cx, min_y + h - 2.0, seg, 2.0, cloud_color);
            cx += dash_len * 2.0;
        }

        // Left and right edges (dashed)
        let mut cy = min_y;
        while cy < min_y + h {
            let seg = dash_len.min(min_y + h - cy);
            dl.push_quad(min_x, cy, 2.0, seg, cloud_color);
            dl.push_quad(min_x + w - 2.0, cy, 2.0, seg, cloud_color);
            cy += dash_len * 2.0;
        }

        // Fill (very transparent)
        dl.push_quad(min_x, min_y, w, h, [color[0], color[1], color[2], 0.08]);

        // Text label at top
        if !ann.text.is_empty() {
            let tw = font::measure_text(&ann.text, 10.0, None);
            dl.push_quad(min_x, min_y - 16.0, tw + 8.0, 14.0, [0.0, 0.0, 0.0, 0.7]);
            emit_text(dl, &ann.text, min_x + 4.0, min_y - 14.0, 10.0, color);
        }
    }

    fn draw_note(&self, dl: &mut DrawList, ann: &Annotation, vp: Mat4, sw: f32, sh: f32, color: [f32; 4]) {
        let Some((sx, sy)) = self.resolve_screen(ann.start, ann.world_space, vp, sw, sh) else { return };

        let tw = font::measure_text(&ann.text, 11.0, None);
        let pad = 6.0;
        dl.push_quad(sx - pad, sy - pad, tw + pad * 2.0, 20.0, [0.0, 0.0, 0.0, 0.7]);
        let border = [color[0], color[1], color[2], 0.5];
        dl.push_quad(sx - pad, sy - pad, tw + pad * 2.0, 1.0, border);
        dl.push_quad(sx - pad, sy - pad + 19.0, tw + pad * 2.0, 1.0, border);
        dl.push_quad(sx - pad, sy - pad, 1.0, 20.0, border);
        dl.push_quad(sx - pad + tw + pad * 2.0 - 1.0, sy - pad, 1.0, 20.0, border);
        emit_text(dl, &ann.text, sx, sy, 11.0, color);
    }

    fn draw_balloon(&self, dl: &mut DrawList, ann: &Annotation, vp: Mat4, sw: f32, sh: f32, color: [f32; 4]) {
        let Some((sx, sy)) = self.resolve_screen(ann.start, ann.world_space, vp, sw, sh) else { return };

        let radius = 12.0;
        // Circle approximation (quad with filled bg)
        dl.push_quad(sx - radius, sy - radius, radius * 2.0, radius * 2.0, color);
        // Hollow center
        dl.push_quad(sx - radius + 2.0, sy - radius + 2.0,
            radius * 2.0 - 4.0, radius * 2.0 - 4.0,
            [0.0, 0.0, 0.0, 0.85]);
        // Number
        let num = ann.balloon_number.to_string();
        let nw = font::measure_text(&num, 11.0, None);
        emit_text(dl, &num, sx - nw * 0.5, sy - 5.0, 11.0, color);
    }
}

impl Default for AnnotationTools {
    fn default() -> Self {
        Self::new()
    }
}

fn emit_text(dl: &mut DrawList, text: &str, x: f32, y: f32, size: f32, color: [f32; 4]) {
    let mut cx = x;
    for c in text.chars() {
        let params = font::CharQuadParams {
            c, x: cx, y, size, color, atlas: None,
        };
        cx += font::emit_char_quads(&params, &mut dl.vertices, &mut dl.indices);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_clear() {
        let mut at = AnnotationTools::new();
        at.add(Annotation::note([0.0, 0.0, 0.0], "Test note"));
        at.add(Annotation::arrow([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
        assert_eq!(at.count(), 2);
        at.clear();
        assert_eq!(at.count(), 0);
    }

    #[test]
    fn remove_annotation() {
        let mut at = AnnotationTools::new();
        at.add(Annotation::note([0.0, 0.0, 0.0], "A"));
        at.add(Annotation::note([1.0, 0.0, 0.0], "B"));
        at.remove(0);
        assert_eq!(at.count(), 1);
        assert_eq!(at.annotations[0].text, "B");
    }

    #[test]
    fn tool_lifecycle() {
        let mut at = AnnotationTools::new();
        assert!(at.active_tool.is_none());
        at.begin_tool(AnnotationType::Callout);
        assert_eq!(at.active_tool, Some(AnnotationType::Callout));
        at.cancel_tool();
        assert!(at.active_tool.is_none());
    }

    #[test]
    fn balloon_number() {
        let ann = Annotation::balloon([0.0, 1.0, 0.0], 42);
        assert_eq!(ann.balloon_number, 42);
        assert_eq!(ann.text, "42");
    }

    #[test]
    fn with_author() {
        let ann = Annotation::note([0.0, 0.0, 0.0], "Check here")
            .with_author("Engineer A");
        assert_eq!(ann.author, "Engineer A");
    }
}
