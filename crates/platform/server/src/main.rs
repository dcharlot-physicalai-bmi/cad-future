//! OpenIE production server.
//!
//! REST API with JWT auth, file storage, and manufacturing machine connectivity.
//! Usage: openie-server [--port PORT] [--data-dir DIR]

use axum::Router;
use axum::routing::{get, post, put, delete};
use std::sync::Arc;
use tower_http::cors::{CorsLayer, Any};

use physical_server::auth::UserStore;
use physical_server::storage::FileStore;
use physical_server::routes::*;
use physical_server::machine_routes;
use physical_connect_core::MachineRegistry;

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT").ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3719);
    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".into());
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
        eprintln!("[openie] WARNING: Using default JWT secret. Set JWT_SECRET in production.");
        "openie-dev-secret-change-in-production".into()
    });

    let state = Arc::new(AppState {
        users: UserStore::new(&jwt_secret),
        files: FileStore::new(&data_dir),
        machines: MachineRegistry::new(),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        // Health
        .route("/api/health", get(health))
        // Auth
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        // Files
        .route("/api/files", get(list_files))
        .route("/api/files", post(upload_file))
        .route("/api/files/{file_id}", get(download_file))
        .route("/api/files/{file_id}", put(update_file))
        .route("/api/files/{file_id}", delete(delete_file))
        // Machine discovery
        .route("/api/machines/discover", get(machine_routes::discover_machines))
        // Machine CRUD
        .route("/api/machines", get(machine_routes::list_machines))
        .route("/api/machines", post(machine_routes::register_machine))
        .route("/api/machines/{id}", get(machine_routes::get_machine))
        .route("/api/machines/{id}", delete(machine_routes::delete_machine))
        // Machine connection
        .route("/api/machines/{id}/ping", post(machine_routes::ping_machine))
        .route("/api/machines/{id}/status", get(machine_routes::machine_status))
        // Jobs
        .route("/api/machines/{id}/jobs", post(machine_routes::submit_job))
        .route("/api/machines/{id}/jobs/{job_id}", get(machine_routes::job_status))
        .route("/api/machines/{id}/jobs/{job_id}/cancel", post(machine_routes::cancel_job))
        .route("/api/machines/{id}/jobs/{job_id}/pause", post(machine_routes::pause_job))
        .route("/api/machines/{id}/jobs/{job_id}/resume", post(machine_routes::resume_job))
        // Manual control
        .route("/api/machines/{id}/command", post(machine_routes::send_command))
        .route("/api/machines/{id}/jog", post(machine_routes::jog))
        .route("/api/machines/{id}/home", post(machine_routes::home))
        // Export
        .route("/api/export/step", post(export_step))
        .route("/api/export/stl", post(export_stl))
        .route("/api/export/3mf", post(export_3mf))
        .route("/api/export/obj", post(export_obj))
        .route("/api/export/gltf", post(export_gltf))
        .route("/api/export/iges", post(export_iges))
        .route("/api/export/dxf", post(export_dxf))
        .route("/api/export/pdf", post(export_pdf))
        // Materials
        .route("/api/materials", get(search_materials))
        .route("/api/materials/categories", get(material_categories))
        .route("/api/materials/{id}", get(get_material))
        // Simulation
        .route("/api/simulate/fea", post(simulate_fea))
        .route("/api/simulate/thermal", post(simulate_thermal))
        .route("/api/simulate/coupled", post(simulate_coupled))
        .route("/api/simulate/modal", post(simulate_modal))
        .route("/api/simulate/dfm", post(simulate_dfm))
        // Versioning
        .route("/api/designs/{id}/versions", get(list_versions))
        .route("/api/designs/{id}/versions", post(create_version))
        .route("/api/designs/{id}/branches", get(list_branches))
        .route("/api/designs/{id}/branches", post(create_branch))
        // MCP proxy
        .route("/api/mcp/tools/call", post(mcp_tool_call))
        .route("/api/mcp/tools/list", get(mcp_tool_list))
        .layer(cors)
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    println!("[openie] API server on http://localhost:{port}");
    println!("[openie] Data directory: {data_dir}");
    println!("[openie] Machine connectivity: enabled (28+ protocols)");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
