//! `materialize --diff FILE` over in-memory ports: the Go scenario is
//! journeys/j001900_diagnosis.feature, "His assistant has the guidance and
//! hers does not, and something compares the two".

use angzarr_diagnostic::Diagnostic;
use library_application::{Flags, PackageRequest};

use super::*;
use crate::fakes::{world, World, CONFIG};
use crate::owner_voice::with_own_fragment;

/// What a materialize of the default profiles writes as the context.
fn written(w: &World) -> String {
    let package = w
        .setup
        .library
        .package(&Flags::default(), &PackageRequest::default())
        .result
        .unwrap();
    with_own_fragment(&package.context, package.use_distilled)
}

fn request(file: &str) -> DiffRequest {
    DiffRequest {
        engines: vec!["claude-code".to_string()],
        file: file.to_string(),
        ..DiffRequest::default()
    }
}

fn put(w: &World, path: &str, text: &str) {
    w.store
        .state()
        .files
        .insert(path.to_string(), text.as_bytes().to_vec());
}

fn compare(w: &World, req: &DiffRequest) -> Result<MaterializeDiff, SetupError> {
    w.setup.materialize_diff(req).map(|done| done.value)
}

#[test]
fn test_a_file_holding_the_written_context_is_identical() {
    let w = world(CONFIG);
    put(&w, "/p/theirs.md", &written(&w));
    let found = compare(&w, &request("theirs.md")).unwrap();
    assert!(found.identical);
    assert_eq!(found.diff, "");
    assert_eq!(found.compared_to, "theirs.md");
}

#[test]
fn test_trailing_newlines_are_not_a_difference() {
    let w = world(CONFIG);
    let context = written(&w);
    for theirs in [context.clone(), format!("{context}\n\n\n")] {
        put(&w, "/p/theirs.md", &theirs);
        assert!(compare(&w, &request("theirs.md")).unwrap().identical);
    }
}

#[test]
fn test_a_file_without_the_guidance_differs_and_the_diff_names_it() {
    let w = world(CONFIG);
    // Bob's copy lacks the project's fragment; hers (the written context) has it.
    let theirs = written(&w).replace("FRAGMENT-BODY-testing", "");
    put(&w, "/p/theirs.md", &theirs);
    let found = compare(&w, &request("theirs.md")).unwrap();
    assert!(!found.identical);
    assert!(
        found.diff.starts_with("--- theirs.md\n+++ "),
        "{}",
        found.diff
    );
    assert!(found.diff.contains("@@ -"), "{}", found.diff);
    assert!(
        found.diff.lines().any(|l| l == "+FRAGMENT-BODY-testing"),
        "{}",
        found.diff
    );
}

#[test]
fn test_the_diff_reads_from_the_file_to_the_context() {
    let w = world(CONFIG);
    put(&w, "/p/theirs.md", &format!("OLD-LINE\n{}", written(&w)));
    let found = compare(&w, &request("theirs.md")).unwrap();
    assert!(
        found.diff.lines().any(|l| l == "-OLD-LINE"),
        "{}",
        found.diff
    );
    assert!(
        found
            .diff
            .contains("+++ the default agent (materialized here)\n"),
        "{}",
        found.diff
    );
}

#[test]
fn test_profiles_are_named_joined_in_the_label_and_header() {
    let w = world(CONFIG);
    put(&w, "/p/theirs.md", "x");
    let mut req = request("theirs.md");
    req.profiles = vec!["dev".to_string()];
    let found = compare(&w, &req).unwrap();
    assert_eq!(found.label, "dev");
    assert_eq!(found.profiles, vec!["dev"]);
    assert!(
        found.diff.contains("+++ dev (materialized here)\n"),
        "{}",
        found.diff
    );
    let none = compare(&w, &request("theirs.md")).unwrap();
    assert_eq!(none.label, "the default agent");
}

#[test]
fn test_a_relative_file_is_read_from_the_working_directory_and_an_absolute_as_it_is() {
    let w = world(CONFIG);
    put(&w, "/p/sub/theirs.md", &written(&w));
    put(&w, "/elsewhere/theirs.md", &written(&w));
    assert!(
        compare(&w, &request("sub/../sub/theirs.md"))
            .unwrap()
            .identical
    );
    assert!(
        compare(&w, &request("/elsewhere/theirs.md"))
            .unwrap()
            .identical
    );
}

#[test]
fn test_a_comparison_writes_nothing() {
    let w = world(CONFIG);
    put(&w, "/p/theirs.md", "something else");
    compare(&w, &request("theirs.md")).unwrap();
    let state = w.store.state();
    assert!(state.claims.is_empty());
    assert!(state.log.is_empty(), "{:?}", state.log);
    assert_eq!(state.files.len(), 1);
}

#[test]
fn test_json_is_the_flat_object_with_the_diff_left_out_when_identical() {
    let w = world(CONFIG);
    put(&w, "/p/theirs.md", &written(&w));
    let same = compare(&w, &request("theirs.md")).unwrap();
    assert_eq!(
        serde_json::to_value(&same).unwrap(),
        serde_json::json!({"profiles": [], "compared_to": "theirs.md", "identical": true})
    );
    put(&w, "/p/theirs.md", "other");
    let mut req = request("theirs.md");
    req.profiles = vec!["dev".to_string()];
    let differs = serde_json::to_value(compare(&w, &req).unwrap()).unwrap();
    assert_eq!(differs["profiles"], serde_json::json!(["dev"]));
    assert_eq!(differs["identical"], false);
    assert!(differs["diff"]
        .as_str()
        .unwrap()
        .starts_with("--- theirs.md\n"));
    assert_eq!(differs.as_object().unwrap().len(), 4);
}

#[test]
fn test_a_missing_file_is_refused_by_name() {
    let w = world(CONFIG);
    let err = compare(&w, &request("nowhere.md")).unwrap_err();
    assert_eq!(err.code(), "setup-application-diff-file-missing");
    assert!(err.to_string().contains("nowhere.md"), "{err}");
    assert_eq!(err.exit_code(), 1);
}

#[test]
fn test_an_unreadable_file_is_refused_with_the_cause() {
    let w = world(CONFIG);
    w.store.state().unreadable = Some("/p/dir".to_string());
    let err = compare(&w, &request("dir")).unwrap_err();
    assert_eq!(err.code(), "setup-application-diff-file-unreadable");
    let said = err.to_string();
    assert!(
        said.contains("dir") && said.contains("Is a directory"),
        "{said}"
    );
}

#[test]
fn test_release_surfaces_without_context_and_two_engines_are_refused() {
    let w = world(CONFIG);
    put(&w, "/p/theirs.md", "x");
    let mut release = request("theirs.md");
    release.release = true;
    let mut no_context = request("theirs.md");
    no_context.surfaces = vec![SurfaceSpec::parse("hooks").unwrap()];
    let mut two = request("theirs.md");
    two.engines = vec!["claude-code".to_string(), "mock".to_string()];
    let mut unnamed = request("theirs.md");
    unnamed.engines.clear();
    for (req, wants) in [
        (release, "--release takes no profiles and no --diff"),
        (no_context, "without context leaves nothing to compare"),
        (two, "name at most one --backend"),
    ] {
        let err = compare(&w, &req).unwrap_err();
        assert!(err.to_string().contains(wants), "{err}");
        assert_eq!(err.exit_code(), 2, "{wants}");
    }
    // The one configured engine is the one compared when none is named.
    assert!(compare(&w, &unnamed).is_ok());
}

#[test]
fn test_a_surface_list_naming_context_still_compares() {
    let w = world(CONFIG);
    put(&w, "/p/theirs.md", &written(&w));
    let mut req = request("theirs.md");
    req.surfaces = vec![
        SurfaceSpec::parse("hooks").unwrap(),
        SurfaceSpec::parse("context").unwrap(),
    ];
    assert!(compare(&w, &req).unwrap().identical);
}

#[test]
fn test_an_unknown_engine_is_refused() {
    let w = world(CONFIG);
    put(&w, "/p/theirs.md", "x");
    let mut req = request("theirs.md");
    req.engines = vec!["bogus".to_string()];
    let err = compare(&w, &req).unwrap_err();
    assert_eq!(err.code(), "setup-application-unknown-engine");
}

#[test]
fn test_fatal_findings_refuse_the_comparison_unless_degraded() {
    let w = world(CONFIG);
    put(&w, "/p/theirs.md", "x");
    let mut req = request("theirs.md");
    req.profiles = vec!["broken".to_string()];
    let err = compare(&w, &req).unwrap_err();
    assert_eq!(err.code(), "setup-application-refused");
    req.flags.degraded = true;
    assert!(compare(&w, &req).is_ok());
}

#[test]
fn test_bytes_that_are_not_utf8_are_compared_lossily() {
    let w = world(CONFIG);
    w.store
        .state()
        .files
        .insert("/p/theirs.md".to_string(), vec![0xff, b'\n']);
    let found = compare(&w, &request("theirs.md")).unwrap();
    assert!(found.diff.contains("-\u{fffd}\n"), "{}", found.diff);
}
