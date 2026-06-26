//! `physical-web` — WASM + WebGPU entry point for OpenIE.
//!
//! This is the single checkpoint for all browser/wasm_bindgen concerns.
//! All platform-specific JS interop lives here. The rest of the codebase is pure Rust.

#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(target_arch = "wasm32")]
pub use wasm::CadApp;

// On non-wasm targets, export nothing — this crate is web-only.
// The native and server platform crates handle other targets.
