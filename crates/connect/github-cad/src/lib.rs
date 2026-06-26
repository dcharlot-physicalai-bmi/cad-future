use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// CAD diff types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeometryChange {
    pub file_path: String,
    pub change_type: ChangeType,
    pub vertex_delta: i64,
    pub face_delta: i64,
    pub volume_delta_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CadDiffResult {
    pub files_changed: usize,
    pub geometry_changes: Vec<GeometryChange>,
    pub summary: String,
}

// ---------------------------------------------------------------------------
// PR comment generation
// ---------------------------------------------------------------------------

pub fn generate_pr_comment(diff: &CadDiffResult) -> String {
    let mut md = String::new();
    md.push_str("## CAD Geometry Review\n\n");
    md.push_str(&format!(
        "**{}** CAD file(s) changed\n\n",
        diff.files_changed
    ));

    if diff.geometry_changes.is_empty() {
        md.push_str("No geometry changes detected.\n");
        return md;
    }

    md.push_str("| File | Change | Vertices | Faces | Volume |\n");
    md.push_str("|------|--------|----------|-------|--------|\n");

    for gc in &diff.geometry_changes {
        let change_str = match gc.change_type {
            ChangeType::Added => "Added",
            ChangeType::Modified => "Modified",
            ChangeType::Deleted => "Deleted",
        };
        let sign = |v: i64| -> String {
            if v >= 0 {
                format!("+{v}")
            } else {
                format!("{v}")
            }
        };
        md.push_str(&format!(
            "| `{}` | {} | {} | {} | {:.1}% |\n",
            gc.file_path,
            change_str,
            sign(gc.vertex_delta),
            sign(gc.face_delta),
            gc.volume_delta_pct,
        ));
    }

    md.push_str(&format!("\n> {}\n", diff.summary));
    md
}

// ---------------------------------------------------------------------------
// Webhook types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    pub event: String,
    pub action: Option<String>,
    pub repository: String,
    #[serde(default)]
    pub sender: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WebhookEvent {
    Push {
        branch: String,
        files: Vec<String>,
    },
    PullRequest {
        number: u64,
        title: String,
        action: String,
        files: Vec<String>,
    },
}

/// Parse a raw JSON webhook body into a `WebhookEvent`.
///
/// Returns `None` when the payload is not a recognised push or pull_request
/// event, or when required fields are missing.
pub fn parse_webhook(json: &str) -> Option<WebhookEvent> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;

    // Determine event type from payload structure
    if let Some(pr) = v.get("pull_request") {
        let number = pr.get("number")?.as_u64()?;
        let title = pr.get("title")?.as_str()?.to_string();
        let action = v.get("action")?.as_str()?.to_string();
        let files = extract_string_array(v.get("files"));
        return Some(WebhookEvent::PullRequest {
            number,
            title,
            action,
            files,
        });
    }

    if let Some(ref_field) = v.get("ref") {
        let ref_str = ref_field.as_str()?;
        let branch = ref_str
            .strip_prefix("refs/heads/")
            .unwrap_or(ref_str)
            .to_string();
        let files = extract_files_from_commits(&v);
        return Some(WebhookEvent::Push { branch, files });
    }

    None
}

fn extract_string_array(val: Option<&serde_json::Value>) -> Vec<String> {
    val.and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn extract_files_from_commits(v: &serde_json::Value) -> Vec<String> {
    let mut files = Vec::new();
    if let Some(commits) = v.get("commits").and_then(|c| c.as_array()) {
        for commit in commits {
            for key in &["added", "modified", "removed"] {
                if let Some(arr) = commit.get(*key).and_then(|a| a.as_array()) {
                    for f in arr {
                        if let Some(s) = f.as_str() {
                            let s = s.to_string();
                            if !files.contains(&s) {
                                files.push(s);
                            }
                        }
                    }
                }
            }
        }
    }
    files
}

// ---------------------------------------------------------------------------
// CAD file detection
// ---------------------------------------------------------------------------

const CAD_EXTENSIONS: &[&str] = &[
    ".step", ".stp", ".stl", ".3mf", ".obj", ".kcl", ".cfl", ".sldprt",
];

/// Filter a list of file paths, returning only those with known CAD extensions.
pub fn detect_cad_files(files: &[String]) -> Vec<String> {
    files
        .iter()
        .filter(|f| {
            let lower = f.to_lowercase();
            CAD_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
        })
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Badge generation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BadgeStats {
    pub cad_files: usize,
    pub total_vertices: u64,
    pub total_faces: u64,
}

/// Return a shields.io badge URL summarising the CAD statistics.
pub fn format_cad_badge(stats: &BadgeStats) -> String {
    let label = "CAD";
    let message = format!(
        "{} files | {}V {}F",
        stats.cad_files, stats.total_vertices, stats.total_faces
    );
    let color = if stats.cad_files == 0 {
        "lightgrey"
    } else {
        "blue"
    };
    format!(
        "https://img.shields.io/badge/{}-{}-{}",
        urlenc(label),
        urlenc(&message),
        color,
    )
}

fn urlenc(s: &str) -> String {
    s.replace(' ', "%20")
        .replace('|', "%7C")
        .replace('#', "%23")
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_diff() -> CadDiffResult {
        CadDiffResult {
            files_changed: 2,
            geometry_changes: vec![
                GeometryChange {
                    file_path: "bracket.step".into(),
                    change_type: ChangeType::Modified,
                    vertex_delta: 120,
                    face_delta: 8,
                    volume_delta_pct: 3.5,
                },
                GeometryChange {
                    file_path: "housing.stl".into(),
                    change_type: ChangeType::Added,
                    vertex_delta: 5000,
                    face_delta: 2400,
                    volume_delta_pct: 100.0,
                },
            ],
            summary: "Bracket modified, new housing added".into(),
        }
    }

    #[test]
    fn pr_comment_contains_table_header() {
        let comment = generate_pr_comment(&sample_diff());
        assert!(comment.contains("| File | Change |"));
    }

    #[test]
    fn pr_comment_lists_files() {
        let comment = generate_pr_comment(&sample_diff());
        assert!(comment.contains("bracket.step"));
        assert!(comment.contains("housing.stl"));
    }

    #[test]
    fn pr_comment_empty_diff() {
        let diff = CadDiffResult {
            files_changed: 0,
            geometry_changes: vec![],
            summary: String::new(),
        };
        let comment = generate_pr_comment(&diff);
        assert!(comment.contains("No geometry changes detected"));
    }

    #[test]
    fn detect_cad_files_filters_correctly() {
        let files: Vec<String> = vec![
            "readme.md".into(),
            "part.step".into(),
            "model.STL".into(),
            "build.rs".into(),
            "fixture.3mf".into(),
            "design.kcl".into(),
            "output.cfl".into(),
            "assembly.sldprt".into(),
        ];
        let cad = detect_cad_files(&files);
        assert_eq!(cad.len(), 6);
        assert!(!cad.contains(&"readme.md".to_string()));
        assert!(!cad.contains(&"build.rs".to_string()));
    }

    #[test]
    fn detect_cad_files_empty_input() {
        let cad = detect_cad_files(&[]);
        assert!(cad.is_empty());
    }

    #[test]
    fn parse_webhook_push() {
        let json = r#"{
            "ref": "refs/heads/main",
            "commits": [
                {
                    "added": ["part.step"],
                    "modified": ["bracket.stl"],
                    "removed": []
                }
            ]
        }"#;
        let evt = parse_webhook(json).unwrap();
        match evt {
            WebhookEvent::Push { branch, files } => {
                assert_eq!(branch, "main");
                assert_eq!(files.len(), 2);
            }
            _ => panic!("expected Push"),
        }
    }

    #[test]
    fn parse_webhook_pull_request() {
        let json = r#"{
            "action": "opened",
            "pull_request": {
                "number": 42,
                "title": "Add housing model"
            },
            "files": ["housing.step", "housing.stl"]
        }"#;
        let evt = parse_webhook(json).unwrap();
        match evt {
            WebhookEvent::PullRequest {
                number,
                title,
                action,
                files,
            } => {
                assert_eq!(number, 42);
                assert_eq!(title, "Add housing model");
                assert_eq!(action, "opened");
                assert_eq!(files.len(), 2);
            }
            _ => panic!("expected PullRequest"),
        }
    }

    #[test]
    fn parse_webhook_invalid_json() {
        assert!(parse_webhook("not json").is_none());
    }

    #[test]
    fn parse_webhook_unknown_event() {
        let json = r#"{"type": "release"}"#;
        assert!(parse_webhook(json).is_none());
    }

    #[test]
    fn badge_url_format() {
        let stats = BadgeStats {
            cad_files: 3,
            total_vertices: 15000,
            total_faces: 8000,
        };
        let url = format_cad_badge(&stats);
        assert!(url.starts_with("https://img.shields.io/badge/"));
        assert!(url.contains("blue"));
    }

    #[test]
    fn badge_empty_stats() {
        let stats = BadgeStats {
            cad_files: 0,
            total_vertices: 0,
            total_faces: 0,
        };
        let url = format_cad_badge(&stats);
        assert!(url.contains("lightgrey"));
    }

    #[test]
    fn geometry_change_serialization_roundtrip() {
        let gc = GeometryChange {
            file_path: "test.step".into(),
            change_type: ChangeType::Deleted,
            vertex_delta: -100,
            face_delta: -50,
            volume_delta_pct: -100.0,
        };
        let json = serde_json::to_string(&gc).unwrap();
        let gc2: GeometryChange = serde_json::from_str(&json).unwrap();
        assert_eq!(gc2.change_type, ChangeType::Deleted);
        assert_eq!(gc2.vertex_delta, -100);
    }
}
