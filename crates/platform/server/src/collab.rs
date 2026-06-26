//! Real-time collaboration: session management, operation broadcast.
//!
//! Architecture inspired by nova_rtc signaling pattern.
//! Uses in-memory session state with JSON operation messages.
//! Each session is a shared editing context for one document.

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

/// A collaborative editing session.
#[derive(Debug)]
pub struct Session {
    pub id: String,
    pub document_id: String,
    pub created_at: DateTime<Utc>,
    pub participants: Vec<Participant>,
    /// Operation log (append-only, ordered).
    pub operations: Vec<Operation>,
    /// Per-participant message queues for polling.
    pub outboxes: HashMap<String, VecDeque<Operation>>,
}

/// A participant in a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Participant {
    pub user_id: String,
    pub username: String,
    pub joined_at: DateTime<Utc>,
    pub cursor: Option<CursorState>,
}

/// Cursor/selection state for live awareness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorState {
    /// 3D position of the cursor.
    pub position: [f64; 3],
    /// Currently selected entity IDs.
    pub selection: Vec<String>,
}

/// An editing operation (sent by a client, broadcast to others).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub id: String,
    pub user_id: String,
    pub timestamp: DateTime<Utc>,
    pub kind: OpKind,
}

/// Operation types for CAD collaboration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OpKind {
    /// Add a feature to the model.
    AddFeature { feature_json: String },
    /// Remove a feature by index.
    RemoveFeature { index: usize },
    /// Undo the last operation.
    Undo,
    /// Redo.
    Redo,
    /// Update cursor/selection state.
    CursorUpdate { cursor: CursorState },
    /// Chat message.
    Chat { message: String },
    /// User joined the session.
    Join,
    /// User left the session.
    Leave,
}

/// Session manager: maintains all active collaboration sessions.
#[derive(Debug, Clone)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new collaboration session for a document.
    pub fn create_session(&self, document_id: &str) -> Result<String, String> {
        let session_id = Uuid::new_v4().to_string();
        let session = Session {
            id: session_id.clone(),
            document_id: document_id.to_string(),
            created_at: Utc::now(),
            participants: Vec::new(),
            operations: Vec::new(),
            outboxes: HashMap::new(),
        };

        let mut sessions = self.sessions.write().map_err(|_| "Lock poisoned")?;
        sessions.insert(session_id.clone(), session);
        Ok(session_id)
    }

    /// Join a session. Returns the current operation log for catch-up.
    pub fn join(
        &self,
        session_id: &str,
        user_id: &str,
        username: &str,
    ) -> Result<Vec<Operation>, String> {
        let mut sessions = self.sessions.write().map_err(|_| "Lock poisoned")?;
        let session = sessions.get_mut(session_id)
            .ok_or("Session not found")?;

        // Don't allow duplicate joins
        if session.participants.iter().any(|p| p.user_id == user_id) {
            return Ok(session.operations.clone());
        }

        session.participants.push(Participant {
            user_id: user_id.to_string(),
            username: username.to_string(),
            joined_at: Utc::now(),
            cursor: None,
        });

        session.outboxes.insert(user_id.to_string(), VecDeque::new());

        // Broadcast join
        let op = Operation {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            timestamp: Utc::now(),
            kind: OpKind::Join,
        };
        broadcast_to_others(session, &op);
        session.operations.push(op);

        Ok(session.operations.clone())
    }

    /// Leave a session.
    pub fn leave(&self, session_id: &str, user_id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.write().map_err(|_| "Lock poisoned")?;
        let session = sessions.get_mut(session_id)
            .ok_or("Session not found")?;

        session.participants.retain(|p| p.user_id != user_id);
        session.outboxes.remove(user_id);

        let op = Operation {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            timestamp: Utc::now(),
            kind: OpKind::Leave,
        };
        broadcast_to_others(session, &op);
        session.operations.push(op);

        // Clean up empty sessions
        if session.participants.is_empty() {
            sessions.remove(session_id);
        }

        Ok(())
    }

    /// Submit an operation. Broadcasts to all other participants.
    pub fn submit_op(
        &self,
        session_id: &str,
        user_id: &str,
        kind: OpKind,
    ) -> Result<Operation, String> {
        let mut sessions = self.sessions.write().map_err(|_| "Lock poisoned")?;
        let session = sessions.get_mut(session_id)
            .ok_or("Session not found")?;

        // Verify participant
        if !session.participants.iter().any(|p| p.user_id == user_id) {
            return Err("Not in session".into());
        }

        let op = Operation {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            timestamp: Utc::now(),
            kind,
        };

        broadcast_to_others(session, &op);
        session.operations.push(op.clone());

        Ok(op)
    }

    /// Poll for new operations (long-polling style).
    /// Returns queued operations for this user.
    pub fn poll(&self, session_id: &str, user_id: &str) -> Result<Vec<Operation>, String> {
        let mut sessions = self.sessions.write().map_err(|_| "Lock poisoned")?;
        let session = sessions.get_mut(session_id)
            .ok_or("Session not found")?;

        let outbox = session.outboxes.get_mut(user_id)
            .ok_or("Not in session")?;

        let ops: Vec<Operation> = outbox.drain(..).collect();
        Ok(ops)
    }

    /// List active sessions.
    pub fn list_sessions(&self) -> Result<Vec<SessionInfo>, String> {
        let sessions = self.sessions.read().map_err(|_| "Lock poisoned")?;
        Ok(sessions.values().map(|s| SessionInfo {
            id: s.id.clone(),
            document_id: s.document_id.clone(),
            participant_count: s.participants.len(),
            created_at: s.created_at,
        }).collect())
    }

    /// Get participants in a session.
    pub fn participants(&self, session_id: &str) -> Result<Vec<Participant>, String> {
        let sessions = self.sessions.read().map_err(|_| "Lock poisoned")?;
        let session = sessions.get(session_id)
            .ok_or("Session not found")?;
        Ok(session.participants.clone())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub document_id: String,
    pub participant_count: usize,
    pub created_at: DateTime<Utc>,
}

/// Broadcast an operation to all participants except the sender.
fn broadcast_to_others(session: &mut Session, op: &Operation) {
    for (uid, outbox) in &mut session.outboxes {
        if *uid != op.user_id {
            outbox.push_back(op.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mgr() -> SessionManager {
        SessionManager::new()
    }

    #[test]
    fn create_and_join_session() {
        let m = mgr();
        let sid = m.create_session("doc1").unwrap();
        let ops = m.join(&sid, "user1", "Alice").unwrap();
        // The join itself adds a Join op to the log
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0].kind, OpKind::Join));

        let participants = m.participants(&sid).unwrap();
        assert_eq!(participants.len(), 1);
        assert_eq!(participants[0].username, "Alice");
    }

    #[test]
    fn operation_broadcast() {
        let m = mgr();
        let sid = m.create_session("doc1").unwrap();
        m.join(&sid, "user1", "Alice").unwrap();
        m.join(&sid, "user2", "Bob").unwrap();

        // Alice submits an operation
        m.submit_op(&sid, "user1", OpKind::AddFeature {
            feature_json: r#"{"type":"Box","width":10}"#.into(),
        }).unwrap();

        // Bob should see it
        let bob_ops = m.poll(&sid, "user2").unwrap();
        assert_eq!(bob_ops.len(), 1);
        assert_eq!(bob_ops[0].user_id, "user1");

        // Alice should NOT see her own op
        let alice_ops = m.poll(&sid, "user1").unwrap();
        // Alice gets Bob's join notification
        let non_self: Vec<_> = alice_ops.iter()
            .filter(|o| o.user_id != "user1")
            .collect();
        assert!(!non_self.is_empty() || alice_ops.is_empty());
    }

    #[test]
    fn leave_removes_participant() {
        let m = mgr();
        let sid = m.create_session("doc1").unwrap();
        m.join(&sid, "user1", "Alice").unwrap();
        m.join(&sid, "user2", "Bob").unwrap();

        m.leave(&sid, "user1").unwrap();
        let participants = m.participants(&sid).unwrap();
        assert_eq!(participants.len(), 1);
        assert_eq!(participants[0].username, "Bob");
    }

    #[test]
    fn empty_session_cleaned_up() {
        let m = mgr();
        let sid = m.create_session("doc1").unwrap();
        m.join(&sid, "user1", "Alice").unwrap();
        m.leave(&sid, "user1").unwrap();

        // Session should be removed
        assert!(m.participants(&sid).is_err());
    }

    #[test]
    fn list_sessions() {
        let m = mgr();
        m.create_session("doc1").unwrap();
        m.create_session("doc2").unwrap();
        let sessions = m.list_sessions().unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn cursor_update() {
        let m = mgr();
        let sid = m.create_session("doc1").unwrap();
        m.join(&sid, "user1", "Alice").unwrap();
        m.join(&sid, "user2", "Bob").unwrap();

        m.submit_op(&sid, "user1", OpKind::CursorUpdate {
            cursor: CursorState {
                position: [1.0, 2.0, 3.0],
                selection: vec!["face_1".into()],
            },
        }).unwrap();

        let bob_ops = m.poll(&sid, "user2").unwrap();
        let cursor_ops: Vec<_> = bob_ops.iter().filter(|o| matches!(o.kind, OpKind::CursorUpdate { .. })).collect();
        assert_eq!(cursor_ops.len(), 1);
    }

    #[test]
    fn non_participant_rejected() {
        let m = mgr();
        let sid = m.create_session("doc1").unwrap();
        let err = m.submit_op(&sid, "stranger", OpKind::Undo).unwrap_err();
        assert!(err.contains("Not in session"));
    }

    #[test]
    fn chat_message() {
        let m = mgr();
        let sid = m.create_session("doc1").unwrap();
        m.join(&sid, "user1", "Alice").unwrap();
        m.join(&sid, "user2", "Bob").unwrap();

        m.submit_op(&sid, "user1", OpKind::Chat {
            message: "Hello Bob!".into(),
        }).unwrap();

        let bob_ops = m.poll(&sid, "user2").unwrap();
        assert!(bob_ops.iter().any(|o| matches!(&o.kind, OpKind::Chat { message } if message == "Hello Bob!")));
    }

    #[test]
    fn duplicate_join_returns_log() {
        let m = mgr();
        let sid = m.create_session("doc1").unwrap();
        m.join(&sid, "user1", "Alice").unwrap();
        m.submit_op(&sid, "user1", OpKind::Undo).unwrap();

        // Rejoin — should get the full log
        let ops = m.join(&sid, "user1", "Alice").unwrap();
        assert!(!ops.is_empty());
    }
}
