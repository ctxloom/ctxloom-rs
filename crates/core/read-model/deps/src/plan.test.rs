use super::*;

const A: &str = "ctxloom+git://h/r//bundles/a";
const B: &str = "ctxloom+git://h/r//bundles/b";
const C: &str = "ctxloom+file:///srv/c.git//bundles/c";
const H1: &str = "1111111111111111111111111111111111111111";
const H2: &str = "2222222222222222222222222222222222222222";

fn pin(identity: &str, hash: &str) -> Pin {
    Pin {
        identity: identity.to_string(),
        hash: hash.to_string(),
        url: "https://h/r".to_string(),
        constraint: String::new(),
        version: String::new(),
        kind: Some(Kind::Branch),
    }
}

fn lock(entries: &[(&str, Entry)]) -> Lock {
    Lock {
        entries: entries
            .iter()
            .map(|(k, e)| (k.to_string(), e.clone()))
            .collect(),
    }
}

// pinnedEntry: sha, url, the expression asked, the tag chosen, the kind.
#[test]
fn test_a_pin_resolves_to_a_lock_entry() {
    let p = Pin {
        identity: A.to_string(),
        hash: H1.to_string(),
        url: "https://h/r".to_string(),
        constraint: "^1".to_string(),
        version: "v1.2.0".to_string(),
        kind: Some(Kind::Version),
    };
    assert_eq!(
        p.entry(),
        Entry {
            sha: H1.to_string(),
            url: "https://h/r".to_string(),
            requested_version: "^1".to_string(),
            version: "v1.2.0".to_string(),
            kind: Some(Kind::Version),
            held: false,
        }
    );
}

// depgraph.go: the first pin of an identity stays; a second commit for the
// same identity is a conflict; what could not be read is remembered.
#[test]
fn test_a_closure_keeps_the_first_pin_and_notes_a_second_commit() {
    let mut closure = Closure::new();
    closure.record(pin(B, H2));
    closure.record(pin(A, H1));
    closure.record(pin(A, H1));
    assert!(closure.conflicts().is_empty());
    closure.record(pin(A, H2));
    let pins = closure.pins();
    assert_eq!(
        pins.iter().map(|p| p.identity.as_str()).collect::<Vec<_>>(),
        vec![A, B]
    );
    assert_eq!(pins[0].hash, H1, "the first pin stays");
    assert_eq!(
        closure.conflicts(),
        vec![Conflict {
            item: A.to_string(),
            hashes: vec![H1.to_string(), H2.to_string()]
        }]
    );
}

#[test]
fn test_what_could_not_be_read_is_remembered_once_and_sorted() {
    let mut closure = Closure::new();
    assert!(closure.unexpanded().is_empty());
    closure.mark_unexpanded("zeta");
    closure.mark_unexpanded("alpha");
    closure.mark_unexpanded("zeta");
    assert_eq!(closure.unexpanded(), vec!["alpha", "zeta"]);
}

// depgraph.go ConflictError
#[test]
fn test_conflicts_are_one_message_with_short_commits() {
    assert_eq!(conflict_text(&[]), "");
    let text = conflict_text(&[
        Conflict {
            item: A.to_string(),
            hashes: vec![H1.to_string(), H2.to_string()],
        },
        Conflict {
            item: B.to_string(),
            hashes: vec!["abc".to_string(), H2.to_string()],
        },
    ]);
    assert_eq!(
        text,
        format!(
            "dependency hash conflict \u{2014} the same item is referenced at differing commits:\n  {A} @ {{1111111, 2222222}}\n  {B} @ {{abc, 2222222}}"
        )
    );
}

// lockfile.go relockEntry: the hold survives a rebuild; the requested
// expression stays while the commit does.
#[test]
fn test_a_rebuild_keeps_the_hold_and_the_expression_of_an_unmoved_pin() {
    let mut held = pin(A, H1).entry();
    held.held = true;
    held.requested_version = "main".to_string();
    let previous = lock(&[(A, held), (B, pin(B, H1).entry())]);
    let mut declared = pin(A, H1);
    declared.constraint = "release".to_string();
    let next = relock(&previous, &[declared, pin(B, H2)]);
    assert!(next.entries[A].held);
    assert_eq!(
        next.entries[A].requested_version, "main",
        "same commit: only upgrade applies a new constraint"
    );
    assert_eq!(next.entries[B].sha, H2);
    assert!(!next.entries[B].held);
}

#[test]
fn test_a_rebuild_takes_the_new_expression_when_the_commit_is_new() {
    let previous = lock(&[(A, pin(A, H1).entry())]);
    let mut moved = pin(A, H2);
    moved.constraint = "release".to_string();
    let next = relock(&previous, &[moved]);
    assert_eq!(next.entries[A].requested_version, "release");
    assert_eq!(next.entries[A].sha, H2);
    // A first pin is as it resolved.
    assert_eq!(
        relock(&Lock::default(), &[pin(C, H1)]).entries[C],
        pin(C, H1).entry()
    );
    // What the walk did not reach is not in a rebuild by itself.
    assert!(!relock(&previous, &[pin(C, H1)]).entries.contains_key(A));
}

#[test]
fn test_an_unreached_entry_is_carried_and_counted() {
    let active = lock(&[(A, pin(A, H1).entry()), (B, pin(B, H1).entry())]);
    let mut next = lock(&[(B, pin(B, H2).entry())]);
    assert_eq!(preserve_unreached(&active, &mut next), 1);
    assert_eq!(next.entries[A].sha, H1);
    assert_eq!(
        next.entries[B].sha, H2,
        "an entry the walk reached is not overwritten"
    );
    assert_eq!(preserve_unreached(&active, &mut next), 0);
}

#[test]
fn test_the_dropped_identities_are_those_after_no_longer_holds() {
    let before = lock(&[
        (B, pin(B, H1).entry()),
        (A, pin(A, H1).entry()),
        (C, pin(C, H1).entry()),
    ]);
    let after = lock(&[(A, pin(A, H2).entry())]);
    assert_eq!(dropped(&before, &after), vec![C, B], "sorted");
    assert!(dropped(&after, &before).is_empty());
}

// upgrade.go decide: a held entry carries forward unchanged; anything else
// lands at the proposed commit.
#[test]
fn test_an_upgrade_moves_what_moved_and_keeps_a_held_entry_frozen() {
    let mut held = pin(B, H1).entry();
    held.held = true;
    let active = lock(&[
        (A, pin(A, H1).entry()),
        (B, held.clone()),
        (C, pin(C, H1).entry()),
    ]);
    let proposed = [pin(A, H2), pin(B, H2), pin(C, H1)];
    let got = upgrade(&active, &proposed, 0, false);
    assert_eq!(got.next.entries[A].sha, H2);
    assert_eq!(got.next.entries[B], held, "a held entry never advances");
    assert_eq!(got.next.entries[C].sha, H1);
    assert_eq!(
        got.moved
            .iter()
            .map(|p| p.identity.as_str())
            .collect::<Vec<_>>(),
        vec![A]
    );
    assert!(got.removed.is_empty());
    assert!(!got.incomplete);
    assert!(!got.nothing_declared);
}

#[test]
fn test_a_first_pin_is_a_move_and_a_dropped_one_is_named() {
    let active = lock(&[(A, pin(A, H1).entry())]);
    let got = upgrade(&active, &[pin(B, H1)], 0, false);
    assert_eq!(got.moved.len(), 1);
    assert_eq!(got.moved[0].identity, B);
    assert_eq!(got.removed, vec![A]);
    assert!(got.next.entries.contains_key(B));
    assert!(!got.next.entries.contains_key(A));
}

// upgrade.go preserveUnreachedEntries: an incomplete closure never erases
// healthy entries.
#[test]
fn test_an_incomplete_closure_carries_the_entries_it_could_not_reach() {
    let active = lock(&[(A, pin(A, H1).entry()), (B, pin(B, H1).entry())]);
    let got = upgrade(&active, &[pin(B, H2)], 1, false);
    assert!(got.incomplete);
    assert_eq!(got.next.entries[A].sha, H1);
    assert_eq!(got.next.entries[B].sha, H2);
    assert!(got.removed.is_empty(), "{:?}", got.removed);
    // A fetch that failed is incomplete, though nothing is carried for it.
    let stale = upgrade(&active, &[pin(A, H1), pin(B, H1)], 0, true);
    assert!(stale.incomplete);
    assert!(stale.moved.is_empty());
}

// upgrade.go checkUpgradedLock: nothing declared and nothing locked is not
// "everything is current"; an incomplete round is not "nothing declared".
#[test]
fn test_nothing_declared_is_told_from_everything_current() {
    let none = upgrade(&Lock::default(), &[], 0, false);
    assert!(none.nothing_declared);
    assert!(none.moved.is_empty());
    let unreached = upgrade(&Lock::default(), &[], 1, false);
    assert!(!unreached.nothing_declared);
    assert!(unreached.incomplete);
    let current = upgrade(&lock(&[(A, pin(A, H1).entry())]), &[pin(A, H1)], 0, false);
    assert!(!current.nothing_declared);
}

// sync.go constraintChanges
#[test]
fn test_a_changed_constraint_is_named_for_every_unheld_pin_it_no_longer_matches() {
    let mut a = pin(A, H1).entry();
    a.requested_version = String::new();
    let mut held = pin(B, H1).entry();
    held.held = true;
    let mut c = pin(C, H1).entry();
    c.requested_version = "main".to_string();
    let lock = lock(&[(A, a), (B, held), (C, c)]);
    let declared = [
        (A.to_string(), "main".to_string()),
        (A.to_string(), "main".to_string()),
        (B.to_string(), "main".to_string()),
        (C.to_string(), "main".to_string()),
        (
            "ctxloom+git://h/r//bundles/unlocked".to_string(),
            "main".to_string(),
        ),
    ];
    assert_eq!(
        constraint_changes(&lock, &declared),
        vec![ConstraintChange {
            identity: A.to_string(),
            pinned: String::new(),
            declared: "main".to_string(),
            sha: H1.to_string(),
        }]
    );
}

#[test]
fn test_a_declared_commit_equal_to_the_pin_is_already_satisfied() {
    let lock = lock(&[(A, pin(A, H1).entry())]);
    assert!(constraint_changes(&lock, &[(A.to_string(), H1.to_string())]).is_empty());
    assert_eq!(
        constraint_changes(&lock, &[(A.to_string(), H2.to_string())]).len(),
        1
    );
}

// deps_reconcile.go: the evidence rule.
fn reconciled(
    installed: &[&str],
    unreachable: &[&str],
    gone: &[&str],
    failing: &[&str],
) -> (Reconcile, Vec<String>, Vec<String>) {
    let installed: Vec<String> = installed.iter().map(|s| s.to_string()).collect();
    let mut reached = Vec::new();
    let mut asked = Vec::new();
    let result = reconcile(
        &installed,
        &mut |url| {
            reached.push(url.to_string());
            if unreachable.contains(&url) {
                Err(format!("cannot read {url}"))
            } else {
                Ok(())
            }
        },
        &mut |reference| {
            asked.push(reference.to_string());
            if gone.contains(&reference) {
                Probe::Gone
            } else if failing.contains(&reference) {
                Probe::Failed("rate limited".to_string())
            } else {
                Probe::Present
            }
        },
    );
    (result, reached, asked)
}

#[test]
fn test_what_a_reachable_repository_no_longer_serves_is_gone() {
    let (got, reached, asked) = reconciled(&[A, B], &[], &[A], &[]);
    assert_eq!(got.gone, vec![A]);
    assert!(got.unreachable.is_empty());
    assert_eq!(
        reached,
        vec!["https://h/r"],
        "reachability is proved once per repository"
    );
    assert_eq!(asked, vec![A, B]);
}

#[test]
fn test_a_repository_that_cannot_be_read_loses_nothing_and_is_never_asked_about_its_items() {
    let (got, reached, asked) = reconciled(&[A, B, C], &["https://h/r"], &[A, B], &[]);
    assert!(got.gone.is_empty(), "{:?}", got.gone);
    assert_eq!(
        got.unreachable,
        vec![Unchecked {
            url: "https://h/r".to_string(),
            refs: vec![A.to_string(), B.to_string()],
            reason: "cannot read https://h/r".to_string(),
        }]
    );
    assert_eq!(
        asked,
        vec![C],
        "the unreadable repository's items are not probed"
    );
    assert_eq!(reached.len(), 2);
}

#[test]
fn test_a_failed_lookup_in_a_readable_repository_proves_nothing() {
    let (got, _, _) = reconciled(&[A, B], &[], &[], &[A]);
    assert!(got.gone.is_empty());
    assert_eq!(
        got.unreachable,
        vec![Unchecked {
            url: "https://h/r".to_string(),
            refs: vec![A.to_string()],
            reason: "rate limited".to_string(),
        }]
    );
}

#[test]
fn test_an_installed_reference_that_does_not_parse_is_reported_never_removed() {
    let (got, reached, asked) = reconciled(&["ctxloom+local:x", "garbage"], &[], &[], &[]);
    assert!(got.gone.is_empty());
    assert!(reached.is_empty() && asked.is_empty());
    assert_eq!(got.unreachable.len(), 1);
    assert_eq!(got.unreachable[0].url, "");
    assert_eq!(got.unreachable[0].refs, vec!["ctxloom+local:x", "garbage"]);
    assert_eq!(
        got.unreachable[0].reason,
        "the reference could not be parsed, so no repository could be checked for it"
    );
}

#[test]
fn test_repositories_are_checked_in_url_order() {
    let (_, reached, _) = reconciled(&[C, A], &[], &[], &[]);
    assert_eq!(reached, vec!["file:///srv/c.git", "https://h/r"]);
}
