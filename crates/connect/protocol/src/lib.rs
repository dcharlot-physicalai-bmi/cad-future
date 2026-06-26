//! OpenIE Manufacturing Protocol (OMP) — unified machine connectivity.
//!
//! A single protocol to replace OctoPrint, Moonraker, Bambu MQTT, PrusaLink,
//! Duet, Repetier, FluidNC, Serial Marlin/GRBL, LinuxCNC, and every other
//! proprietary machine interface.
//!
//! ## Design principles
//!
//! 1. **One transport**: WebSocket + JSON-RPC 2.0. Push and pull on one connection.
//! 2. **Capability negotiation**: machine declares what it can do on connect.
//! 3. **Typed everything**: no string parsing, no regex, no guessing.
//! 4. **Bidirectional streaming**: G-code down, acks up, status interleaved.
//! 5. **Job lifecycle**: first-class state machine with structured errors.
//! 6. **Format agnostic**: upload bytes + MIME type, machine declares what it accepts.
//! 7. **Auth done right**: mTLS for LAN, OAuth2 for cloud, capability-based permissions.
//! 8. **Embeddable**: `no_std` compatible core for ESP32 / RP2040 firmware.
//!
//! ## Wire format
//!
//! All messages are JSON-RPC 2.0 over WebSocket text frames:
//!
//! ```json
//! // Request (client → machine)
//! {"jsonrpc":"2.0","id":1,"method":"job.submit","params":{...}}
//!
//! // Response (machine → client)
//! {"jsonrpc":"2.0","id":1,"result":{...}}
//!
//! // Notification (machine → client, no id)
//! {"jsonrpc":"2.0","method":"status.update","params":{...}}
//! ```
//!
//! Binary payloads (G-code files, 3MF archives) use WebSocket binary frames
//! with a 16-byte header: `[8-byte upload_id][4-byte offset][4-byte total_size]`.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod capability;
pub mod error;
pub mod message;
pub mod method;
pub mod status;
pub mod job;
pub mod stream;
pub mod auth;

/// Protocol version. Machines and clients negotiate on this.
pub const PROTOCOL_VERSION: &str = "0.1.0";

/// Default WebSocket port for OMP.
pub const DEFAULT_PORT: u16 = 3720;

/// mDNS service type for discovery.
pub const MDNS_SERVICE: &str = "_openie-mfg._tcp";
