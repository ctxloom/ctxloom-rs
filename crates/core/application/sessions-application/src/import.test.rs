//! `session import`: the OWNER'S D5 RULINGS, not Go's. Committed red and
//! WITHHELD from implementation until the owner reviews them: the stable id
//! of an imported session is UUIDv5(project, harp), its name is claimed
//! first, and a rerun adds nothing. What is Go's is the listing the import
//! must give for each sidecar fixture (schema 1 to 3): "the same listing Go
//! gives for it" (plan S4).
//!
//! Open, and so untested here: whether Go's `run_ends[]` are recorded at all
//! (plan 9: "for S4's red tests to settle with the owner").

use angzarr_embedded::angzarr_ports::memory::MemoryStore;
use angzarr_embedded::{EventStore, Kernel, OnRetryable, Sampled, StreamRef, Timestamp};
use session_ports::GoSession;
use sessions::{ClaimName, NameClaimAggregate, NameClaimState, Origin};

use crate::fakes::{
    cover, minted, project_at, ready, FakeClock, FakeDelay, FakeFiles, FakeGo, FakeIdentity,
    FakeLiveness, FakeStores,
};
use crate::{
    imported_id, ImportRequest, ImportService, Importer, ListQuery, Service, SessionService, Why,
};

const SCHEMA1: &str = include_str!("../fixtures/go-sidecar/schema1.yaml");
const SCHEMA2: &str = include_str!("../fixtures/go-sidecar/schema2.yaml");
const SCHEMA3: &str = include_str!("../fixtures/go-sidecar/schema3.yaml");
const HERE: &str = "jolly-mute-magma";
const OWL: &str = "amber-swift-owl";
const MOTH: &str = "brisk-copper-moth";
const WREN: &str = "calm-dusty-wren";

type Imp = Importer<FakeIdentity, FakeStores, FakeGo, FakeClock, FakeDelay>;

fn go(harp: &str, sidecar: &str, keep_marker: bool) -> GoSession {
    GoSession {
        harp: harp.to_string(),
        sidecar: Ok(sidecar.to_string()),
        keep_marker,
    }
}

/// An importer over Go's three fixture sessions, all of project `/work/proj`,
/// which the registry holds as `jolly-mute-magma`.
fn importer(sessions: Vec<GoSession>) -> Imp {
    Importer::new(
        FakeIdentity {
            bound: Some(project_at(HERE, "/work/proj")),
            every: vec![project_at(HERE, "/work/proj")],
            fails: None,
        },
        FakeStores::default(),
        FakeGo(sessions),
        FakeClock::default(),
        FakeDelay::default(),
    )
}

fn three() -> Vec<GoSession> {
    vec![
        go(OWL, SCHEMA1, false),
        go(MOTH, SCHEMA2, false),
        go(WREN, SCHEMA3, false),
    ]
}

/// The sessions the import left, read back as `session list --all` would.
fn listing(imp: Imp) -> Vec<crate::View> {
    let mut service = Service::new(
        imp.identity,
        imp.stores,
        FakeFiles::default(),
        FakeLiveness::default(),
        FakeClock::default(),
        FakeDelay::default(),
    );
    ready(service.list(ListQuery {
        all: true,
        ..ListQuery::default()
    }))
    .unwrap()
}

// ------------------------------------------------------------------ the id

// D5 Q1: UUIDv5(project, harp).
#[test]
fn the_imported_id_is_a_uuid_v5_of_the_project_and_the_harp() {
    let id = imported_id(HERE, OWL);
    assert_eq!(id, imported_id(HERE, OWL), "a function of its inputs");
    assert_ne!(id, imported_id(HERE, MOTH), "per harp");
    assert_ne!(id, imported_id("brisk-tall-oak", OWL), "per project");
    let parsed = uuid::Uuid::parse_str(&id).expect("a UUID");
    assert_eq!(parsed.get_version_num(), 5);
}

// ------------------------------------------------------------------ the import

#[test]
fn each_go_session_is_imported_into_the_project_whose_root_is_its_directory() {
    let mut imp = importer(three());
    let report = ready(imp.import(ImportRequest::default())).unwrap();
    let mut got: Vec<(String, String, String)> = report
        .imported
        .iter()
        .map(|i| (i.harp.clone(), i.project.clone(), i.id.clone()))
        .collect();
    got.sort();
    let mut want: Vec<(String, String, String)> = [OWL, MOTH, WREN]
        .iter()
        .map(|h| (h.to_string(), HERE.to_string(), imported_id(HERE, h)))
        .collect();
    want.sort();
    assert_eq!(got, want);
    assert!(report.already.is_empty() && report.left.is_empty());
}

// plan S4: "Importing every Go session.yaml fixture (schema 1 to 3) gives the
// same listing Go gives for it, on ids UUIDv5(project, harp)".
#[test]
fn the_listing_after_an_import_is_the_listing_go_gives() {
    let mut imp = importer(three());
    ready(imp.import(ImportRequest::default())).unwrap();
    let views = listing(imp);
    let by = |harp: &str| {
        views
            .iter()
            .find(|v| v.record.harp == harp)
            .expect(harp)
            .record
            .clone()
    };

    let owl = by(OWL);
    assert_eq!(owl.id, imported_id(HERE, OWL));
    assert_eq!(owl.engine, "claude-code");
    assert_eq!(owl.project_dir, "/work/proj");
    assert_eq!(
        owl.output_dir,
        "/home/u/Documents/ctxloom/proj/amber-swift-owl"
    );
    assert_eq!(
        owl.started_at,
        Timestamp {
            seconds: 1_775_779_200,
            nanos: 0
        }
    );
    assert_eq!(
        owl.ended_at,
        Some(Timestamp {
            seconds: 1_775_782_800,
            nanos: 0
        })
    );
    assert_eq!(owl.native_id, "9a1c0d1e-1111-4111-8111-000000000001");
    assert_eq!(
        owl.transcript_path,
        "/home/u/.claude/projects/proj/9a1c0d1e.jsonl"
    );
    assert!(!owl.kept && owl.purged_at.is_none() && owl.rotations.is_empty());

    let moth = by(MOTH);
    assert!(moth.kept, "schema 2's keep mark");
    assert_eq!(
        moth.started_at,
        Timestamp {
            seconds: 1_777_710_600,
            nanos: 481_000_000
        }
    );

    let wren = by(WREN);
    assert!(wren.kept);
    assert_eq!(
        wren.purged_at,
        Some(Timestamp {
            seconds: 1_782_864_000,
            nanos: 0
        })
    );
    assert_eq!(wren.engine_version, "2.1.229 (Claude Code)");
    assert_eq!(wren.origin, Some(Origin::Agent));
    assert_eq!(wren.rotations.len(), 1);
    assert_eq!(
        wren.rotations[0].native_id,
        "9a1c0d1e-3333-0000-8333-000000000000"
    );
    assert_eq!(
        wren.rotations[0].rotated_at,
        Some(Timestamp {
            seconds: 1_780_317_000,
            nanos: 0
        }),
        "when it really rotated, not when it was imported"
    );
}

// plan S4: "a re-import adds nothing".
#[test]
fn a_rerun_adds_nothing() {
    let mut imp = importer(three());
    ready(imp.import(ImportRequest::default())).unwrap();
    let heads = |imp: &Imp| -> Vec<u64> {
        let store: &MemoryStore = imp.stores.stores.get(HERE).expect("a store");
        [OWL, MOTH, WREN]
            .iter()
            .map(|h| {
                let id = uuid::Uuid::parse_str(&imported_id(HERE, h)).unwrap();
                let cover = cover_of(id);
                let stream = StreamRef::try_from(&cover).unwrap();
                ready(EventStore::head(store, stream))
                    .unwrap()
                    .next_sequence
            })
            .collect()
    };
    let before = heads(&imp);
    assert!(before.iter().all(|n| *n > 0));
    let again = ready(imp.import(ImportRequest::default())).unwrap();
    assert!(again.imported.is_empty(), "{again:?}");
    let mut already = again.already.clone();
    already.sort();
    assert_eq!(
        already,
        [WREN, OWL, MOTH]
            .map(String::from)
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    );
    assert_eq!(heads(&imp), before, "no event was added");
}

fn cover_of(id: uuid::Uuid) -> angzarr_embedded::v1::Cover {
    angzarr_embedded::v1::Cover {
        domain: sessions::SESSION_DOMAIN.to_string(),
        root: Some(angzarr_embedded::v1::Uuid::from_bytes(*id.as_bytes())),
        edition: None,
    }
}

// Plan 9 / 3.1: the name is claimed for the session first.
#[test]
fn an_imported_session_holds_its_name() {
    let mut imp = importer(vec![go(OWL, SCHEMA1, false)]);
    ready(imp.import(ImportRequest::default())).unwrap();
    let store = imp.stores.stores.get(HERE).expect("a store");
    let mut registry = angzarr_embedded::Registry::new();
    registry
        .aggregate::<NameClaimAggregate>(NameClaimAggregate::default)
        .unwrap();
    let claim_cover = angzarr_embedded::v1::Cover {
        domain: sessions::NAMES_DOMAIN.to_string(),
        root: Some(sessions::claim_root(HERE, OWL)),
        edition: None,
    };
    let (claim, _) =
        ready(Kernel::new(&registry).load::<NameClaimAggregate, _>(store, &claim_cover)).unwrap();
    let state: &NameClaimState = claim.state().state();
    assert_eq!(state.session, imported_id(HERE, OWL));
}

// D5 Q1: a name another session holds is not taken: the Go session is left,
// named, not renamed.
#[test]
fn a_name_another_session_holds_leaves_the_go_session() {
    let mut imp = importer(vec![go(OWL, SCHEMA1, false)]);
    {
        let store = imp.stores.stores.entry(HERE.to_string()).or_default();
        minted(store, 9, OWL, "/work/proj", 1_700_000_000);
        let registry = {
            let mut r = angzarr_embedded::Registry::new();
            r.aggregate::<NameClaimAggregate>(NameClaimAggregate::default)
                .unwrap();
            r
        };
        let book = angzarr_embedded::v1::CommandBook {
            cover: Some(angzarr_embedded::v1::Cover {
                domain: sessions::NAMES_DOMAIN.to_string(),
                root: Some(sessions::claim_root(HERE, OWL)),
                edition: None,
            }),
            page: Some(angzarr_embedded::v1::CommandPage {
                body: Some(angzarr_embedded::v1::command_page::Body::Command(
                    angzarr_embedded::v1::Command {
                        payload: Some(angzarr_embedded::Payload::to_any(
                            &sessions::NameClaimCommand::from(ClaimName {
                                project_id: HERE.to_string(),
                                harp: OWL.to_string(),
                                session: "other-session".to_string(),
                            }),
                        )),
                    },
                )),
                ..angzarr_embedded::v1::CommandPage::default()
            }),
            handling: None,
        };
        let sampled = Sampled {
            now: Timestamp {
                seconds: 1_700_000_000,
                nanos: 0,
            },
            command_id: angzarr_embedded::v1::Uuid::from_bytes([0xcc; 16]),
        };
        ready(
            Kernel::new(&registry)
                .on_retryable(OnRetryable {
                    max_retries: 0,
                    ..OnRetryable::default()
                })
                .handle::<NameClaimAggregate, _>(store, book, &sampled),
        )
        .unwrap();
        let _ = cover(9);
    }
    let report = ready(imp.import(ImportRequest::default())).unwrap();
    assert!(report.imported.is_empty());
    assert_eq!(report.left.len(), 1);
    assert_eq!(report.left[0].harp, OWL);
    assert_eq!(
        report.left[0].why,
        Why::NameTaken {
            harp: OWL.to_string()
        }
    );
}

#[test]
fn a_go_session_of_a_directory_no_project_holds_is_left_named() {
    let elsewhere = SCHEMA1.replace("/work/proj", "/work/elsewhere");
    let mut imp = importer(vec![go(OWL, &elsewhere, false), go(MOTH, SCHEMA2, false)]);
    let report = ready(imp.import(ImportRequest::default())).unwrap();
    assert_eq!(report.imported.len(), 1);
    assert_eq!(report.imported[0].harp, MOTH, "the rest are still imported");
    assert_eq!(
        report.left,
        [crate::Left {
            harp: OWL.to_string(),
            why: Why::NoProject {
                project_dir: "/work/elsewhere".to_string()
            }
        }]
    );
}

#[test]
fn a_sidecar_this_build_cannot_read_is_left_and_the_others_still_import() {
    let newer = SCHEMA2.replace("schema_version: 2", "schema_version: 9");
    let mut imp = importer(vec![go(OWL, SCHEMA1, false), go(MOTH, &newer, false)]);
    let report = ready(imp.import(ImportRequest::default())).unwrap();
    assert_eq!(report.imported.len(), 1);
    assert!(matches!(
        report.left.as_slice(),
        [crate::Left {
            why: Why::Sidecar(crate::GoSidecarError::TooNew {
                found: 9,
                current: 3
            }),
            ..
        }]
    ));
}

#[test]
fn a_sidecar_that_could_not_be_read_from_disk_is_left() {
    let broken = GoSession {
        harp: OWL.to_string(),
        sidecar: Err(angzarr_diagnostic::Report::new("session-files-io")),
        keep_marker: false,
    };
    let mut imp = importer(vec![broken, go(MOTH, SCHEMA2, false)]);
    let report = ready(imp.import(ImportRequest::default())).unwrap();
    assert_eq!(report.imported.len(), 1);
    assert!(matches!(
        report.left.as_slice(),
        [crate::Left {
            why: Why::Unreadable(_),
            ..
        }]
    ));
}

// plan 9: "`KeptChanged` from `keep`" (the hand-placed marker file too).
#[test]
fn a_keep_marker_file_marks_the_session_kept() {
    let mut imp = importer(vec![go(OWL, SCHEMA1, true)]);
    ready(imp.import(ImportRequest::default())).unwrap();
    let views = listing(imp);
    assert!(views[0].record.kept);
}

fn listed(sidecar: &str, keep_marker: bool) -> crate::View {
    let mut imp = importer(vec![go(OWL, sidecar, keep_marker)]);
    ready(imp.import(ImportRequest::default())).unwrap();
    listing(imp).remove(0)
}

// Go's origin: an internal one-shot stays one.
#[test]
fn an_origin_of_oneshot_is_imported_as_one() {
    let oneshot = SCHEMA1.replace(
        "backend: claude-code",
        "backend: claude-code\norigin: oneshot",
    );
    assert_eq!(listed(&oneshot, false).record.origin, Some(Origin::Oneshot));
    assert_eq!(listed(SCHEMA1, false).record.origin, Some(Origin::Session));
}

// The engine's binding is recorded when Go recorded an id or a transcript, and
// not when it recorded neither.
#[test]
fn a_binding_is_imported_if_go_recorded_any_part_of_it() {
    let id_only = SCHEMA1.replace(
        "transcript_path: /home/u/.claude/projects/proj/9a1c0d1e.jsonl\n",
        "",
    );
    let v = listed(&id_only, false).record;
    assert_eq!(
        (v.native_id.as_str(), v.transcript_path.as_str()),
        ("9a1c0d1e-1111-4111-8111-000000000001", "")
    );
    let path_only = SCHEMA1.replace("session_id: 9a1c0d1e-1111-4111-8111-000000000001\n", "");
    let v = listed(&path_only, false).record;
    assert_eq!(v.native_id, "");
    assert_eq!(
        v.transcript_path,
        "/home/u/.claude/projects/proj/9a1c0d1e.jsonl"
    );
    let neither = path_only.replace(
        "transcript_path: /home/u/.claude/projects/proj/9a1c0d1e.jsonl\n",
        "",
    );
    let v = listed(&neither, false).record;
    assert_eq!((v.native_id.as_str(), v.transcript_path.as_str()), ("", ""));
}

// Neither Go's keep mark nor a marker file: not kept.
#[test]
fn a_session_with_no_keep_mark_is_not_kept() {
    assert!(!listed(SCHEMA1, false).record.kept);
    assert!(listed(SCHEMA1, true).record.kept);
    assert!(listed(SCHEMA2, false).record.kept);
}
