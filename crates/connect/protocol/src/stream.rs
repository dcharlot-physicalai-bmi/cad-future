//! G-code streaming protocol — bidirectional flow control.
//!
//! Unlike legacy serial protocols that use character-counting or line-based
//! flow control, OMP uses explicit buffer management:
//!
//! 1. Machine declares `stream_buffer_size` in capabilities.
//! 2. Client sends G-code lines via `gcode.send` with sequence numbers.
//! 3. Machine acknowledges each line via `gcode.ack` notification.
//! 4. Client tracks unacknowledged lines and respects buffer capacity.
//!
//! This replaces Marlin's `ok` counting, GRBL's 128-byte character counting,
//! and every other ad-hoc flow control mechanism.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// G-code send request — client → machine.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GcodeSendRequest {
    /// G-code lines to execute. Can be one or more.
    pub lines: Vec<String>,
    /// Starting sequence number for this batch.
    /// Must be monotonically increasing across the session.
    pub sequence: u64,
}

/// G-code send response — machine → client.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GcodeSendResponse {
    /// Number of lines accepted into the buffer.
    pub buffered: u32,
    /// Number of lines rejected (buffer full).
    pub rejected: u32,
}

/// G-code acknowledgment — machine → client (notification).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GcodeAck {
    /// Sequence number of the acknowledged line.
    pub sequence: u64,
    /// Machine response (e.g., "ok", temperature report).
    pub response: String,
    /// The G-code line that was executed.
    pub line: String,
}

/// G-code error — machine → client (notification).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GcodeError {
    /// Sequence number of the failed line.
    pub sequence: u64,
    /// Error message.
    pub error: String,
    /// The G-code line that failed.
    pub line: String,
    /// Whether execution continues (true) or halts (false).
    pub continues: bool,
}

/// Buffer status — response to `gcode.buffer` query.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BufferStatus {
    /// Total buffer capacity in lines.
    pub capacity: u32,
    /// Lines currently buffered (waiting to execute).
    pub used: u32,
    /// Available space.
    pub free: u32,
    /// Last acknowledged sequence number.
    pub last_ack_sequence: u64,
}

/// Client-side stream tracker.
///
/// Tracks sent vs. acknowledged lines for flow control.
#[derive(Debug)]
pub struct StreamTracker {
    /// Next sequence number to assign.
    next_sequence: u64,
    /// Highest acknowledged sequence number.
    last_ack: u64,
    /// Machine's declared buffer size in lines.
    buffer_capacity: u32,
    /// Number of lines currently in-flight (sent but not yet acked).
    in_flight: u32,
}

impl StreamTracker {
    pub fn new(buffer_capacity: u32) -> Self {
        Self {
            next_sequence: 0,
            last_ack: 0,
            buffer_capacity,
            in_flight: 0,
        }
    }

    /// How many lines can be sent without overflowing the machine's buffer.
    pub fn available(&self) -> u32 {
        self.buffer_capacity.saturating_sub(self.in_flight)
    }

    /// Whether the machine's buffer can accept more lines.
    pub fn can_send(&self) -> bool {
        self.available() > 0
    }

    /// Record that lines were sent. Returns the sequence number for this batch.
    pub fn mark_sent(&mut self, count: u32) -> u64 {
        let seq = self.next_sequence;
        self.next_sequence += count as u64;
        self.in_flight += count;
        seq
    }

    /// Record that a line was acknowledged.
    pub fn mark_acked(&mut self, sequence: u64) {
        if sequence > self.last_ack {
            let delta = (sequence - self.last_ack) as u32;
            self.last_ack = sequence;
            self.in_flight = self.in_flight.saturating_sub(delta);
        }
    }

    /// Number of lines currently in-flight.
    pub fn in_flight(&self) -> u32 {
        self.in_flight
    }

    /// Next sequence number that will be assigned.
    pub fn next_sequence(&self) -> u64 {
        self.next_sequence
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_tracker_basic() {
        let mut tracker = StreamTracker::new(16);
        assert_eq!(tracker.available(), 16);
        assert!(tracker.can_send());

        // Send 5 lines
        let seq = tracker.mark_sent(5);
        assert_eq!(seq, 0);
        assert_eq!(tracker.in_flight(), 5);
        assert_eq!(tracker.available(), 11);

        // Ack 3 lines
        tracker.mark_acked(3);
        assert_eq!(tracker.in_flight(), 2);
        assert_eq!(tracker.available(), 14);
    }

    #[test]
    fn stream_tracker_full() {
        let mut tracker = StreamTracker::new(4);

        tracker.mark_sent(4);
        assert_eq!(tracker.available(), 0);
        assert!(!tracker.can_send());

        tracker.mark_acked(2);
        assert_eq!(tracker.available(), 2);
        assert!(tracker.can_send());
    }

    #[test]
    fn gcode_send_roundtrip() {
        let req = GcodeSendRequest {
            lines: alloc::vec!["G28".into(), "G1 X10 Y10 F1000".into()],
            sequence: 42,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: GcodeSendRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.lines.len(), 2);
        assert_eq!(parsed.sequence, 42);
    }

    #[test]
    fn sequence_monotonic() {
        let mut tracker = StreamTracker::new(100);
        let s1 = tracker.mark_sent(3);
        let s2 = tracker.mark_sent(2);
        let s3 = tracker.mark_sent(1);
        assert_eq!(s1, 0);
        assert_eq!(s2, 3);
        assert_eq!(s3, 5);
        assert_eq!(tracker.next_sequence(), 6);
    }
}
