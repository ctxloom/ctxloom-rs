//! A tree through the kernel (`Harness`). RED (S5): the aggregate is a stub.
//! Every test here is NEW-DESIGN (plan 3.2; Go's tree is the coordinator's
//! root and has no aggregate to port).

use angzarr_embedded::testing::{Harness, Samples};
use angzarr_embedded::v1::{Cover, Uuid};
use angzarr_embedded::{Failure, Registry};

use crate::{
    FoundTree, OccupyTree, TreeAggregate, TreeCommand, TreeEvent, TreeFounded, TreeOccupied,
    TreeState, TREE_DOMAIN,
};

fn tree() -> Harness<TreeAggregate> {
    let mut registry = Registry::new();
    registry
        .aggregate::<TreeAggregate>(TreeAggregate::default)
        .unwrap();
    let cover = Cover {
        domain: TREE_DOMAIN.to_string(),
        root: Some(Uuid::from_bytes([9; 16])),
        edition: None,
    };
    Harness::new(registry, cover).with_samples(Samples::starting_at(100))
}

fn found() -> FoundTree {
    FoundTree {
        owner_session: "s-owner".to_string(),
        ephemeral: false,
        concurrency: 4,
        depth: 2,
    }
}

fn run(
    harness: &mut Harness<TreeAggregate>,
    command: impl Into<TreeCommand>,
) -> Result<Vec<TreeEvent>, String> {
    match harness.when(command).result() {
        Ok(events) => Ok(events),
        Err(Failure::Rejected(rejected)) => {
            Err(angzarr_diagnostic::Diagnostic::code(&rejected).to_string())
        }
        Err(other) => panic!("not a rejection: {other:?}"),
    }
}

// NEW-DESIGN (plan 3.2: "founding and the limits (delegation.{concurrency,
// depth} snapshotted at founding); ephemeral").
#[test]
fn founding_snapshots_the_limits_and_the_owner_occupies() {
    let mut harness = tree();
    let events = run(&mut harness, found()).unwrap();
    assert_eq!(
        events,
        vec![TreeEvent::TreeFounded(TreeFounded {
            owner_session: "s-owner".to_string(),
            ephemeral: false,
            concurrency: 4,
            depth: 2,
        })]
    );
    let state: TreeState = harness.state().state().clone();
    assert_eq!((state.concurrency, state.depth), (4, 2));
    assert_eq!(state.occupant, "s-owner");
}

// NEW-DESIGN: a tree is founded once.
#[test]
fn a_second_founding_is_refused() {
    let mut harness = tree();
    run(&mut harness, found()).unwrap();
    assert_eq!(
        run(&mut harness, found()).unwrap_err(),
        "trees-already-founded"
    );
}

// NEW-DESIGN (plan 3.2, Q2: whoever occupies the tree after `--session` can
// address and stop the children and receives their mail). Occupying again as
// the occupant records nothing.
#[test]
fn occupying_changes_the_occupant_and_again_records_nothing() {
    let mut harness = tree();
    run(&mut harness, found()).unwrap();
    let occupy = OccupyTree {
        session: "s-next".to_string(),
    };
    assert_eq!(
        run(&mut harness, occupy.clone()).unwrap(),
        vec![TreeEvent::TreeOccupied(TreeOccupied {
            session: "s-next".to_string()
        })]
    );
    assert!(run(&mut harness, occupy).unwrap().is_empty());
    assert_eq!(harness.state().state().occupant, "s-next");
}

// NEW-DESIGN: occupying a tree never founded is refused.
#[test]
fn occupying_an_unfounded_tree_is_refused() {
    let mut harness = tree();
    let occupy = OccupyTree {
        session: "s".to_string(),
    };
    assert_eq!(run(&mut harness, occupy).unwrap_err(), "trees-no-tree");
}
