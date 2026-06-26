//! JSON-RPC 2.0 message types.
//!
//! All OMP communication uses JSON-RPC 2.0 over WebSocket text frames.
//! This module defines the wire-level message structures.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 request (client → machine).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub id: MessageId,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 response (machine → client).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: MessageId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// JSON-RPC 2.0 notification (no id, no response expected).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Notification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// Message ID — integer or string.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum MessageId {
    Int(u64),
    Str(String),
}

/// JSON-RPC 2.0 error object.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Any message that can appear on the wire.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Message {
    Request(Request),
    Response(Response),
    Notification(Notification),
    Batch(Vec<Message>),
}

// Standard JSON-RPC error codes
pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

// OMP-specific error codes (-32000 to -32099)
pub const AUTH_REQUIRED: i32 = -32000;
pub const AUTH_FAILED: i32 = -32001;
pub const PERMISSION_DENIED: i32 = -32002;
pub const MACHINE_BUSY: i32 = -32010;
pub const MACHINE_ERROR: i32 = -32011;
pub const MACHINE_OFFLINE: i32 = -32012;
pub const JOB_NOT_FOUND: i32 = -32020;
pub const JOB_INVALID_STATE: i32 = -32021;
pub const FORMAT_NOT_ACCEPTED: i32 = -32030;
pub const UPLOAD_FAILED: i32 = -32031;
pub const STREAM_OVERFLOW: i32 = -32040;
pub const EMERGENCY_STOP: i32 = -32050;

impl Request {
    pub fn new(id: u64, method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id: MessageId::Int(id),
            method: method.into(),
            params,
        }
    }
}

impl Response {
    pub fn ok(id: MessageId, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: MessageId, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

impl Notification {
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            method: method.into(),
            params,
        }
    }
}

/// Binary frame header for file uploads.
///
/// WebSocket binary frames carry file data with this 16-byte prefix:
/// `[8-byte upload_id: u64 LE][4-byte offset: u32 LE][4-byte total_size: u32 LE]`
#[derive(Clone, Copy, Debug)]
pub struct BinaryHeader {
    pub upload_id: u64,
    pub offset: u32,
    pub total_size: u32,
}

impl BinaryHeader {
    pub const SIZE: usize = 16;

    pub fn encode(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&self.upload_id.to_le_bytes());
        buf[8..12].copy_from_slice(&self.offset.to_le_bytes());
        buf[12..16].copy_from_slice(&self.total_size.to_le_bytes());
        buf
    }

    pub fn decode(buf: &[u8; 16]) -> Self {
        Self {
            upload_id: u64::from_le_bytes(buf[0..8].try_into().unwrap()),
            offset: u32::from_le_bytes(buf[8..12].try_into().unwrap()),
            total_size: u32::from_le_bytes(buf[12..16].try_into().unwrap()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_roundtrip() {
        let req = Request::new(1, "machine.capabilities", None);
        let json = serde_json::to_string(&req).unwrap();
        let parsed: Request = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, MessageId::Int(1));
        assert_eq!(parsed.method, "machine.capabilities");
    }

    #[test]
    fn response_ok() {
        let resp = Response::ok(MessageId::Int(1), serde_json::json!({"version": "0.1.0"}));
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("result"));
        assert!(!json.contains("error"));
    }

    #[test]
    fn response_err() {
        let resp = Response::err(MessageId::Int(1), AUTH_FAILED, "bad key");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("error"));
        assert!(json.contains("-32001"));
    }

    #[test]
    fn notification_no_id() {
        let notif = Notification::new("status.update", Some(serde_json::json!({"state": "idle"})));
        let json = serde_json::to_string(&notif).unwrap();
        assert!(!json.contains("\"id\""));
        assert!(json.contains("status.update"));
    }

    #[test]
    fn binary_header_roundtrip() {
        let header = BinaryHeader {
            upload_id: 42,
            offset: 1024,
            total_size: 65536,
        };
        let encoded = header.encode();
        let decoded = BinaryHeader::decode(&encoded);
        assert_eq!(decoded.upload_id, 42);
        assert_eq!(decoded.offset, 1024);
        assert_eq!(decoded.total_size, 65536);
    }
}
