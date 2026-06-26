//! Render settings — environment, lighting, and camera controls for realistic rendering.
//!
//! Inspired by KeyShot, Fusion 360 Render Workspace, and Blender Render Properties.
//! Controls environment HDR, ground plane, tone mapping, lighting presets,
//! and camera depth-of-field / exposure settings.

use crate::draw::DrawList;
use crate::font;

/// Environment preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvironmentPreset {
    Studio,
    Outdoor,
    Workshop,
    Neutral,
    DarkRoom,
    Sunset,
    Custom,
}

impl EnvironmentPreset {
    pub fn label(self) -> &'static str {
        match self {
            Self::Studio => "Studio",
            Self::Outdoor => "Outdoor",
            Self::Workshop => "Workshop",
            Self::Neutral => "Neutral",
            Self::DarkRoom => "Dark Room",
            Self::Sunset => "Sunset",
            Self::Custom => "Custom",
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::Studio, Self::Outdoor, Self::Workshop, Self::Neutral,
          Self::DarkRoom, Self::Sunset, Self::Custom]
    }
}

/// Tone mapping operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToneMapping {
    None,
    Reinhard,
    ACES,
    Filmic,
}

impl ToneMapping {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Reinhard => "Reinhard",
            Self::ACES => "ACES",
            Self::Filmic => "Filmic",
        }
    }
}

/// Render quality preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderQuality {
    Draft,
    Medium,
    High,
    Ultra,
}

impl RenderQuality {
    pub fn label(self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Ultra => "Ultra",
        }
    }

    pub fn samples(self) -> u32 {
        match self {
            Self::Draft => 16,
            Self::Medium => 64,
            Self::High => 256,
            Self::Ultra => 1024,
        }
    }
}

/// The render settings panel.
pub struct RenderSettings {
    /// Whether the panel is visible.
    pub visible: bool,
    /// Environment preset.
    pub environment: EnvironmentPreset,
    /// Environment rotation (degrees).
    pub env_rotation: f32,
    /// Environment intensity.
    pub env_intensity: f32,
    /// Show ground plane.
    pub ground_plane: bool,
    /// Ground plane shadow.
    pub ground_shadow: bool,
    /// Tone mapping.
    pub tone_mapping: ToneMapping,
    /// Exposure (EV).
    pub exposure: f32,
    /// Render quality.
    pub quality: RenderQuality,
    /// Depth of field enabled.
    pub dof_enabled: bool,
    /// DOF focal distance.
    pub dof_focal_distance: f32,
    /// DOF aperture (f-stop).
    pub dof_aperture: f32,
    /// Background color.
    pub bg_color: [f32; 4],
    /// Panel width.
    pub panel_width: f32,
    /// Which section is expanded (bitmask: 1=env, 2=camera, 4=output).
    pub expanded_sections: u8,
}

impl RenderSettings {
    pub fn new() -> Self {
        Self {
            visible: false,
            environment: EnvironmentPreset::Studio,
            env_rotation: 0.0,
            env_intensity: 1.0,
            ground_plane: true,
            ground_shadow: true,
            tone_mapping: ToneMapping::ACES,
            exposure: 0.0,
            quality: RenderQuality::Medium,
            dof_enabled: false,
            dof_focal_distance: 5.0,
            dof_aperture: 2.8,
            bg_color: [0.8, 0.8, 0.82, 1.0],
            panel_width: 240.0,
            expanded_sections: 0b111,
        }
    }

    /// Toggle visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Toggle a section (1=env, 2=camera, 4=output).
    pub fn toggle_section(&mut self, bit: u8) {
        self.expanded_sections ^= bit;
    }

    /// Check if section is expanded.
    pub fn section_expanded(&self, bit: u8) -> bool {
        self.expanded_sections & bit != 0
    }

    /// Draw the render settings panel.
    pub fn draw(
        &self,
        dl: &mut DrawList,
        panel_x: f32,
        panel_y: f32,
        bg_color: [f32; 4],
        text_color: [f32; 4],
        accent_color: [f32; 4],
    ) {
        if !self.visible { return; }

        let mut cy = panel_y;
        let section_h = 20.0;
        let row_h = 18.0;

        // Estimate total height
        let mut total_h = 28.0; // title
        total_h += section_h; // Environment header
        if self.section_expanded(1) { total_h += row_h * 5.0; }
        total_h += section_h; // Camera header
        if self.section_expanded(2) { total_h += row_h * 4.0; }
        total_h += section_h; // Output header
        if self.section_expanded(4) { total_h += row_h * 2.0; }
        total_h += 8.0;

        // Background
        dl.push_quad(panel_x, panel_y, self.panel_width, total_h, bg_color);
        let border = [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.8];
        dl.push_quad(panel_x, panel_y, 1.0, total_h, border);

        let muted = [text_color[0] * 0.5, text_color[1] * 0.5, text_color[2] * 0.5, text_color[3]];

        // Title
        emit_text(dl, "Render Settings", panel_x + 8.0, cy + 5.0, 11.0, text_color);
        cy += 28.0;

        // ── Environment section ──
        {
            let arrow = if self.section_expanded(1) { "v" } else { ">" };
            dl.push_quad(panel_x, cy, self.panel_width, section_h, [bg_color[0] + 0.02, bg_color[1] + 0.02, bg_color[2] + 0.02, 1.0]);
            emit_text(dl, arrow, panel_x + 4.0, cy + 4.0, 9.0, muted);
            emit_text(dl, "Environment", panel_x + 16.0, cy + 4.0, 10.0, text_color);
            cy += section_h;

            if self.section_expanded(1) {
                // Preset
                emit_text(dl, "Preset", panel_x + 12.0, cy + 3.0, 8.0, muted);
                emit_text(dl, self.environment.label(), panel_x + 80.0, cy + 3.0, 9.0, text_color);
                cy += row_h;

                // Rotation
                emit_text(dl, "Rotation", panel_x + 12.0, cy + 3.0, 8.0, muted);
                let rot = format!("{:.0}°", self.env_rotation);
                emit_text(dl, &rot, panel_x + 80.0, cy + 3.0, 9.0, text_color);
                cy += row_h;

                // Intensity
                emit_text(dl, "Intensity", panel_x + 12.0, cy + 3.0, 8.0, muted);
                let inten = format!("{:.1}", self.env_intensity);
                emit_text(dl, &inten, panel_x + 80.0, cy + 3.0, 9.0, text_color);
                cy += row_h;

                // Ground plane
                let gp_bg = if self.ground_plane { accent_color } else { [0.3, 0.3, 0.3, 0.5] };
                dl.push_quad(panel_x + 12.0, cy + 2.0, 10.0, 10.0, gp_bg);
                emit_text(dl, "Ground plane", panel_x + 28.0, cy + 3.0, 8.0, text_color);
                cy += row_h;

                // Shadow
                let sh_bg = if self.ground_shadow { accent_color } else { [0.3, 0.3, 0.3, 0.5] };
                dl.push_quad(panel_x + 12.0, cy + 2.0, 10.0, 10.0, sh_bg);
                emit_text(dl, "Ground shadow", panel_x + 28.0, cy + 3.0, 8.0, text_color);
                cy += row_h;
            }
        }

        // ── Camera section ──
        {
            let arrow = if self.section_expanded(2) { "v" } else { ">" };
            dl.push_quad(panel_x, cy, self.panel_width, section_h, [bg_color[0] + 0.02, bg_color[1] + 0.02, bg_color[2] + 0.02, 1.0]);
            emit_text(dl, arrow, panel_x + 4.0, cy + 4.0, 9.0, muted);
            emit_text(dl, "Camera", panel_x + 16.0, cy + 4.0, 10.0, text_color);
            cy += section_h;

            if self.section_expanded(2) {
                // Exposure
                emit_text(dl, "Exposure", panel_x + 12.0, cy + 3.0, 8.0, muted);
                let exp = format!("{:+.1} EV", self.exposure);
                emit_text(dl, &exp, panel_x + 80.0, cy + 3.0, 9.0, text_color);
                cy += row_h;

                // Tone mapping
                emit_text(dl, "Tone map", panel_x + 12.0, cy + 3.0, 8.0, muted);
                emit_text(dl, self.tone_mapping.label(), panel_x + 80.0, cy + 3.0, 9.0, text_color);
                cy += row_h;

                // DOF
                let dof_bg = if self.dof_enabled { accent_color } else { [0.3, 0.3, 0.3, 0.5] };
                dl.push_quad(panel_x + 12.0, cy + 2.0, 10.0, 10.0, dof_bg);
                emit_text(dl, "Depth of Field", panel_x + 28.0, cy + 3.0, 8.0, text_color);
                cy += row_h;

                if self.dof_enabled {
                    let dof_info = format!("f/{:.1}  {:.1}m", self.dof_aperture, self.dof_focal_distance);
                    emit_text(dl, &dof_info, panel_x + 28.0, cy + 3.0, 8.0, muted);
                }
                cy += row_h;
            }
        }

        // ── Output section ──
        {
            let arrow = if self.section_expanded(4) { "v" } else { ">" };
            dl.push_quad(panel_x, cy, self.panel_width, section_h, [bg_color[0] + 0.02, bg_color[1] + 0.02, bg_color[2] + 0.02, 1.0]);
            emit_text(dl, arrow, panel_x + 4.0, cy + 4.0, 9.0, muted);
            emit_text(dl, "Output", panel_x + 16.0, cy + 4.0, 10.0, text_color);
            cy += section_h;

            if self.section_expanded(4) {
                emit_text(dl, "Quality", panel_x + 12.0, cy + 3.0, 8.0, muted);
                let q = format!("{} ({}spp)", self.quality.label(), self.quality.samples());
                emit_text(dl, &q, panel_x + 80.0, cy + 3.0, 9.0, text_color);
                cy += row_h;

                // BG color swatch
                emit_text(dl, "Background", panel_x + 12.0, cy + 3.0, 8.0, muted);
                dl.push_quad(panel_x + 80.0, cy + 1.0, 14.0, 14.0, self.bg_color);
                let _ = cy;
            }
        }
    }
}

impl Default for RenderSettings {
    fn default() -> Self { Self::new() }
}

fn emit_text(dl: &mut DrawList, text: &str, x: f32, y: f32, size: f32, color: [f32; 4]) {
    let mut cx = x;
    for c in text.chars() {
        let params = font::CharQuadParams { c, x: cx, y, size, color, atlas: None };
        cx += font::emit_char_quads(&params, &mut dl.vertices, &mut dl.indices);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_visibility() {
        let mut rs = RenderSettings::new();
        assert!(!rs.visible);
        rs.toggle();
        assert!(rs.visible);
    }

    #[test]
    fn section_toggle() {
        let mut rs = RenderSettings::new();
        assert!(rs.section_expanded(1));
        rs.toggle_section(1);
        assert!(!rs.section_expanded(1));
    }

    #[test]
    fn quality_samples() {
        assert_eq!(RenderQuality::Draft.samples(), 16);
        assert_eq!(RenderQuality::Ultra.samples(), 1024);
    }

    #[test]
    fn environment_presets() {
        for e in EnvironmentPreset::all() {
            assert!(!e.label().is_empty());
        }
    }

    #[test]
    fn defaults() {
        let rs = RenderSettings::new();
        assert_eq!(rs.tone_mapping, ToneMapping::ACES);
        assert_eq!(rs.quality, RenderQuality::Medium);
        assert!(rs.ground_plane);
    }
}
