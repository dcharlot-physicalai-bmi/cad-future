//! REST API routes for manufacturing machine management.
//!
//! Provides endpoints for:
//! - Machine discovery (scan network)
//! - Machine CRUD (register, list, get, update, delete)
//! - Connection management (connect, ping)
//! - Job management (submit, status, cancel, pause, resume)
//! - Manual control (G-code command, jog, home)

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use axum::body::Bytes;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use physical_connect_core::*;

use crate::auth::AuthError;
use crate::routes::AppState;

fn extract_claims_machine(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<crate::auth::Claims, (StatusCode, Json<AuthError>)> {
    crate::routes::extract_claims(headers, &state.users)
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct DiscoverResponse {
    pub machines: Vec<DiscoveredMachine>,
}

pub async fn discover_machines(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<DiscoverResponse>, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims_machine(&headers, &state)?;

    // Discovery is done by protocol-specific drivers.
    // For now, return empty — real implementations will scan the network.
    Ok(Json(DiscoverResponse {
        machines: Vec::new(),
    }))
}

// ---------------------------------------------------------------------------
// Machine CRUD
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct MachineListResponse {
    pub machines: Vec<MachineInfo>,
}

pub async fn list_machines(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<MachineListResponse>, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims_machine(&headers, &state)?;
    let machines = state.machines.list().await;
    Ok(Json(MachineListResponse { machines }))
}

pub async fn get_machine(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(machine_id): Path<String>,
) -> Result<Json<MachineInfo>, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims_machine(&headers, &state)?;
    let id = MachineId::new(machine_id);
    state
        .machines
        .get_info(&id)
        .await
        .map(Json)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(AuthError {
                    error: "machine not found".into(),
                }),
            )
        })
}

#[derive(Deserialize)]
pub struct RegisterMachineRequest {
    pub config: MachineConfig,
}

#[derive(Serialize)]
pub struct RegisterMachineResponse {
    pub id: MachineId,
    pub info: MachineInfo,
}

pub async fn register_machine(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<RegisterMachineRequest>,
) -> Result<Json<RegisterMachineResponse>, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims_machine(&headers, &state)?;

    // Create the connection based on protocol
    let connection: Box<dyn MachineConnection> = create_connection(&req.config).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                error: format!("failed to create connection: {e}"),
            }),
        )
    })?;

    let id = connection.info().id.clone();
    let info = connection.info().clone();
    state.machines.register(id.clone(), connection).await;

    Ok(Json(RegisterMachineResponse { id, info }))
}

pub async fn delete_machine(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(machine_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims_machine(&headers, &state)?;
    let id = MachineId::new(machine_id);
    state.machines.remove(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                error: e.to_string(),
            }),
        )
    })?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Connection
// ---------------------------------------------------------------------------

pub async fn ping_machine(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(machine_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims_machine(&headers, &state)?;
    let id = MachineId::new(machine_id);

    let conn = state.machines.get_connection(&id).await.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(AuthError {
                error: "machine not found".into(),
            }),
        )
    })?;

    let conn = conn.read().await;
    conn.ping().await.map_err(|e| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(AuthError {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(StatusCode::OK)
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

pub async fn machine_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(machine_id): Path<String>,
) -> Result<Json<MachineStatus>, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims_machine(&headers, &state)?;
    let id = MachineId::new(machine_id);

    state.machines.status(&id).await.map(Json).map_err(|e| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(AuthError {
                error: e.to_string(),
            }),
        )
    })
}

// ---------------------------------------------------------------------------
// Jobs
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct SubmitJobQuery {
    pub name: Option<String>,
    pub format: Option<String>,
    pub auto_start: Option<bool>,
}

pub async fn submit_job(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(machine_id): Path<String>,
    body: Bytes,
) -> Result<Json<JobHandle>, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims_machine(&headers, &state)?;
    let id = MachineId::new(machine_id);

    let name = headers
        .get("x-job-name")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("job")
        .to_string();

    let format_str = headers
        .get("x-job-format")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("gcode");

    let format = match format_str {
        "gcode" => AcceptedFormat::Gcode,
        "bgcode" | "binary_gcode" => AcceptedFormat::BinaryGcode,
        "3mf" | "threemf" => AcceptedFormat::ThreeMf,
        "stl" => AcceptedFormat::Stl,
        "ufp" => AcceptedFormat::Ufp,
        "rd" | "ruida" => AcceptedFormat::RuidaRd,
        "dxf" => AcceptedFormat::Dxf,
        _ => AcceptedFormat::Gcode,
    };

    let auto_start = headers
        .get("x-auto-start")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == "true" || v == "1")
        .unwrap_or(true);

    let job = JobSubmission {
        name,
        format,
        payload: body.to_vec(),
        auto_start,
    };

    let conn = state.machines.get_connection(&id).await.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(AuthError {
                error: "machine not found".into(),
            }),
        )
    })?;

    let conn = conn.read().await;
    conn.submit_job(job).await.map(Json).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                error: e.to_string(),
            }),
        )
    })
}

pub async fn cancel_job(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((machine_id, job_id)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims_machine(&headers, &state)?;
    let id = MachineId::new(machine_id);

    let conn = state.machines.get_connection(&id).await.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(AuthError {
                error: "machine not found".into(),
            }),
        )
    })?;

    let handle = JobHandle {
        job_id: job_id.clone(),
        filename: job_id,
    };

    let conn = conn.read().await;
    conn.cancel_job(&handle).await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                error: e.to_string(),
            }),
        )
    })?;
    Ok(StatusCode::OK)
}

pub async fn pause_job(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((machine_id, job_id)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims_machine(&headers, &state)?;
    let id = MachineId::new(machine_id);

    let conn = state.machines.get_connection(&id).await.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(AuthError {
                error: "machine not found".into(),
            }),
        )
    })?;

    let handle = JobHandle {
        job_id: job_id.clone(),
        filename: job_id,
    };

    let conn = conn.read().await;
    conn.pause_job(&handle).await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                error: e.to_string(),
            }),
        )
    })?;
    Ok(StatusCode::OK)
}

pub async fn resume_job(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((machine_id, job_id)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims_machine(&headers, &state)?;
    let id = MachineId::new(machine_id);

    let conn = state.machines.get_connection(&id).await.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(AuthError {
                error: "machine not found".into(),
            }),
        )
    })?;

    let handle = JobHandle {
        job_id: job_id.clone(),
        filename: job_id,
    };

    let conn = conn.read().await;
    conn.resume_job(&handle).await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                error: e.to_string(),
            }),
        )
    })?;
    Ok(StatusCode::OK)
}

pub async fn job_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((machine_id, job_id)): Path<(String, String)>,
) -> Result<Json<JobStatus>, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims_machine(&headers, &state)?;
    let id = MachineId::new(machine_id);

    let conn = state.machines.get_connection(&id).await.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(AuthError {
                error: "machine not found".into(),
            }),
        )
    })?;

    let handle = JobHandle {
        job_id: job_id.clone(),
        filename: job_id,
    };

    let conn = conn.read().await;
    conn.job_status(&handle).await.map(Json).map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            Json(AuthError {
                error: e.to_string(),
            }),
        )
    })
}

// ---------------------------------------------------------------------------
// Manual control
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct CommandRequest {
    pub command: String,
}

pub async fn send_command(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(machine_id): Path<String>,
    Json(req): Json<CommandRequest>,
) -> Result<Json<String>, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims_machine(&headers, &state)?;
    let id = MachineId::new(machine_id);

    let conn = state.machines.get_connection(&id).await.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(AuthError {
                error: "machine not found".into(),
            }),
        )
    })?;

    let conn = conn.read().await;
    conn.send_command(&req.command)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(AuthError {
                    error: e.to_string(),
                }),
            )
        })
}

#[derive(Deserialize)]
pub struct JogRequest {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub z: Option<f64>,
    pub feed_rate: Option<f64>,
}

pub async fn jog(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(machine_id): Path<String>,
    Json(req): Json<JogRequest>,
) -> Result<StatusCode, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims_machine(&headers, &state)?;
    let id = MachineId::new(machine_id);

    let conn = state.machines.get_connection(&id).await.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(AuthError {
                error: "machine not found".into(),
            }),
        )
    })?;

    let feed = req.feed_rate.unwrap_or(1000.0);
    let x = req.x.unwrap_or(0.0);
    let y = req.y.unwrap_or(0.0);
    let z = req.z.unwrap_or(0.0);

    let cmd = format!("G91\nG1 X{x:.3} Y{y:.3} Z{z:.3} F{feed:.0}\nG90");

    let conn = conn.read().await;
    conn.send_command(&cmd).await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                error: e.to_string(),
            }),
        )
    })?;
    Ok(StatusCode::OK)
}

pub async fn home(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(machine_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims_machine(&headers, &state)?;
    let id = MachineId::new(machine_id);

    let conn = state.machines.get_connection(&id).await.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(AuthError {
                error: "machine not found".into(),
            }),
        )
    })?;

    let conn = conn.read().await;
    conn.send_command("G28").await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                error: e.to_string(),
            }),
        )
    })?;
    Ok(StatusCode::OK)
}

// ---------------------------------------------------------------------------
// Connection factory
// ---------------------------------------------------------------------------

/// Create a machine connection from config.
fn create_connection(config: &MachineConfig) -> Result<Box<dyn MachineConnection>, ConnectError> {
    match config.protocol {
        Protocol::OctoPrint => {
            Ok(Box::new(physical_connect_octoprint::OctoPrintConnection::new(config)?))
        }
        Protocol::Moonraker => {
            Ok(Box::new(physical_connect_moonraker::MoonrakerConnection::new(config)?))
        }
        Protocol::BambuLan | Protocol::BambuCloud => {
            Ok(Box::new(physical_connect_bambu::BambuConnection::new(config)?))
        }
        Protocol::PrusaLink => {
            Ok(Box::new(physical_connect_prusalink::PrusaLinkConnection::new(config)?))
        }
        Protocol::Duet => {
            Ok(Box::new(physical_connect_duet::DuetConnection::new(config)?))
        }
        Protocol::Repetier => {
            Ok(Box::new(physical_connect_repetier::RepetierConnection::new(config)?))
        }
        Protocol::FluidNc => {
            Ok(Box::new(physical_connect_fluidnc::FluidNcConnection::new(config)?))
        }
        Protocol::LinuxCnc => {
            Ok(Box::new(physical_connect_linuxcnc::LinuxCncConnection::new(config)?))
        }
        Protocol::CncJs => {
            Ok(Box::new(physical_connect_cncjs::CncJsConnection::new(config)?))
        }
        Protocol::HaasMdc => {
            Ok(Box::new(physical_connect_haas::HaasMdcConnection::new(config)?))
        }
        Protocol::UltiMaker => {
            Ok(Box::new(physical_connect_ultimaker::UltiMakerConnection::new(config)?))
        }
        Protocol::Formlabs => {
            Ok(Box::new(physical_connect_formlabs::FormlabsConnection::new(config)?))
        }
        Protocol::LightBurnBridge => {
            Ok(Box::new(physical_connect_lightburn::LightBurnConnection::new(config)?))
        }
        Protocol::OpenIE => {
            Ok(Box::new(physical_connect_openie::OmpConnection::new(config)?))
        }
        _ => Err(ConnectError::Unsupported(format!(
            "protocol {:?} not yet implemented",
            config.protocol
        ))),
    }
}
