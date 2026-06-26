//! MTConnect HTTP/XML connectivity driver.
//!
//! Implements the MTConnect standard for industrial CNC machine monitoring.
//! MTConnect is a read-only protocol: it provides machine state, position,
//! and telemetry but cannot submit jobs or control the machine.
//!
//! Endpoints:
//! - `/probe`   — Device information and available data items
//! - `/current` — Current snapshot of all data items
//! - `/sample`  — Historical data items with sequence filtering
//!
//! Reference: <https://www.mtconnect.org/standard>

use async_trait::async_trait;
use physical_connect_core::*;
use quick_xml::Reader;
use quick_xml::events::Event;
use reqwest::Client;

/// MTConnect connection via HTTP + XML.
pub struct MtConnectConnection {
    info: MachineInfo,
    client: Client,
    base_url: String,
}

impl MtConnectConnection {
    /// Create a new MTConnect connection.
    ///
    /// Accepts `AuthConfig::None` only (MTConnect has no standard auth).
    pub fn new(config: &MachineConfig) -> Result<Self, ConnectError> {
        if !matches!(config.auth, AuthConfig::None) {
            return Err(ConnectError::AuthFailed(
                "MTConnect does not use authentication".into(),
            ));
        }

        let base_url = if config.address.starts_with("http") {
            config.address.clone()
        } else {
            format!("http://{}", config.address)
        };

        let id = MachineId::new(format!(
            "mtconnect-{}",
            config.address.replace([':', '/', '.'], "-")
        ));

        Ok(Self {
            info: MachineInfo {
                id,
                name: config.name.clone(),
                kind: MachineKind::CncMill,
                protocol: Protocol::MtConnect,
                address: config.address.clone(),
                accepted_formats: vec![], // Monitoring only
                build_volume: None,
                firmware: None,
            },
            client: Client::new(),
            base_url,
        })
    }

    /// Fetch XML from an MTConnect endpoint.
    async fn fetch_xml(&self, path: &str) -> Result<String, ConnectError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ConnectError::ConnectionRefused(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ConnectError::Protocol(format!("HTTP {}", resp.status())));
        }

        resp.text()
            .await
            .map_err(|e| ConnectError::Protocol(e.to_string()))
    }
}

/// Extract a named DataItem value from MTConnect XML.
///
/// Searches for elements whose `dataItemId` or `name` attribute matches,
/// or whose local tag name matches `item_name`, and returns the text content.
fn extract_data_item(xml: &str, item_name: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let item_lower = item_name.to_lowercase();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.local_name().as_ref()).to_lowercase();

                // Check tag name match
                let matches_tag = tag_name == item_lower;

                // Check dataItemId or name attribute match
                let matches_attr = e.attributes().filter_map(|a| a.ok()).any(|a| {
                    let key = String::from_utf8_lossy(a.key.as_ref()).to_lowercase();
                    let val = String::from_utf8_lossy(&a.value).to_lowercase();
                    (key == "dataitemid" || key == "name") && val == item_lower
                });

                if matches_tag || matches_attr {
                    // Read text content
                    if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) {
                        return Some(
                            quick_xml::escape::unescape(
                                &String::from_utf8_lossy(&t),
                            )
                            .map(|s| s.to_string())
                            .unwrap_or_default(),
                        );
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

/// Parse execution state from MTConnect XML into MachineState.
fn parse_execution_state(xml: &str) -> MachineState {
    match extract_data_item(xml, "Execution").as_deref() {
        Some("ACTIVE") => MachineState::Busy,
        Some("INTERRUPTED") | Some("FEED_HOLD") => MachineState::Paused,
        Some("STOPPED") | Some("READY") => MachineState::Idle,
        Some("UNAVAILABLE") => MachineState::Offline,
        _ => MachineState::Offline,
    }
}

/// Parse position data from MTConnect XML.
fn parse_position(xml: &str) -> MachinePosition {
    MachinePosition {
        x: extract_data_item(xml, "Xposition")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0),
        y: extract_data_item(xml, "Yposition")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0),
        z: extract_data_item(xml, "Zposition")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0),
    }
}

#[async_trait]
impl MachineConnection for MtConnectConnection {
    fn info(&self) -> &MachineInfo {
        &self.info
    }

    async fn ping(&self) -> Result<(), ConnectError> {
        self.fetch_xml("/probe").await?;
        Ok(())
    }

    async fn status(&self) -> Result<MachineStatus, ConnectError> {
        let xml = self.fetch_xml("/current").await?;

        let state = parse_execution_state(&xml);
        let position = parse_position(&xml);

        // Extract spindle temperature if available
        let mut temperatures = Vec::new();
        if let Some(temp_str) = extract_data_item(&xml, "Temperature") {
            if let Ok(temp_val) = temp_str.parse::<f64>() {
                temperatures.push(Temperature {
                    name: "spindle".into(),
                    actual: temp_val,
                    target: 0.0,
                });
            }
        }

        // Extract program info for active job
        let active_job = extract_data_item(&xml, "Program").map(|prog| {
            let job_state = match state {
                MachineState::Busy => JobState::Printing,
                MachineState::Paused => JobState::Paused,
                _ => JobState::Complete,
            };
            JobStatus {
                state: job_state,
                progress_pct: 0.0,
                elapsed_s: 0.0,
                remaining_s: None,
                layers: None,
                filename: prog,
            }
        });

        Ok(MachineStatus {
            state,
            temperatures,
            position,
            active_job,
        })
    }

    async fn submit_job(&self, _job: JobSubmission) -> Result<JobHandle, ConnectError> {
        Err(ConnectError::Unsupported(
            "MTConnect is a read-only monitoring protocol".into(),
        ))
    }

    async fn cancel_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        Err(ConnectError::Unsupported(
            "MTConnect is a read-only monitoring protocol".into(),
        ))
    }

    async fn pause_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        Err(ConnectError::Unsupported(
            "MTConnect is a read-only monitoring protocol".into(),
        ))
    }

    async fn resume_job(&self, _handle: &JobHandle) -> Result<(), ConnectError> {
        Err(ConnectError::Unsupported(
            "MTConnect is a read-only monitoring protocol".into(),
        ))
    }

    async fn job_status(&self, handle: &JobHandle) -> Result<JobStatus, ConnectError> {
        // We can at least check the current execution state
        let xml = self.fetch_xml("/current").await?;
        let state = parse_execution_state(&xml);

        let job_state = match state {
            MachineState::Busy => JobState::Printing,
            MachineState::Paused => JobState::Paused,
            MachineState::Idle => JobState::Complete,
            _ => JobState::Failed,
        };

        Ok(JobStatus {
            state: job_state,
            progress_pct: 0.0,
            elapsed_s: 0.0,
            remaining_s: None,
            layers: None,
            filename: handle.filename.clone(),
        })
    }

    async fn send_command(&self, _cmd: &str) -> Result<String, ConnectError> {
        Err(ConnectError::Unsupported(
            "MTConnect is a read-only monitoring protocol".into(),
        ))
    }

    async fn disconnect(&mut self) -> Result<(), ConnectError> {
        // HTTP is stateless; nothing to close.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_connection() {
        let config = MachineConfig {
            name: "Shop Floor Agent".into(),
            kind: MachineKind::CncMill,
            protocol: Protocol::MtConnect,
            address: "192.168.1.100:5000".into(),
            auth: AuthConfig::None,
        };
        let conn = MtConnectConnection::new(&config).unwrap();
        assert_eq!(conn.info().protocol, Protocol::MtConnect);
        assert_eq!(conn.info().kind, MachineKind::CncMill);
        assert!(conn.info().accepted_formats.is_empty());
        assert_eq!(conn.info().name, "Shop Floor Agent");
    }

    #[test]
    fn reject_wrong_auth() {
        let config = MachineConfig {
            name: "Test".into(),
            kind: MachineKind::CncMill,
            protocol: Protocol::MtConnect,
            address: "localhost".into(),
            auth: AuthConfig::BearerToken {
                token: "nope".into(),
            },
        };
        assert!(MtConnectConnection::new(&config).is_err());
    }

    #[test]
    fn extract_execution_from_xml() {
        let xml = r#"<?xml version="1.0"?>
<MTConnectStreams>
  <Streams>
    <DeviceStream>
      <ComponentStream>
        <Events>
          <Execution dataItemId="exec1">ACTIVE</Execution>
          <ControllerMode dataItemId="mode1">AUTOMATIC</ControllerMode>
        </Events>
      </ComponentStream>
    </DeviceStream>
  </Streams>
</MTConnectStreams>"#;

        assert_eq!(parse_execution_state(xml), MachineState::Busy);
    }

    #[test]
    fn extract_position_from_xml() {
        let xml = r#"<?xml version="1.0"?>
<MTConnectStreams>
  <Streams>
    <DeviceStream>
      <ComponentStream>
        <Samples>
          <Position dataItemId="Xposition">12.345</Position>
          <Position dataItemId="Yposition">-6.789</Position>
          <Position dataItemId="Zposition">0.100</Position>
        </Samples>
      </ComponentStream>
    </DeviceStream>
  </Streams>
</MTConnectStreams>"#;

        let pos = parse_position(xml);
        assert!((pos.x - 12.345).abs() < 0.001);
        assert!((pos.y - -6.789).abs() < 0.001);
        assert!((pos.z - 0.1).abs() < 0.001);
    }

    #[test]
    fn extract_stopped_state() {
        let xml = r#"<Events><Execution dataItemId="exec1">STOPPED</Execution></Events>"#;
        assert_eq!(parse_execution_state(xml), MachineState::Idle);
    }

    #[test]
    fn extract_unavailable_state() {
        let xml = r#"<Events><Execution dataItemId="exec1">UNAVAILABLE</Execution></Events>"#;
        assert_eq!(parse_execution_state(xml), MachineState::Offline);
    }

    #[test]
    fn extract_data_item_by_name() {
        let xml = r#"<Samples><Temperature name="Temperature">42.5</Temperature></Samples>"#;
        assert_eq!(
            extract_data_item(xml, "Temperature"),
            Some("42.5".to_string())
        );
    }

    #[test]
    fn empty_formats_for_monitoring() {
        let config = MachineConfig {
            name: "Monitor".into(),
            kind: MachineKind::CncMill,
            protocol: Protocol::MtConnect,
            address: "10.0.0.1:5000".into(),
            auth: AuthConfig::None,
        };
        let conn = MtConnectConnection::new(&config).unwrap();
        assert!(conn.info().accepted_formats.is_empty());
    }
}
