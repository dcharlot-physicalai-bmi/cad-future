//! OpenIE Manufacturing Protocol client driver.
//!
//! Implements `MachineConnection` over the OMP WebSocket protocol.
//! This is the native, first-class driver — all other protocol drivers
//! are legacy bridges.

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use physical_connect_core::*;
use physical_connect_protocol::capability::MachineCapabilities;
use physical_connect_protocol::job as omp_job;
use physical_connect_protocol::message::*;
use physical_connect_protocol::method::{self, AuthMethod, HelloRequest};
use physical_connect_protocol::status as omp_status;
use physical_connect_protocol::stream::StreamTracker;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::tungstenite::Message as WsMessage;

/// OMP client connection.
pub struct OmpConnection {
    info: MachineInfo,
    capabilities: Arc<RwLock<Option<MachineCapabilities>>>,
    last_status: Arc<RwLock<Option<omp_status::MachineStatus>>>,
    sender: Arc<RwLock<Option<mpsc::Sender<WsMessage>>>>,
    next_id: Arc<RwLock<u64>>,
    pending: Arc<RwLock<std::collections::HashMap<MessageId, tokio::sync::oneshot::Sender<Response>>>>,
    stream_tracker: Arc<RwLock<StreamTracker>>,
    ws_url: String,
    auth: AuthMethod,
}

impl OmpConnection {
    /// Create a new OMP connection (does not connect yet).
    pub fn new(config: &MachineConfig) -> Result<Self, ConnectError> {
        let auth = match &config.auth {
            AuthConfig::None => AuthMethod::None,
            AuthConfig::ApiKey { key } => AuthMethod::ApiKey { key: key.clone() },
            AuthConfig::BearerToken { token } => AuthMethod::Bearer { token: token.clone() },
            _ => AuthMethod::None,
        };

        let ws_url = if config.address.starts_with("ws") {
            config.address.clone()
        } else {
            format!(
                "ws://{}:{}",
                config.address,
                physical_connect_protocol::DEFAULT_PORT
            )
        };

        let id = MachineId::new(format!(
            "omp-{}",
            config.address.replace([':', '/', '.'], "-")
        ));

        Ok(Self {
            info: MachineInfo {
                id,
                name: config.name.clone(),
                kind: config.kind,
                protocol: Protocol::OctoPrint, // Will be updated after hello
                address: config.address.clone(),
                accepted_formats: Vec::new(), // Will be populated from capabilities
                build_volume: None,
                firmware: None,
            },
            capabilities: Arc::new(RwLock::new(None)),
            last_status: Arc::new(RwLock::new(None)),
            sender: Arc::new(RwLock::new(None)),
            next_id: Arc::new(RwLock::new(1)),
            pending: Arc::new(RwLock::new(std::collections::HashMap::new())),
            stream_tracker: Arc::new(RwLock::new(StreamTracker::new(16))),
            ws_url,
            auth,
        })
    }

    /// Establish WebSocket connection and perform hello handshake.
    pub async fn connect(&mut self) -> Result<(), ConnectError> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(&self.ws_url)
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        let (mut ws_write, mut ws_read) = ws_stream.split();

        // Set up channel for sending messages
        let (tx, mut rx) = mpsc::channel::<WsMessage>(64);
        {
            let mut sender = self.sender.write().await;
            *sender = Some(tx.clone());
        }

        // Spawn writer task
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if ws_write.send(msg).await.is_err() {
                    break;
                }
            }
        });

        // Send hello
        let hello = HelloRequest {
            protocol_version: physical_connect_protocol::PROTOCOL_VERSION.into(),
            client_name: "OpenIE CAD".into(),
            client_version: "0.1.0".into(),
            auth: self.auth.clone(),
        };

        let hello_req = Request::new(
            0,
            method::names::HELLO,
            Some(serde_json::to_value(&hello).unwrap()),
        );

        tx.send(WsMessage::Text(serde_json::to_string(&hello_req).unwrap().into()))
            .await
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;

        // Wait for hello response with capabilities
        let pending = self.pending.clone();
        let capabilities = self.capabilities.clone();
        let last_status = self.last_status.clone();
        let stream_tracker = self.stream_tracker.clone();

        // Spawn reader task
        tokio::spawn(async move {
            while let Some(Ok(msg)) = ws_read.next().await {
                if let WsMessage::Text(text) = msg {
                    // Try to parse as Response
                    if let Ok(resp) = serde_json::from_str::<Response>(&text) {
                        // Check if this is the hello response (id=0)
                        if resp.id == MessageId::Int(0) {
                            if let Some(result) = &resp.result {
                                if let Ok(caps) = serde_json::from_value::<MachineCapabilities>(result.clone()) {
                                    let mut tracker = stream_tracker.write().await;
                                    *tracker = StreamTracker::new(caps.stream_buffer_size);
                                    let mut c = capabilities.write().await;
                                    *c = Some(caps);
                                }
                            }
                        }

                        let mut p = pending.write().await;
                        if let Some(tx) = p.remove(&resp.id) {
                            let _ = tx.send(resp);
                        }
                        continue;
                    }

                    // Try to parse as Notification
                    if let Ok(notif) = serde_json::from_str::<Notification>(&text) {
                        match notif.method.as_str() {
                            method::names::STATUS_UPDATE => {
                                if let Some(params) = notif.params {
                                    if let Ok(status) = serde_json::from_value::<omp_status::MachineStatus>(params) {
                                        let mut s = last_status.write().await;
                                        *s = Some(status);
                                    }
                                }
                            }
                            _ => {} // Handle other notifications as needed
                        }
                    }
                }
            }
        });

        // Give the hello response time to arrive
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Update info from capabilities
        if let Some(caps) = self.capabilities.read().await.as_ref() {
            self.info.name = caps.name.clone();
            self.info.firmware = Some(caps.firmware_version.clone());
            self.info.accepted_formats = caps
                .accepted_formats
                .iter()
                .map(|f| match f.extension.as_str() {
                    "gcode" => AcceptedFormat::Gcode,
                    "3mf" => AcceptedFormat::ThreeMf,
                    "bgcode" => AcceptedFormat::BinaryGcode,
                    "stl" => AcceptedFormat::Stl,
                    "ufp" => AcceptedFormat::Ufp,
                    "rd" => AcceptedFormat::RuidaRd,
                    "dxf" => AcceptedFormat::Dxf,
                    _ => AcceptedFormat::Gcode,
                })
                .collect();
            self.info.build_volume = Some([
                caps.build_volume.x_mm,
                caps.build_volume.y_mm,
                caps.build_volume.z_mm,
            ]);
        }

        Ok(())
    }

    /// Send a JSON-RPC request and wait for the response.
    async fn rpc_call(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, ConnectError> {
        let id = {
            let mut next = self.next_id.write().await;
            let id = *next;
            *next += 1;
            id
        };

        let req = Request::new(id, method, params);
        let msg_id = req.id.clone();

        // Create response channel
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut pending = self.pending.write().await;
            pending.insert(msg_id, tx);
        }

        // Send request
        let sender = self.sender.read().await;
        let sender = sender
            .as_ref()
            .ok_or_else(|| ConnectError::ConnectionRefused("not connected".into()))?;

        sender
            .send(WsMessage::Text(serde_json::to_string(&req).unwrap().into()))
            .await
            .map_err(|e| ConnectError::Protocol(e.to_string()))?;

        // Wait for response (5 second timeout)
        let resp = tokio::time::timeout(std::time::Duration::from_secs(5), rx)
            .await
            .map_err(|_| ConnectError::Timeout("RPC call timed out".into()))?
            .map_err(|_| ConnectError::Protocol("response channel dropped".into()))?;

        if let Some(err) = resp.error {
            return Err(ConnectError::Protocol(format!(
                "RPC error {}: {}",
                err.code, err.message
            )));
        }

        Ok(resp.result.unwrap_or(serde_json::Value::Null))
    }
}

#[async_trait]
impl MachineConnection for OmpConnection {
    fn info(&self) -> &MachineInfo {
        &self.info
    }

    async fn ping(&self) -> Result<(), ConnectError> {
        self.rpc_call(method::names::STATUS, None).await?;
        Ok(())
    }

    async fn status(&self) -> Result<MachineStatus, ConnectError> {
        // Try cached status first (from push notifications)
        if let Some(omp_st) = self.last_status.read().await.as_ref() {
            return Ok(convert_status(omp_st));
        }

        // Fall back to explicit query
        let result = self.rpc_call(method::names::STATUS, None).await?;
        let omp_st: omp_status::MachineStatus = serde_json::from_value(result)?;

        let mut cached = self.last_status.write().await;
        *cached = Some(omp_st.clone());

        Ok(convert_status(&omp_st))
    }

    async fn submit_job(&self, job: JobSubmission) -> Result<JobHandle, ConnectError> {
        let mime = match job.format {
            AcceptedFormat::Gcode => "application/x-gcode",
            AcceptedFormat::ThreeMf => "application/vnd.ms-package.3dmanufacturing-3dmodel+xml",
            AcceptedFormat::BinaryGcode => "application/x-bgcode",
            AcceptedFormat::Stl => "application/sla",
            AcceptedFormat::Ufp => "application/x-ufp",
            AcceptedFormat::RuidaRd => "application/x-ruida-rd",
            AcceptedFormat::Dxf => "application/dxf",
            _ => "application/octet-stream",
        };

        let ext = match job.format {
            AcceptedFormat::Gcode => "gcode",
            AcceptedFormat::ThreeMf => "3mf",
            AcceptedFormat::BinaryGcode => "bgcode",
            AcceptedFormat::Stl => "stl",
            AcceptedFormat::Ufp => "ufp",
            AcceptedFormat::RuidaRd => "rd",
            AcceptedFormat::Dxf => "dxf",
            _ => "bin",
        };

        let submit_req = omp_job::JobSubmitRequest {
            name: job.name.clone(),
            mime_type: mime.into(),
            extension: ext.into(),
            size_bytes: job.payload.len() as u64,
            auto_start: job.auto_start,
            metadata: None,
        };

        let result = self
            .rpc_call(
                method::names::JOB_SUBMIT,
                Some(serde_json::to_value(&submit_req).unwrap()),
            )
            .await?;

        let resp: omp_job::JobSubmitResponse = serde_json::from_value(result)?;

        // Upload file data via binary frames
        let sender = self.sender.read().await;
        let sender = sender
            .as_ref()
            .ok_or_else(|| ConnectError::ConnectionRefused("not connected".into()))?;

        let chunk_size = resp.max_chunk_bytes as usize;
        let total = job.payload.len() as u32;

        for (i, chunk) in job.payload.chunks(chunk_size).enumerate() {
            let offset = (i * chunk_size) as u32;
            let header = BinaryHeader {
                upload_id: resp.upload_id,
                offset,
                total_size: total,
            };

            let mut frame = Vec::with_capacity(BinaryHeader::SIZE + chunk.len());
            frame.extend_from_slice(&header.encode());
            frame.extend_from_slice(chunk);

            sender
                .send(WsMessage::Binary(frame.into()))
                .await
                .map_err(|e| ConnectError::Protocol(e.to_string()))?;
        }

        Ok(JobHandle {
            job_id: resp.job_id,
            filename: job.name,
        })
    }

    async fn cancel_job(&self, handle: &JobHandle) -> Result<(), ConnectError> {
        self.rpc_call(
            method::names::JOB_CANCEL,
            Some(serde_json::json!({ "job_id": handle.job_id })),
        )
        .await?;
        Ok(())
    }

    async fn pause_job(&self, handle: &JobHandle) -> Result<(), ConnectError> {
        self.rpc_call(
            method::names::JOB_PAUSE,
            Some(serde_json::json!({ "job_id": handle.job_id })),
        )
        .await?;
        Ok(())
    }

    async fn resume_job(&self, handle: &JobHandle) -> Result<(), ConnectError> {
        self.rpc_call(
            method::names::JOB_RESUME,
            Some(serde_json::json!({ "job_id": handle.job_id })),
        )
        .await?;
        Ok(())
    }

    async fn job_status(&self, handle: &JobHandle) -> Result<JobStatus, ConnectError> {
        let result = self
            .rpc_call(
                method::names::JOB_STATUS,
                Some(serde_json::json!({ "job_id": handle.job_id })),
            )
            .await?;

        let omp_js: omp_job::JobStatus = serde_json::from_value(result)?;

        Ok(JobStatus {
            state: match omp_js.state {
                omp_job::JobState::Queued => physical_connect_core::JobState::Queued,
                omp_job::JobState::Running => physical_connect_core::JobState::Printing,
                omp_job::JobState::Paused => physical_connect_core::JobState::Paused,
                omp_job::JobState::Complete => physical_connect_core::JobState::Complete,
                omp_job::JobState::Cancelled => physical_connect_core::JobState::Cancelled,
                omp_job::JobState::Failed => physical_connect_core::JobState::Failed,
            },
            progress_pct: omp_js.progress_pct,
            elapsed_s: omp_js.elapsed_s,
            remaining_s: omp_js.remaining_s,
            layers: omp_js.layer.map(|l| (l.current, l.total)),
            filename: omp_js.filename,
        })
    }

    async fn send_command(&self, cmd: &str) -> Result<String, ConnectError> {
        let lines: Vec<String> = cmd.lines().map(|l| l.to_string()).collect();
        let seq = {
            let mut tracker = self.stream_tracker.write().await;
            tracker.mark_sent(lines.len() as u32)
        };

        let send_req = physical_connect_protocol::stream::GcodeSendRequest {
            lines,
            sequence: seq,
        };

        let result = self
            .rpc_call(
                method::names::GCODE_SEND,
                Some(serde_json::to_value(&send_req).unwrap()),
            )
            .await?;

        Ok(result.to_string())
    }

    async fn disconnect(&mut self) -> Result<(), ConnectError> {
        // Send goodbye
        let _ = self.rpc_call(method::names::GOODBYE, None).await;

        // Drop sender to close WebSocket
        let mut sender = self.sender.write().await;
        *sender = None;

        Ok(())
    }
}

/// Convert OMP status to core MachineStatus.
fn convert_status(omp: &omp_status::MachineStatus) -> MachineStatus {
    let state = match omp.state {
        omp_status::MachineState::Idle => MachineState::Idle,
        omp_status::MachineState::Running => MachineState::Busy,
        omp_status::MachineState::Paused => MachineState::Paused,
        omp_status::MachineState::Error | omp_status::MachineState::Emergency => {
            MachineState::Error
        }
        omp_status::MachineState::Booting
        | omp_status::MachineState::Homing
        | omp_status::MachineState::Probing
        | omp_status::MachineState::ToolChanging
        | omp_status::MachineState::Heating
        | omp_status::MachineState::Cooling
        | omp_status::MachineState::Updating => MachineState::Busy,
    };

    let temperatures = omp
        .heaters
        .iter()
        .map(|h| Temperature {
            name: h.name.clone(),
            actual: h.actual_c,
            target: h.target_c,
        })
        .collect();

    let position = MachinePosition {
        x: omp.position.work.x,
        y: omp.position.work.y,
        z: omp.position.work.z,
    };

    let active_job = omp.job.as_ref().map(|j| {
        physical_connect_core::JobStatus {
            state: if omp.state == omp_status::MachineState::Paused {
                physical_connect_core::JobState::Paused
            } else {
                physical_connect_core::JobState::Printing
            },
            progress_pct: j.progress_pct,
            elapsed_s: j.elapsed_s,
            remaining_s: j.remaining_s,
            layers: j.layer.as_ref().map(|l| (l.current, l.total)),
            filename: j.filename.clone(),
        }
    });

    MachineStatus {
        state,
        temperatures,
        position,
        active_job,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_connection() {
        let config = MachineConfig {
            name: "Test Machine".into(),
            kind: MachineKind::Fdm,
            protocol: Protocol::OctoPrint, // Will be OMP in practice
            address: "192.168.1.100".into(),
            auth: AuthConfig::ApiKey {
                key: "test-key".into(),
            },
        };
        let conn = OmpConnection::new(&config).unwrap();
        assert_eq!(
            conn.ws_url,
            format!("ws://192.168.1.100:{}", physical_connect_protocol::DEFAULT_PORT)
        );
    }

    #[test]
    fn ws_url_passthrough() {
        let config = MachineConfig {
            name: "Test".into(),
            kind: MachineKind::CncMill,
            protocol: Protocol::OctoPrint,
            address: "ws://custom:9999/omp".into(),
            auth: AuthConfig::None,
        };
        let conn = OmpConnection::new(&config).unwrap();
        assert_eq!(conn.ws_url, "ws://custom:9999/omp");
    }

    #[test]
    fn convert_idle_status() {
        let omp_st = omp_status::MachineStatus {
            state: omp_status::MachineState::Idle,
            heaters: vec![omp_status::HeaterStatus {
                name: "extruder_0".into(),
                actual_c: 25.0,
                target_c: 0.0,
                power: 0.0,
            }],
            position: omp_status::Position::default(),
            job: None,
            spindle: None,
            laser: None,
            fans: Vec::new(),
            error: None,
            uptime_s: 100.0,
            free_storage_bytes: None,
        };

        let status = convert_status(&omp_st);
        assert_eq!(status.state, MachineState::Idle);
        assert_eq!(status.temperatures.len(), 1);
        assert!(status.active_job.is_none());
    }
}
