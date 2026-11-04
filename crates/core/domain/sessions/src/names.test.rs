//! The session-name claim (D5 Q1): the OWNER'S RULING, not Go's. Committed
//! red and WITHHELD from implementation until the owner reviews them (the
//! brief of slice S4): `names.rs` holds only stubs.
//!
//! One claim stream per (project, harp text). A session takes its name
//! first, on a mint or a rename; the name is refused while another session
//! holds it; a rename moves the claim (claim the new name, release the old
//! one, plan 3.1); a forgotten session's name stays claimed.

use std::collections::BTreeMap;

use angzarr_diagnostic::{Arg, Diagnostic};
use angzarr_embedded::testing::{Harness, Samples};
use angzarr_embedded::v1::{Cover, Uuid};
use angzarr_embedded::{Failure, Registry};

use crate::{
    ClaimName, Claimed, NameClaimAggregate, NameClaimCommand, NameClaimEvent, NameClaimState,
    ReleaseName, Released, SessionRejection, NAMES_DOMAIN,
};

const HARP: &str = "amber-swift-owl";
const FIRST: &str = "5f0c7a52-1111-5111-8111-000000000001";
const SECOND: &str = "5f0c7a52-2222-5222-8222-000000000002";

type Said = (String, BTreeMap<String, String>);

fn said(diagnostic: &dyn Diagnostic) -> Said {
    let args = diagnostic
        .args()
        .into_iter()
        .map(|(name, arg)| {
            let text = match arg {
                Arg::Int(n) => n.to_string(),
                Arg::Uint(n) => n.to_string(),
                Arg::Str(s) => s,
                Arg::Nested(cause) => cause.code().to_string(),
            };
            (name.to_string(), text)
        })
        .collect();
    (diagnostic.code().to_string(), args)
}

/// One name's claim stream over the kernel's in-memory store.
struct Claim {
    harness: Harness<NameClaimAggregate>,
}

impl Claim {
    fn new() -> Claim {
        let mut registry = Registry::new();
        registry
            .aggregate::<NameClaimAggregate>(NameClaimAggregate::default)
            .unwrap();
        let cover = Cover {
            domain: NAMES_DOMAIN.to_string(),
            root: Some(Uuid::from_bytes([9; 16])),
            edition: None,
        };
        Claim {
            harness: Harness::new(registry, cover).with_samples(Samples::starting_at(100)),
        }
    }

    fn run(&mut self, command: impl Into<NameClaimCommand>) -> Result<Vec<NameClaimEvent>, Said> {
        match self.harness.when(command).result() {
            Ok(events) => Ok(events),
            Err(Failure::Rejected(rejected)) => Err(said(&rejected)),
            Err(other) => panic!("not a rejection: {other:?}"),
        }
    }

    fn state(&self) -> NameClaimState {
        self.harness.state().state().clone()
    }

    fn pages(&self) -> usize {
        self.harness.pages().len()
    }
}

fn claim(session: &str) -> ClaimName {
    ClaimName {
        project_id: "calm-otter".to_string(),
        harp: HARP.to_string(),
        session: session.to_string(),
    }
}

fn release(session: &str) -> ReleaseName {
    ReleaseName {
        session: session.to_string(),
    }
}

// D5 Q1: a free name is claimed by the first session to ask.
#[test]
fn a_free_name_is_claimed_by_the_first_session_to_ask() {
    let mut names = Claim::new();
    let events = names.run(claim(FIRST)).unwrap();
    assert_eq!(
        events,
        [NameClaimEvent::Claimed(Claimed {
            project_id: "calm-otter".to_string(),
            harp: HARP.to_string(),
            session: FIRST.to_string(),
        })]
    );
    let state = names.state();
    assert_eq!(state.session, FIRST);
    assert_eq!(state.harp, HARP);
    assert!(state.claimed);
}

// D5 Q1, plan S4 "a name already claimed is refused (`session-names-taken`)".
// The code is the crate's name then `names-taken` (`sessions-names-taken`):
// see the report's questions.
#[test]
fn a_name_another_session_holds_is_refused_names_taken_and_nothing_is_recorded() {
    let mut names = Claim::new();
    names.run(claim(FIRST)).unwrap();
    let before = names.pages();
    let refusal = names.run(claim(SECOND)).unwrap_err();
    assert_eq!(
        refusal,
        said(&SessionRejection::NamesTaken {
            harp: HARP.to_string()
        })
    );
    assert_eq!(refusal.0, "sessions-names-taken");
    assert_eq!(names.pages(), before);
    assert_eq!(names.state().session, FIRST, "the holder keeps it");
}

// The holder asking again (a retry of the mint) changes nothing.
#[test]
fn the_holder_asking_again_records_nothing() {
    let mut names = Claim::new();
    names.run(claim(FIRST)).unwrap();
    let before = names.pages();
    assert!(names.run(claim(FIRST)).unwrap().is_empty());
    assert_eq!(names.pages(), before);
}

// D5 Q1, "rename ... frees the old name": a release by the holder frees it,
// and another session may then claim it.
#[test]
fn a_released_name_is_free_for_another_session() {
    let mut names = Claim::new();
    names.run(claim(FIRST)).unwrap();
    let events = names.run(release(FIRST)).unwrap();
    assert_eq!(events, [NameClaimEvent::Released(Released {})]);
    assert_eq!(names.state().session, "");
    names.run(claim(SECOND)).unwrap();
    assert_eq!(names.state().session, SECOND);
}

// Only the holder gives a name back; a rename of another session must not free
// a name it never held.
#[test]
fn a_release_by_a_session_that_does_not_hold_the_name_is_refused() {
    let mut names = Claim::new();
    names.run(claim(FIRST)).unwrap();
    assert_eq!(
        names.run(release(SECOND)).unwrap_err(),
        said(&SessionRejection::NotHolder {
            harp: HARP.to_string()
        })
    );
    assert_eq!(names.state().session, FIRST);
}

#[test]
fn a_release_of_a_name_nobody_holds_is_refused() {
    let mut names = Claim::new();
    assert_eq!(
        names.run(release(FIRST)).unwrap_err(),
        said(&SessionRejection::NoClaim)
    );
    names.run(claim(FIRST)).unwrap();
    names.run(release(FIRST)).unwrap();
    assert_eq!(
        names.run(release(FIRST)).unwrap_err(),
        said(&SessionRejection::NoClaim),
        "a name once released is held by nobody"
    );
}

#[test]
fn the_claim_state_round_trips_through_its_wire_form() {
    let mut names = Claim::new();
    names.run(claim(FIRST)).unwrap();
    let state = names.state();
    let wire = sessions_proto::v1::NameClaimState::from(state.clone());
    assert_eq!(NameClaimState::try_from(wire).unwrap(), state);
}

// D5 Q1: the claim's root is derived from the project and the harp text, so a
// name is unique in a project because a stream is.
#[test]
fn the_claim_stream_is_derived_from_the_project_and_the_name() {
    let a = crate::claim_root("calm-otter", HARP);
    assert_eq!(
        a,
        crate::claim_root("calm-otter", HARP),
        "a function of its inputs"
    );
    assert_ne!(
        a,
        crate::claim_root("calm-otter", "brisk-copper-moth"),
        "per name"
    );
    assert_ne!(a, crate::claim_root("brisk-tall-oak", HARP), "per project");
    // UUIDv5: version nibble 5, RFC 4122 variant.
    assert_eq!(a.value[6] >> 4, 5);
    assert_eq!(a.value[8] >> 6, 0b10);
}
