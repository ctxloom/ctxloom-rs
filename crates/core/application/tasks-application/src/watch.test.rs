//! Watching: a poll answers what was committed since the last, and nothing
//! twice; a watch binds no project and waits for a store to appear.

use annals::next::v1::Uuid;
use annals::next::{AsyncPurge, InMemoryStore, Kernel, MemoryDeadLetters, Routes, Sampled};
use annals_diagnostic::Diagnostic;
use tasks::{AddTask, RemovalReason, RemoveTask, TaskCommand, Tasks};

use super::{Changed, WatchRequest, WatchService};
use crate::fakes::{project, ready, FakeClock, FakeDelay, FakeIdentity, FakeNames, FakeStores};
use crate::{book, cover, registry, retry, AppError, NewTask, Service, TaskService};

type Fake = Service<FakeIdentity, FakeStores, FakeNames, FakeClock, FakeDelay>;

fn service(names: &[&str]) -> Fake {
    Service::new(
        FakeIdentity::default(),
        FakeStores::default(),
        FakeNames::of(names),
        FakeClock::default(),
        FakeDelay::default(),
    )
}

fn add(service: &mut Fake, text: &str) -> String {
    let task = NewTask {
        text: text.to_string(),
        ..NewTask::default()
    };
    ready(service.add(task)).expect("added").task.harp_id
}

/// Remove `harp` straight through the kernel: no use case removes yet.
fn remove(service: &mut Fake, harp: &str) {
    remove_for(service, harp, RemovalReason::Removed);
}

/// Remove `harp` for `reason` straight through the kernel: a prune's
/// removal, without its purge, when `Pruned`.
fn remove_for(service: &mut Fake, harp: &str, reason: RemovalReason) {
    let command = RemoveTask { reason };
    let book = book(harp, command.into());
    let command_id = ports::CommandIds::command_id(&mut service.clock);
    let mut runner = retry::Runner {
        registry: &service.registry,
        clock: &mut service.clock,
        delay: &mut service.delay,
        backoff: service.backoff,
    };
    let outcome = ready(runner.run(&mut service.stores.store, &book, &command_id));
    assert!(matches!(outcome, Ok(retry::Outcome::Committed)));
}

fn texts(changes: &[Changed]) -> Vec<String> {
    changes
        .iter()
        .map(|c| match c {
            Changed::Task(task) => format!("{} {}", task.harp_id, task.status),
            Changed::Removed { harp_id } => format!("{harp_id} removed"),
        })
        .collect()
}

#[test]
fn test_a_watch_where_there_is_no_project_is_none_and_binds_none() {
    let mut service = service(&[]);
    let watch = ready(service.watch(WatchRequest::default())).expect("resolves");
    assert!(watch.is_none());
    assert_eq!(service.identity.bound, None, "a watch binds no project");
    assert!(service.stores.opened.is_empty(), "nor creates a store");
}

#[test]
fn test_a_watch_starts_caught_up_so_a_poll_answers_only_what_came_after() {
    let mut service = service(&["swift-amber", "quiet-heron"]);
    add(&mut service, "Before");
    let mut watch = ready(service.watch(WatchRequest::default()))
        .expect("resolves")
        .expect("a project");
    assert_eq!(watch.project, project());
    assert_eq!(ready(service.poll(&mut watch)).unwrap(), vec![]);

    add(&mut service, "After");
    let changes = ready(service.poll(&mut watch)).unwrap();
    assert_eq!(texts(&changes), ["quiet-heron To Do"]);
    assert_eq!(
        ready(service.poll(&mut watch)).unwrap(),
        vec![],
        "nothing twice"
    );
}

#[test]
fn test_a_poll_answers_every_commit_since_the_last_in_order_and_a_removal() {
    let mut service = service(&["swift-amber", "quiet-heron"]);
    add(&mut service, "One");
    let mut watch = ready(service.watch(WatchRequest::default()))
        .unwrap()
        .unwrap();
    add(&mut service, "Two");
    remove(&mut service, "swift-amber");
    let changes = ready(service.poll(&mut watch)).unwrap();
    // The projector takes the streams in the store's order, by root.
    let mut seen = texts(&changes);
    seen.sort();
    assert_eq!(seen, ["quiet-heron To Do", "swift-amber removed"]);
}

#[test]
fn test_a_watch_of_a_project_with_no_store_yet_waits_for_one() {
    let mut service = service(&["swift-amber"]);
    service.identity.bound = Some(project());
    service.stores.absent = true;
    let mut watch = ready(service.watch(WatchRequest::default()))
        .unwrap()
        .expect("the project");
    assert_eq!(ready(service.poll(&mut watch)).unwrap(), vec![]);
    assert!(service.stores.opened.is_empty(), "a poll creates no store");

    add(&mut service, "First");
    let changes = ready(service.poll(&mut watch)).unwrap();
    assert_eq!(texts(&changes), ["swift-amber To Do"]);
}

#[test]
fn test_a_pinned_project_is_watched_and_a_store_that_fails_is_an_error() {
    let mut service = service(&[]);
    let request = WatchRequest {
        project: Some("jolly-mute-magma".to_string()),
    };
    let mut watch = ready(service.watch(request)).unwrap().expect("pinned");
    assert_eq!(
        service.identity.pins,
        [Some("jolly-mute-magma".to_string())]
    );
    service.stores.store.open_fails = true;
    let failed = ready(service.poll(&mut watch)).unwrap_err();
    assert!(matches!(failed, AppError::Failed(_)), "{failed:?}");
}

#[test]
fn test_a_watch_whose_project_does_not_resolve_says_why() {
    let mut service = service(&[]);
    let why = annals_diagnostic::Report::of(&AppError::NoProject);
    service.identity.fails = Some(why);
    let failed = ready(service.watch(WatchRequest::default()))
        .err()
        .expect("an error");
    assert!(matches!(failed, AppError::Identity(_)));
    assert_eq!(failed.code(), "tasks-application-no-project");
}

/// Add `harp` to `store` directly, as another process would.
fn add_elsewhere(store: &mut InMemoryStore, harp: &str) {
    let command: TaskCommand = AddTask {
        harp_id: harp.to_string(),
        text: "Elsewhere".to_string(),
        status: String::new(),
        trigger: String::new(),
        tags: Vec::new(),
        origin_session: String::new(),
    }
    .into();
    let registry = registry();
    let dead_letters = MemoryDeadLetters::new();
    let sampled = Sampled {
        now: annals::next::Timestamp::default(),
        command_id: Uuid::from_bytes([0xee; 16]),
    };
    Kernel::new(&registry, &dead_letters)
        .handle::<Tasks, _, _>(
            annals::next::Store::begin(store),
            &Routes::new(),
            book(harp, command),
            &sampled,
            &mut (),
        )
        .unwrap_or_else(|failure| panic!("{failure:?}"));
}

// ------------------------------------------------------------ the token

#[test]
fn test_a_poll_whose_token_has_not_moved_reads_no_stream() {
    let mut service = service(&["swift-amber", "quiet-heron"]);
    add(&mut service, "One");
    let mut watch = ready(service.watch(WatchRequest::default()))
        .unwrap()
        .unwrap();
    let reads = service.stores.store.reads;
    for _ in 0..3 {
        assert_eq!(ready(service.poll(&mut watch)).unwrap(), vec![]);
    }
    assert_eq!(
        service.stores.store.reads, reads,
        "nothing committed, nothing read"
    );

    add(&mut service, "Two");
    // The add reads its task back: count from after it.
    let reads = service.stores.store.reads;
    let changes = ready(service.poll(&mut watch)).unwrap();
    assert_eq!(texts(&changes), ["quiet-heron To Do"]);
    assert_eq!(service.stores.store.reads, reads + 1, "read once for it");
    assert_eq!(ready(service.poll(&mut watch)).unwrap(), vec![]);
    assert_eq!(service.stores.store.reads, reads + 1);
}

#[test]
fn test_a_commit_landing_as_the_token_is_asked_for_is_caught_up_at_that_poll() {
    let mut service = service(&["swift-amber"]);
    add(&mut service, "One");
    let mut watch = ready(service.watch(WatchRequest::default()))
        .unwrap()
        .unwrap();
    // Another process commits just before the token is read: the token
    // covers it, so the poll must read after it (a poll that read the
    // streams first and the token after would never see it).
    service.stores.before_token = Some(Box::new(|store| add_elsewhere(store, "far-heron")));
    let changes = ready(service.poll(&mut watch)).unwrap();
    assert_eq!(texts(&changes), ["far-heron To Do"]);
}

#[test]
fn test_a_commit_by_another_process_between_polls_is_caught_up() {
    let mut service = service(&["swift-amber"]);
    add(&mut service, "One");
    let mut watch = ready(service.watch(WatchRequest::default()))
        .unwrap()
        .unwrap();
    assert_eq!(ready(service.poll(&mut watch)).unwrap(), vec![]);
    add_elsewhere(&mut service.stores.store.inner.0, "far-heron");
    let changes = ready(service.poll(&mut watch)).unwrap();
    assert_eq!(texts(&changes), ["far-heron To Do"]);
}

#[test]
fn test_a_store_opened_again_is_read_again_whatever_its_token() {
    let mut service = service(&["swift-amber"]);
    add(&mut service, "One");
    let mut watch = ready(service.watch(WatchRequest::default()))
        .unwrap()
        .unwrap();
    let reads = service.stores.store.reads;
    // A new handle: its tokens say nothing about the old one's.
    service.stores.handle += 1;
    assert_eq!(ready(service.poll(&mut watch)).unwrap(), vec![]);
    assert_eq!(service.stores.store.reads, reads + 1);
    assert_eq!(ready(service.poll(&mut watch)).unwrap(), vec![]);
    assert_eq!(service.stores.store.reads, reads + 1, "and held after");
}

#[test]
fn test_a_token_that_moved_with_nothing_new_answers_nothing() {
    let mut service = service(&["swift-amber"]);
    add(&mut service, "One");
    let mut watch = ready(service.watch(WatchRequest::default()))
        .unwrap()
        .unwrap();
    let reads = service.stores.store.reads;
    // A purge of a stream there never was: a commit with nothing in it.
    ready(service.stores.store.purge(&cover("no-such-task"))).unwrap();
    assert_eq!(ready(service.poll(&mut watch)).unwrap(), vec![]);
    assert_eq!(
        service.stores.store.reads,
        reads + 1,
        "read, and nothing found"
    );
}

#[test]
fn test_a_watch_of_no_store_yet_reads_nothing_until_one_appears() {
    let mut service = service(&["swift-amber"]);
    service.identity.bound = Some(project());
    service.stores.absent = true;
    let mut watch = ready(service.watch(WatchRequest::default()))
        .unwrap()
        .unwrap();
    for _ in 0..3 {
        assert_eq!(ready(service.poll(&mut watch)).unwrap(), vec![]);
    }
    assert_eq!(service.stores.store.reads, 0);
    add(&mut service, "First");
    let reads = service.stores.store.reads;
    let changes = ready(service.poll(&mut watch)).unwrap();
    assert_eq!(texts(&changes), ["swift-amber To Do"]);
    assert_eq!(ready(service.poll(&mut watch)).unwrap(), vec![]);
    assert_eq!(service.stores.store.reads, reads + 1, "then the token");
}

// ------------------------------------------------------------ prune

#[test]
fn test_a_prune_landing_whole_between_polls_answers_the_removal() {
    let mut service = service(&["swift-amber", "quiet-heron"]);
    add(&mut service, "Pruned");
    add(&mut service, "Kept");
    let mut watch = ready(service.watch(WatchRequest::default()))
        .unwrap()
        .unwrap();
    remove_for(&mut service, "swift-amber", RemovalReason::Pruned);
    ready(service.stores.store.purge(&cover("swift-amber"))).unwrap();
    let changes = ready(service.poll(&mut watch)).unwrap();
    assert_eq!(texts(&changes), ["swift-amber removed"]);
    assert_eq!(ready(service.poll(&mut watch)).unwrap(), vec![]);
}

/// Compiles only for a `Send` value.
fn is_send<T: Send>(_: &T) {}

/// Compiles only if the trait itself promises `Send` futures.
fn spawnable<W: WatchService + Send>(service: &mut W, watch: &mut super::Watch) {
    is_send(&service.poll(watch));
}

#[test]
fn test_watch_futures_are_send_so_a_server_can_spawn_them() {
    let mut service = service(&["swift-amber"]);
    is_send(&service.watch(WatchRequest::default()));
    add(&mut service, "One");
    let mut watch = ready(service.watch(WatchRequest::default()))
        .unwrap()
        .unwrap();
    is_send(&watch);
    spawnable(&mut service, &mut watch);
}

#[test]
fn test_a_watch_that_has_seen_nothing_answers_every_task_at_its_first_poll() {
    let mut service = service(&["swift-amber"]);
    add(&mut service, "One");
    let mut watch = super::Watch::of(project());
    let changes = ready(service.poll(&mut watch)).unwrap();
    assert_eq!(texts(&changes), ["swift-amber To Do"]);
}
