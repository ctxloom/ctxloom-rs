//! Pruning over fake ports and annals' in-memory store: what is selected,
//! what is reported, what is removed and purged, and what a prune does
//! when the store changes under it.

use annals::next::v1::Uuid;
use annals::next::{
    AsyncEventStore, InMemoryStore, Kernel, MemoryDeadLetters, Routes, Sampled, Timestamp,
};
use annals_diagnostic::Diagnostic;
use tasks::{RemovalReason, RemoveTask, SetStatus, TaskCommand, Tasks};

use super::{cut, PruneRefusal};
use crate::fakes::{ready, FakeClock, FakeDelay, FakeIdentity, FakeNames, FakeStores, Fault};
use crate::{
    book, cover, registry, AppError, Dangling, ListQuery, NewTask, PruneRequest, PruneService,
    Pruning, Service, TaskService, Via,
};

type Fake = Service<FakeIdentity, FakeStores, FakeNames, FakeClock, FakeDelay>;

const DAY: i64 = 86_400;

fn at(seconds: i64) -> Timestamp {
    Timestamp { seconds, nanos: 0 }
}

// ------------------------------------------------------------ the cut

const NOW: i64 = 1_800_000_000;

fn cut_at(age: &str) -> Result<Timestamp, PruneRefusal> {
    cut(age, at(NOW))
}

#[test]
fn test_an_age_in_days_or_weeks_is_that_long_before_now() {
    assert_eq!(cut_at("7d"), Ok(at(NOW - 7 * DAY)));
    assert_eq!(cut_at(" 12w "), Ok(at(NOW - 84 * DAY)));
    assert_eq!(
        cut_at("+2d"),
        Ok(at(NOW - 2 * DAY)),
        "Go's Atoi takes a sign"
    );
}

#[test]
fn test_an_age_in_gos_duration_grammar_is_that_long_before_now() {
    assert_eq!(cut_at("720h"), Ok(at(NOW - 30 * DAY)));
    assert_eq!(cut_at("1h30m"), Ok(at(NOW - 5400)));
    assert_eq!(cut_at("1.5h"), Ok(at(NOW - 5400)));
    assert_eq!(cut_at("90s"), Ok(at(NOW - 90)));
    assert_eq!(cut_at("+1m"), Ok(at(NOW - 60)));
    assert_eq!(
        cut_at("500ms"),
        Ok(Timestamp {
            seconds: NOW - 1,
            nanos: 500_000_000
        }),
        "the nanoseconds borrow a second"
    );
    assert_eq!(
        cut_at("1us"),
        Ok(Timestamp {
            seconds: NOW - 1,
            nanos: 999_999_000
        })
    );
    assert_eq!(
        cut_at("2\u{b5}s3ns"),
        Ok(Timestamp {
            seconds: NOW - 1,
            nanos: 999_997_997
        })
    );
    assert_eq!(cut_at(".5h"), Ok(at(NOW - 1800)));
    assert_eq!(cut_at("1.h"), Ok(at(NOW - 3600)), "Go takes a bare point");
    assert_eq!(cut_at("1.25m"), Ok(at(NOW - 75)));
    assert_eq!(
        cut(
            "1s",
            Timestamp {
                seconds: NOW,
                nanos: 250
            }
        ),
        Ok(Timestamp {
            seconds: NOW - 1,
            nanos: 250
        }),
        "now's nanoseconds are kept"
    );
    assert!(
        cut_at("9223372036854775807ns").is_ok(),
        "the longest duration Go's int64 holds"
    );
}

#[test]
fn test_a_date_is_its_midnight_utc() {
    assert_eq!(cut_at("2026-01-01"), Ok(at(1_767_225_600)));
    assert_eq!(cut_at("1970-01-01"), Ok(at(0)));
    assert_eq!(cut_at("2024-02-29"), Ok(at(1_709_164_800)), "a leap day");
    assert_eq!(cut_at("2000-02-29"), Ok(at(951_782_400)), "400 years leap");
    assert_eq!(cut_at("2026-12-31"), Ok(at(1_798_675_200)));
    assert_eq!(cut_at("2026-03-01"), Ok(at(1_772_323_200)));
    assert_eq!(
        cut_at("2026-04-30"),
        Ok(at(1_777_507_200)),
        "a 30-day month"
    );
    assert_eq!(cut_at("2026-02-28"), Ok(at(1_772_236_800)), "February");
    assert_eq!(cut_at("1969-12-31"), Ok(at(-DAY)));
}

#[test]
fn test_an_age_of_nothing_is_refused() {
    assert_eq!(cut_at(""), Err(PruneRefusal::AgeRequired));
    assert_eq!(cut_at("  "), Err(PruneRefusal::AgeRequired));
}

#[test]
fn test_an_age_of_zero_or_less_is_refused() {
    for age in ["0d", "0", "-1h", "0s", "-3w"] {
        assert_eq!(
            cut_at(age),
            Err(PruneRefusal::AgeNotPositive {
                age: age.to_string()
            }),
            "{age}"
        );
    }
}

#[test]
fn test_an_age_that_is_neither_an_offset_nor_a_date_is_refused() {
    for age in [
        "soon",
        "7",
        "7x",
        "d",
        "h",
        ".h",
        "1.2.3h",
        "-",
        "+",
        "1h-2m",
        "2026-13-01",
        "2026-00-10",
        "2026-02-30",
        "2026-04-31",
        "2026-01-00",
        "2023-02-29",
        "1900-02-29",
        "2026-1-01",
        "2026/01/01",
        "20260-1-01",
        "2026-01-+1",
        "99999999999999h",
        "9223372036854775808ns",
        "2562048h",
        "2026-01/01",
        "2026/01-01",
        "1.5",
        "1hh",
    ] {
        assert_eq!(
            cut_at(age),
            Err(PruneRefusal::AgeUnreadable {
                age: age.to_string()
            }),
            "{age}"
        );
    }
}

#[test]
fn test_each_refusal_renders_its_english_and_names_its_age() {
    assert_eq!(
        PruneRefusal::AgeUnreadable {
            age: "soon".to_string()
        }
        .to_string(),
        "the age soon is neither an offset (30d, 12w, 720h) nor a date (2026-01-01)"
    );
    assert_eq!(
        PruneRefusal::AgeNotPositive {
            age: "0d".to_string()
        }
        .to_string(),
        "the age 0d is not a positive age"
    );
    assert_eq!(
        PruneRefusal::AgeRequired.to_string(),
        "a prune needs an age: an offset like 30d, or a date like 2026-01-01"
    );
    assert_eq!(
        PruneRefusal::NoStatus.to_string(),
        "a prune needs at least one status to select by: an empty status set is never read as every status"
    );
    let args = PruneRefusal::AgeUnreadable {
        age: "soon".to_string(),
    }
    .args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].0, "age");
    assert!(PruneRefusal::NoStatus.args().is_empty());
    assert!(PruneRefusal::AgeRequired.args().is_empty());
}

// ------------------------------------------------------------ the use case

fn service(names: &[&str]) -> Fake {
    Service::new(
        FakeIdentity::default(),
        FakeStores::default(),
        FakeNames::of(names),
        FakeClock::default(),
        FakeDelay::default(),
    )
}

fn add(service: &mut Fake, text: &str, status: &str, trigger: &str, tags: &[&str]) -> String {
    let task = NewTask {
        text: text.to_string(),
        status: status.to_string(),
        trigger: trigger.to_string(),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        ..NewTask::default()
    };
    ready(service.add(task)).unwrap().task.harp_id
}

/// Ten days on: past the default cut for everything added so far.
fn later(service: &mut Fake) {
    service.clock.seconds += 10 * DAY;
}

fn request(apply: bool) -> PruneRequest {
    PruneRequest {
        statuses: vec!["Done".to_string()],
        older_than: "7d".to_string(),
        apply,
        ..PruneRequest::default()
    }
}

fn prune(service: &mut Fake, request: PruneRequest) -> Result<Pruning, AppError> {
    ready(service.prune(request))
}

fn harps(pruning: &Pruning) -> Vec<&str> {
    pruning
        .pruned
        .iter()
        .map(|p| p.task.harp_id.as_str())
        .collect()
}

fn listed(service: &mut Fake) -> Vec<String> {
    let all = ListQuery {
        all: true,
        ..ListQuery::default()
    };
    ready(service.list(all))
        .unwrap()
        .rows
        .into_iter()
        .map(|row| row.task.harp_id)
        .collect()
}

/// A project with a task in each status Go's prune treats differently.
fn seeded() -> Fake {
    let mut service = service(&["done-one", "archived-one", "to-do-one", "done-two"]);
    add(&mut service, "Shipped", "Done", "", &[]);
    add(&mut service, "Put away", "Archived", "", &[]);
    add(&mut service, "Still to do", "", "", &[]);
    add(&mut service, "Shipped too", "Done", "", &["release"]);
    service
}

#[test]
fn test_without_apply_a_prune_reports_and_changes_nothing() {
    let mut service = seeded();
    later(&mut service);
    let before = listed(&mut service);
    let pruning = prune(&mut service, request(false)).unwrap();
    assert!(!pruning.applied);
    assert_eq!(
        harps(&pruning),
        ["done-one", "done-two"],
        "in the order added"
    );
    assert_eq!(pruning.tasks_remaining, 2);
    assert_eq!(
        pruning.events_removed, 4,
        "each task's add, and the removal a prune would record"
    );
    assert_eq!(pruning.statuses, ["Done"]);
    assert_eq!(pruning.project, Some(crate::fakes::project()));
    assert_eq!(listed(&mut service), before);
    assert_eq!(service.stores.store.begun, 4, "the adds' units alone");
    assert!(
        service.stores.opened.len() == 4,
        "opened to write by the adds only"
    );
}

#[test]
fn test_a_prune_reports_each_task_with_when_its_status_last_changed() {
    let mut service = seeded();
    later(&mut service);
    let pruning = prune(&mut service, request(false)).unwrap();
    let first = &pruning.pruned[0];
    assert_eq!(first.task.text, "Shipped");
    assert_eq!(first.task.status, "Done");
    assert_eq!(first.status_set_at, first.task.created_at, "the add set it");
    assert_eq!(
        pruning.before,
        at(1_700_000_000 + 10 * DAY + 5 - 7 * DAY),
        "seven days before the prune's reading of the clock"
    );
}

#[test]
fn test_a_task_whose_status_changed_within_the_cut_is_kept() {
    let mut service = seeded();
    // Not ten days on: nothing is older than seven days.
    let pruning = prune(&mut service, request(false)).unwrap();
    assert!(pruning.pruned.is_empty());
    assert_eq!(pruning.tasks_remaining, 4);

    let mut just = request(false);
    just.older_than = "2s".to_string();
    let pruning = prune(&mut service, just).unwrap();
    assert_eq!(
        harps(&pruning),
        ["done-one"],
        "done-two's status was set at the cut, not before it"
    );
}

#[test]
fn test_a_prune_selects_by_each_status_asked_for_trimmed() {
    let mut service = seeded();
    later(&mut service);
    let mut both = request(false);
    both.statuses = vec![" Archived ".to_string(), "Done".to_string(), String::new()];
    let pruning = prune(&mut service, both).unwrap();
    assert_eq!(harps(&pruning), ["done-one", "archived-one", "done-two"]);
    assert_eq!(pruning.statuses, ["Archived", "Done"]);
}

#[test]
fn test_a_tag_query_narrows_the_selection() {
    let mut service = seeded();
    later(&mut service);
    let mut tagged = request(false);
    tagged.tag_query = "release".to_string();
    let pruning = prune(&mut service, tagged).unwrap();
    assert_eq!(harps(&pruning), ["done-two"]);
    assert_eq!(pruning.tag_query, "release");
    assert_eq!(pruning.tasks_remaining, 3);
}

#[test]
fn test_a_prune_with_no_status_is_refused() {
    let mut service = seeded();
    let mut none = request(false);
    none.statuses = vec![" ".to_string()];
    let error = prune(&mut service, none).unwrap_err();
    assert_eq!(error.code(), "tasks-application-prune-no-status");
}

#[test]
fn test_a_status_class_is_refused_as_a_list_refuses_it() {
    let mut service = seeded();
    let mut class = request(false);
    class.statuses = vec!["@terminal".to_string()];
    let error = prune(&mut service, class).unwrap_err();
    assert_eq!(error.code(), "tasks-status-reserved");
}

#[test]
fn test_a_malformed_age_or_tag_query_is_refused_before_anything_is_read() {
    let mut service = seeded();
    let mut aged = request(true);
    aged.older_than = "soon".to_string();
    let error = prune(&mut service, aged).unwrap_err();
    assert_eq!(error.code(), "tasks-application-prune-age-unreadable");
    assert_eq!(
        error.to_string(),
        "the age soon is neither an offset (30d, 12w, 720h) nor a date (2026-01-01)"
    );

    let mut queried = request(true);
    queried.tag_query = "and".to_string();
    let error = prune(&mut service, queried).unwrap_err();
    assert_eq!(error.code(), "tasks-application-tag-query");
    assert_eq!(service.stores.store.begun, 4, "nothing was run");
}

#[test]
fn test_a_prune_where_there_is_no_project_prunes_nothing_and_binds_none() {
    let mut service = service(&[]);
    let pruning = prune(&mut service, request(true)).unwrap();
    assert_eq!(pruning.project, None);
    assert!(pruning.pruned.is_empty());
    assert!(pruning.applied);
    assert!(service.identity.bound.is_none());
    assert!(service.stores.opened.is_empty());
}

#[test]
fn test_a_prune_of_a_project_with_no_store_yet_creates_none() {
    let mut service = service(&[]);
    service.identity.bound = Some(crate::fakes::project());
    service.stores.absent = true;
    let pruning = prune(&mut service, request(true)).unwrap();
    assert_eq!(pruning.project, Some(crate::fakes::project()));
    assert!(pruning.pruned.is_empty());
    assert!(service.stores.opened.is_empty(), "never opened to write");
}

#[test]
fn test_a_prune_whose_project_does_not_resolve_fails_with_why() {
    let mut service = seeded();
    service.identity.fails = Some(annals_diagnostic::Report::new("x-lost", "lost"));
    let error = prune(&mut service, request(false)).unwrap_err();
    assert_eq!(error.code(), "x-lost");
}

#[test]
fn test_a_prune_whose_store_does_not_open_fails() {
    let mut service = seeded();
    service.stores.store.open_fails = true;
    let error = prune(&mut service, request(false)).unwrap_err();
    assert_eq!(error.code(), "annals-next-store-unavailable");
}

#[test]
fn test_an_applied_prune_removes_and_purges_each_selected_task() {
    let mut service = seeded();
    later(&mut service);
    let pruning = prune(&mut service, request(true)).unwrap();
    assert!(pruning.applied);
    assert_eq!(harps(&pruning), ["done-one", "done-two"]);
    assert_eq!(
        pruning.events_removed, 4,
        "as the store reports: each add, and each removal recorded first"
    );
    assert_eq!(pruning.tasks_remaining, 2);
    assert_eq!(listed(&mut service), ["archived-one", "to-do-one"]);
    for harp in ["done-one", "done-two"] {
        let held = ready(service.stores.store.read_from(&cover(harp), 0)).unwrap();
        assert_eq!(held, None, "{harp}'s stream is purged");
    }
    assert_eq!(
        service.stores.store.begun,
        4 + 4,
        "a removal and a purge, each its own unit, per task"
    );
}

#[test]
fn test_a_harp_freed_by_a_prune_can_be_minted_again_and_is_listed() {
    let mut service = seeded();
    later(&mut service);
    prune(&mut service, request(true)).unwrap();
    service.names = FakeNames::of(&["done-one"]);
    let again = add(&mut service, "A new task", "", "", &[]);
    assert_eq!(again, "done-one", "the harp is free again");
    let all = ListQuery {
        all: true,
        ..ListQuery::default()
    };
    let listing = ready(service.list(all)).unwrap();
    let texts: Vec<&str> = listing.tasks().map(|t| t.text.as_str()).collect();
    assert_eq!(texts, ["Put away", "Still to do", "A new task"]);
}

#[test]
fn test_a_removed_tasks_harp_stays_reserved_and_it_is_never_pruned() {
    let mut service = service(&["gone-one", "gone-one", "other"]);
    let harp = add(&mut service, "Removed by name", "Done", "", &[]);
    run(&mut service.stores.store.inner.0, &harp, remove_by_name());
    later(&mut service);
    let pruning = prune(&mut service, request(true)).unwrap();
    assert!(pruning.pruned.is_empty());
    assert_eq!(pruning.tasks_remaining, 0);
    assert!(
        ready(service.stores.store.read_from(&cover(&harp), 0))
            .unwrap()
            .is_some(),
        "its stream is kept"
    );
    assert_eq!(
        add(&mut service, "Another", "", "", &[]),
        "other",
        "its harp is still taken"
    );
}

// ------------------------------------------------------------ dangling

#[test]
fn test_a_survivor_naming_a_pruned_task_is_reported_and_left_as_written() {
    let mut service = service(&["old-task", "keeper", "waiter", "bystander"]);
    add(&mut service, "Finished", "Done", "", &[]);
    add(
        &mut service,
        "Follows old-task, and old-task again; not old-task-two",
        "",
        "",
        &["relates:old-task", "plain"],
    );
    add(
        &mut service,
        "Waiting",
        "Deferred",
        "when old-task ships",
        &[],
    );
    add(&mut service, "Unrelated old task", "", "", &[]);
    later(&mut service);
    let pruning = prune(&mut service, request(true)).unwrap();
    let dangling = |harp: &str, via: Via| Dangling {
        harp_id: harp.to_string(),
        pruned: "old-task".to_string(),
        via,
    };
    assert_eq!(
        pruning.dangling,
        [
            dangling("keeper", Via::Tag("relates:old-task".to_string())),
            dangling("keeper", Via::Text),
            dangling("waiter", Via::Trigger),
        ]
    );
    let all = ListQuery {
        all: true,
        ..ListQuery::default()
    };
    let listing = ready(service.list(all)).unwrap();
    assert_eq!(listing.rows[0].task.tags[0].to_string(), "plain");
    assert_eq!(listing.rows[0].task.tags[1].to_string(), "relates:old-task");
}

#[test]
fn test_a_report_names_the_dangling_references_a_prune_would_leave() {
    let mut service = service(&["old-task", "keeper"]);
    add(&mut service, "Finished", "Done", "", &[]);
    add(&mut service, "After old-task", "", "", &[]);
    later(&mut service);
    let pruning = prune(&mut service, request(false)).unwrap();
    assert_eq!(pruning.dangling.len(), 1);
    assert_eq!(pruning.dangling[0].via.to_string(), "text");
}

#[test]
fn test_a_via_is_spelled_as_gos() {
    assert_eq!(
        Via::Tag("relates:x".to_string()).to_string(),
        "tag relates:x"
    );
    assert_eq!(Via::Text.to_string(), "text");
    assert_eq!(Via::Trigger.to_string(), "trigger");
}

// ------------------------------------------------------------ races

/// Run `command` on `harp`'s stream in `store` directly, as another
/// process would.
fn run(store: &mut InMemoryStore, harp: &str, command: TaskCommand) {
    let registry = registry();
    let dead_letters = MemoryDeadLetters::new();
    let sampled = Sampled {
        now: at(1_700_000_000 + 20 * DAY),
        command_id: Uuid::from_bytes([0xee; 16]),
    };
    Kernel::new(&registry, &dead_letters)
        .handle::<Tasks, _, _>(
            store.begin(),
            &Routes::new(),
            book(harp, command),
            &sampled,
            &mut (),
        )
        .unwrap_or_else(|failure| panic!("{failure:?}"));
}

fn remove_by_name() -> TaskCommand {
    RemoveTask {
        reason: RemovalReason::Removed,
    }
    .into()
}

#[test]
fn test_a_task_moved_since_the_selection_is_not_pruned() {
    let mut service = seeded();
    later(&mut service);
    let reopen: TaskCommand = SetStatus {
        status: "In Progress".to_string(),
        trigger: String::new(),
    }
    .into();
    service.stores.store.meanwhile = Some(Box::new(move |store| run(store, "done-one", reopen)));
    let pruning = prune(&mut service, request(true)).unwrap();
    assert_eq!(harps(&pruning), ["done-two"]);
    assert_eq!(pruning.tasks_remaining, 3);
    assert!(listed(&mut service).contains(&"done-one".to_string()));
}

#[test]
fn test_a_task_retagged_out_of_the_query_since_the_selection_is_not_pruned() {
    let mut service = seeded();
    later(&mut service);
    let untag: TaskCommand = tasks::Retag {
        add: Vec::new(),
        remove: vec![crate::parse_tag("release").unwrap().unwrap()],
    }
    .into();
    service.stores.store.meanwhile = Some(Box::new(move |store| run(store, "done-two", untag)));
    let mut tagged = request(true);
    tagged.tag_query = "release".to_string();
    let pruning = prune(&mut service, tagged).unwrap();
    assert!(pruning.pruned.is_empty());
    assert!(listed(&mut service).contains(&"done-two".to_string()));
}

#[test]
fn test_a_task_removed_by_another_since_the_selection_is_skipped_and_kept() {
    let mut service = seeded();
    later(&mut service);
    service.stores.store.meanwhile =
        Some(Box::new(|store| run(store, "done-one", remove_by_name())));
    let pruning = prune(&mut service, request(true)).unwrap();
    assert_eq!(harps(&pruning), ["done-two"]);
    assert!(
        ready(service.stores.store.read_from(&cover("done-one"), 0))
            .unwrap()
            .is_some(),
        "not this prune's to purge"
    );
}

#[test]
fn test_a_task_purged_by_another_since_the_selection_is_skipped() {
    let mut service = seeded();
    later(&mut service);
    service.stores.store.meanwhile = Some(Box::new(|store| {
        annals::next::Purge::purge(store, &cover("done-one")).unwrap();
    }));
    let pruning = prune(&mut service, request(true)).unwrap();
    assert_eq!(harps(&pruning), ["done-two"]);
}

#[test]
fn test_a_removal_whose_answer_was_lost_is_purged_after_its_retry() {
    let mut service = seeded();
    later(&mut service);
    service.stores.store.faults = [Fault::LandsThenFails].into();
    let pruning = prune(&mut service, request(true)).unwrap();
    assert_eq!(harps(&pruning), ["done-one", "done-two"]);
    assert_eq!(
        ready(service.stores.store.read_from(&cover("done-one"), 0)).unwrap(),
        None
    );
}

#[test]
fn test_a_purge_is_retried_through_store_failures() {
    let mut service = seeded();
    later(&mut service);
    service.stores.store.faults = [Fault::Sound, Fault::Unavailable, Fault::Conflict].into();
    let pruning = prune(&mut service, request(true)).unwrap();
    assert_eq!(harps(&pruning), ["done-one", "done-two"]);
    assert_eq!(pruning.events_removed, 4);
    assert_eq!(service.delay.slept.len(), 2, "two waits, one per failure");
    assert_eq!(
        ready(service.stores.store.read_from(&cover("done-one"), 0)).unwrap(),
        None
    );
}

#[test]
fn test_a_purge_that_no_retry_mends_fails_the_prune() {
    let mut service = seeded();
    later(&mut service);
    service.stores.store.faults = [Fault::Sound, Fault::Discontinuous].into();
    let error = prune(&mut service, request(true)).unwrap_err();
    assert_eq!(error.code(), "annals-next-store-discontinuous");
    assert!(
        !listed(&mut service).contains(&"done-one".to_string()),
        "removed, though not purged"
    );
}

/// A project left half pruned: `done-one` removed by a prune whose purge
/// then failed for good, so its stream, and its harp, are still held.
fn half_pruned() -> Fake {
    let mut service = seeded();
    later(&mut service);
    service.stores.store.faults = [Fault::Sound, Fault::Discontinuous].into();
    prune(&mut service, request(true)).unwrap_err();
    assert!(
        ready(service.stores.store.read_from(&cover("done-one"), 0))
            .unwrap()
            .is_some(),
        "removed, not purged"
    );
    service
}

#[test]
fn test_a_report_names_a_task_a_prune_left_removed_but_not_purged() {
    let mut service = half_pruned();
    let begun = service.stores.store.begun;
    let pruning = prune(&mut service, request(false)).unwrap();
    assert_eq!(harps(&pruning), ["done-one", "done-two"]);
    assert_eq!(
        pruning.events_removed,
        2 + 2,
        "done-one's add and the removal it holds; done-two's add and the removal to come"
    );
    assert_eq!(pruning.tasks_remaining, 2);
    assert_eq!(service.stores.store.begun, begun, "a report writes nothing");
    assert!(ready(service.stores.store.read_from(&cover("done-one"), 0))
        .unwrap()
        .is_some());
}

#[test]
fn test_an_applied_prune_finishes_a_task_left_removed_but_not_purged() {
    let mut service = half_pruned();
    let pruning = prune(&mut service, request(true)).unwrap();
    assert_eq!(harps(&pruning), ["done-one", "done-two"]);
    assert_eq!(pruning.events_removed, 4, "as the store reports");
    assert_eq!(
        ready(service.stores.store.read_from(&cover("done-one"), 0)).unwrap(),
        None,
        "its purge finished"
    );
    service.names = FakeNames::of(&["done-one"]);
    assert_eq!(
        add(&mut service, "A new task", "", "", &[]),
        "done-one",
        "its harp is free again"
    );
}

#[test]
fn test_a_half_done_prune_is_finished_whatever_this_prune_selects_by() {
    // The prune that removed it selected it; this one only finishes it.
    let mut service = half_pruned();
    let mut archived = request(true);
    archived.statuses = vec!["Archived".to_string()];
    archived.tag_query = "release".to_string();
    let pruning = prune(&mut service, archived).unwrap();
    assert_eq!(harps(&pruning), ["done-one"]);
    assert_eq!(pruning.events_removed, 2);
    assert_eq!(
        ready(service.stores.store.read_from(&cover("done-one"), 0)).unwrap(),
        None
    );
}

#[test]
fn test_a_half_done_prune_finished_by_another_meanwhile_is_skipped() {
    let mut service = half_pruned();
    service.stores.store.meanwhile = Some(Box::new(|store| {
        annals::next::Purge::purge(store, &cover("done-one")).unwrap();
    }));
    let mut archived = request(true);
    archived.statuses = vec!["Archived".to_string()];
    archived.tag_query = "release".to_string();
    let pruning = prune(&mut service, archived).unwrap();
    assert!(pruning.pruned.is_empty());
    assert_eq!(pruning.events_removed, 0);
}

#[test]
fn test_a_prune_whose_future_is_send() {
    fn is_send<T: Send>(_: &T) {}
    let mut service = seeded();
    is_send(&service.prune(request(true)));
}
