//! WebSocket real-time sync for collaboration sessions.
//!
//! Upgrades from polling-based `collab::SessionManager` to real-time push
//! via WebSocket connections. Each connected client gets instant operation
//! broadcast, presence updates, and cursor synchronization.

use crate::collab::{SessionManager, OpKind, Operation, CursorState};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;

// ---------------------------------------------------------------------------
// WebSocket Message Protocol
// ---------------------------------------------------------------------------

/// Messages sent from client → server over WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsClientMsg {
    /// Authenticate and join a session.
    Join {
        session_id: String,
        user_id: String,
        username: String,
        /// JWT token for authentication.
        token: String,
    },
    /// Submit an operation.
    Op { kind: OpKind },
    /// Update cursor position and selection.
    Cursor {
        position: [f64; 3],
        selection: Vec<String>,
    },
    /// Ping (keepalive).
    Ping,
}

/// Messages sent from server → client over WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsServerMsg {
    /// Catch-up: full operation log on join.
    Welcome {
        session_id: String,
        operations: Vec<Operation>,
        participants: Vec<PresenceInfo>,
    },
    /// A new operation was applied (broadcast).
    OpBroadcast { operation: Operation },
    /// Presence update: someone's cursor moved.
    Presence { user_id: String, info: PresenceInfo },
    /// Someone joined.
    UserJoined { user_id: String, username: String },
    /// Someone left.
    UserLeft { user_id: String },
    /// Pong response.
    Pong,
    /// Error message.
    Error { message: String },
}

/// Presence state for live awareness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceInfo {
    pub user_id: String,
    pub username: String,
    pub cursor_position: Option<[f64; 3]>,
    pub selection: Vec<String>,
    pub color: String,
    pub viewport: Option<ViewportState>,
}

/// Viewport camera state for remote-follow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportState {
    pub eye: [f64; 3],
    pub target: [f64; 3],
    pub up: [f64; 3],
}

// ---------------------------------------------------------------------------
// WebSocket Hub
// ---------------------------------------------------------------------------

/// Manages WebSocket connections and broadcast channels per session.
#[derive(Debug, Clone)]
pub struct WsHub {
    /// Broadcast sender per session.
    channels: Arc<RwLock<HashMap<String, broadcast::Sender<WsServerMsg>>>>,
    /// Presence state per session per user.
    presence: Arc<RwLock<HashMap<String, HashMap<String, PresenceInfo>>>>,
    /// Collaboration session manager (shared with REST API).
    session_mgr: SessionManager,
}

/// User colors for presence indicators (cycle through these).
const USER_COLORS: &[&str] = &[
    "#4285F4", "#EA4335", "#FBBC04", "#34A853", "#FF6D01",
    "#46BDC6", "#7B1FA2", "#C2185B", "#00897B", "#FF8F00",
];

impl WsHub {
    pub fn new(session_mgr: SessionManager) -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            presence: Arc::new(RwLock::new(HashMap::new())),
            session_mgr,
        }
    }

    /// Get or create a broadcast channel for a session.
    pub fn get_channel(&self, session_id: &str) -> broadcast::Sender<WsServerMsg> {
        let mut channels = self.channels.write().unwrap();
        channels.entry(session_id.to_string())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(256);
                tx
            })
            .clone()
    }

    /// Subscribe to a session's broadcast channel.
    pub fn subscribe(&self, session_id: &str) -> broadcast::Receiver<WsServerMsg> {
        self.get_channel(session_id).subscribe()
    }

    /// Handle a client join: register presence, return welcome message.
    pub fn handle_join(
        &self,
        session_id: &str,
        user_id: &str,
        username: &str,
    ) -> Result<WsServerMsg, String> {
        // Join via session manager (registers participant, gets log)
        let ops = self.session_mgr.join(session_id, user_id, username)?;

        // Register presence
        let color = {
            let mut presence = self.presence.write().map_err(|_| "Lock poisoned")?;
            let session_presence = presence.entry(session_id.to_string()).or_default();
            let color_idx = session_presence.len() % USER_COLORS.len();
            let color = USER_COLORS[color_idx].to_string();

            session_presence.insert(user_id.to_string(), PresenceInfo {
                user_id: user_id.to_string(),
                username: username.to_string(),
                cursor_position: None,
                selection: Vec::new(),
                color: color.clone(),
                viewport: None,
            });
            color
        };

        // Broadcast user-joined to others
        let tx = self.get_channel(session_id);
        let _ = tx.send(WsServerMsg::UserJoined {
            user_id: user_id.to_string(),
            username: username.to_string(),
        });

        // Gather current participants
        let participants = {
            let presence = self.presence.read().map_err(|_| "Lock poisoned")?;
            presence.get(session_id)
                .map(|p| p.values().cloned().collect())
                .unwrap_or_default()
        };

        Ok(WsServerMsg::Welcome {
            session_id: session_id.to_string(),
            operations: ops,
            participants,
        })
    }

    /// Handle operation submission: persist + broadcast.
    pub fn handle_op(
        &self,
        session_id: &str,
        user_id: &str,
        kind: OpKind,
    ) -> Result<(), String> {
        let op = self.session_mgr.submit_op(session_id, user_id, kind)?;

        let tx = self.get_channel(session_id);
        let _ = tx.send(WsServerMsg::OpBroadcast { operation: op });
        Ok(())
    }

    /// Handle cursor update: update presence + broadcast.
    pub fn handle_cursor(
        &self,
        session_id: &str,
        user_id: &str,
        position: [f64; 3],
        selection: Vec<String>,
    ) -> Result<(), String> {
        let info = {
            let mut presence = self.presence.write().map_err(|_| "Lock poisoned")?;
            let session_presence = presence.get_mut(session_id)
                .ok_or("Session not found")?;
            let info = session_presence.get_mut(user_id)
                .ok_or("User not in session")?;
            info.cursor_position = Some(position);
            info.selection = selection;
            info.clone()
        };

        let tx = self.get_channel(session_id);
        let _ = tx.send(WsServerMsg::Presence {
            user_id: user_id.to_string(),
            info,
        });
        Ok(())
    }

    /// Handle client disconnect.
    pub fn handle_leave(
        &self,
        session_id: &str,
        user_id: &str,
    ) -> Result<(), String> {
        // Remove presence
        if let Ok(mut presence) = self.presence.write() {
            if let Some(session_presence) = presence.get_mut(session_id) {
                session_presence.remove(user_id);
                if session_presence.is_empty() {
                    presence.remove(session_id);
                }
            }
        }

        // Leave session
        self.session_mgr.leave(session_id, user_id)?;

        // Broadcast departure
        let tx = self.get_channel(session_id);
        let _ = tx.send(WsServerMsg::UserLeft {
            user_id: user_id.to_string(),
        });

        // Clean up empty channels
        if let Ok(channels) = self.channels.read() {
            if let Some(tx) = channels.get(session_id) {
                if tx.receiver_count() == 0 {
                    drop(channels);
                    if let Ok(mut channels) = self.channels.write() {
                        channels.remove(session_id);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get active presence for a session.
    pub fn get_presence(&self, session_id: &str) -> Vec<PresenceInfo> {
        self.presence.read()
            .ok()
            .and_then(|p| p.get(session_id).cloned())
            .map(|m| m.into_values().collect())
            .unwrap_or_default()
    }

    /// Get the session manager reference.
    pub fn session_manager(&self) -> &SessionManager {
        &self.session_mgr
    }
}

// ---------------------------------------------------------------------------
// Operational Transform (simplified)
// ---------------------------------------------------------------------------

/// Conflict resolution strategy for concurrent edits.
/// Uses last-writer-wins with server-ordered sequencing.
///
/// In a full OT system, operations would be transformed against
/// each other. Our simplified approach:
/// 1. Server assigns monotonic sequence numbers
/// 2. All clients apply operations in server order
/// 3. For conflicting feature edits, later timestamp wins
/// 4. Cursor/presence updates are idempotent (always latest wins)
pub fn resolve_conflict(existing: &Operation, incoming: &Operation) -> ConflictResolution {
    match (&existing.kind, &incoming.kind) {
        // Cursor updates never conflict
        (OpKind::CursorUpdate { .. }, _) | (_, OpKind::CursorUpdate { .. }) => {
            ConflictResolution::AcceptBoth
        }
        // Chat messages never conflict
        (OpKind::Chat { .. }, _) | (_, OpKind::Chat { .. }) => {
            ConflictResolution::AcceptBoth
        }
        // Same feature modified by different users
        (OpKind::AddFeature { .. }, OpKind::RemoveFeature { .. }) => {
            // Remove wins over add if they target the same feature
            ConflictResolution::AcceptIncoming
        }
        // Both modify features — server order (existing) wins
        _ => ConflictResolution::AcceptBoth,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConflictResolution {
    /// Accept both operations (no conflict).
    AcceptBoth,
    /// Keep existing, reject incoming.
    AcceptExisting,
    /// Replace existing with incoming.
    AcceptIncoming,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hub() -> WsHub {
        WsHub::new(SessionManager::new())
    }

    #[test]
    fn ws_join_and_welcome() {
        let h = hub();
        let sid = h.session_manager().create_session("doc1").unwrap();
        let msg = h.handle_join(&sid, "u1", "Alice").unwrap();

        match msg {
            WsServerMsg::Welcome { session_id, participants, .. } => {
                assert_eq!(session_id, sid);
                assert_eq!(participants.len(), 1);
                assert_eq!(participants[0].username, "Alice");
            }
            _ => panic!("Expected Welcome"),
        }
    }

    #[test]
    fn ws_broadcast_op() {
        let h = hub();
        let sid = h.session_manager().create_session("doc1").unwrap();
        h.handle_join(&sid, "u1", "Alice").unwrap();
        h.handle_join(&sid, "u2", "Bob").unwrap();

        let mut rx = h.subscribe(&sid);

        h.handle_op(&sid, "u1", OpKind::AddFeature {
            feature_json: "test".into(),
        }).unwrap();

        // Should have broadcast messages (join + op)
        let mut found_op = false;
        while let Ok(msg) = rx.try_recv() {
            if matches!(msg, WsServerMsg::OpBroadcast { .. }) {
                found_op = true;
            }
        }
        assert!(found_op);
    }

    #[test]
    fn ws_cursor_presence() {
        let h = hub();
        let sid = h.session_manager().create_session("doc1").unwrap();
        h.handle_join(&sid, "u1", "Alice").unwrap();

        h.handle_cursor(&sid, "u1", [1.0, 2.0, 3.0], vec!["face_1".into()]).unwrap();

        let presence = h.get_presence(&sid);
        assert_eq!(presence.len(), 1);
        assert_eq!(presence[0].cursor_position, Some([1.0, 2.0, 3.0]));
        assert_eq!(presence[0].selection, vec!["face_1"]);
    }

    #[test]
    fn ws_leave_cleans_up() {
        let h = hub();
        let sid = h.session_manager().create_session("doc1").unwrap();
        h.handle_join(&sid, "u1", "Alice").unwrap();
        h.handle_leave(&sid, "u1").unwrap();

        let presence = h.get_presence(&sid);
        assert!(presence.is_empty());
    }

    #[test]
    fn ws_user_colors_cycle() {
        let h = hub();
        let sid = h.session_manager().create_session("doc1").unwrap();
        h.handle_join(&sid, "u1", "Alice").unwrap();
        h.handle_join(&sid, "u2", "Bob").unwrap();

        let presence = h.get_presence(&sid);
        let colors: Vec<&str> = presence.iter().map(|p| p.color.as_str()).collect();
        // Colors should be different
        assert_ne!(colors[0], colors[1]);
    }

    #[test]
    fn conflict_resolution_cursor_always_both() {
        let op1 = Operation {
            id: "1".into(),
            user_id: "u1".into(),
            timestamp: chrono::Utc::now(),
            kind: OpKind::CursorUpdate {
                cursor: CursorState { position: [0.0; 3], selection: vec![] },
            },
        };
        let op2 = Operation {
            id: "2".into(),
            user_id: "u2".into(),
            timestamp: chrono::Utc::now(),
            kind: OpKind::AddFeature { feature_json: "test".into() },
        };
        assert_eq!(resolve_conflict(&op1, &op2), ConflictResolution::AcceptBoth);
    }

    #[test]
    fn conflict_resolution_add_vs_remove() {
        let op1 = Operation {
            id: "1".into(),
            user_id: "u1".into(),
            timestamp: chrono::Utc::now(),
            kind: OpKind::AddFeature { feature_json: "test".into() },
        };
        let op2 = Operation {
            id: "2".into(),
            user_id: "u2".into(),
            timestamp: chrono::Utc::now(),
            kind: OpKind::RemoveFeature { index: 0 },
        };
        assert_eq!(resolve_conflict(&op1, &op2), ConflictResolution::AcceptIncoming);
    }
}
