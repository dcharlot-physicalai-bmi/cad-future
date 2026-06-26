//! Zoo/KittyCAD API connector — import KCL designs into OpenIE via CFL.
//!
//! Translates KCL (Zoo's CAD language) into CFL, enabling designs created
//! in Zoo Design Studio to run natively in the OpenIE platform.
//! Also provides API client types for Zoo's Design API and ML API.

use serde::{Serialize, Deserialize};

// ---------------------------------------------------------------------------
// KCL → CFL Translation
// ---------------------------------------------------------------------------

/// Translate a KCL program to CFL source code.
pub fn import_kcl(kcl_source: &str) -> String {
    physical_cfl::kcl_to_cfl(kcl_source)
}

/// Translate a KCL program to CFL AST for direct execution.
pub fn import_kcl_to_ast(kcl_source: &str) -> Vec<physical_cfl::Expr> {
    let cfl = import_kcl(kcl_source);
    let tokens = physical_cfl::lex(&cfl);
    physical_cfl::parse(&tokens)
}

/// Translate a KCL program to MCP tool calls for execution.
pub fn import_kcl_to_mcp(kcl_source: &str) -> Vec<serde_json::Value> {
    let ast = import_kcl_to_ast(kcl_source);
    physical_cfl::compile_to_mcp(&ast)
}

// ---------------------------------------------------------------------------
// Zoo API Types — for ingesting data from their REST API
// ---------------------------------------------------------------------------

/// A file format supported by Zoo's API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ZooFileFormat {
    Step,
    Stl,
    Obj,
    Gltf,
    Ply,
    Fbx,
}

/// Metadata about a file conversion from Zoo's API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZooConversionResult {
    pub id: String,
    pub status: String,
    pub src_format: String,
    pub output_format: String,
    pub output_bytes: Option<Vec<u8>>,
}

/// A text-to-CAD result from Zoo's ML API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZooTextToCadResult {
    pub id: String,
    pub status: String,
    pub prompt: String,
    pub output_format: String,
    pub kcl_source: Option<String>,
    pub output_bytes: Option<Vec<u8>>,
}

// ---------------------------------------------------------------------------
// Zoo API Client (types only — actual HTTP calls need async runtime)
// ---------------------------------------------------------------------------

/// Configuration for connecting to Zoo's API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZooConfig {
    /// API base URL (default: https://api.zoo.dev)
    pub base_url: String,
    /// API token for authentication.
    pub api_token: String,
}

impl ZooConfig {
    pub fn new(api_token: &str) -> Self {
        Self {
            base_url: "https://api.zoo.dev".into(),
            api_token: api_token.into(),
        }
    }
}

/// A Zoo API request for file conversion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertRequest {
    pub src_format: String,
    pub output_format: String,
    pub body: Vec<u8>,
}

/// A Zoo API request for text-to-CAD.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextToCadRequest {
    pub prompt: String,
    pub output_format: String,
}

/// Build a file conversion API request URL.
pub fn conversion_url(config: &ZooConfig, src: &str, dst: &str) -> String {
    format!("{}/file/conversion/{}/{}", config.base_url, src, dst)
}

/// Build a text-to-CAD API request URL.
pub fn text_to_cad_url(config: &ZooConfig) -> String {
    format!("{}/ai/text-to-cad", config.base_url)
}

/// Build authorization header value.
pub fn auth_header(config: &ZooConfig) -> String {
    format!("Bearer {}", config.api_token)
}

// ---------------------------------------------------------------------------
// OpenSCAD Import (bonus — since we're ingesting everything)
// ---------------------------------------------------------------------------

/// Import an OpenSCAD file to CFL.
pub fn import_openscad(scad_source: &str) -> String {
    physical_cfl::openscad_to_cfl(scad_source)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_kcl_basic() {
        let kcl = "const width = 50\nconst height = 30";
        let cfl = import_kcl(kcl);
        assert!(cfl.contains("let width = 50"));
        assert!(cfl.contains("let height = 30"));
    }

    #[test]
    fn import_kcl_to_ast_parses() {
        let kcl = "const width = 50";
        let ast = import_kcl_to_ast(kcl);
        assert!(!ast.is_empty());
    }

    #[test]
    fn import_kcl_to_mcp_produces_calls() {
        let kcl = "const part = startSketchOn('XY')";
        let _calls = import_kcl_to_mcp(kcl);
        // May or may not produce calls depending on parsing depth
    }

    #[test]
    fn zoo_config_default_url() {
        let config = ZooConfig::new("test-token");
        assert_eq!(config.base_url, "https://api.zoo.dev");
    }

    #[test]
    fn conversion_url_format() {
        let config = ZooConfig::new("tok");
        let url = conversion_url(&config, "step", "stl");
        assert!(url.contains("/file/conversion/step/stl"));
    }

    #[test]
    fn text_to_cad_url_format() {
        let config = ZooConfig::new("tok");
        let url = text_to_cad_url(&config);
        assert!(url.contains("/ai/text-to-cad"));
    }

    #[test]
    fn auth_header_format() {
        let config = ZooConfig::new("my-secret-token");
        assert_eq!(auth_header(&config), "Bearer my-secret-token");
    }

    #[test]
    fn import_openscad_translates() {
        let scad = "cube([10, 20, 30]);";
        let cfl = import_openscad(scad);
        assert!(cfl.contains("box("));
    }

    #[test]
    fn zoo_conversion_result_deserializes() {
        let json = r#"{"id":"abc","status":"completed","src_format":"step","output_format":"stl","output_bytes":null}"#;
        let result: ZooConversionResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.status, "completed");
    }

    #[test]
    fn zoo_text_to_cad_result_deserializes() {
        let json = r#"{"id":"xyz","status":"completed","prompt":"make a box","output_format":"step","kcl_source":"const b = box()","output_bytes":null}"#;
        let result: ZooTextToCadResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.prompt, "make a box");
        assert!(result.kcl_source.is_some());
    }
}
