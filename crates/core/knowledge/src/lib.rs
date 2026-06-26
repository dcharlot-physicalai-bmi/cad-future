//! Tribal knowledge capture and retrieval for experienced engineers.
//!
//! Encodes process knowledge, rules of thumb, and hard-won lessons
//! so that institutional expertise is searchable and contextual.

use serde::{Deserialize, Serialize};

/// Category of engineering knowledge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KnowledgeCategory {
    Material,
    Process,
    Design,
    Quality,
    Assembly,
}

/// A single knowledge entry — a rule of thumb, lesson learned, or best practice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    pub id: u64,
    pub title: String,
    pub category: KnowledgeCategory,
    pub content: String,
    pub tags: Vec<String>,
    pub material_id: Option<String>,
    pub process: Option<String>,
    pub created_by: String,
    /// Confidence in the entry, 0.0 to 1.0.
    pub confidence: f64,
}

/// Central store of engineering knowledge.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KnowledgeBase {
    entries: Vec<KnowledgeEntry>,
    next_id: u64,
}

impl KnowledgeBase {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
        }
    }

    /// Create a knowledge base pre-populated with common engineering rules of thumb.
    pub fn with_defaults() -> Self {
        let mut kb = Self::new();
        let defaults = vec![
            ("Min tap depth for 6061-T6", KnowledgeCategory::Process,
             "6061-T6 needs 1.5D min tap depth for steel-strength thread engagement.",
             vec!["tapping", "thread", "aluminum"],
             Some("6061-T6"), Some("tapping"), 0.95),
            ("Stock on bearing bores", KnowledgeCategory::Process,
             "Always add 0.5mm stock on bearing bores for final grinding.",
             vec!["bearing", "grinding", "tolerance"],
             None, Some("grinding"), 0.9),
            ("CNC pocket depth ratio", KnowledgeCategory::Design,
             "Minimum 3:1 depth-to-width ratio for CNC pocket milling.",
             vec!["pocket", "milling", "cnc"],
             None, Some("milling"), 0.95),
            ("Stainless steel machining", KnowledgeCategory::Process,
             "Stainless steel work-hardens — use climb milling, sharp tools, constant feed.",
             vec!["stainless", "work-hardening", "milling"],
             Some("304-SS"), Some("milling"), 0.95),
            ("Wall thickness for injection molding", KnowledgeCategory::Design,
             "Maintain uniform wall thickness within 10% variation to avoid sink marks and warping in injection-molded parts.",
             vec!["injection-molding", "wall-thickness", "plastic"],
             None, Some("injection-molding"), 0.9),
            ("Fillet radius for castings", KnowledgeCategory::Design,
             "Inside corners on castings need minimum R3mm fillet to prevent hot tears and stress concentration.",
             vec!["casting", "fillet", "stress"],
             None, Some("casting"), 0.85),
            ("Sheet metal bend radius", KnowledgeCategory::Material,
             "Minimum bend radius for mild steel sheet is 1x material thickness; for aluminum 6061-T6 use 2x.",
             vec!["sheet-metal", "bending", "radius"],
             Some("mild-steel"), Some("bending"), 0.95),
            ("GD&T datum strategy", KnowledgeCategory::Quality,
             "Primary datum should be the largest stable surface; secondary datum constrains rotation; tertiary locks remaining DOF.",
             vec!["gd&t", "datum", "inspection"],
             None, None, 0.9),
            ("Press-fit interference", KnowledgeCategory::Assembly,
             "For steel-to-steel press fits, use 0.001 inch per inch of shaft diameter interference as starting point.",
             vec!["press-fit", "interference", "assembly"],
             Some("steel"), Some("pressing"), 0.85),
            ("Surface finish for sealing", KnowledgeCategory::Quality,
             "O-ring groove surfaces need 16 Ra micro-inch or better finish for reliable sealing.",
             vec!["surface-finish", "o-ring", "sealing"],
             None, Some("turning"), 0.9),
            ("EDM corner radius", KnowledgeCategory::Process,
             "Wire EDM leaves minimum internal corner radius of wire diameter + 0.05mm overcut per side.",
             vec!["edm", "corner", "wire"],
             None, Some("edm"), 0.9),
            ("Anodize dimensional growth", KnowledgeCategory::Material,
             "Type III hard anodize adds ~0.001 inch per side (50% penetration, 50% growth). Account in toleranced features.",
             vec!["anodize", "aluminum", "coating"],
             Some("6061-T6"), Some("anodizing"), 0.9),
        ];

        for (title, cat, content, tags, mat, proc, conf) in defaults {
            kb.add_entry(
                title.into(),
                cat,
                content.into(),
                tags.into_iter().map(String::from).collect(),
                mat.map(String::from),
                proc.map(String::from),
                "system".into(),
                conf,
            );
        }
        kb
    }

    /// Add a new knowledge entry. Returns the assigned id.
    pub fn add_entry(
        &mut self,
        title: String,
        category: KnowledgeCategory,
        content: String,
        tags: Vec<String>,
        material_id: Option<String>,
        process: Option<String>,
        created_by: String,
        confidence: f64,
    ) -> u64 {
        let confidence = confidence.clamp(0.0, 1.0);
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(KnowledgeEntry {
            id,
            title,
            category,
            content,
            tags,
            material_id,
            process,
            created_by,
            confidence,
        });
        id
    }

    /// Keyword search across title, content, and tags. Case-insensitive.
    pub fn search(&self, query: &str) -> Vec<&KnowledgeEntry> {
        let q = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.title.to_lowercase().contains(&q)
                    || e.content.to_lowercase().contains(&q)
                    || e.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .collect()
    }

    /// All entries related to a specific material.
    pub fn entries_for_material(&self, material_id: &str) -> Vec<&KnowledgeEntry> {
        let mid = material_id.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.material_id
                    .as_ref()
                    .map(|m| m.to_lowercase() == mid)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// All entries related to a specific process.
    pub fn entries_for_process(&self, process_name: &str) -> Vec<&KnowledgeEntry> {
        let pn = process_name.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.process
                    .as_ref()
                    .map(|p| p.to_lowercase() == pn)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Contextual knowledge retrieval — returns entries relevant to the given
    /// combination of material, process, and geometry type.
    pub fn suggest_for_context(
        &self,
        material_id: Option<&str>,
        process: Option<&str>,
        geometry_type: Option<&str>,
    ) -> Vec<&KnowledgeEntry> {
        self.entries
            .iter()
            .filter(|e| {
                let mut score = 0u8;
                if let Some(mid) = material_id {
                    if e.material_id
                        .as_ref()
                        .map(|m| m.to_lowercase() == mid.to_lowercase())
                        .unwrap_or(false)
                    {
                        score += 1;
                    }
                }
                if let Some(proc) = process {
                    if e.process
                        .as_ref()
                        .map(|p| p.to_lowercase() == proc.to_lowercase())
                        .unwrap_or(false)
                    {
                        score += 1;
                    }
                }
                if let Some(geo) = geometry_type {
                    let g = geo.to_lowercase();
                    if e.tags.iter().any(|t| t.to_lowercase().contains(&g))
                        || e.content.to_lowercase().contains(&g)
                    {
                        score += 1;
                    }
                }
                score > 0
            })
            .collect()
    }

    /// All entries.
    pub fn entries(&self) -> &[KnowledgeEntry] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_search() {
        let mut kb = KnowledgeBase::new();
        kb.add_entry(
            "Test rule".into(),
            KnowledgeCategory::Design,
            "Use 2mm minimum wall thickness".into(),
            vec!["wall".into(), "thickness".into()],
            None,
            Some("milling".into()),
            "alice".into(),
            0.8,
        );
        let results = kb.search("wall");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Test rule");
    }

    #[test]
    fn test_search_case_insensitive() {
        let mut kb = KnowledgeBase::new();
        kb.add_entry(
            "Aluminum Tip".into(),
            KnowledgeCategory::Material,
            "Some content".into(),
            vec![],
            None,
            None,
            "bob".into(),
            0.7,
        );
        assert_eq!(kb.search("aluminum").len(), 1);
        assert_eq!(kb.search("ALUMINUM").len(), 1);
    }

    #[test]
    fn test_search_by_tag() {
        let mut kb = KnowledgeBase::new();
        kb.add_entry(
            "Rule".into(),
            KnowledgeCategory::Process,
            "Content".into(),
            vec!["milling".into(), "cnc".into()],
            None,
            None,
            "c".into(),
            0.9,
        );
        assert_eq!(kb.search("cnc").len(), 1);
    }

    #[test]
    fn test_entries_for_material() {
        let kb = KnowledgeBase::with_defaults();
        let al = kb.entries_for_material("6061-T6");
        assert!(al.len() >= 2);
    }

    #[test]
    fn test_entries_for_process() {
        let kb = KnowledgeBase::with_defaults();
        let mill = kb.entries_for_process("milling");
        assert!(mill.len() >= 2);
    }

    #[test]
    fn test_suggest_for_context_material_only() {
        let kb = KnowledgeBase::with_defaults();
        let suggestions = kb.suggest_for_context(Some("6061-T6"), None, None);
        assert!(!suggestions.is_empty());
    }

    #[test]
    fn test_suggest_for_context_combined() {
        let kb = KnowledgeBase::with_defaults();
        let suggestions =
            kb.suggest_for_context(Some("304-SS"), Some("milling"), None);
        assert!(!suggestions.is_empty());
    }

    #[test]
    fn test_suggest_for_context_geometry() {
        let kb = KnowledgeBase::with_defaults();
        let suggestions = kb.suggest_for_context(None, None, Some("pocket"));
        assert!(!suggestions.is_empty());
    }

    #[test]
    fn test_confidence_clamped() {
        let mut kb = KnowledgeBase::new();
        kb.add_entry(
            "Over".into(),
            KnowledgeCategory::Quality,
            "".into(),
            vec![],
            None,
            None,
            "x".into(),
            1.5,
        );
        assert_eq!(kb.entries()[0].confidence, 1.0);
    }

    #[test]
    fn test_defaults_populated() {
        let kb = KnowledgeBase::with_defaults();
        assert!(kb.entries().len() >= 10);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let kb = KnowledgeBase::with_defaults();
        let json = serde_json::to_string(&kb).unwrap();
        let kb2: KnowledgeBase = serde_json::from_str(&json).unwrap();
        assert_eq!(kb.entries().len(), kb2.entries().len());
    }
}
