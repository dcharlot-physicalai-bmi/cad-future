//! HTTP client for the OpenIE server REST API.
//!
//! Runs in the browser via the Fetch API (`web_sys::Request` / `web_sys::Response`).
//! No tokio, no reqwest -- pure WASM-compatible async using `wasm_bindgen_futures`.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, Request, RequestInit, Response};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors returned by the API client.
#[derive(Debug, Clone)]
pub enum ApiError {
    /// Network or fetch-level failure.
    Network(String),
    /// Server returned a non-2xx status.
    Http { status: u16, body: String },
    /// JSON deserialization failure.
    Parse(String),
}

impl core::fmt::Display for ApiError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ApiError::Network(msg) => write!(f, "network error: {msg}"),
            ApiError::Http { status, body } => write!(f, "HTTP {status}: {body}"),
            ApiError::Parse(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl From<JsValue> for ApiError {
    fn from(v: JsValue) -> Self {
        let msg = v
            .as_string()
            .unwrap_or_else(|| format!("{v:?}"));
        ApiError::Network(msg)
    }
}

// ---------------------------------------------------------------------------
// Mirror types -- lightweight serde structs matching the server responses.
// These intentionally do NOT import from connect-core (tokio dependency).
// ---------------------------------------------------------------------------

/// Auth response from POST /api/auth/login and /api/auth/register.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub user_id: String,
    pub username: String,
}

/// A machine discovered via network scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredMachine {
    pub name: String,
    pub address: String,
    pub protocol: String,
}

/// Information about a registered machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub protocol: String,
    pub address: String,
}

/// Configuration sent when registering a new machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineConfig {
    pub name: String,
    pub protocol: String,
    pub address: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub serial_number: Option<String>,
    #[serde(default)]
    pub access_code: Option<String>,
}

/// Response when registering a machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterMachineResponse {
    pub id: String,
    pub info: MachineInfo,
}

/// Live status of a machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineStatus {
    pub state: String,
    #[serde(default)]
    pub temperatures: serde_json::Value,
    #[serde(default)]
    pub position: serde_json::Value,
}

/// Handle returned after submitting a print/cut job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobHandle {
    pub job_id: String,
    pub filename: String,
}

/// Status of an in-progress job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStatus {
    pub job_id: String,
    pub state: String,
    #[serde(default)]
    pub progress: f64,
    #[serde(default)]
    pub elapsed_secs: f64,
    #[serde(default)]
    pub remaining_secs: f64,
}

/// Metadata for a stored file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub id: String,
    pub name: String,
    pub user_id: String,
    pub size_bytes: u64,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// API Client
// ---------------------------------------------------------------------------

/// Browser-side HTTP client for the OpenIE server.
///
/// Uses the Fetch API through `web_sys` to make authenticated requests.
pub struct ApiClient {
    base_url: String,
    token: Option<String>,
}

impl ApiClient {
    /// Create a new client pointed at the given server origin.
    ///
    /// Example: `ApiClient::new("http://localhost:3719")`
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            token: None,
        }
    }

    /// Returns `true` if the client has a stored JWT token.
    pub fn is_authenticated(&self) -> bool {
        self.token.is_some()
    }

    /// Return the current JWT token, if any.
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    // -----------------------------------------------------------------------
    // Auth
    // -----------------------------------------------------------------------

    /// Register a new account and store the JWT token on success.
    pub async fn register(
        &mut self,
        username: &str,
        email: &str,
        password: &str,
    ) -> Result<AuthResponse, ApiError> {
        let body = serde_json::json!({
            "username": username,
            "email": email,
            "password": password,
        });
        let resp: AuthResponse =
            self.fetch_json("POST", "/api/auth/register", Some(&body.to_string())).await?;
        self.token = Some(resp.token.clone());
        Ok(resp)
    }

    /// Log in and store the JWT token on success.
    pub async fn login(
        &mut self,
        username: &str,
        password: &str,
    ) -> Result<AuthResponse, ApiError> {
        let body = serde_json::json!({
            "username": username,
            "password": password,
        });
        let resp: AuthResponse =
            self.fetch_json("POST", "/api/auth/login", Some(&body.to_string())).await?;
        self.token = Some(resp.token.clone());
        Ok(resp)
    }

    /// Clear the stored JWT token (log out client-side).
    pub fn logout(&mut self) {
        self.token = None;
    }

    // -----------------------------------------------------------------------
    // Machine discovery
    // -----------------------------------------------------------------------

    /// Discover machines on the local network.
    pub async fn discover_machines(&self) -> Result<Vec<DiscoveredMachine>, ApiError> {
        #[derive(Deserialize)]
        struct Wrapper {
            machines: Vec<DiscoveredMachine>,
        }
        let w: Wrapper = self.fetch_json("GET", "/api/machines/discover", None).await?;
        Ok(w.machines)
    }

    // -----------------------------------------------------------------------
    // Machine CRUD
    // -----------------------------------------------------------------------

    /// List all registered machines.
    pub async fn list_machines(&self) -> Result<Vec<MachineInfo>, ApiError> {
        #[derive(Deserialize)]
        struct Wrapper {
            machines: Vec<MachineInfo>,
        }
        let w: Wrapper = self.fetch_json("GET", "/api/machines", None).await?;
        Ok(w.machines)
    }

    /// Register a new machine with the server.
    pub async fn register_machine(
        &self,
        config: &MachineConfig,
    ) -> Result<RegisterMachineResponse, ApiError> {
        let body = serde_json::json!({ "config": config });
        self.fetch_json("POST", "/api/machines", Some(&body.to_string())).await
    }

    /// Get a single machine's info.
    pub async fn get_machine(&self, id: &str) -> Result<MachineInfo, ApiError> {
        self.fetch_json("GET", &format!("/api/machines/{id}"), None).await
    }

    /// Delete (unregister) a machine.
    pub async fn delete_machine(&self, id: &str) -> Result<(), ApiError> {
        self.fetch_empty("DELETE", &format!("/api/machines/{id}")).await
    }

    /// Ping a machine to check connectivity.
    pub async fn ping_machine(&self, id: &str) -> Result<(), ApiError> {
        self.fetch_empty("POST", &format!("/api/machines/{id}/ping")).await
    }

    /// Get the live status of a machine.
    pub async fn machine_status(&self, id: &str) -> Result<MachineStatus, ApiError> {
        self.fetch_json("GET", &format!("/api/machines/{id}/status"), None).await
    }

    // -----------------------------------------------------------------------
    // Jobs
    // -----------------------------------------------------------------------

    /// Submit a manufacturing job (G-code, STL, 3MF, etc.) to a machine.
    ///
    /// The payload is sent as raw bytes with metadata in custom headers.
    pub async fn submit_job(
        &self,
        machine_id: &str,
        name: &str,
        format: &str,
        data: &[u8],
    ) -> Result<JobHandle, ApiError> {
        let path = format!("/api/machines/{machine_id}/jobs");
        let extra_headers: &[(&str, &str)] = &[
            ("x-job-name", name),
            ("x-job-format", format),
        ];
        let raw = self
            .fetch_raw("POST", &path, Some(data), extra_headers)
            .await?;
        serde_json::from_slice(&raw).map_err(|e| ApiError::Parse(e.to_string()))
    }

    /// Get the status of a specific job.
    pub async fn job_status(
        &self,
        machine_id: &str,
        job_id: &str,
    ) -> Result<JobStatus, ApiError> {
        self.fetch_json(
            "GET",
            &format!("/api/machines/{machine_id}/jobs/{job_id}"),
            None,
        )
        .await
    }

    /// Cancel a running job.
    pub async fn cancel_job(&self, machine_id: &str, job_id: &str) -> Result<(), ApiError> {
        self.fetch_empty(
            "POST",
            &format!("/api/machines/{machine_id}/jobs/{job_id}/cancel"),
        )
        .await
    }

    /// Pause a running job.
    pub async fn pause_job(&self, machine_id: &str, job_id: &str) -> Result<(), ApiError> {
        self.fetch_empty(
            "POST",
            &format!("/api/machines/{machine_id}/jobs/{job_id}/pause"),
        )
        .await
    }

    /// Resume a paused job.
    pub async fn resume_job(&self, machine_id: &str, job_id: &str) -> Result<(), ApiError> {
        self.fetch_empty(
            "POST",
            &format!("/api/machines/{machine_id}/jobs/{job_id}/resume"),
        )
        .await
    }

    // -----------------------------------------------------------------------
    // Manual control
    // -----------------------------------------------------------------------

    /// Send a raw G-code command to a machine.
    pub async fn send_command(
        &self,
        machine_id: &str,
        command: &str,
    ) -> Result<String, ApiError> {
        let body = serde_json::json!({ "command": command });
        self.fetch_json(
            "POST",
            &format!("/api/machines/{machine_id}/command"),
            Some(&body.to_string()),
        )
        .await
    }

    /// Jog a machine axis by relative distance.
    pub async fn jog(
        &self,
        machine_id: &str,
        x: f64,
        y: f64,
        z: f64,
        feed_rate: f64,
    ) -> Result<(), ApiError> {
        let body = serde_json::json!({
            "x": x,
            "y": y,
            "z": z,
            "feed_rate": feed_rate,
        });
        let _: serde_json::Value = self
            .fetch_json(
                "POST",
                &format!("/api/machines/{machine_id}/jog"),
                Some(&body.to_string()),
            )
            .await
            .or_else(|e| {
                // Jog returns 200 with no body on success -- treat empty parse as OK.
                if matches!(&e, ApiError::Parse(_)) {
                    Ok(serde_json::Value::Null)
                } else {
                    Err(e)
                }
            })?;
        Ok(())
    }

    /// Home all axes on a machine.
    pub async fn home(&self, machine_id: &str) -> Result<(), ApiError> {
        self.fetch_empty("POST", &format!("/api/machines/{machine_id}/home"))
            .await
    }

    // -----------------------------------------------------------------------
    // File operations
    // -----------------------------------------------------------------------

    /// List the authenticated user's stored files.
    pub async fn list_files(&self) -> Result<Vec<FileInfo>, ApiError> {
        #[derive(Deserialize)]
        struct Wrapper {
            files: Vec<FileInfo>,
        }
        let w: Wrapper = self.fetch_json("GET", "/api/files", None).await?;
        Ok(w.files)
    }

    /// Upload a file to the server.
    pub async fn upload_file(&self, name: &str, data: &[u8]) -> Result<FileInfo, ApiError> {
        let extra_headers: &[(&str, &str)] = &[("x-file-name", name)];
        let raw = self
            .fetch_raw("POST", "/api/files", Some(data), extra_headers)
            .await?;
        serde_json::from_slice(&raw).map_err(|e| ApiError::Parse(e.to_string()))
    }

    /// Download a file by ID.
    pub async fn download_file(&self, file_id: &str) -> Result<Vec<u8>, ApiError> {
        self.fetch_raw("GET", &format!("/api/files/{file_id}"), None, &[])
            .await
    }

    /// Delete a file by ID.
    pub async fn delete_file(&self, file_id: &str) -> Result<(), ApiError> {
        self.fetch_empty("DELETE", &format!("/api/files/{file_id}"))
            .await
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Build standard headers (Content-Type + Authorization).
    fn build_headers(&self, extra: &[(&str, &str)]) -> Result<Headers, ApiError> {
        let headers = Headers::new().map_err(ApiError::from)?;
        headers
            .set("Content-Type", "application/json")
            .map_err(ApiError::from)?;
        if let Some(ref tok) = self.token {
            headers
                .set("Authorization", &format!("Bearer {tok}"))
                .map_err(ApiError::from)?;
        }
        for &(k, v) in extra {
            headers.set(k, v).map_err(ApiError::from)?;
        }
        Ok(headers)
    }

    /// Perform a fetch that expects a JSON response body and deserialize it.
    async fn fetch_json<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<T, ApiError> {
        let raw = self.fetch_raw(
            method,
            path,
            body.map(|s| s.as_bytes()),
            &[],
        ).await?;
        serde_json::from_slice(&raw).map_err(|e| ApiError::Parse(e.to_string()))
    }

    /// Perform a fetch for endpoints that return no meaningful body (2xx = success).
    async fn fetch_empty(&self, method: &str, path: &str) -> Result<(), ApiError> {
        let _ = self.fetch_raw(method, path, None, &[]).await?;
        Ok(())
    }

    /// Low-level fetch: build a `Request`, call `window.fetch`, await the `Response`,
    /// check the status, and return the raw body bytes.
    async fn fetch_raw(
        &self,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
        extra_headers: &[(&str, &str)],
    ) -> Result<Vec<u8>, ApiError> {
        let url = format!("{}{path}", self.base_url);

        let mut opts = RequestInit::new();
        opts.method(method);

        let headers = self.build_headers(extra_headers)?;
        opts.headers(&headers);

        if let Some(bytes) = body {
            let js_array = js_sys::Uint8Array::from(bytes);
            opts.body(Some(&js_array));
        }

        let request = Request::new_with_str_and_init(&url, &opts).map_err(ApiError::from)?;

        let window = web_sys::window().ok_or_else(|| ApiError::Network("no window".into()))?;
        let resp_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(ApiError::from)?;

        let resp: Response = resp_value
            .dyn_into()
            .map_err(|_| ApiError::Network("fetch did not return a Response".into()))?;

        let status = resp.status();

        // Read body as ArrayBuffer then convert to Vec<u8>.
        let buf_promise = resp
            .array_buffer()
            .map_err(|_| ApiError::Network("failed to read response body".into()))?;
        let buf = JsFuture::from(buf_promise)
            .await
            .map_err(ApiError::from)?;
        let uint8 = js_sys::Uint8Array::new(&buf);
        let bytes = uint8.to_vec();

        if status >= 200 && status < 300 {
            Ok(bytes)
        } else {
            let body_text = String::from_utf8_lossy(&bytes).to_string();
            Err(ApiError::Http {
                status,
                body: body_text,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_creation() {
        let client = ApiClient::new("http://localhost:3719");
        assert!(!client.is_authenticated());
        assert_eq!(client.token(), None);
    }

    #[test]
    fn client_strips_trailing_slash() {
        let client = ApiClient::new("http://localhost:3719/");
        assert_eq!(client.base_url, "http://localhost:3719");
    }

    #[test]
    fn auth_response_deserialize() {
        let json = r#"{"token":"abc.def.ghi","user_id":"u1","username":"alice"}"#;
        let resp: AuthResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.token, "abc.def.ghi");
        assert_eq!(resp.user_id, "u1");
        assert_eq!(resp.username, "alice");
    }

    #[test]
    fn machine_info_deserialize() {
        let json = r#"{
            "id": "m1",
            "name": "Prusa MK4",
            "kind": "fdm_printer",
            "protocol": "prusalink",
            "address": "192.168.1.50"
        }"#;
        let info: MachineInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.id, "m1");
        assert_eq!(info.protocol, "prusalink");
    }

    #[test]
    fn machine_status_deserialize() {
        let json = r#"{"state":"printing","temperatures":{"hotend":215.0},"position":{"x":50.0,"y":100.0,"z":5.2}}"#;
        let status: MachineStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.state, "printing");
    }

    #[test]
    fn machine_status_minimal() {
        let json = r#"{"state":"idle"}"#;
        let status: MachineStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.state, "idle");
        assert_eq!(status.temperatures, serde_json::Value::Null);
    }

    #[test]
    fn job_handle_deserialize() {
        let json = r#"{"job_id":"j-123","filename":"bracket.gcode"}"#;
        let handle: JobHandle = serde_json::from_str(json).unwrap();
        assert_eq!(handle.job_id, "j-123");
        assert_eq!(handle.filename, "bracket.gcode");
    }

    #[test]
    fn job_status_deserialize() {
        let json = r#"{
            "job_id": "j-456",
            "state": "printing",
            "progress": 42.5,
            "elapsed_secs": 3600.0,
            "remaining_secs": 5040.0
        }"#;
        let status: JobStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.state, "printing");
        assert!((status.progress - 42.5).abs() < f64::EPSILON);
    }

    #[test]
    fn job_status_minimal() {
        let json = r#"{"job_id":"j-0","state":"queued"}"#;
        let status: JobStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.progress, 0.0);
    }

    #[test]
    fn file_info_deserialize() {
        let json = r#"{
            "id": "f-1",
            "name": "bracket.oie",
            "user_id": "u-1",
            "size_bytes": 4096,
            "created_at": "2026-03-25T10:00:00Z",
            "updated_at": "2026-03-25T12:00:00Z"
        }"#;
        let info: FileInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.name, "bracket.oie");
        assert_eq!(info.size_bytes, 4096);
    }

    #[test]
    fn machine_config_serialize() {
        let config = MachineConfig {
            name: "My Printer".into(),
            protocol: "octoprint".into(),
            address: "192.168.1.100".into(),
            api_key: Some("abc123".into()),
            serial_number: None,
            access_code: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("octoprint"));
        assert!(json.contains("abc123"));
    }

    #[test]
    fn discovered_machine_deserialize() {
        let json = r#"{"name":"Bambu X1","address":"10.0.0.5","protocol":"bambu_lan"}"#;
        let m: DiscoveredMachine = serde_json::from_str(json).unwrap();
        assert_eq!(m.protocol, "bambu_lan");
    }

    #[test]
    fn api_error_display() {
        let e = ApiError::Http {
            status: 401,
            body: "unauthorized".into(),
        };
        assert!(format!("{e}").contains("401"));

        let e2 = ApiError::Network("timeout".into());
        assert!(format!("{e2}").contains("timeout"));

        let e3 = ApiError::Parse("expected value".into());
        assert!(format!("{e3}").contains("expected value"));
    }

    #[test]
    fn register_machine_response_deserialize() {
        let json = r#"{
            "id": "m-abc",
            "info": {
                "id": "m-abc",
                "name": "CNC Router",
                "kind": "cnc",
                "protocol": "fluidnc",
                "address": "192.168.1.200"
            }
        }"#;
        let resp: RegisterMachineResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, "m-abc");
        assert_eq!(resp.info.kind, "cnc");
    }
}
