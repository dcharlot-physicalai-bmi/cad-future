//! `physical-server` — Axum-based cloud server for the Physical AI platform.
//!
//! Provides:
//! - JWT-based user authentication (`auth`)
//! - Filesystem-backed project file storage (`storage`)
//! - REST route handlers for auth, files, and health (`routes`)
//! - Manufacturing machine CRUD, job management, and manual control (`machine_routes`)
//! - Real-time collaboration sessions with polling support (`collab`)
//! - WebSocket push for live operation broadcast and presence (`websocket`)
//! - CRDT-based conflict-free concurrent editing (`crdt`)
//! - Git-like design versioning with branches, merges, and rollback (`versioning`)

pub mod auth;
pub mod collab;
pub mod crdt;
pub mod machine_routes;
pub mod routes;
pub mod storage;
pub mod versioning;
pub mod websocket;

// ---------------------------------------------------------------------------
// Auth re-exports
// ---------------------------------------------------------------------------
pub use auth::{AuthError, AuthResponse, Claims, LoginRequest, RegisterRequest, User, UserStore};

// ---------------------------------------------------------------------------
// Storage re-exports
// ---------------------------------------------------------------------------
pub use storage::{FileMeta, FileStore};

// ---------------------------------------------------------------------------
// Routes re-exports
// ---------------------------------------------------------------------------
pub use routes::AppState;

// ---------------------------------------------------------------------------
// Collab re-exports
// ---------------------------------------------------------------------------
pub use collab::{
    CursorState, OpKind, Operation, Participant, Session, SessionInfo, SessionManager,
};

// ---------------------------------------------------------------------------
// WebSocket re-exports
// ---------------------------------------------------------------------------
pub use websocket::{
    ConflictResolution, PresenceInfo, ViewportState, WsClientMsg, WsHub, WsServerMsg,
};

// ---------------------------------------------------------------------------
// CRDT re-exports
// ---------------------------------------------------------------------------
pub use crdt::{
    AddTag, CausalNode, CausalTree, Conflict, ConflictType, CrdtDelta, CrdtDocument, FeatureOp,
    GCounter, LwwRegister, OpId, OrSet, PnCounter, SiteId,
};

// ---------------------------------------------------------------------------
// Versioning re-exports
// ---------------------------------------------------------------------------
pub use versioning::{
    Branch, BranchInfo, DesignHistory, MergeConflict, MergeResult, RollbackResult, Version,
    VersionManager, VersionedOp,
};
