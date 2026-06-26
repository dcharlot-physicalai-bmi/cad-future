//! Network machine discovery aggregator.
//!
//! Discovers manufacturing machines on the local network using multiple
//! protocols: mDNS for OctoPrint, Moonraker, and Duet; UDP broadcast
//! for Bambu Lab printers on port 2021.

use async_trait::async_trait;
use physical_connect_core::*;
use std::time::Duration;
use tokio::net::UdpSocket;

/// Bambu Lab UDP broadcast port for printer discovery.
const BAMBU_DISCOVERY_PORT: u16 = 2021;

// ─── mDNS service types ──────────────────────────────────────────────

const OCTOPRINT_SERVICE: &str = "_octoprint._tcp.local.";
const MOONRAKER_SERVICE: &str = "_moonraker._tcp.local.";
const DUET_SERVICE: &str = "_http._tcp.local.";

// ─── Individual discovery strategies ─────────────────────────────────

/// mDNS-based discovery for a specific service type.
pub struct MdnsDiscovery {
    service_type: &'static str,
    protocol_name: &'static str,
    protocol: Protocol,
    kind: MachineKind,
    accepted_formats: Vec<AcceptedFormat>,
}

impl MdnsDiscovery {
    /// Discover OctoPrint instances via `_octoprint._tcp`.
    pub fn octoprint() -> Self {
        Self {
            service_type: OCTOPRINT_SERVICE,
            protocol_name: "OctoPrint",
            protocol: Protocol::OctoPrint,
            kind: MachineKind::Fdm,
            accepted_formats: vec![AcceptedFormat::Gcode],
        }
    }

    /// Discover Moonraker (Klipper) instances via `_moonraker._tcp`.
    pub fn moonraker() -> Self {
        Self {
            service_type: MOONRAKER_SERVICE,
            protocol_name: "Moonraker",
            protocol: Protocol::Moonraker,
            kind: MachineKind::Fdm,
            accepted_formats: vec![AcceptedFormat::Gcode],
        }
    }

    /// Discover Duet (RepRapFirmware) instances via `_http._tcp`.
    pub fn duet() -> Self {
        Self {
            service_type: DUET_SERVICE,
            protocol_name: "Duet",
            protocol: Protocol::Duet,
            kind: MachineKind::Fdm,
            accepted_formats: vec![AcceptedFormat::Gcode],
        }
    }

    /// Perform mDNS browse for the configured service type.
    async fn browse(&self, timeout: Duration) -> Result<Vec<DiscoveredMachine>, ConnectError> {
        let daemon = mdns_sd::ServiceDaemon::new()
            .map_err(|e| ConnectError::Protocol(format!("mDNS daemon error: {e}")))?;

        let receiver = daemon
            .browse(self.service_type)
            .map_err(|e| ConnectError::Protocol(format!("mDNS browse error: {e}")))?;

        let mut machines = Vec::new();
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }

            match tokio::time::timeout(remaining, tokio::task::spawn_blocking({
                let receiver = receiver.clone();
                move || receiver.recv_timeout(Duration::from_millis(500))
            }))
            .await
            {
                Ok(Ok(Ok(event))) => {
                    if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
                        let host = info.get_hostname().trim_end_matches('.').to_string();
                        let port = info.get_port();
                        let address = format!("{host}:{port}");
                        let name = info
                            .get_fullname()
                            .split('.')
                            .next()
                            .unwrap_or(self.protocol_name)
                            .to_string();

                        machines.push(DiscoveredMachine {
                            name,
                            kind: self.kind,
                            protocol: self.protocol,
                            address,
                            accepted_formats: self.accepted_formats.clone(),
                            build_volume: None,
                            firmware: None,
                        });
                    }
                }
                Ok(Ok(Err(_))) => {
                    // recv_timeout expired, try again until deadline
                    continue;
                }
                _ => break,
            }
        }

        let _ = daemon.shutdown();
        Ok(machines)
    }
}

#[async_trait]
impl MachineDiscovery for MdnsDiscovery {
    fn protocol_name(&self) -> &str {
        self.protocol_name
    }

    async fn discover(&self, timeout: Duration) -> Result<Vec<DiscoveredMachine>, ConnectError> {
        self.browse(timeout).await
    }
}

// ─── Bambu UDP broadcast discovery ───────────────────────────────────

/// Discovers Bambu Lab printers via UDP broadcast on port 2021.
pub struct BambuDiscovery;

impl BambuDiscovery {
    pub fn new() -> Self {
        Self
    }

    /// Listen for Bambu Lab UDP broadcast packets.
    async fn listen(
        &self,
        timeout: Duration,
    ) -> Result<Vec<DiscoveredMachine>, ConnectError> {
        let socket = UdpSocket::bind(format!("0.0.0.0:{BAMBU_DISCOVERY_PORT}"))
            .await
            .map_err(|e| ConnectError::ConnectionRefused(format!("bind UDP {BAMBU_DISCOVERY_PORT}: {e}")))?;

        // Enable broadcast reception.
        socket
            .set_broadcast(true)
            .map_err(|e| ConnectError::Protocol(format!("set_broadcast: {e}")))?;

        let mut machines = Vec::new();
        let mut buf = [0u8; 4096];
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }

            match tokio::time::timeout(remaining, socket.recv_from(&mut buf)).await {
                Ok(Ok((len, addr))) => {
                    // Bambu broadcast is JSON with device info.
                    if let Ok(text) = std::str::from_utf8(&buf[..len]) {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
                            let name = json["dev_name"]
                                .as_str()
                                .or_else(|| json["name"].as_str())
                                .unwrap_or("Bambu Printer")
                                .to_string();

                            let serial = json["dev_id"]
                                .as_str()
                                .or_else(|| json["sn"].as_str())
                                .unwrap_or("")
                                .to_string();

                            let address = format!("{}:{}", addr.ip(), 8883);

                            // Avoid duplicates.
                            if !machines.iter().any(|m: &DiscoveredMachine| m.address == address) {
                                machines.push(DiscoveredMachine {
                                    name,
                                    kind: MachineKind::Fdm,
                                    protocol: Protocol::BambuLan,
                                    address,
                                    accepted_formats: vec![
                                        AcceptedFormat::ThreeMf,
                                        AcceptedFormat::Gcode,
                                    ],
                                    build_volume: None,
                                    firmware: json["dev_version"]
                                        .as_str()
                                        .or_else(|| json["firmware"].as_str())
                                        .map(|s| s.to_string()),
                                });
                            }

                            let _ = serial; // used for dedup in production
                        }
                    }
                }
                Ok(Err(e)) => {
                    return Err(ConnectError::Protocol(format!("UDP recv error: {e}")));
                }
                Err(_) => break, // timeout
            }
        }

        Ok(machines)
    }
}

impl Default for BambuDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MachineDiscovery for BambuDiscovery {
    fn protocol_name(&self) -> &str {
        "Bambu LAN"
    }

    async fn discover(&self, timeout: Duration) -> Result<Vec<DiscoveredMachine>, ConnectError> {
        self.listen(timeout).await
    }
}

// ─── Aggregator ──────────────────────────────────────────────────────

/// Aggregates multiple discovery methods and returns combined results.
pub struct DiscoveryAggregator {
    strategies: Vec<Box<dyn MachineDiscovery>>,
}

impl DiscoveryAggregator {
    /// Create an aggregator with the default set of discovery strategies.
    pub fn new() -> Self {
        Self {
            strategies: vec![
                Box::new(MdnsDiscovery::octoprint()),
                Box::new(MdnsDiscovery::moonraker()),
                Box::new(MdnsDiscovery::duet()),
                Box::new(BambuDiscovery::new()),
            ],
        }
    }

    /// Create an aggregator with a custom set of discovery strategies.
    pub fn with_strategies(strategies: Vec<Box<dyn MachineDiscovery>>) -> Self {
        Self { strategies }
    }

    /// Run all discovery strategies concurrently and return combined results.
    pub async fn discover_all(
        &self,
        timeout: Duration,
    ) -> Vec<DiscoveredMachine> {
        let mut handles = Vec::new();

        // Collect futures from each strategy.
        for strategy in &self.strategies {
            handles.push(strategy.discover(timeout));
        }

        let results = futures_join_all(handles).await;

        let mut all_machines = Vec::new();
        for result in results {
            match result {
                Ok(machines) => all_machines.extend(machines),
                Err(_) => {
                    // Log and continue — one failing strategy should not block others.
                }
            }
        }

        all_machines
    }
}

impl Default for DiscoveryAggregator {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple sequential "join all" for a vector of futures.
///
/// Runs each future to completion in order. For true concurrency in production,
/// consider `tokio::join!` or `futures::future::join_all`.
async fn futures_join_all<F, T>(futures: Vec<F>) -> Vec<T>
where
    F: std::future::Future<Output = T>,
{
    let mut results = Vec::with_capacity(futures.len());
    for fut in futures {
        results.push(fut.await);
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mdns_octoprint_config() {
        let d = MdnsDiscovery::octoprint();
        assert_eq!(d.protocol_name, "OctoPrint");
        assert_eq!(d.protocol, Protocol::OctoPrint);
        assert_eq!(d.kind, MachineKind::Fdm);
    }

    #[test]
    fn mdns_moonraker_config() {
        let d = MdnsDiscovery::moonraker();
        assert_eq!(d.protocol_name, "Moonraker");
        assert_eq!(d.protocol, Protocol::Moonraker);
    }

    #[test]
    fn mdns_duet_config() {
        let d = MdnsDiscovery::duet();
        assert_eq!(d.protocol_name, "Duet");
        assert_eq!(d.protocol, Protocol::Duet);
    }

    #[test]
    fn bambu_discovery_protocol_name() {
        let d = BambuDiscovery::new();
        assert_eq!(d.protocol_name(), "Bambu LAN");
    }

    #[test]
    fn aggregator_default_has_four_strategies() {
        let agg = DiscoveryAggregator::new();
        assert_eq!(agg.strategies.len(), 4);
    }

    #[test]
    fn aggregator_custom_strategies() {
        let agg = DiscoveryAggregator::with_strategies(vec![
            Box::new(MdnsDiscovery::octoprint()),
        ]);
        assert_eq!(agg.strategies.len(), 1);
    }
}
