//! Machine registry — manages all connected machines.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{ConnectError, MachineConnection, MachineId, MachineInfo, MachineStatus};

/// Thread-safe registry of all known machine connections.
///
/// The server maintains a single `MachineRegistry` in its `AppState`.
/// Protocol drivers are registered here after connection.
#[derive(Clone)]
pub struct MachineRegistry {
    machines: Arc<RwLock<HashMap<MachineId, Arc<RwLock<Box<dyn MachineConnection>>>>>>,
}

impl MachineRegistry {
    pub fn new() -> Self {
        Self {
            machines: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new machine connection.
    pub async fn register(
        &self,
        id: MachineId,
        connection: Box<dyn MachineConnection>,
    ) {
        let mut machines = self.machines.write().await;
        machines.insert(id, Arc::new(RwLock::new(connection)));
    }

    /// Remove a machine from the registry, disconnecting it.
    pub async fn remove(&self, id: &MachineId) -> Result<(), ConnectError> {
        let mut machines = self.machines.write().await;
        if let Some(conn) = machines.remove(id) {
            let mut conn = conn.write().await;
            conn.disconnect().await?;
        }
        Ok(())
    }

    /// Get info for all registered machines.
    pub async fn list(&self) -> Vec<MachineInfo> {
        let machines = self.machines.read().await;
        let mut infos = Vec::with_capacity(machines.len());
        for conn in machines.values() {
            let conn = conn.read().await;
            infos.push(conn.info().clone());
        }
        infos
    }

    /// Get info for a specific machine.
    pub async fn get_info(&self, id: &MachineId) -> Option<MachineInfo> {
        let machines = self.machines.read().await;
        let conn = machines.get(id)?;
        let conn = conn.read().await;
        Some(conn.info().clone())
    }

    /// Query status of a specific machine.
    pub async fn status(&self, id: &MachineId) -> Result<MachineStatus, ConnectError> {
        let machines = self.machines.read().await;
        let conn = machines
            .get(id)
            .ok_or_else(|| ConnectError::ConnectionRefused(format!("machine {id} not found")))?;
        let conn = conn.read().await;
        conn.status().await
    }

    /// Get a cloned Arc to a machine's connection for direct interaction.
    pub async fn get_connection(
        &self,
        id: &MachineId,
    ) -> Option<Arc<RwLock<Box<dyn MachineConnection>>>> {
        let machines = self.machines.read().await;
        machines.get(id).cloned()
    }

    /// Number of registered machines.
    pub async fn count(&self) -> usize {
        let machines = self.machines.read().await;
        machines.len()
    }
}

impl Default for MachineRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_registry() {
        let reg = MachineRegistry::new();
        assert_eq!(reg.count().await, 0);
        assert!(reg.list().await.is_empty());
    }
}
