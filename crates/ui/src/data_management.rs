//! Data management panel — PDM integration, check-in/out, lifecycle.
//!
//! Inspired by SolidWorks PDM, Onshape Document Management,
//! and Fusion 360 Data Panel. Provides file lifecycle management,
//! check-in/check-out workflow, and release states.

use crate::draw::DrawList;
use crate::font;

/// File lifecycle state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleState {
    InWork,
    InReview,
    Released,
    Obsolete,
}

impl LifecycleState {
    pub fn label(self) -> &'static str {
        match self {
            Self::InWork => "In Work",
            Self::InReview => "In Review",
            Self::Released => "Released",
            Self::Obsolete => "Obsolete",
        }
    }

    pub fn color(self) -> [f32; 4] {
        match self {
            Self::InWork => [0.3, 0.6, 0.9, 0.9],
            Self::InReview => [0.9, 0.7, 0.2, 0.9],
            Self::Released => [0.3, 0.8, 0.3, 0.9],
            Self::Obsolete => [0.5, 0.5, 0.5, 0.6],
        }
    }

    pub fn next(self) -> Option<Self> {
        match self {
            Self::InWork => Some(Self::InReview),
            Self::InReview => Some(Self::Released),
            Self::Released => Some(Self::Obsolete),
            Self::Obsolete => None,
        }
    }
}

/// Check-out status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckoutStatus {
    Available,
    CheckedOutByMe,
    CheckedOutByOther(String),
}

impl CheckoutStatus {
    pub fn label(&self) -> &str {
        match self {
            Self::Available => "Available",
            Self::CheckedOutByMe => "Checked out (you)",
            Self::CheckedOutByOther(name) => name,
        }
    }
}

/// A managed document.
#[derive(Clone, Debug)]
pub struct ManagedDocument {
    /// Document name.
    pub name: String,
    /// Part number.
    pub part_number: String,
    /// Current revision.
    pub revision: String,
    /// Lifecycle state.
    pub state: LifecycleState,
    /// Checkout status.
    pub checkout: CheckoutStatus,
    /// Last modified timestamp.
    pub last_modified: String,
    /// Last modified by.
    pub modified_by: String,
}

impl ManagedDocument {
    pub fn new(name: &str, part_number: &str) -> Self {
        Self {
            name: name.to_string(),
            part_number: part_number.to_string(),
            revision: "A".to_string(),
            state: LifecycleState::InWork,
            checkout: CheckoutStatus::Available,
            last_modified: String::new(),
            modified_by: String::new(),
        }
    }
}

/// The data management panel.
pub struct DataManagement {
    /// Whether the panel is visible.
    pub visible: bool,
    /// Managed documents.
    pub documents: Vec<ManagedDocument>,
    /// Selected document index.
    pub selected: Option<usize>,
    /// Hovered document index.
    pub hovered: Option<usize>,
    /// Panel width.
    pub width: f32,
    /// Scroll offset.
    pub scroll_offset: usize,
}

impl DataManagement {
    pub fn new() -> Self {
        Self {
            visible: false,
            documents: Vec::new(),
            selected: None,
            hovered: None,
            width: 320.0,
            scroll_offset: 0,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Add a document.
    pub fn add(&mut self, doc: ManagedDocument) {
        self.documents.push(doc);
    }

    /// Check out a document (by index).
    pub fn checkout(&mut self, idx: usize) {
        if let Some(doc) = self.documents.get_mut(idx) {
            if doc.checkout == CheckoutStatus::Available {
                doc.checkout = CheckoutStatus::CheckedOutByMe;
            }
        }
    }

    /// Check in a document.
    pub fn checkin(&mut self, idx: usize) {
        if let Some(doc) = self.documents.get_mut(idx) {
            if doc.checkout == CheckoutStatus::CheckedOutByMe {
                doc.checkout = CheckoutStatus::Available;
            }
        }
    }

    /// Advance lifecycle state.
    pub fn advance_state(&mut self, idx: usize) {
        if let Some(doc) = self.documents.get_mut(idx) {
            if let Some(next) = doc.state.next() {
                doc.state = next;
            }
        }
    }

    /// Count documents in each state.
    pub fn state_counts(&self) -> [usize; 4] {
        let mut counts = [0; 4];
        for doc in &self.documents {
            match doc.state {
                LifecycleState::InWork => counts[0] += 1,
                LifecycleState::InReview => counts[1] += 1,
                LifecycleState::Released => counts[2] += 1,
                LifecycleState::Obsolete => counts[3] += 1,
            }
        }
        counts
    }

    /// Draw the data management panel.
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

        let header_h = 52.0;
        let row_h = 36.0;
        let rows = self.documents.len().min(10);
        let panel_h = header_h + rows as f32 * row_h + 8.0;

        // Background
        dl.push_quad(panel_x, panel_y, self.width, panel_h, bg_color);
        let border = [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.8];
        dl.push_quad(panel_x, panel_y, 1.0, panel_h, border);

        let muted = [text_color[0] * 0.5, text_color[1] * 0.5, text_color[2] * 0.5, text_color[3]];

        // Title
        emit_text(dl, "Data Management", panel_x + 8.0, panel_y + 5.0, 11.0, text_color);

        // State summary
        let counts = self.state_counts();
        let summary = format!("{}W {}R {}Rel", counts[0], counts[1], counts[2]);
        let sw = font::measure_text(&summary, 8.0, None);
        emit_text(dl, &summary, panel_x + self.width - sw - 8.0, panel_y + 8.0, 8.0, muted);

        // Column headers
        let ch_y = panel_y + 26.0;
        dl.push_quad(panel_x, ch_y, self.width, 20.0,
            [bg_color[0] + 0.02, bg_color[1] + 0.02, bg_color[2] + 0.02, 1.0]);
        emit_text(dl, "Document", panel_x + 8.0, ch_y + 4.0, 7.0, muted);
        emit_text(dl, "Rev", panel_x + 160.0, ch_y + 4.0, 7.0, muted);
        emit_text(dl, "State", panel_x + 190.0, ch_y + 4.0, 7.0, muted);
        emit_text(dl, "Status", panel_x + 250.0, ch_y + 4.0, 7.0, muted);

        // Document rows
        for (i, doc) in self.documents.iter().enumerate().take(rows) {
            let ry = panel_y + header_h + i as f32 * row_h;

            let is_sel = self.selected == Some(i);
            let is_hov = self.hovered == Some(i);

            if is_sel {
                dl.push_quad(panel_x, ry, self.width, row_h,
                    [accent_color[0] * 0.2, accent_color[1] * 0.2, accent_color[2] * 0.2, 0.4]);
            } else if is_hov {
                dl.push_quad(panel_x, ry, self.width, row_h, [1.0, 1.0, 1.0, 0.04]);
            }

            // Name + PN
            emit_text(dl, &doc.name, panel_x + 8.0, ry + 4.0, 9.0, text_color);
            emit_text(dl, &doc.part_number, panel_x + 8.0, ry + 16.0, 7.0, muted);

            // Revision
            emit_text(dl, &doc.revision, panel_x + 160.0, ry + 8.0, 10.0, text_color);

            // State badge
            let state_color = doc.state.color();
            let state_label = doc.state.label();
            let slw = font::measure_text(state_label, 7.0, None);
            dl.push_quad(panel_x + 190.0, ry + 6.0, slw + 6.0, 14.0,
                [state_color[0] * 0.3, state_color[1] * 0.3, state_color[2] * 0.3, 0.5]);
            emit_text(dl, state_label, panel_x + 193.0, ry + 8.0, 7.0, state_color);

            // Checkout status
            let checkout_color = match &doc.checkout {
                CheckoutStatus::Available => [0.5, 0.5, 0.5, 0.5],
                CheckoutStatus::CheckedOutByMe => [0.3, 0.8, 0.3, 0.8],
                CheckoutStatus::CheckedOutByOther(_) => [0.9, 0.3, 0.3, 0.8],
            };
            let co_label = match &doc.checkout {
                CheckoutStatus::Available => "-",
                CheckoutStatus::CheckedOutByMe => "You",
                CheckoutStatus::CheckedOutByOther(name) => name,
            };
            emit_text(dl, co_label, panel_x + 254.0, ry + 8.0, 8.0, checkout_color);

            // Modified by
            if !doc.modified_by.is_empty() {
                emit_text(dl, &doc.modified_by, panel_x + 190.0, ry + 22.0, 6.0, muted);
            }
        }
    }
}

impl Default for DataManagement {
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
    fn checkout_workflow() {
        let mut dm = DataManagement::new();
        dm.add(ManagedDocument::new("Bracket", "PN-001"));
        assert_eq!(dm.documents[0].checkout, CheckoutStatus::Available);
        dm.checkout(0);
        assert_eq!(dm.documents[0].checkout, CheckoutStatus::CheckedOutByMe);
        dm.checkin(0);
        assert_eq!(dm.documents[0].checkout, CheckoutStatus::Available);
    }

    #[test]
    fn lifecycle_progression() {
        let mut dm = DataManagement::new();
        dm.add(ManagedDocument::new("Shaft", "PN-002"));
        assert_eq!(dm.documents[0].state, LifecycleState::InWork);
        dm.advance_state(0);
        assert_eq!(dm.documents[0].state, LifecycleState::InReview);
        dm.advance_state(0);
        assert_eq!(dm.documents[0].state, LifecycleState::Released);
    }

    #[test]
    fn state_counts() {
        let mut dm = DataManagement::new();
        dm.add(ManagedDocument::new("A", "PN-1"));
        dm.add(ManagedDocument::new("B", "PN-2"));
        dm.documents[1].state = LifecycleState::Released;
        let counts = dm.state_counts();
        assert_eq!(counts[0], 1); // in work
        assert_eq!(counts[2], 1); // released
    }

    #[test]
    fn lifecycle_labels() {
        let states = [LifecycleState::InWork, LifecycleState::InReview,
                     LifecycleState::Released, LifecycleState::Obsolete];
        for s in states {
            assert!(!s.label().is_empty());
        }
    }

    #[test]
    fn toggle_panel() {
        let mut dm = DataManagement::new();
        assert!(!dm.visible);
        dm.toggle();
        assert!(dm.visible);
    }
}
