//! The survey's lookups and the sources' names.

use std::path::PathBuf;

use super::{IdSource, Registered, Survey};

fn entry(id: &str, path: &str) -> Registered {
    Registered {
        id: id.to_string(),
        path: path.to_string(),
        canonical: path.to_string(),
        exists: true,
    }
}

fn survey() -> Survey {
    Survey {
        work_root: "/w".to_string(),
        root: "/w".to_string(),
        resolved: None,
        marker: None,
        legacy_marker: None,
        registry: vec![entry("a", "/w"), entry("b", "/x"), entry("a", "/y")],
        databases: Vec::new(),
        projects: PathBuf::from("/h/.ctxloom/projects"),
    }
}

#[test]
fn test_a_projects_store_is_its_state_db() {
    assert_eq!(
        survey().store("a"),
        PathBuf::from("/h/.ctxloom/projects/a/state.db")
    );
}

#[test]
fn test_entries_are_found_by_path_and_by_id() {
    let survey = survey();
    let at: Vec<&str> = survey.entries_at("/w").map(|e| e.id.as_str()).collect();
    assert_eq!(at, ["a"]);
    let of: Vec<&str> = survey.entries_of("a").map(|e| e.path.as_str()).collect();
    assert_eq!(of, ["/w", "/y"]);
}

#[test]
fn test_each_source_has_a_stable_name() {
    let names: Vec<&str> = [
        IdSource::Flag,
        IdSource::Environment,
        IdSource::Marker,
        IdSource::LegacyMarker,
        IdSource::Registry,
    ]
    .iter()
    .map(|s| s.as_str())
    .collect();
    assert_eq!(
        names,
        ["flag", "environment", "marker", "legacy-marker", "registry"]
    );
}
