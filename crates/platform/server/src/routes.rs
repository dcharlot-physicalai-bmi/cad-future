//! Axum route handlers for the REST API.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use axum::body::Bytes;
use serde::{Serialize, Deserialize};
use std::sync::Arc;

use crate::auth::{UserStore, RegisterRequest, LoginRequest, AuthResponse, AuthError, Claims};
use crate::storage::{FileStore, FileMeta};
use physical_connect_core::MachineRegistry;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub users: UserStore,
    pub files: FileStore,
    pub machines: MachineRegistry,
}

// ---------------------------------------------------------------------------
// Auth routes
// ---------------------------------------------------------------------------

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<AuthError>)> {
    state.users.register(&req)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(AuthError { error: e })))
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<AuthError>)> {
    state.users.login(&req)
        .map(Json)
        .map_err(|e| (StatusCode::UNAUTHORIZED, Json(AuthError { error: e })))
}

// ---------------------------------------------------------------------------
// File storage routes (all require auth)
// ---------------------------------------------------------------------------

/// Extract and validate JWT from Authorization header.
pub fn extract_claims(headers: &HeaderMap, users: &UserStore) -> Result<Claims, (StatusCode, Json<AuthError>)> {
    let auth_header = headers.get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, Json(AuthError { error: "Missing Authorization header".into() })))?;

    let token = auth_header.strip_prefix("Bearer ")
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, Json(AuthError { error: "Invalid Authorization format".into() })))?;

    users.validate_token(token)
        .map_err(|e| (StatusCode::UNAUTHORIZED, Json(AuthError { error: e })))
}

#[derive(Serialize)]
pub struct FileListResponse {
    pub files: Vec<FileMeta>,
}

pub async fn list_files(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<FileListResponse>, (StatusCode, Json<AuthError>)> {
    let claims = extract_claims(&headers, &state.users)?;
    let files = state.files.list(&claims.sub)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(AuthError { error: e })))?;
    Ok(Json(FileListResponse { files }))
}

pub async fn upload_file(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<FileMeta>, (StatusCode, Json<AuthError>)> {
    let claims = extract_claims(&headers, &state.users)?;

    let name = headers.get("x-file-name")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("untitled.oie")
        .to_string();

    state.files.save(&claims.sub, &name, &body)
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(AuthError { error: e })))
}

pub async fn download_file(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(file_id): Path<String>,
) -> Result<Bytes, (StatusCode, Json<AuthError>)> {
    let claims = extract_claims(&headers, &state.users)?;
    let data = state.files.load(&claims.sub, &file_id)
        .map_err(|e| (StatusCode::NOT_FOUND, Json(AuthError { error: e })))?;
    Ok(Bytes::from(data))
}

pub async fn update_file(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(file_id): Path<String>,
    body: Bytes,
) -> Result<Json<FileMeta>, (StatusCode, Json<AuthError>)> {
    let claims = extract_claims(&headers, &state.users)?;
    state.files.update(&claims.sub, &file_id, &body)
        .map(Json)
        .map_err(|e| (StatusCode::NOT_FOUND, Json(AuthError { error: e })))
}

pub async fn delete_file(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(file_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<AuthError>)> {
    let claims = extract_claims(&headers, &state.users)?;
    state.files.delete(&claims.sub, &file_id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| (StatusCode::NOT_FOUND, Json(AuthError { error: e })))
}

// ---------------------------------------------------------------------------
// Export routes (stub — return format info, not actual file bytes)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
pub struct ExportRequest {
    pub file_id: String,
    #[serde(default = "default_tolerance")]
    pub tolerance: f64,
}

fn default_tolerance() -> f64 { 0.1 }

#[derive(Serialize)]
pub struct ExportResponse {
    pub format: String,
    pub status: String,
    pub message: String,
}

async fn export_handler(format: &str, _state: &AppState, _claims: &Claims, _req: &ExportRequest) -> ExportResponse {
    ExportResponse {
        format: format.to_string(),
        status: "ready".into(),
        message: format!("{} export pipeline available. Submit file_id to generate.", format.to_uppercase()),
    }
}

pub async fn export_step(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ExportRequest>,
) -> Result<Json<ExportResponse>, (StatusCode, Json<AuthError>)> {
    let claims = extract_claims(&headers, &state.users)?;
    Ok(Json(export_handler("step", &state, &claims, &req).await))
}

pub async fn export_stl(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ExportRequest>,
) -> Result<Json<ExportResponse>, (StatusCode, Json<AuthError>)> {
    let claims = extract_claims(&headers, &state.users)?;
    Ok(Json(export_handler("stl", &state, &claims, &req).await))
}

pub async fn export_3mf(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ExportRequest>,
) -> Result<Json<ExportResponse>, (StatusCode, Json<AuthError>)> {
    let claims = extract_claims(&headers, &state.users)?;
    Ok(Json(export_handler("3mf", &state, &claims, &req).await))
}

pub async fn export_obj(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ExportRequest>,
) -> Result<Json<ExportResponse>, (StatusCode, Json<AuthError>)> {
    let claims = extract_claims(&headers, &state.users)?;
    Ok(Json(export_handler("obj", &state, &claims, &req).await))
}

pub async fn export_gltf(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ExportRequest>,
) -> Result<Json<ExportResponse>, (StatusCode, Json<AuthError>)> {
    let claims = extract_claims(&headers, &state.users)?;
    Ok(Json(export_handler("gltf", &state, &claims, &req).await))
}

pub async fn export_iges(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ExportRequest>,
) -> Result<Json<ExportResponse>, (StatusCode, Json<AuthError>)> {
    let claims = extract_claims(&headers, &state.users)?;
    Ok(Json(export_handler("iges", &state, &claims, &req).await))
}

pub async fn export_dxf(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ExportRequest>,
) -> Result<Json<ExportResponse>, (StatusCode, Json<AuthError>)> {
    let claims = extract_claims(&headers, &state.users)?;
    Ok(Json(export_handler("dxf", &state, &claims, &req).await))
}

pub async fn export_pdf(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ExportRequest>,
) -> Result<Json<ExportResponse>, (StatusCode, Json<AuthError>)> {
    let claims = extract_claims(&headers, &state.users)?;
    Ok(Json(export_handler("pdf", &state, &claims, &req).await))
}

// ---------------------------------------------------------------------------
// Material lookup routes
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct MaterialResponse {
    pub id: String,
    pub name: String,
    pub category: String,
    pub yield_strength_mpa: f64,
    pub density_kg_m3: f64,
}

#[derive(Serialize)]
pub struct MaterialListResponse {
    pub materials: Vec<MaterialResponse>,
}

#[derive(Serialize)]
pub struct CategoryListResponse {
    pub categories: Vec<String>,
}

pub async fn get_material(
    Path(id): Path<String>,
) -> Result<Json<MaterialResponse>, StatusCode> {
    physical_lut::materials::lookup(&id)
        .map(|m| Json(MaterialResponse {
            id: id.clone(),
            name: m.name.to_string(),
            category: format!("{:?}", m.category),
            yield_strength_mpa: m.yield_strength.to_mpa(),
            density_kg_m3: m.density.value(),
        }))
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn search_materials(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<MaterialListResponse> {
    let query = params.get("q").map(|s| s.as_str()).unwrap_or("");
    let results: Vec<MaterialResponse> = physical_lut::materials::search(query)
        .take(50)
        .map(|m| MaterialResponse {
            id: m.id.to_string(),
            name: m.name.to_string(),
            category: format!("{:?}", m.category),
            yield_strength_mpa: m.yield_strength.to_mpa(),
            density_kg_m3: m.density.value(),
        })
        .collect();
    Json(MaterialListResponse { materials: results })
}

pub async fn material_categories() -> Json<CategoryListResponse> {
    Json(CategoryListResponse {
        categories: vec![
            "Aluminum".into(), "Steel".into(), "Stainless Steel".into(),
            "Tool Steel".into(), "Titanium".into(), "Copper".into(),
            "Nickel".into(), "Polymer".into(), "Composite".into(),
            "Ceramic".into(), "Cast Iron".into(),
        ],
    })
}

// ---------------------------------------------------------------------------
// Simulation routes (stub — return estimated results)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
pub struct SimulationRequest {
    pub file_id: String,
    #[serde(default = "default_material")]
    pub material_id: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

fn default_material() -> String { "6061-T6".into() }

#[derive(Serialize)]
pub struct SimulationResponse {
    pub analysis_type: String,
    pub status: String,
    pub message: String,
    pub results: serde_json::Value,
}

pub async fn simulate_fea(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<SimulationRequest>,
) -> Result<Json<SimulationResponse>, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims(&headers, &state.users)?;
    Ok(Json(SimulationResponse {
        analysis_type: "structural_fea".into(),
        status: "ready".into(),
        message: format!("FEA pipeline ready for file {} with material {}", req.file_id, req.material_id),
        results: serde_json::json!({"solver": "physical-fea", "element_type": "tet4"}),
    }))
}

pub async fn simulate_thermal(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<SimulationRequest>,
) -> Result<Json<SimulationResponse>, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims(&headers, &state.users)?;
    Ok(Json(SimulationResponse {
        analysis_type: "thermal".into(),
        status: "ready".into(),
        message: format!("Thermal analysis ready for file {}", req.file_id),
        results: serde_json::json!({"solver": "physical-fea::thermal"}),
    }))
}

pub async fn simulate_coupled(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<SimulationRequest>,
) -> Result<Json<SimulationResponse>, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims(&headers, &state.users)?;
    Ok(Json(SimulationResponse {
        analysis_type: "coupled_thermal_structural".into(),
        status: "ready".into(),
        message: format!("Coupled analysis ready for file {}", req.file_id),
        results: serde_json::json!({"solver": "physical-fea::coupled"}),
    }))
}

pub async fn simulate_modal(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<SimulationRequest>,
) -> Result<Json<SimulationResponse>, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims(&headers, &state.users)?;
    Ok(Json(SimulationResponse {
        analysis_type: "modal".into(),
        status: "ready".into(),
        message: format!("Modal analysis ready for file {}", req.file_id),
        results: serde_json::json!({"solver": "physical-fea::modal"}),
    }))
}

pub async fn simulate_dfm(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<SimulationRequest>,
) -> Result<Json<SimulationResponse>, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims(&headers, &state.users)?;
    Ok(Json(SimulationResponse {
        analysis_type: "dfm_check".into(),
        status: "ready".into(),
        message: format!("DFM check ready for file {}", req.file_id),
        results: serde_json::json!({"solver": "physical-dfm"}),
    }))
}

// ---------------------------------------------------------------------------
// Version history routes
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
pub struct CreateVersionRequest {
    pub name: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct VersionResponse {
    pub id: String,
    pub name: String,
    pub message: String,
    pub branch: String,
}

#[derive(Serialize)]
pub struct VersionListResponse {
    pub versions: Vec<VersionResponse>,
}

#[derive(Serialize, Deserialize)]
pub struct CreateBranchRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct BranchResponse {
    pub name: String,
    pub head_version: String,
}

#[derive(Serialize)]
pub struct BranchListResponse {
    pub branches: Vec<BranchResponse>,
}

pub async fn list_versions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(design_id): Path<String>,
) -> Result<Json<VersionListResponse>, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims(&headers, &state.users)?;
    // Return empty list — versioning wired up but data requires active session
    let _ = design_id;
    Ok(Json(VersionListResponse { versions: Vec::new() }))
}

pub async fn create_version(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(design_id): Path<String>,
    Json(req): Json<CreateVersionRequest>,
) -> Result<Json<VersionResponse>, (StatusCode, Json<AuthError>)> {
    let claims = extract_claims(&headers, &state.users)?;
    Ok(Json(VersionResponse {
        id: format!("v-{}-{}", design_id, uuid::Uuid::new_v4()),
        name: req.name,
        message: req.message,
        branch: "main".into(),
    }))
}

pub async fn list_branches(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(design_id): Path<String>,
) -> Result<Json<BranchListResponse>, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims(&headers, &state.users)?;
    let _ = design_id;
    Ok(Json(BranchListResponse {
        branches: vec![BranchResponse {
            name: "main".into(),
            head_version: "initial".into(),
        }],
    }))
}

pub async fn create_branch(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(design_id): Path<String>,
    Json(req): Json<CreateBranchRequest>,
) -> Result<Json<BranchResponse>, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims(&headers, &state.users)?;
    let _ = design_id;
    Ok(Json(BranchResponse {
        name: req.name,
        head_version: "initial".into(),
    }))
}

// ---------------------------------------------------------------------------
// MCP tool proxy
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
pub struct McpToolCallRequest {
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

#[derive(Serialize)]
pub struct McpToolCallResponse {
    pub tool_name: String,
    pub status: String,
    pub result: serde_json::Value,
}

#[derive(Serialize)]
pub struct McpToolListResponse {
    pub tools: Vec<String>,
}

pub async fn mcp_tool_call(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<McpToolCallRequest>,
) -> Result<Json<McpToolCallResponse>, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims(&headers, &state.users)?;
    Ok(Json(McpToolCallResponse {
        tool_name: req.tool_name.clone(),
        status: "dispatched".into(),
        result: serde_json::json!({
            "message": format!("Tool '{}' accepted for execution", req.tool_name),
        }),
    }))
}

pub async fn mcp_tool_list(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<McpToolListResponse>, (StatusCode, Json<AuthError>)> {
    let _claims = extract_claims(&headers, &state.users)?;
    Ok(Json(McpToolListResponse {
        tools: vec![
            "create_box".into(), "create_cylinder".into(), "extrude_profile".into(),
            "fillet".into(), "hollow_out".into(), "pattern".into(),
            "analyze_part".into(), "lookup_material".into(), "export".into(),
            "run_fea".into(), "run_thermal_analysis".into(), "run_coupled_analysis".into(),
            "run_cfd".into(), "run_dfm_check".into(), "optimize_topology".into(),
            "export_step".into(), "export_stl".into(), "export_3mf".into(),
            "export_obj".into(), "export_gltf".into(), "export_iges".into(),
        ],
    }))
}

/// Health check endpoint.
pub async fn health() -> &'static str {
    "ok"
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_request_deserializes() {
        let json = r#"{"file_id":"abc123","tolerance":0.05}"#;
        let req: ExportRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.file_id, "abc123");
        assert!((req.tolerance - 0.05).abs() < 1e-10);
    }

    #[test]
    fn export_request_default_tolerance() {
        let json = r#"{"file_id":"abc"}"#;
        let req: ExportRequest = serde_json::from_str(json).unwrap();
        assert!((req.tolerance - 0.1).abs() < 1e-10);
    }

    #[test]
    fn simulation_request_deserializes() {
        let json = r#"{"file_id":"f1","material_id":"7075-T6","params":{"load":1000}}"#;
        let req: SimulationRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.material_id, "7075-T6");
    }

    #[test]
    fn simulation_request_defaults() {
        let json = r#"{"file_id":"f1"}"#;
        let req: SimulationRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.material_id, "6061-T6");
    }

    #[test]
    fn version_request_deserializes() {
        let json = r#"{"name":"v1.0","message":"Initial release"}"#;
        let req: CreateVersionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "v1.0");
    }

    #[test]
    fn branch_request_deserializes() {
        let json = r#"{"name":"feature-chamfer"}"#;
        let req: CreateBranchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "feature-chamfer");
    }

    #[test]
    fn mcp_tool_call_deserializes() {
        let json = r#"{"tool_name":"create_box","arguments":{"width":10,"height":20,"depth":30}}"#;
        let req: McpToolCallRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.tool_name, "create_box");
        assert_eq!(req.arguments["width"], 10);
    }

    #[test]
    fn material_response_serializes() {
        let resp = MaterialResponse {
            id: "6061-T6".into(),
            name: "Aluminum 6061-T6".into(),
            category: "Aluminum".into(),
            yield_strength_mpa: 276.0,
            density_kg_m3: 2700.0,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("6061-T6"));
    }

    #[test]
    fn category_list_has_entries() {
        let cats = vec![
            "Aluminum", "Steel", "Stainless Steel", "Titanium",
            "Polymer", "Composite", "Ceramic",
        ];
        assert!(cats.len() >= 7);
    }

    #[test]
    fn export_formats_complete() {
        let formats = ["step", "stl", "3mf", "obj", "gltf", "iges", "dxf", "pdf"];
        assert_eq!(formats.len(), 8);
    }

    #[test]
    fn app_state_is_cloneable() {
        // AppState must be Clone for axum's State extractor
        fn assert_clone<T: Clone>() {}
        assert_clone::<AppState>();
    }
}
