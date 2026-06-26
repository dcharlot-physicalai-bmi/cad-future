//! Authentication and authorization.
//!
//! OMP supports three auth modes:
//! 1. **None** — open access on trusted LAN.
//! 2. **API Key** — pre-shared key, simple and effective.
//! 3. **mTLS** — mutual TLS for zero-trust LAN. Machine and client both present certificates.
//! 4. **OAuth2 Bearer** — for cloud relay connections.
//!
//! Authorization uses capability-based permissions. The machine declares
//! what the authenticated client can do.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Permissions granted to an authenticated client.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Permissions {
    /// Allowed operations.
    pub allowed: Vec<Permission>,
}

/// Individual permission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// Can read machine status.
    StatusRead,
    /// Can submit and manage jobs.
    JobManage,
    /// Can send manual G-code commands.
    ManualControl,
    /// Can change temperatures, fan speeds, etc.
    SetParameters,
    /// Can trigger emergency stop.
    EmergencyStop,
    /// Can update firmware.
    FirmwareUpdate,
    /// Can manage other users/keys.
    Admin,
    /// Full access (superset of all).
    Full,
}

impl Permissions {
    /// Check if a specific permission is granted.
    pub fn has(&self, perm: &Permission) -> bool {
        self.allowed.contains(&Permission::Full) || self.allowed.contains(perm)
    }

    /// Full access.
    pub fn full() -> Self {
        Self {
            allowed: alloc::vec![Permission::Full],
        }
    }

    /// Read-only access.
    pub fn read_only() -> Self {
        Self {
            allowed: alloc::vec![Permission::StatusRead],
        }
    }

    /// Operator access (status + jobs + control, no admin/firmware).
    pub fn operator() -> Self {
        Self {
            allowed: alloc::vec![
                Permission::StatusRead,
                Permission::JobManage,
                Permission::ManualControl,
                Permission::SetParameters,
                Permission::EmergencyStop,
            ],
        }
    }
}

/// Auth challenge sent by machine if auth is required.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthChallenge {
    /// Auth methods the machine accepts.
    pub methods: Vec<String>,
    /// Optional challenge nonce for digest-style auth.
    pub nonce: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_has_everything() {
        let perms = Permissions::full();
        assert!(perms.has(&Permission::StatusRead));
        assert!(perms.has(&Permission::JobManage));
        assert!(perms.has(&Permission::Admin));
        assert!(perms.has(&Permission::FirmwareUpdate));
    }

    #[test]
    fn read_only_limited() {
        let perms = Permissions::read_only();
        assert!(perms.has(&Permission::StatusRead));
        assert!(!perms.has(&Permission::JobManage));
        assert!(!perms.has(&Permission::Admin));
    }

    #[test]
    fn operator_no_admin() {
        let perms = Permissions::operator();
        assert!(perms.has(&Permission::JobManage));
        assert!(perms.has(&Permission::EmergencyStop));
        assert!(!perms.has(&Permission::Admin));
        assert!(!perms.has(&Permission::FirmwareUpdate));
    }
}
