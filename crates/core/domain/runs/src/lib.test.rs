//! The rules that need no stream: the run's root and the catalog.
//! RED (S5): `run_root` is a stub.

use crate::{diagnostic_samples, run_root, EN_US, RUN_DOMAIN};

const SESSION: &str = "5f0c7a52-1111-5111-8111-000000000001";
const OTHER: &str = "5f0c7a52-2222-5222-8222-000000000002";

// NEW-DESIGN (plan 3.3, D5 Q4: "root = UUIDv5(session, incarnation)"). Pins
// the formula S4 already fixed in `ctxloom_wiring::run_id`: the incarnation's
// decimal text, in the session's own UUID namespace (the ctxloom-wiring
// `core_link` test holds the two to each other).
#[test]
fn the_run_root_is_the_uuid_v5_of_the_incarnation_in_the_sessions_namespace() {
    let session = uuid::Uuid::parse_str(SESSION).unwrap();
    let expected = uuid::Uuid::new_v5(&session, b"1");
    assert_eq!(run_root(SESSION, 1).value, expected.as_bytes().to_vec());
}

// NEW-DESIGN: one run per incarnation, so a session's incarnations and two
// sessions' incarnations are all distinct streams, and the same input is the
// same stream (a retry rebuilds the same root).
#[test]
fn incarnations_and_sessions_have_distinct_run_roots_and_the_same_input_the_same_root() {
    assert_ne!(run_root(SESSION, 1), run_root(SESSION, 2));
    assert_ne!(run_root(SESSION, 1), run_root(OTHER, 1));
    assert_eq!(run_root(SESSION, 2), run_root(SESSION, 2));
}

#[test]
fn the_domain_name_is_the_one_the_plan_gives() {
    assert_eq!(RUN_DOMAIN, "runs");
}

// The crate's catalog test, as sessions' (the binary's holds the same for the
// whole graph).
#[test]
fn every_diagnostic_has_a_message_that_uses_exactly_its_args() {
    let (name, catalog) = EN_US;
    assert_eq!(name, "runs");
    for sample in diagnostic_samples() {
        let code = sample.code();
        assert!(code.starts_with("runs-"), "{code} is prefixed");
        let line = catalog
            .lines()
            .find(|l| l.starts_with(&format!("{code} =")))
            .unwrap_or_else(|| panic!("{code} has no message"));
        let mut args: Vec<&str> = sample.args().into_iter().map(|(n, _)| n).collect();
        args.sort_unstable();
        let mut used: Vec<&str> = line
            .split('$')
            .skip(1)
            .map(|rest| {
                rest.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
                    .next()
                    .unwrap()
            })
            .collect();
        used.sort_unstable();
        used.dedup();
        assert_eq!(args, used, "{code}");
    }
}

// NEW-DESIGN: a rejection is its code and args, and the catalog test has
// samples to check.
#[test]
fn rejections_display_their_code_and_the_samples_are_not_empty() {
    use crate::RunRejection;
    assert_eq!(
        RunRejection::TurnInFlight { turn: 4 }.to_string(),
        "runs-turn-in-flight turn=4"
    );
    assert_eq!(diagnostic_samples().len(), 7);
}
