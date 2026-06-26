//! Bambu Lab connectivity driver — MQTT + FTPS (LAN mode).
//!
//! Supports Bambu X1C, X1E, P1S, P1P, A1, A1 Mini via LAN protocol.
//! - Status monitoring via MQTT on port 8883 (TLS, self-signed cert)
//! - File upload via FTPS on port 990
//! - Print commands via MQTT publish
//!
//! Authentication: user "bblp" + device access code (from printer LCD).

use async_trait::async_trait;
use physical_connect_core::*;
use rumqttc::{AsyncClient, MqttOptions, QoS, Transport};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Bambu Lab LAN connection.
pub struct BambuConnection {
    info: MachineInfo,
    serial: String,
    access_code: String,
    host: String,
    mqtt_client: Option<AsyncClient>,
    last_status: Arc<RwLock<Option<BambuReport>>>,
}

/// Bambu MQTT status report (subset of the full JSON payload).
#[derive(Clone, Debug, Deserialize, Default)]
#[serde(default)]
struct BambuReport {
    #[serde(rename = "gcode_state")]
    gcode_state: String,
    #[serde(rename = "mc_percent")]
    progress: u32,
    #[serde(rename = "mc_remaining_time")]
    remaining_min: u32,
    nozzle_temper: f64,
    nozzle_target_temper: f64,
    bed_temper: f64,
    bed_target_temper: f64,
    chamber_temper: Option<f64>,
    #[serde(rename = "gcode_file")]
    filename: String,
    layer_num: Option<u32>,
    total_layer_num: Option<u32>,
}

impl BambuConnection {
    pub fn new(config: &MachineConfig) -> Result<Self, ConnectError> {
        let (access_code, serial) = match &config.auth {
            AuthConfig::BambuLan {
                access_code,
                serial,
            } => (access_code.clone(), serial.clone()),
            _ => {
                return Err(ConnectError::AuthFailed(
                    "Bambu LAN requires BambuLan auth with access_code and serial".into(),
                ))
            }
        };

        let host = config.address.split(':').next().unwrap_or(&config.address);

        let id = MachineId::new(format!("bambu-{serial}"));

        Ok(Self {
            info: MachineInfo {
                id,
                name: config.name.clone(),
                kind: MachineKind::Fdm,
                protocol: Protocol::BambuLan,
                address: config.address.clone(),
                accepted_formats: vec![AcceptedFormat::ThreeMf, AcceptedFormat::Gcode],
                build_volume: None,
                firmware: None,
            },
            serial,
            access_code,
            host: host.to_string(),
            mqtt_client: None,
            last_status: Arc::new(RwLock::new(None)),
        })
    }

    /// Initialize MQTT connection to the printer.
    pub async fn ensure_mqtt(&mut self) -> Result<(), ConnectError> {
        if self.mqtt_client.is_some() {
            return Ok(());
        }

        let mut opts = MqttOptions::new("physical-connect", &self.host, 8883);
        opts.set_credentials("bblp", &self.access_code);
        opts.set_transport(Transport::tls_with_default_config());
        opts.set_keep_alive(std::time::Duration::from_secs(30));

        let (client, mut eventloop) = AsyncClient::new(opts, 10);

        // Subscribe to device report topic
        let topic = format!("device/{}/report", self.serial);
        client
            .subscribe(&topic, QoS::AtLeastOnce)
            .await
            .map_err(|e| ConnectError::Protocol(format!("MQTT subscribe failed: {e}")))?;

        // Spawn background task to process incoming messages
        let status = self.last_status.clone();
        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(msg))) => {
                        if let Ok(report) =
                            serde_json::from_slice::<serde_json::Value>(&msg.payload)
                        {
                            if let Some(print_data) = report.get("print") {
                                if let Ok(parsed) =
                                    serde_json::from_value::<BambuReport>(print_data.clone())
                                {
                                    let mut s = status.write().await;
                                    *s = Some(parsed);
                                }
                            }
                        }
                    }
                    Err(_) => break,
                    _ => {}
                }
            }
        });

        self.mqtt_client = Some(client);
        Ok(())
    }

    async fn publish_command(&self, payload: &serde_json::Value) -> Result<(), ConnectError> {
        let client = self
            .mqtt_client
            .as_ref()
            .ok_or_else(|| ConnectError::ConnectionRefused("MQTT not connected".into()))?;

        let topic = format!("device/{}/request", self.serial);
        let data = serde_json::to_vec(payload)?;

        client
            .publish(&topic, QoS::AtLeastOnce, false, data)
            .await
            .map_err(|e| ConnectError::Protocol(format!("MQTT publish failed: {e}")))?;

        Ok(())
    }

    /// Upload a file to the printer via FTPS on port 990.
    async fn upload_ftps(&self, filename: &str, data: &[u8]) -> Result<(), ConnectError> {
        use suppaftp::tokio::AsyncFtpStream;

        let addr = format!("{}:990", self.host);
        let mut ftp: AsyncFtpStream = AsyncFtpStream::connect(&addr)
            .await
            .map_err(|e| ConnectError::ConnectionRefused(format!("FTPS connect failed: {e}")))?;

        ftp.login("bblp", &self.access_code)
            .await
            .map_err(|e| ConnectError::AuthFailed(format!("FTPS login failed: {e}")))?;

        let remote_path = format!("/sdcard/{filename}");
        let mut reader = std::io::Cursor::new(data.to_vec());
        ftp.put_file(&remote_path, &mut reader)
            .await
            .map_err(|e| ConnectError::Protocol(format!("FTPS upload failed: {e}")))?;

        ftp.quit()
            .await
            .map_err(|e| ConnectError::Protocol(format!("FTPS quit failed: {e}")))?;

        Ok(())
    }
}

#[async_trait]
impl MachineConnection for BambuConnection {
    fn info(&self) -> &MachineInfo {
        &self.info
    }

    async fn ping(&self) -> Result<(), ConnectError> {
        // Try a TCP connect to MQTT port as a basic reachability check
        let addr = format!("{}:8883", self.host);
        tokio::net::TcpStream::connect(&addr)
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;
        Ok(())
    }

    async fn status(&self) -> Result<MachineStatus, ConnectError> {
        let report = self.last_status.read().await;

        match report.as_ref() {
            Some(r) => {
                let state = match r.gcode_state.as_str() {
                    "RUNNING" => MachineState::Busy,
                    "PAUSE" => MachineState::Paused,
                    "IDLE" | "FINISH" => MachineState::Idle,
                    "FAILED" => MachineState::Error,
                    _ => MachineState::Idle,
                };

                let mut temps = vec![
                    Temperature {
                        name: "extruder".into(),
                        actual: r.nozzle_temper,
                        target: r.nozzle_target_temper,
                    },
                    Temperature {
                        name: "bed".into(),
                        actual: r.bed_temper,
                        target: r.bed_target_temper,
                    },
                ];
                if let Some(ct) = r.chamber_temper {
                    temps.push(Temperature {
                        name: "chamber".into(),
                        actual: ct,
                        target: 0.0,
                    });
                }

                let active_job = if state == MachineState::Busy || state == MachineState::Paused {
                    Some(JobStatus {
                        state: if state == MachineState::Busy {
                            JobState::Printing
                        } else {
                            JobState::Paused
                        },
                        progress_pct: r.progress as f64,
                        elapsed_s: 0.0, // Bambu doesn't report elapsed in the same field
                        remaining_s: Some(r.remaining_min as f64 * 60.0),
                        layers: match (r.layer_num, r.total_layer_num) {
                            (Some(c), Some(t)) => Some((c, t)),
                            _ => None,
                        },
                        filename: r.filename.clone(),
                    })
                } else {
                    None
                };

                Ok(MachineStatus {
                    state,
                    temperatures: temps,
                    position: MachinePosition::default(),
                    active_job,
                })
            }
            None => Ok(MachineStatus {
                state: MachineState::Offline,
                temperatures: Vec::new(),
                position: MachinePosition::default(),
                active_job: None,
            }),
        }
    }

    async fn submit_job(&self, job: JobSubmission) -> Result<JobHandle, ConnectError> {
        let filename = if job.name.ends_with(".3mf") || job.name.ends_with(".gcode") {
            job.name.clone()
        } else {
            match job.format {
                AcceptedFormat::ThreeMf => format!("{}.3mf", job.name),
                _ => format!("{}.gcode", job.name),
            }
        };

        // Upload via FTPS
        self.upload_ftps(&filename, &job.payload).await?;

        // Send print command via MQTT
        if job.auto_start {
            let cmd = serde_json::json!({
                "print": {
                    "command": "project_file",
                    "param": format!("Metadata/plate_1.gcode"),
                    "subtask_name": &filename,
                    "url": format!("ftp://sdcard/{filename}"),
                    "timelapse": false,
                    "bed_leveling": true,
                    "flow_cali": true,
                    "vibration_cali": true,
                    "use_ams": true
                }
            });
            self.publish_command(&cmd).await?;
        }

        Ok(JobHandle {
            job_id: filename.clone(),
            filename,
        })
    }

    async fn cancel_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        let cmd = serde_json::json!({ "print": { "command": "stop" } });
        self.publish_command(&cmd).await
    }

    async fn pause_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        let cmd = serde_json::json!({ "print": { "command": "pause" } });
        self.publish_command(&cmd).await
    }

    async fn resume_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        let cmd = serde_json::json!({ "print": { "command": "resume" } });
        self.publish_command(&cmd).await
    }

    async fn job_status(&self, _handle: &JobHandle) -> Result<JobStatus, ConnectError> {
        let status = self.status().await?;
        status
            .active_job
            .ok_or_else(|| ConnectError::JobNotFound("no active job".into()))
    }

    async fn send_command(&self, cmd: &str) -> Result<String, ConnectError> {
        let payload = serde_json::json!({
            "print": {
                "command": "gcode_line",
                "param": cmd
            }
        });
        self.publish_command(&payload).await?;
        Ok("ok".into())
    }

    async fn disconnect(&mut self) -> Result<(), ConnectError> {
        if let Some(client) = self.mqtt_client.take() {
            let _ = client.disconnect().await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_bambu() {
        let config = MachineConfig {
            name: "X1C".into(),
            kind: MachineKind::Fdm,
            protocol: Protocol::BambuLan,
            address: "192.168.1.50".into(),
            auth: AuthConfig::BambuLan {
                access_code: "12345678".into(),
                serial: "01S00C123456789".into(),
            },
        };
        let conn = BambuConnection::new(&config).unwrap();
        assert_eq!(conn.info().protocol, Protocol::BambuLan);
        assert!(conn
            .info()
            .accepted_formats
            .contains(&AcceptedFormat::ThreeMf));
    }

    #[test]
    fn reject_wrong_auth() {
        let config = MachineConfig {
            name: "X1C".into(),
            kind: MachineKind::Fdm,
            protocol: Protocol::BambuLan,
            address: "192.168.1.50".into(),
            auth: AuthConfig::ApiKey {
                key: "wrong".into(),
            },
        };
        assert!(BambuConnection::new(&config).is_err());
    }
}
