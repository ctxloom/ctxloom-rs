//! The isolation decision table (S11). NEW-DESIGN tests: the table's shape
//! is this port's proposal, so they are written first, committed red, and
//! not implemented until reviewed. The rules they pin trace to Go 0.7's
//! chain (`isolation_degrade_guard_test.go`, `chain_adjacency_test.go`),
//! j002200_isolation.feature's refusal rows, and plan 5.5.

use super::{decide, Demand, Facts, Finding, Ownership, Plan, Runtime, Warning, Workspace};

const WORKSPACES: [Workspace; 2] = [Workspace::None, Workspace::Worktree];
const RUNTIMES: [Runtime; 3] = [
    Runtime::Host,
    Runtime::Container(Ownership::Rootless),
    Runtime::Container(Ownership::Rootful),
];

fn everything() -> Facts {
    Facts {
        git_repository: true,
        offers: vec![Ownership::Rootless, Ownership::Rootful],
        image_available: true,
    }
}

fn demand(runtime: Runtime, workspace: Workspace) -> Demand {
    Demand { runtime, workspace }
}

// All six combinations, with everything on offer, keep both axes as asked, in
// both modes (j002200: "the same run lands in the workspace its axis dictates").
#[test]
fn six_combinations_with_everything_on_offer_are_granted_as_asked() {
    for degraded in [false, true] {
        for runtime in RUNTIMES {
            for workspace in WORKSPACES {
                let plan = decide(demand(runtime, workspace), &everything(), degraded);
                assert_eq!(
                    plan,
                    Ok(Plan {
                        runtime,
                        workspace,
                        warnings: Vec::new()
                    }),
                    "{runtime:?} x {workspace:?} degraded={degraded}"
                );
            }
        }
    }
}

// j002200 "Asking for a worktree outside git runs in the live project
// directory instead of refusing": worktree -> none only warns.
#[test]
fn a_worktree_outside_git_falls_to_the_live_directory_with_a_warning() {
    let facts = Facts {
        git_repository: false,
        ..everything()
    };
    for degraded in [false, true] {
        for runtime in RUNTIMES {
            let plan = decide(demand(runtime, Workspace::Worktree), &facts, degraded)
                .expect("a lost worktree is never fatal");
            assert_eq!(plan.runtime, runtime, "only the workspace axis degrades");
            assert_eq!(plan.workspace, Workspace::None);
            assert_eq!(plan.warnings, vec![Warning::WorktreeUnavailable]);
        }
    }
}

// j002200 "Requesting a container with no runtime REFUSES, and --degraded
// does not rescue it": exit 3, never a host plan.
#[test]
fn a_container_with_no_runtime_is_fatal_in_both_modes_and_never_lands_on_the_host() {
    let facts = Facts {
        offers: Vec::new(),
        ..everything()
    };
    for degraded in [false, true] {
        for ownership in [Ownership::Rootless, Ownership::Rootful] {
            for workspace in WORKSPACES {
                let found = decide(
                    demand(Runtime::Container(ownership), workspace),
                    &facts,
                    degraded,
                );
                assert_eq!(
                    found,
                    Err(Finding::NoRuntime {
                        demanded: ownership
                    }),
                    "{ownership:?} x {workspace:?} degraded={degraded}"
                );
                assert_eq!(found.unwrap_err().exit_code(), 3);
            }
        }
    }
}

// Plan 5.5: a mismatch of ownership is fatal in both modes and is NEVER a
// substitution of the other ownership (nor of the host).
#[test]
fn an_ownership_mismatch_is_fatal_in_both_modes_and_never_a_substitution() {
    let cases = [
        (Ownership::Rootless, Ownership::Rootful),
        (Ownership::Rootful, Ownership::Rootless),
    ];
    for (demanded, offered) in cases {
        let facts = Facts {
            offers: vec![offered],
            ..everything()
        };
        for degraded in [false, true] {
            for workspace in WORKSPACES {
                let found = decide(
                    demand(Runtime::Container(demanded), workspace),
                    &facts,
                    degraded,
                );
                assert_eq!(
                    found,
                    Err(Finding::OwnershipMismatch {
                        demanded,
                        offered: vec![offered]
                    }),
                    "{demanded:?} offered {offered:?} degraded={degraded}"
                );
            }
        }
    }
}

// j002200 "A container that was selected and cannot start REFUSES, and
// --degraded does not rescue it" (including the containerized worktree).
#[test]
fn a_container_whose_image_cannot_be_had_is_fatal_in_both_modes() {
    let facts = Facts {
        image_available: false,
        ..everything()
    };
    for degraded in [false, true] {
        for workspace in WORKSPACES {
            assert_eq!(
                decide(
                    demand(Runtime::Container(Ownership::Rootless), workspace),
                    &facts,
                    degraded
                ),
                Err(Finding::ImageUnavailable {
                    demanded: Ownership::Rootless
                })
            );
        }
    }
}

// The host needs neither a runtime nor an image: container facts never
// change a host decision.
#[test]
fn the_host_ignores_what_the_container_runtime_offers() {
    let facts = Facts {
        offers: Vec::new(),
        image_available: false,
        ..everything()
    };
    for degraded in [false, true] {
        for workspace in WORKSPACES {
            let plan = decide(demand(Runtime::Host, workspace), &facts, degraded).unwrap();
            assert_eq!(plan.runtime, Runtime::Host);
            assert_eq!(plan.workspace, workspace);
        }
    }
}

// A containerized worktree outside git keeps the container: the degrade drops
// the workspace axis, not the boundary.
#[test]
fn a_containerized_worktree_outside_git_keeps_its_container() {
    let facts = Facts {
        git_repository: false,
        ..everything()
    };
    let plan = decide(
        demand(Runtime::Container(Ownership::Rootful), Workspace::Worktree),
        &facts,
        true,
    )
    .unwrap();
    assert_eq!(plan.runtime, Runtime::Container(Ownership::Rootful));
    assert_eq!(plan.workspace, Workspace::None);
}

// The flag makes no difference to any decision: the two modes agree on every
// combination of demand and facts.
#[test]
fn degraded_never_changes_a_decision() {
    let machines = [
        everything(),
        Facts {
            git_repository: false,
            ..everything()
        },
        Facts {
            offers: Vec::new(),
            ..everything()
        },
        Facts {
            offers: vec![Ownership::Rootful],
            ..everything()
        },
        Facts {
            image_available: false,
            ..everything()
        },
    ];
    for facts in &machines {
        for runtime in RUNTIMES {
            for workspace in WORKSPACES {
                assert_eq!(
                    decide(demand(runtime, workspace), facts, false),
                    decide(demand(runtime, workspace), facts, true),
                    "{runtime:?} x {workspace:?} on {facts:?}"
                );
            }
        }
    }
}
