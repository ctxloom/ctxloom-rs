//! Go's session sidecar, schema 1 to 3. The fixtures are Go-shaped documents
//! (`fixtures/go-sidecar/`, the key order `yamlx.Marshal` writes); the rules
//! are Go's `sidecar_schemaver_test.go` and the migration steps' tests
//! (`internal/migrations/sessionsidecar`, release/0.7).

use angzarr_diagnostic::Diagnostic;
use angzarr_embedded::Timestamp;

use super::{decode, GoSidecarError};

const SCHEMA1: &str = include_str!("../fixtures/go-sidecar/schema1.yaml");
const SCHEMA2: &str = include_str!("../fixtures/go-sidecar/schema2.yaml");
const SCHEMA3: &str = include_str!("../fixtures/go-sidecar/schema3.yaml");
const MINIMAL: &str = include_str!("../fixtures/go-sidecar/schema3-minimal.yaml");

fn at(seconds: i64, nanos: i32) -> Timestamp {
    Timestamp { seconds, nanos }
}

// 2026-04-10T00:00:00Z.
const APRIL_10: i64 = 1_775_779_200;

// Go: generation 1 passes through unchanged ("carries no keep mark") and its
// `source_entries` goes with the 2 to 3 step ("session compaction is gone").
#[test]
fn a_generation_1_sidecar_loads_and_its_source_entries_are_dropped() {
    let entry = decode(SCHEMA1).unwrap();
    assert_eq!(entry.session_id, "9a1c0d1e-1111-4111-8111-000000000001");
    assert_eq!(entry.backend, "claude-code");
    assert_eq!(entry.project_dir, "/work/proj");
    assert_eq!(
        entry.output_dir,
        "/home/u/Documents/ctxloom/proj/amber-swift-owl"
    );
    assert_eq!(entry.started_at, at(APRIL_10, 0));
    assert_eq!(entry.ended_at, Some(at(APRIL_10 + 3600, 0)));
    assert_eq!(
        entry.transcript_path,
        "/home/u/.claude/projects/proj/9a1c0d1e.jsonl"
    );
    assert!(!entry.keep, "generation 1 carries no keep mark");
    assert_eq!(entry.purged_at, None);
    assert!(entry.rotations.is_empty() && entry.run_ends.is_empty());
    assert_eq!(entry.end_reason, None);
}

// Go: a sidecar's instants are RFC 3339 with the fraction the writer had.
#[test]
fn a_generation_2_sidecar_has_the_keep_mark_and_exact_instants() {
    let entry = decode(SCHEMA2).unwrap();
    assert!(entry.keep);
    assert_eq!(entry.started_at, at(1_777_710_600, 481_000_000));
    assert_eq!(entry.ended_at, Some(at(1_777_712_400, 0)));
}

// Go: the full generation 3 sidecar: purged, kept, the engine's version,
// origin, the displaced bindings, how a run ended and why a session never
// launched.
#[test]
fn a_generation_3_sidecar_carries_everything_go_records() {
    let entry = decode(SCHEMA3).unwrap();
    assert!(entry.keep);
    assert_eq!(entry.purged_at, Some(at(1_782_864_000, 0)));
    assert_eq!(entry.engine_version, "2.1.229 (Claude Code)");
    assert_eq!(entry.origin, "agent");
    assert_eq!(entry.rotations.len(), 1);
    assert_eq!(
        entry.rotations[0].session_id,
        "9a1c0d1e-3333-0000-8333-000000000000"
    );
    assert_eq!(
        entry.rotations[0].transcript_path,
        "/home/u/.claude/projects/proj/9a1c0d1e-3333-0000.jsonl"
    );
    assert_eq!(entry.rotations[0].rotated_at, at(1_780_317_000, 0));
    assert_eq!(entry.run_ends.len(), 1);
    let end = &entry.run_ends[0];
    assert_eq!(end.run_id, "11111111-1111-4111-8111-111111111111");
    assert_eq!(
        (end.cause.as_str(), end.detail.as_str()),
        ("stopped", "asked to stop")
    );
    assert_eq!((end.exit_code, end.failed), (Some(130), true));
    let reason = entry.end_reason.expect("an end reason");
    assert_eq!((reason.exit, reason.class.as_str()), (2, "refused"));
    assert_eq!(reason.message, "the engine refused its credential");
    assert_eq!(reason.findings, ["credential refused"]);
}

// Go: `versionedSidecar` has only the fields a mint writes; the rest default.
#[test]
fn a_sidecar_with_only_a_mints_fields_defaults_the_rest() {
    let entry = decode(MINIMAL).unwrap();
    assert_eq!(entry.project_dir, "/work/proj");
    assert_eq!(entry.session_id, "");
    assert_eq!(
        entry.origin, "",
        "an empty origin reads as a human's session"
    );
    assert!(!entry.keep);
    assert_eq!(entry.ended_at, None);
}

// Go: TestSidecar_KeylessIsRefused: "A sidecar that declares no generation is
// refused, not guessed at."
#[test]
fn a_sidecar_declaring_no_generation_is_refused_as_too_old() {
    let keyless = "project_dir: /proj/a\nbackend: claude-code\nstarted_at: 2026-09-01T10:00:00Z\n";
    assert_eq!(
        decode(keyless).unwrap_err(),
        GoSidecarError::TooOld {
            found: 0,
            oldest: 1
        }
    );
}

// Go: TestSidecar_NewerIsRefusedNamingBothNumbers.
#[test]
fn a_newer_sidecar_is_refused_naming_both_numbers() {
    let newer = format!(
        "schema_version: 4\n{}",
        MINIMAL.trim_start_matches("schema_version: 3\n")
    );
    let error = decode(&newer).unwrap_err();
    assert_eq!(
        error,
        GoSidecarError::TooNew {
            found: 4,
            current: 3
        }
    );
    assert_eq!(error.code(), "sessions-application-go-sidecar-too-new");
}

// Go: TestSidecar_MalformedIsAParseFailureNotAVersionFault.
#[test]
fn a_sidecar_that_is_not_yaml_is_its_parse_failure_not_a_version_fault() {
    let error = decode("project_dir: [unterminated\n").unwrap_err();
    assert!(matches!(error, GoSidecarError::Unreadable(_)), "{error:?}");
}

// Strict for all files, Go-written ones included (D15, 2026-10-31).
#[test]
fn a_key_this_build_does_not_know_is_refused() {
    let odd = format!("{MINIMAL}essence: a summary an older ctxloom wrote\n");
    assert!(matches!(
        decode(&odd).unwrap_err(),
        GoSidecarError::Unreadable(_)
    ));
    // `source_entries` was dropped by the 2 to 3 step: a generation-3 sidecar
    // holding it is not one Go wrote.
    let stale = format!("{MINIMAL}source_entries: 12\n");
    assert!(matches!(
        decode(&stale).unwrap_err(),
        GoSidecarError::Unreadable(_)
    ));
}

#[test]
fn an_instant_that_is_not_rfc_3339_is_refused_naming_the_field() {
    let bad = MINIMAL.replace("2026-09-01T10:00:00Z", "yesterday");
    assert_eq!(
        decode(&bad).unwrap_err(),
        GoSidecarError::Instant {
            field: "started_at".to_string(),
            value: "yesterday".to_string()
        }
    );
}

#[test]
fn a_flag_that_is_not_a_bool_is_refused() {
    let bad = format!("{MINIMAL}keep: maybe\n");
    assert!(matches!(
        decode(&bad).unwrap_err(),
        GoSidecarError::Unreadable(_)
    ));
}
