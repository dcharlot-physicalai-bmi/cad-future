//! `physical-emit-oie` — Native .oie format serialization for parametric model documents.

use physical_parametric::ModelDocument;

/// Serialize a `ModelDocument` to the OIE binary format.
///
/// Currently uses JSON as the underlying encoding.
pub fn save_oie(doc: &ModelDocument) -> Vec<u8> {
    serde_json::to_vec(doc).expect("ModelDocument should be serializable")
}

/// Deserialize a `ModelDocument` from OIE binary format.
///
/// Returns `None` if the data is corrupted or not valid OIE format.
pub fn load_oie(data: &[u8]) -> Option<ModelDocument> {
    serde_json::from_slice(data).ok()
}
