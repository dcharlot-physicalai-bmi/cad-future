# OpenIE Manufacturing Protocol (OMP) v0.1.0

**A unified, open protocol for controlling any manufacturing machine.**

OMP replaces the 28+ proprietary protocols across 3D printers, CNC machines, and laser cutters with a single WebSocket-based JSON-RPC 2.0 protocol. One client, any machine.

---

## Transport

| Layer | Choice | Rationale |
|---|---|---|
| Transport | WebSocket (RFC 6455) | Full-duplex, firewall-friendly, browser-native |
| Framing | Text frames for JSON-RPC, binary frames for file uploads | Efficient file transfer without base64 overhead |
| Encoding | JSON-RPC 2.0 | Standard, debuggable, well-tooled |
| Discovery | mDNS `_openie-mfg._tcp` | Zero-config LAN discovery |
| Default Port | **3720** | Registered for OMP |
| TLS | Optional (recommended for non-LAN) | WSS for encrypted connections |

### Connection URL

```
ws://{host}:3720/omp
wss://{host}:3720/omp
```

---

## Message Format

All text frames are JSON-RPC 2.0 messages.

### Request (client → machine)

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "machine.status",
  "params": {}
}
```

### Response (machine → client)

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": { "state": "idle", ... }
}
```

### Error Response

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": { "code": -32001, "message": "Authentication failed" }
}
```

### Notification (machine → client, no `id`)

```json
{
  "jsonrpc": "2.0",
  "method": "status.update",
  "params": { "state": "running", ... }
}
```

### Batch

Multiple messages may be sent as a JSON array.

---

## Connection Lifecycle

```
Client                          Machine
  |                                |
  |--- WebSocket connect --------->|
  |                                |
  |--- hello ---------------------->|
  |<-- capabilities response ------|
  |                                |
  |    (authenticated session)     |
  |                                |
  |--- goodbye ------------------->|
  |<-- close ----------------------|
```

### `hello` — Handshake

**Client → Machine.** First message after WebSocket connect.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "hello",
  "params": {
    "protocol_version": "0.1.0",
    "client_name": "OpenIE CAD",
    "client_version": "1.0.0",
    "auth": { "method": "api_key", "key": "..." }
  }
}
```

**Auth methods:**

| Method | Params | Use Case |
|---|---|---|
| `none` | — | Open access on trusted LAN |
| `api_key` | `{ "key": "..." }` | Pre-shared key |
| `bearer` | `{ "token": "..." }` | OAuth2 / cloud relay |
| `mtls` | — | Mutual TLS (auth at transport layer) |

**Response:** Full `MachineCapabilities` object (see below).

### `goodbye` — Graceful Disconnect

```json
{ "jsonrpc": "2.0", "id": 99, "method": "goodbye" }
```

---

## Machine Capabilities

Returned in the `hello` response. The client adapts its UI and behavior based on what the machine declares — no hardcoded per-vendor knowledge.

```json
{
  "protocol_version": "0.1.0",
  "machine_id": "PRINTER-001",
  "name": "Bambu Lab X1C",
  "manufacturer": "Bambu Lab",
  "model": "X1 Carbon",
  "firmware_version": "1.8.2",
  "machine_type": {
    "type": "fdm",
    "extruder_count": 1,
    "heated_bed": true,
    "heated_chamber": true,
    "filament_sensor": true,
    "auto_bed_leveling": true
  },
  "build_volume": { "x_mm": 256, "y_mm": 256, "z_mm": 256, "is_cylindrical": false },
  "accepted_formats": [
    { "mime_type": "application/x-gcode", "extension": "gcode", "preferred": false },
    { "mime_type": "application/vnd.ms-package.3dmanufacturing-3dmodel+xml", "extension": "3mf", "preferred": true }
  ],
  "axes": [
    { "name": "X", "min_mm": 0, "max_mm": 256, "max_feed_mm_min": 12000, "max_accel_mm_s2": 5000, "home_mm": 0, "homeable": true },
    { "name": "Y", "min_mm": 0, "max_mm": 256, "max_feed_mm_min": 12000, "max_accel_mm_s2": 5000, "home_mm": 0, "homeable": true },
    { "name": "Z", "min_mm": 0, "max_mm": 256, "max_feed_mm_min": 600, "max_accel_mm_s2": 500, "home_mm": 0, "homeable": true }
  ],
  "heaters": [
    { "name": "extruder_0", "kind": "extruder", "max_temp_c": 300, "pid": true },
    { "name": "bed", "kind": "bed", "max_temp_c": 110, "pid": true }
  ],
  "spindles": [],
  "lasers": [],
  "tool_changer": null,
  "enclosure": { "heated": true, "filtered": true, "camera": true, "lighting": true },
  "features": [
    "pause_resume", "layer_tracking", "filament_tracking", "camera_stream",
    "gcode_streaming", "raw_gcode", "emergency_stop", "jog", "home",
    "filament_runout", "power_loss_recovery"
  ],
  "max_queue_depth": 1,
  "stream_buffer_size": 4096
}
```

### Machine Types

| Type | Tag | Extra Fields |
|---|---|---|
| FDM/FFF 3D Printer | `fdm` | extruder_count, heated_bed, heated_chamber, filament_sensor, auto_bed_leveling |
| SLA/DLP Resin | `sla` | technology, pixel_size_um, uv_power_w |
| CNC Mill | `cnc_mill` | axis_count, atc_slots, coolant, probe |
| CNC Lathe | `cnc_lathe` | live_tooling, turret_positions |
| Laser Cutter | `laser` | source (co2/fiber/diode/green_dpss/uv), power_w, pulse_capable, rotary_axis |

### Features

| Feature | Description |
|---|---|
| `pause_resume` | Can pause and resume jobs |
| `layer_tracking` | Reports current layer number |
| `filament_tracking` | Reports filament usage |
| `camera_stream` | Provides camera stream URL |
| `power_loss_recovery` | Can resume after power loss |
| `firmware_update` | Supports remote firmware update |
| `filament_runout` | Has filament runout detection |
| `clog_detection` | Has nozzle clog detection |
| `gcode_streaming` | Can stream G-code in real-time |
| `raw_gcode` | Accepts raw G-code commands |
| `emergency_stop` | Supports emergency stop |
| `jog` | Can do relative jog moves |
| `home` | Can home axes |
| `probe` | Can probe (bed leveling, tool length) |

---

## Methods Reference

### Machine Info

| Method | Direction | Description |
|---|---|---|
| `machine.capabilities` | Client → Machine | Get full capability declaration |
| `machine.status` | Client → Machine | Get current status snapshot |
| `machine.status.subscribe` | Client → Machine | Subscribe to status pushes |
| `machine.status.unsubscribe` | Client → Machine | Unsubscribe from status pushes |

#### `machine.status.subscribe`

```json
{ "jsonrpc": "2.0", "id": 2, "method": "machine.status.subscribe", "params": { "interval_ms": 500 } }
```

Interval range: 100–5000 ms. Machine will send `status.update` notifications at the requested rate.

### Job Management

| Method | Direction | Description |
|---|---|---|
| `job.submit` | Client → Machine | Submit a new job |
| `job.start` | Client → Machine | Start a queued job |
| `job.pause` | Client → Machine | Pause a running job |
| `job.resume` | Client → Machine | Resume a paused job |
| `job.cancel` | Client → Machine | Cancel a job |
| `job.status` | Client → Machine | Get job status |
| `job.list` | Client → Machine | List all jobs |
| `job.upload.complete` | Machine → Client | Upload fully received |

#### `job.submit`

```json
{
  "jsonrpc": "2.0", "id": 3,
  "method": "job.submit",
  "params": {
    "name": "bracket_v3.gcode",
    "format": "gcode",
    "size_bytes": 1048576,
    "auto_start": true,
    "metadata": { "material": "PLA", "layer_height_mm": 0.2 }
  }
}
```

**Response:**

```json
{
  "jsonrpc": "2.0", "id": 3,
  "result": {
    "job_id": "job-abc123",
    "upload_id": 42,
    "state": "queued"
  }
}
```

After receiving the `upload_id`, the client sends the file data as binary WebSocket frames.

#### Job State Machine

```
Queued → Running → Complete
   ↓        ↓
   ↓      Paused → Running
   ↓        ↓
   └→ Cancelled ←────┘
   └→ Failed ←───────┘
```

Valid transitions:
- Queued → Running, Cancelled
- Running → Paused, Complete, Cancelled, Failed
- Paused → Running, Cancelled, Failed

Terminal states: Complete, Cancelled, Failed.

### G-code Streaming

For real-time G-code execution (vs. upload-first workflows).

| Method | Direction | Description |
|---|---|---|
| `gcode.send` | Client → Machine | Send G-code lines |
| `gcode.ack` | Machine → Client | Line acknowledged |
| `gcode.error` | Machine → Client | Line error |
| `gcode.buffer` | Client → Machine | Query buffer state |

#### Flow Control

The machine declares `stream_buffer_size` in capabilities. The client tracks available buffer space:

```
available = capacity - in_flight

if available >= lines_to_send:
    send gcode.send { lines, sequence }
    in_flight += lines_to_send

on gcode.ack:
    in_flight -= acked_count

on gcode.error:
    if continues: in_flight -= 1  (error line skipped, execution continues)
    else: stop sending (machine halted)
```

#### `gcode.send`

```json
{
  "jsonrpc": "2.0", "id": 10,
  "method": "gcode.send",
  "params": {
    "lines": ["G28", "G1 X50 Y50 Z10 F3000", "G1 X100 F6000"],
    "sequence": 1
  }
}
```

**Response:** `{ "buffered": 3, "rejected": 0 }`

#### `gcode.ack` (notification)

```json
{ "jsonrpc": "2.0", "method": "gcode.ack", "params": { "sequence": 1, "count": 3, "response": "ok" } }
```

#### `gcode.error` (notification)

```json
{
  "jsonrpc": "2.0", "method": "gcode.error",
  "params": {
    "sequence": 1,
    "line": "G1 X999",
    "error": "Soft endstop exceeded",
    "continues": true
  }
}
```

### Manual Control

| Method | Direction | Description |
|---|---|---|
| `control.home` | Client → Machine | Home axes |
| `control.jog` | Client → Machine | Relative jog move |
| `control.temperature` | Client → Machine | Set heater temperature |
| `control.fan` | Client → Machine | Set fan speed |
| `control.spindle` | Client → Machine | Set spindle RPM |
| `control.laser` | Client → Machine | Set laser power |
| `control.speed` | Client → Machine | Set speed override |
| `control.flow` | Client → Machine | Set flow override |
| `control.emergency_stop` | Client → Machine | **EMERGENCY STOP** |
| `control.reset` | Client → Machine | Reset after E-stop |

#### `control.home`

```json
{ "jsonrpc": "2.0", "id": 20, "method": "control.home", "params": { "axes": ["X", "Y", "Z"] } }
```

Empty `axes` array = home all.

#### `control.jog`

```json
{ "jsonrpc": "2.0", "id": 21, "method": "control.jog", "params": { "x": 10.0, "y": 0, "z": 0, "feed_mm_min": 3000 } }
```

#### `control.temperature`

```json
{ "jsonrpc": "2.0", "id": 22, "method": "control.temperature", "params": { "heater": "extruder_0", "target_c": 215.0 } }
```

#### `control.emergency_stop`

```json
{ "jsonrpc": "2.0", "id": 99, "method": "control.emergency_stop" }
```

**MUST** be processed immediately. Machine halts all motion, disables heaters/spindle/laser. No params required.

### Status Notifications (Machine → Client)

| Method | Description |
|---|---|
| `status.update` | Periodic status push (subscribed) |
| `job.state_changed` | Job transitioned state |
| `error.occurred` | Machine error detected |
| `error.cleared` | Machine error cleared |

#### `status.update`

```json
{
  "jsonrpc": "2.0",
  "method": "status.update",
  "params": {
    "state": "running",
    "heaters": [
      { "name": "extruder_0", "current_c": 214.8, "target_c": 215.0, "power": 0.12 },
      { "name": "bed", "current_c": 59.7, "target_c": 60.0, "power": 0.05 }
    ],
    "position": {
      "machine": { "x": 125.3, "y": 89.1, "z": 12.4 },
      "work": { "x": 125.3, "y": 89.1, "z": 12.4 }
    },
    "job": {
      "progress_pct": 42.3,
      "current_layer": 84,
      "total_layers": 200,
      "elapsed_s": 1820,
      "remaining_s": 2480,
      "feed_multiplier": 1.0,
      "flow_multiplier": 1.0
    },
    "fans": [{ "name": "part_cooling", "speed": 1.0 }],
    "errors": []
  }
}
```

### Machine States

| State | Description |
|---|---|
| `booting` | Firmware initializing |
| `idle` | Ready for commands |
| `running` | Executing a job |
| `paused` | Job paused |
| `homing` | Axes homing in progress |
| `probing` | Bed/tool probing |
| `tool_changing` | Tool change in progress |
| `heating` | Waiting for temperature target |
| `cooling` | Cooling down |
| `error` | Recoverable error |
| `emergency` | Emergency stop active |
| `updating` | Firmware update in progress |

---

## Binary Frame Protocol

File uploads use WebSocket binary frames with a 16-byte header:

```
Bytes 0–7:   upload_id  (u64 LE)  — matches job.submit response
Bytes 8–11:  offset     (u32 LE)  — byte offset in file
Bytes 12–15: total_size (u32 LE)  — total file size
Bytes 16+:   payload              — file data chunk
```

Chunk size is implementation-defined (recommended: 32 KB). Client sends chunks sequentially. Machine sends `job.upload.complete` notification when all bytes received.

---

## Error Codes

### Standard JSON-RPC

| Code | Name | Description |
|---|---|---|
| -32700 | Parse Error | Invalid JSON |
| -32600 | Invalid Request | Not a valid JSON-RPC request |
| -32601 | Method Not Found | Method does not exist |
| -32602 | Invalid Params | Invalid method parameters |
| -32603 | Internal Error | Server internal error |

### OMP-Specific

| Code | Name | Description |
|---|---|---|
| -32000 | Auth Required | Authentication required but not provided |
| -32001 | Auth Failed | Authentication credentials rejected |
| -32002 | Permission Denied | Authenticated but insufficient permissions |
| -32010 | Machine Busy | Machine cannot accept command in current state |
| -32011 | Machine Error | Machine is in error state |
| -32012 | Machine Offline | Machine is not responding |
| -32020 | Job Not Found | Referenced job does not exist |
| -32021 | Job Invalid State | Job cannot transition to requested state |
| -32030 | Format Not Accepted | File format not in machine's accepted_formats |
| -32031 | Upload Failed | File upload failed (checksum, disk space, etc.) |
| -32040 | Stream Overflow | G-code buffer full, try again |
| -32050 | Emergency Stop | Machine is in emergency stop state |

---

## Authorization

Capability-based permissions system. After authentication, the machine declares what the client can do.

### Permission Levels

| Permission | Description |
|---|---|
| `status_read` | Can read machine status |
| `job_manage` | Can submit and manage jobs |
| `manual_control` | Can send manual G-code and control commands |
| `set_parameters` | Can change temperatures, fan speeds, etc. |
| `emergency_stop` | Can trigger emergency stop |
| `firmware_update` | Can update firmware |
| `admin` | Can manage users and API keys |
| `full` | Superset of all permissions |

### Presets

| Preset | Permissions |
|---|---|
| **Full** | `full` |
| **Operator** | `status_read`, `job_manage`, `manual_control`, `set_parameters`, `emergency_stop` |
| **Read-Only** | `status_read` |

---

## Discovery

OMP machines advertise via mDNS:

- Service type: `_openie-mfg._tcp`
- Port: 3720
- TXT records:
  - `omp_version=0.1.0`
  - `machine_id=PRINTER-001`
  - `machine_name=My Printer`
  - `machine_type=fdm`

---

## Implementation Notes

### For Firmware Authors

The `physical-connect-firmware` Rust crate provides a ready-made server implementation. Implement the `MachineHandler` trait (16 methods mapping OMP commands to your hardware) and the crate handles all protocol parsing, message dispatch, and state management.

```rust
pub trait MachineHandler {
    fn status(&self) -> MachineStatus;
    fn execute_gcode(&mut self, lines: &[String]) -> u32;
    fn start_job(&mut self, job_id: &str) -> Result<(), String>;
    fn pause_job(&mut self) -> Result<(), String>;
    fn resume_job(&mut self) -> Result<(), String>;
    fn cancel_job(&mut self) -> Result<(), String>;
    fn job_status(&self, job_id: &str) -> Option<JobStatus>;
    fn home(&mut self, axes: &[String]) -> Result<(), String>;
    fn jog(&mut self, x: f64, y: f64, z: f64, feed: f64) -> Result<(), String>;
    fn set_temperature(&mut self, heater: &str, target_c: f64) -> Result<(), String>;
    fn set_fan(&mut self, fan: &str, speed: f64) -> Result<(), String>;
    fn emergency_stop(&mut self) -> Result<(), String>;
    fn reset(&mut self) -> Result<(), String>;
    fn receive_upload_chunk(&mut self, upload_id: u64, offset: u32, data: &[u8]) -> Result<(), String>;
    fn begin_uploaded_job(&mut self, job_id: &str) -> Result<(), String>;
    fn buffer_status(&self) -> BufferStatus;
}
```

The protocol crate (`physical-connect-protocol`) is `no_std` compatible and runs on ESP32 and RP2040.

### For Client Authors

The `physical-connect-openie` Rust crate provides a full client implementing the `MachineConnection` trait. Handles WebSocket connection, hello handshake, background notification processing, and G-code flow control.

---

## Versioning

OMP uses semantic versioning. The `hello` handshake includes `protocol_version` from both sides. Machines SHOULD reject clients with incompatible major versions.

- **0.x.y** — Development versions. Breaking changes expected.
- **1.0.0** — First stable release. Backward-compatible additions only in 1.x.

---

*OpenIE Manufacturing Protocol — one protocol to replace them all.*
