//! Collaboration panel — real-time cursors, comments, and sharing.
//!
//! Inspired by Figma multiplayer, Onshape real-time collaboration,
//! and Google Docs commenting. Shows connected users, their cursor
//! positions, and threaded comments on geometry.

use crate::draw::DrawList;
use crate::font;

/// User presence info.
#[derive(Clone, Debug)]
pub struct Collaborator {
    /// User display name.
    pub name: String,
    /// User avatar color.
    pub color: [f32; 4],
    /// Cursor position on screen (None = off-screen).
    pub cursor: Option<[f32; 2]>,
    /// What they're currently doing.
    pub status: String,
    /// Whether they're actively connected.
    pub online: bool,
}

impl Collaborator {
    pub fn new(name: &str, color: [f32; 4]) -> Self {
        Self {
            name: name.to_string(),
            color,
            cursor: None,
            status: "Viewing".to_string(),
            online: true,
        }
    }
}

/// A comment on geometry.
#[derive(Clone, Debug)]
pub struct Comment {
    /// Author name.
    pub author: String,
    /// Comment text.
    pub text: String,
    /// Timestamp (ISO-like string).
    pub timestamp: String,
    /// Screen anchor position.
    pub anchor: [f32; 2],
    /// Whether this comment is resolved.
    pub resolved: bool,
    /// Replies.
    pub replies: Vec<CommentReply>,
}

/// A reply to a comment.
#[derive(Clone, Debug)]
pub struct CommentReply {
    pub author: String,
    pub text: String,
    pub timestamp: String,
}

impl Comment {
    pub fn new(author: &str, text: &str, x: f32, y: f32) -> Self {
        Self {
            author: author.to_string(),
            text: text.to_string(),
            timestamp: String::new(),
            anchor: [x, y],
            resolved: false,
            replies: Vec::new(),
        }
    }

    pub fn with_timestamp(mut self, ts: &str) -> Self {
        self.timestamp = ts.to_string();
        self
    }

    pub fn add_reply(&mut self, author: &str, text: &str) {
        self.replies.push(CommentReply {
            author: author.to_string(),
            text: text.to_string(),
            timestamp: String::new(),
        });
    }
}

/// The collaboration panel.
pub struct Collaboration {
    /// Whether the panel is visible.
    pub visible: bool,
    /// Connected collaborators.
    pub users: Vec<Collaborator>,
    /// Comments on the model.
    pub comments: Vec<Comment>,
    /// Selected comment index.
    pub selected_comment: Option<usize>,
    /// Whether comment creation mode is active.
    pub creating_comment: bool,
    /// Show resolved comments.
    pub show_resolved: bool,
    /// Panel width.
    pub width: f32,
    /// Hovered user index.
    pub hovered_user: Option<usize>,
}

impl Collaboration {
    pub fn new() -> Self {
        Self {
            visible: false,
            users: Vec::new(),
            comments: Vec::new(),
            selected_comment: None,
            creating_comment: false,
            show_resolved: false,
            width: 260.0,
            hovered_user: None,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Online user count.
    pub fn online_count(&self) -> usize {
        self.users.iter().filter(|u| u.online).count()
    }

    /// Unresolved comment count.
    pub fn unresolved_count(&self) -> usize {
        self.comments.iter().filter(|c| !c.resolved).count()
    }

    /// Add a comment.
    pub fn add_comment(&mut self, comment: Comment) {
        self.comments.push(comment);
    }

    /// Resolve a comment.
    pub fn resolve_comment(&mut self, idx: usize) {
        if let Some(c) = self.comments.get_mut(idx) {
            c.resolved = true;
        }
    }

    /// Draw collaborator cursors on canvas.
    pub fn draw_cursors(&self, dl: &mut DrawList) {
        for user in &self.users {
            if !user.online { continue; }
            if let Some([cx, cy]) = user.cursor {
                // Cursor arrow
                dl.push_quad(cx, cy, 2.0, 12.0, user.color);
                dl.push_quad(cx, cy, 8.0, 2.0, user.color);

                // Name label
                let lw = font::measure_text(&user.name, 8.0, None);
                dl.push_quad(cx + 10.0, cy - 2.0, lw + 6.0, 12.0, user.color);
                emit_text(dl, &user.name, cx + 13.0, cy, 8.0, [1.0, 1.0, 1.0, 1.0]);
            }
        }
    }

    /// Draw comment pins on canvas.
    pub fn draw_comment_pins(
        &self,
        dl: &mut DrawList,
        accent_color: [f32; 4],
    ) {
        for (i, comment) in self.comments.iter().enumerate() {
            if comment.resolved && !self.show_resolved { continue; }
            let [ax, ay] = comment.anchor;
            if ax == 0.0 && ay == 0.0 { continue; }

            let is_sel = self.selected_comment == Some(i);
            let pin_color = if comment.resolved {
                [0.4, 0.7, 0.4, 0.6]
            } else if is_sel {
                accent_color
            } else {
                [0.9, 0.7, 0.2, 0.8]
            };

            // Pin
            dl.push_quad(ax - 6.0, ay - 6.0, 12.0, 12.0, pin_color);

            // Reply count badge
            if !comment.replies.is_empty() {
                let count = format!("{}", comment.replies.len());
                let cw = font::measure_text(&count, 7.0, None);
                dl.push_quad(ax + 4.0, ay - 10.0, cw + 4.0, 10.0, [0.0, 0.0, 0.0, 0.6]);
                emit_text(dl, &count, ax + 6.0, ay - 9.0, 7.0, [1.0, 1.0, 1.0, 0.9]);
            }

            // Expanded comment (if selected)
            if is_sel {
                let bw = 200.0_f32;
                let bh = 60.0 + comment.replies.len() as f32 * 20.0;
                dl.push_quad(ax + 14.0, ay - 4.0, bw, bh, [0.12, 0.12, 0.14, 0.95]);
                dl.push_quad(ax + 14.0, ay - 4.0, bw, 1.0, pin_color);

                emit_text(dl, &comment.author, ax + 20.0, ay + 2.0, 9.0, pin_color);
                emit_text(dl, &comment.text, ax + 20.0, ay + 16.0, 8.0, [0.9, 0.9, 0.9, 1.0]);

                for (ri, reply) in comment.replies.iter().enumerate() {
                    let ry = ay + 36.0 + ri as f32 * 20.0;
                    emit_text(dl, &reply.author, ax + 28.0, ry, 8.0, [0.6, 0.6, 0.8, 0.8]);
                    emit_text(dl, &reply.text, ax + 28.0, ry + 10.0, 7.0, [0.8, 0.8, 0.8, 0.9]);
                }
            }
        }
    }

    /// Draw the collaboration panel.
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

        let user_section_h = 28.0 + self.users.len() as f32 * 22.0;
        let comment_section_h = 28.0 + self.comments.iter().filter(|c| !c.resolved || self.show_resolved).count() as f32 * 24.0;
        let panel_h = user_section_h + comment_section_h + 8.0;

        dl.push_quad(panel_x, panel_y, self.width, panel_h, bg_color);

        let muted = [text_color[0] * 0.5, text_color[1] * 0.5, text_color[2] * 0.5, text_color[3]];

        // Users section
        emit_text(dl, "Collaborators", panel_x + 8.0, panel_y + 6.0, 10.0, text_color);
        let online = format!("{} online", self.online_count());
        let ow = font::measure_text(&online, 8.0, None);
        emit_text(dl, &online, panel_x + self.width - ow - 8.0, panel_y + 8.0, 8.0, [0.3, 0.8, 0.3, 0.8]);

        for (i, user) in self.users.iter().enumerate() {
            let ry = panel_y + 24.0 + i as f32 * 22.0;
            let is_hov = self.hovered_user == Some(i);
            if is_hov {
                dl.push_quad(panel_x, ry, self.width, 22.0, [1.0, 1.0, 1.0, 0.05]);
            }

            // Avatar dot
            let dot_color = if user.online { user.color } else { [0.4, 0.4, 0.4, 0.5] };
            dl.push_quad(panel_x + 8.0, ry + 5.0, 10.0, 10.0, dot_color);

            // Name
            let nc = if user.online { text_color } else { muted };
            emit_text(dl, &user.name, panel_x + 24.0, ry + 4.0, 9.0, nc);

            // Status
            let sw = font::measure_text(&user.status, 7.0, None);
            emit_text(dl, &user.status, panel_x + self.width - sw - 8.0, ry + 6.0, 7.0, muted);
        }

        // Comments section
        let comments_y = panel_y + user_section_h;
        emit_text(dl, "Comments", panel_x + 8.0, comments_y + 6.0, 10.0, text_color);
        let uc = format!("{} open", self.unresolved_count());
        let ucw = font::measure_text(&uc, 8.0, None);
        emit_text(dl, &uc, panel_x + self.width - ucw - 8.0, comments_y + 8.0, 8.0, accent_color);

        let mut cy = comments_y + 24.0;
        for comment in &self.comments {
            if comment.resolved && !self.show_resolved { continue; }

            let rc = if comment.resolved { [0.4, 0.7, 0.4, 0.6] } else { [0.9, 0.7, 0.2, 0.8] };
            dl.push_quad(panel_x + 8.0, cy + 5.0, 6.0, 6.0, rc);
            emit_text(dl, &comment.author, panel_x + 20.0, cy + 3.0, 8.0, text_color);

            let preview = if comment.text.len() > 30 { &comment.text[..30] } else { &comment.text };
            emit_text(dl, preview, panel_x + 20.0, cy + 13.0, 7.0, muted);
            cy += 24.0;
        }

        // Border
        dl.push_quad(panel_x, panel_y, 1.0, panel_h,
            [bg_color[0] + 0.1, bg_color[1] + 0.1, bg_color[2] + 0.1, 0.8]);
    }
}

impl Default for Collaboration {
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
    fn online_count() {
        let mut collab = Collaboration::new();
        collab.users.push(Collaborator::new("Alice", [0.3, 0.6, 0.9, 1.0]));
        collab.users.push(Collaborator::new("Bob", [0.9, 0.4, 0.3, 1.0]));
        collab.users[1].online = false;
        assert_eq!(collab.online_count(), 1);
    }

    #[test]
    fn comment_with_replies() {
        let mut c = Comment::new("Alice", "Check this edge", 100.0, 200.0);
        c.add_reply("Bob", "Looks good to me");
        assert_eq!(c.replies.len(), 1);
    }

    #[test]
    fn resolve_comment() {
        let mut collab = Collaboration::new();
        collab.add_comment(Comment::new("Alice", "Fix tolerance", 50.0, 50.0));
        assert_eq!(collab.unresolved_count(), 1);
        collab.resolve_comment(0);
        assert_eq!(collab.unresolved_count(), 0);
    }

    #[test]
    fn toggle_panel() {
        let mut collab = Collaboration::new();
        assert!(!collab.visible);
        collab.toggle();
        assert!(collab.visible);
    }

    #[test]
    fn cursor_position() {
        let mut user = Collaborator::new("Test", [1.0, 0.0, 0.0, 1.0]);
        assert!(user.cursor.is_none());
        user.cursor = Some([100.0, 200.0]);
        assert_eq!(user.cursor, Some([100.0, 200.0]));
    }
}
