/// A batch of UI geometry to be rendered in one draw call.
pub struct DrawList {
    /// Vertex data for this draw list.
    pub vertices: Vec<UiVertex>,
    /// Index data referencing vertices.
    pub indices: Vec<u32>,
    /// Optional scissor rectangle: `[x, y, width, height]` in pixels.
    pub clip_rect: Option<[f32; 4]>,
}

impl DrawList {
    /// Create a new empty draw list.
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            clip_rect: None,
        }
    }

    /// Push a colored quad defined by its corners.
    pub fn push_quad(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
    ) {
        let base = self.vertices.len() as u32;
        self.vertices.push(UiVertex { pos: [x, y], uv: [0.0, 0.0], color });
        self.vertices.push(UiVertex { pos: [x + w, y], uv: [1.0, 0.0], color });
        self.vertices.push(UiVertex { pos: [x + w, y + h], uv: [1.0, 1.0], color });
        self.vertices.push(UiVertex { pos: [x, y + h], uv: [0.0, 1.0], color });
        self.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Push a colored quad defined by four arbitrary corner positions.
    pub fn push_quad_vertices(
        &mut self,
        p0: [f32; 2],
        p1: [f32; 2],
        p2: [f32; 2],
        p3: [f32; 2],
        color: [f32; 4],
    ) {
        let base = self.vertices.len() as u32;
        self.vertices.push(UiVertex { pos: p0, uv: [0.0, 0.0], color });
        self.vertices.push(UiVertex { pos: p1, uv: [1.0, 0.0], color });
        self.vertices.push(UiVertex { pos: p2, uv: [1.0, 1.0], color });
        self.vertices.push(UiVertex { pos: p3, uv: [0.0, 1.0], color });
        self.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

impl Default for DrawList {
    fn default() -> Self {
        Self::new()
    }
}

/// A single UI vertex sent to the GPU.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiVertex {
    /// Position in pixel coordinates.
    pub pos: [f32; 2],
    /// Texture coordinates.
    pub uv: [f32; 2],
    /// RGBA color.
    pub color: [f32; 4],
}
