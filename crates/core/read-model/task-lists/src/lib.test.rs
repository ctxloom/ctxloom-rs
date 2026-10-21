//! A task list, projected live: each command runs through annals' kernel
//! with the list attached as a projection, so the list sees each committed
//! event with the task's state just after it. A fresh list catches up from the store, as every read and watch of the
//! binary's does.

use std::marker::PhantomData;

use crate::{Change, Delta, TaskList};
use annals::next::v1::{command_page, CommandBook, CommandPage, Cover, Uuid};
use annals::next::{
    ids, InMemoryStore, Kernel, MemoryDeadLetters, Payload, ProjectionRoute, Ready, Registry,
    Routes, Rulebook, Sampled, Timestamp, Upcaster,
};
use tasks::{
    AddTask, RemovalReason, RemoveTask, SetStatus, TaskCommand, TaskState, Tasks, DEFERRED,
};

const FIRST: &str = "swift-amber";
const SECOND: &str = "calm-otter";

/// One project's store, and the clock its commands are sampled at.
struct Project {
    store: InMemoryStore,
    registry: Registry,
    seconds: i64,
}

impl Project {
    fn new() -> Project {
        let mut registry = Registry::new();
        registry
            .register::<Tasks>(Rulebook::new("v1", ()), Upcaster::new("tasks", "tasks"))
            .unwrap();
        Project {
            store: InMemoryStore::new(),
            registry,
            seconds: 100,
        }
    }

    /// Run `command` on task `harp`'s stream, projecting into `list`.
    fn run(&mut self, list: &mut TaskList, harp: &str, command: impl Into<TaskCommand>) {
        self.seconds += 1;
        let sampled = Sampled {
            now: Timestamp {
                seconds: self.seconds,
                nanos: 0,
            },
            command_id: Uuid::from_bytes([self.seconds as u8; 16]),
        };
        let dead_letters = MemoryDeadLetters::new();
        let mut route = ProjectionRoute {
            name: "list",
            projection: list,
            domain: PhantomData::<fn() -> Tasks>,
        };
        Kernel::new(&self.registry, &dead_letters)
            .handle::<Tasks, _, _>(
                self.store.begin(),
                &Routes::new(),
                book(harp, command.into().to_any()),
                &sampled,
                &mut route,
            )
            .unwrap_or_else(|failure| panic!("{failure:?}"));
    }
}

fn cover(harp: &str) -> Cover {
    Cover {
        domain: "tasks".to_string(),
        root: Some(ids::compute_root("tasks", harp).unwrap()),
        edition: None,
    }
}

fn book(harp: &str, payload: annals::next::Any) -> CommandBook {
    CommandBook {
        cover: Some(cover(harp)),
        page: Some(CommandPage {
            body: Some(command_page::Body::Command(annals::next::v1::Command {
                payload: Some(payload),
            })),
            ..CommandPage::default()
        }),
        handling: None,
    }
}

fn add(harp: &str, text: &str) -> AddTask {
    AddTask {
        harp_id: harp.to_string(),
        text: text.to_string(),
        status: String::new(),
        trigger: String::new(),
        tags: Vec::new(),
        origin_session: String::new(),
    }
}

fn status(status: &str, trigger: &str) -> SetStatus {
    SetStatus {
        status: status.to_string(),
        trigger: trigger.to_string(),
    }
}

fn every_task() -> TaskList {
    TaskList::new(Box::new(|_| true))
}

fn not_done() -> TaskList {
    TaskList::new(Box::new(|t: &TaskState| t.status != "Done"))
}

fn deferred() -> TaskList {
    TaskList::new(Box::new(|t: &TaskState| t.status == DEFERRED))
}

fn harps(list: &TaskList) -> Vec<&str> {
    list.entries().iter().map(|t| t.harp_id.as_str()).collect()
}

// ------------------------------------------------------------ membership

#[test]
fn test_a_list_holds_each_live_task_its_membership_admits() {
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(&mut list, SECOND, add(SECOND, "Second"));
    assert_eq!(harps(&list), [SECOND, FIRST], "by harp id");
    assert_eq!(list.sequence(), 2);
}

#[test]
fn test_each_change_to_the_list_takes_the_next_sequence() {
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(&mut list, FIRST, status("In Progress", ""));
    let deltas = list.since(0).expect("from the start");
    assert_eq!(deltas.len(), 2);
    assert_eq!(deltas[0].sequence, 1);
    assert_eq!(deltas[1].sequence, 2);
    let Change::Upsert(task) = &deltas[1].change else {
        panic!("a listed task changed: {deltas:?}");
    };
    assert_eq!(
        task.status, "In Progress",
        "the task's state after the event"
    );
}

#[test]
fn test_a_task_leaving_the_membership_is_removed_from_the_list() {
    let mut project = Project::new();
    let mut list = not_done();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(&mut list, FIRST, status("Done", ""));
    assert!(list.entries().is_empty());
    assert_eq!(
        list.since(1),
        Some(
            &[Delta {
                sequence: 2,
                change: Change::Remove {
                    harp_id: FIRST.to_string()
                },
            }][..]
        )
    );
}

#[test]
fn test_a_task_entering_the_membership_is_added_to_the_list() {
    let mut project = Project::new();
    let mut list = deferred();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    assert_eq!(list.sequence(), 0, "not deferred: not listed");
    project.run(&mut list, FIRST, status(DEFERRED, "after the release"));
    assert_eq!(harps(&list), [FIRST]);
    assert_eq!(list.sequence(), 1);
}

#[test]
fn test_a_removed_task_leaves_the_list() {
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(
        &mut list,
        FIRST,
        RemoveTask {
            reason: RemovalReason::Removed,
        },
    );
    assert!(list.entries().is_empty());
    assert_eq!(list.sequence(), 2);
}

#[test]
fn test_a_change_outside_the_membership_takes_no_sequence() {
    let mut project = Project::new();
    let mut list = deferred();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(&mut list, FIRST, status("In Progress", ""));
    assert_eq!(list.sequence(), 0);
    assert_eq!(list.since(0), Some(&[][..]));
}

// ------------------------------------------------------------ cursors

#[test]
fn test_since_gives_the_changes_after_a_cursor() {
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(&mut list, SECOND, add(SECOND, "Second"));
    let after_first = list.since(1).unwrap();
    assert_eq!(after_first.len(), 1);
    assert_eq!(after_first[0].sequence, 2);
    assert_eq!(list.since(2), Some(&[][..]), "caught up");
}

#[test]
fn test_since_a_cursor_past_the_sequence_is_none() {
    // A cursor the list never issued (a list rebuilt under the reader):
    // the reader takes the list whole.
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    assert_eq!(list.since(5), None);
}

// ------------------------------------------------------------ catch-up

#[test]
fn test_a_list_caught_up_from_the_store_holds_what_the_live_list_holds() {
    let mut project = Project::new();
    let mut live = not_done();
    project.run(&mut live, FIRST, add(FIRST, "First"));
    project.run(&mut live, SECOND, add(SECOND, "Second"));
    project.run(&mut live, FIRST, status("Done", ""));

    let mut rebuilt = not_done();
    rebuilt
        .catch_up(&mut project.store, &project.registry)
        .unwrap();
    assert_eq!(rebuilt.entries(), live.entries());
    assert_eq!(harps(&rebuilt), [SECOND]);
}

#[test]
fn test_a_redelivered_event_changes_nothing() {
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    list.catch_up(&mut project.store, &project.registry)
        .unwrap();
    assert_eq!(
        list.sequence(),
        1,
        "the store's events were already projected"
    );
}

/// Run a future whose every await is ready at once (a [`Ready`] store's),
/// as the async port's tests need no runtime.
fn ready<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(out) => out,
        std::task::Poll::Pending => panic!("a ready store's futures finish at once"),
    }
}

#[test]
fn test_a_list_caught_up_over_the_async_port_holds_what_the_live_list_holds() {
    let mut project = Project::new();
    let mut live = not_done();
    project.run(&mut live, FIRST, add(FIRST, "First"));
    project.run(&mut live, SECOND, add(SECOND, "Second"));
    project.run(&mut live, FIRST, status("Done", ""));

    let mut rebuilt = not_done();
    let mut store = Ready(std::mem::take(&mut project.store));
    ready(rebuilt.catch_up_async(&mut store, &project.registry)).unwrap();
    assert_eq!(rebuilt.entries(), live.entries());
    assert_eq!(harps(&rebuilt), [SECOND]);
    assert_eq!(
        rebuilt.sequence(),
        3,
        "each event the list saw: two adds, one leaving"
    );
}

#[test]
fn test_an_async_catch_up_projects_only_what_is_new() {
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    let mut store = Ready(std::mem::take(&mut project.store));
    ready(list.catch_up_async(&mut store, &project.registry)).unwrap();
    assert_eq!(list.sequence(), 1, "already projected");
}

// ------------------------------------------------------------ pruning

fn pruned() -> RemoveTask {
    RemoveTask {
        reason: RemovalReason::Pruned,
    }
}

fn removed() -> RemoveTask {
    RemoveTask {
        reason: RemovalReason::Removed,
    }
}

#[test]
fn test_a_task_minted_on_a_pruned_harp_is_listed_though_its_stream_restarted() {
    // Prune records the removal, then purges the stream; a new task minted
    // on the freed harp starts the stream again from its first sequence,
    // which the list must not skip as already projected.
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(&mut list, FIRST, status("Done", ""));
    project.run(&mut list, FIRST, pruned());
    assert!(list.entries().is_empty(), "the removal drops it at once");
    let deleted = annals::next::Purge::purge(&mut project.store, &cover(FIRST)).unwrap();
    assert_eq!(deleted, 3);

    project.run(&mut list, FIRST, add(FIRST, "Again"));
    let texts: Vec<&str> = list.entries().iter().map(|t| t.text.as_str()).collect();
    assert_eq!(texts, ["Again"], "the new task, not the pruned one");
    assert_eq!(
        list.sequence(),
        4,
        "the add, its status, the removal, the new add"
    );
}

#[test]
fn test_a_task_removed_by_name_keeps_its_streams_place() {
    // Only a prune frees a stream; a removal by name keeps it, so its
    // events redelivered are still seen as projected.
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(&mut list, FIRST, removed());
    list.catch_up(&mut project.store, &project.registry)
        .unwrap();
    assert!(list.entries().is_empty());
    assert_eq!(
        list.sequence(),
        2,
        "nothing redelivered was projected again"
    );
}

#[test]
fn test_a_list_caught_up_after_a_prune_holds_the_task_minted_since() {
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(&mut list, FIRST, pruned());
    annals::next::Purge::purge(&mut project.store, &cover(FIRST)).unwrap();
    let mut other = every_task();
    project.run(&mut other, FIRST, add(FIRST, "Again"));
    list.catch_up(&mut project.store, &project.registry)
        .unwrap();
    let texts: Vec<&str> = list.entries().iter().map(|t| t.text.as_str()).collect();
    assert_eq!(texts, ["Again"]);
}

#[test]
fn test_a_task_pruned_and_purged_unseen_leaves_the_list_at_the_next_catch_up() {
    // A long-lived list (a watch's) catching up after a prune landed whole
    // between two of its reads: it never saw the removal, and the stream
    // is gone, so only the stream's absence says the task went.
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(&mut list, SECOND, add(SECOND, "Second"));
    let mut elsewhere = every_task();
    project.run(&mut elsewhere, FIRST, pruned());
    annals::next::Purge::purge(&mut project.store, &cover(FIRST)).unwrap();

    list.catch_up(&mut project.store, &project.registry)
        .unwrap();
    assert_eq!(harps(&list), [SECOND]);
    assert_eq!(
        list.since(2).unwrap(),
        [Delta {
            sequence: 3,
            change: Change::Remove {
                harp_id: FIRST.to_string()
            }
        }]
    );
    assert!(unpurged(&list).is_empty(), "its stream is gone");
    list.catch_up(&mut project.store, &project.registry)
        .unwrap();
    assert_eq!(list.sequence(), 3, "gone once");

    // The freed harp minted again: its stream starts over, and is seen.
    project.run(&mut elsewhere, FIRST, add(FIRST, "Again"));
    let mut store = Ready(std::mem::take(&mut project.store));
    ready(list.catch_up_async(&mut store, &project.registry)).unwrap();
    let texts: Vec<&str> = list.entries().iter().map(|t| t.text.as_str()).collect();
    assert_eq!(texts, ["Second", "Again"], "by harp");
}

#[test]
fn test_an_async_catch_up_drops_a_task_whose_stream_is_gone() {
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(&mut list, FIRST, status("Done", ""));
    let mut elsewhere = every_task();
    project.run(&mut elsewhere, FIRST, pruned());
    annals::next::Purge::purge(&mut project.store, &cover(FIRST)).unwrap();
    let mut store = Ready(std::mem::take(&mut project.store));
    ready(list.catch_up_async(&mut store, &project.registry)).unwrap();
    assert!(list.entries().is_empty());
    assert_eq!(list.sequence(), 3);
}

#[test]
fn test_a_task_the_list_does_not_hold_leaves_no_change_when_its_stream_goes() {
    let mut project = Project::new();
    let mut list = not_done();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(&mut list, FIRST, status("Done", ""));
    assert_eq!(list.sequence(), 2, "in, then out");
    let mut elsewhere = every_task();
    project.run(&mut elsewhere, FIRST, pruned());
    annals::next::Purge::purge(&mut project.store, &cover(FIRST)).unwrap();
    list.catch_up(&mut project.store, &project.registry)
        .unwrap();
    assert_eq!(list.sequence(), 2, "nothing listed left");
}

fn unpurged(list: &TaskList) -> Vec<&str> {
    list.unpurged().iter().map(|t| t.harp_id.as_str()).collect()
}

#[test]
fn test_a_task_pruned_but_not_yet_purged_is_held_as_unpurged() {
    // A prune whose purge failed after its removal landed leaves the
    // stream behind, its harp taken: the list names it, for a prune to
    // finish.
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(&mut list, FIRST, status("Done", ""));
    project.run(&mut list, SECOND, add(SECOND, "Second"));
    assert!(unpurged(&list).is_empty(), "nothing pruned yet");
    project.run(&mut list, FIRST, pruned());
    project.run(&mut list, SECOND, removed());
    assert_eq!(unpurged(&list), [FIRST], "a removal by name is no prune's");
    assert!(list.entries().is_empty());
    assert_eq!(list.unpurged()[0].status, "Done", "as it was pruned");

    let mut fresh = every_task();
    fresh
        .catch_up(&mut project.store, &project.registry)
        .unwrap();
    assert_eq!(unpurged(&fresh), [FIRST], "read back from the store");
    fresh
        .catch_up(&mut project.store, &project.registry)
        .unwrap();
    assert_eq!(unpurged(&fresh), [FIRST], "read again, still once");
}

#[test]
fn test_a_harp_minted_again_after_its_purge_is_no_longer_unpurged() {
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(&mut list, FIRST, pruned());
    annals::next::Purge::purge(&mut project.store, &cover(FIRST)).unwrap();
    project.run(&mut list, FIRST, add(FIRST, "Again"));
    assert!(unpurged(&list).is_empty(), "the stream holds a new task");
    assert_eq!(harps(&list), [FIRST]);
}

/// A store whose every stream reads back empty: what a stream purged
/// between a listing's `covers` and its read can look like.
struct Hollow(InMemoryStore);

impl annals::next::EventStore for Hollow {
    fn append(
        &mut self,
        stream: &Cover,
        expected: u64,
        pages: &[annals::next::v1::EventPage],
    ) -> Result<u64, annals::next::StoreError> {
        self.0.append(stream, expected, pages)
    }

    fn read_from(
        &mut self,
        stream: &Cover,
        _: u64,
    ) -> Result<Option<annals::next::Slice>, annals::next::StoreError> {
        Ok(Some(annals::next::Slice {
            stream: stream.clone(),
            next_sequence: 0,
            last_recorded_at: None,
            pages: Vec::new(),
        }))
    }

    fn find_command(
        &mut self,
        stream: &Cover,
        command_id: &Uuid,
    ) -> Result<Vec<annals::next::v1::EventPage>, annals::next::StoreError> {
        self.0.find_command(stream, command_id)
    }

    fn covers(&mut self, domain: &str) -> Result<Vec<Cover>, annals::next::StoreError> {
        self.0.covers(domain)
    }
}

/// Its read-only unit reads as hollowly.
impl annals::next::ReadStore for Hollow {
    type Reads<'s> = &'s mut Hollow;

    fn begin_read(&mut self) -> &mut Hollow {
        self
    }
}

impl annals::next::ReadUnit for &mut Hollow {
    fn covers(&mut self, domain: &str) -> Result<Vec<Cover>, annals::next::StoreError> {
        annals::next::EventStore::covers(*self, domain)
    }

    fn read_from(
        &mut self,
        stream: &Cover,
        from: u64,
    ) -> Result<Option<annals::next::Slice>, annals::next::StoreError> {
        annals::next::EventStore::read_from(*self, stream, from)
    }
}

#[test]
fn test_a_stream_that_replays_to_no_events_is_never_a_blank_task() {
    let mut project = Project::new();
    let mut live = every_task();
    project.run(&mut live, FIRST, add(FIRST, "First"));
    let mut store = Hollow(std::mem::take(&mut project.store));

    let mut list = every_task();
    list.catch_up(&mut store, &project.registry).unwrap();
    assert!(list.entries().is_empty(), "{:?}", list.entries());
    assert_eq!(list.sequence(), 0);

    let mut list = every_task();
    let mut store = Ready(store);
    ready(list.catch_up_async(&mut store, &project.registry)).unwrap();
    assert!(list.entries().is_empty(), "{:?}", list.entries());
}

// ------------------------------------------------------------ restarted streams

fn texts(list: &TaskList) -> Vec<&str> {
    list.entries().iter().map(|t| t.text.as_str()).collect()
}

/// A prune's removal, its purge and a task minted again on the freed harp,
/// all by another hand, in the store: what a long-lived list (a watch's)
/// can miss whole between two catch-ups.
fn prune_purge_and_mint_again(project: &mut Project, harp: &str, text: &str) {
    let mut elsewhere = every_task();
    project.run(&mut elsewhere, harp, pruned());
    annals::next::Purge::purge(&mut project.store, &cover(harp)).unwrap();
    project.run(&mut elsewhere, harp, add(harp, text));
}

#[test]
fn test_a_stream_pruned_purged_and_minted_again_between_catch_ups_lists_the_new_task() {
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(&mut list, FIRST, status("Done", ""));
    list.catch_up(&mut project.store, &project.registry)
        .unwrap();
    assert_eq!(list.sequence(), 2, "the add and its status");

    // The new stream is shorter than the old: its first event's sequence
    // is one the list has already passed.
    prune_purge_and_mint_again(&mut project, FIRST, "Again");
    list.catch_up(&mut project.store, &project.registry)
        .unwrap();
    assert_eq!(texts(&list), ["Again"], "the new task, not the pruned one");
    let deltas = list.since(2).unwrap();
    assert_eq!(deltas.len(), 1, "{deltas:?}");
    let Change::Upsert(task) = &deltas[0].change else {
        panic!("the new task: {deltas:?}");
    };
    assert_eq!(
        (task.text.as_str(), task.status.as_str()),
        ("Again", "To Do"),
        "not the old Done"
    );
    assert_eq!(deltas[0].sequence, 3);

    list.catch_up(&mut project.store, &project.registry)
        .unwrap();
    assert_eq!(list.sequence(), 3, "the new stream is now the one seen");
}

#[test]
fn test_an_async_catch_up_lists_a_task_minted_again_unseen() {
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(&mut list, FIRST, status("Done", ""));
    prune_purge_and_mint_again(&mut project, FIRST, "Again");
    let mut store = Ready(std::mem::take(&mut project.store));
    ready(list.catch_up_async(&mut store, &project.registry)).unwrap();
    assert_eq!(texts(&list), ["Again"]);
    assert_eq!(list.sequence(), 3);
}

#[test]
fn test_a_task_minted_again_outside_the_membership_leaves_the_list() {
    // The old task was listed and the new one is not: the old one goes.
    let mut project = Project::new();
    let mut list = deferred();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(&mut list, FIRST, status(DEFERRED, "later"));
    assert_eq!(harps(&list), [FIRST]);
    prune_purge_and_mint_again(&mut project, FIRST, "Again");
    list.catch_up(&mut project.store, &project.registry)
        .unwrap();
    assert!(list.entries().is_empty(), "{:?}", list.entries());
    assert_eq!(
        list.since(1).unwrap(),
        [Delta {
            sequence: 2,
            change: Change::Remove {
                harp_id: FIRST.to_string()
            }
        }]
    );
}

#[test]
fn test_an_append_seen_at_a_catch_up_is_no_restart() {
    // The stream read back from its start: its first event is the one
    // already seen, so only the new event is projected.
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    let mut elsewhere = every_task();
    project.run(&mut elsewhere, FIRST, status("In Progress", ""));
    list.catch_up(&mut project.store, &project.registry)
        .unwrap();
    let deltas = list.since(1).unwrap();
    assert_eq!(deltas.len(), 1, "only the status: {deltas:?}");
    let Change::Upsert(task) = &deltas[0].change else {
        panic!("{deltas:?}");
    };
    assert_eq!(task.status, "In Progress");
    list.catch_up(&mut project.store, &project.registry)
        .unwrap();
    assert_eq!(list.sequence(), 2, "nothing new");
}

#[test]
fn test_a_cursor_from_before_a_restart_resumes_to_the_new_task() {
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(&mut list, SECOND, add(SECOND, "Second"));
    let cursor = list.sequence();
    prune_purge_and_mint_again(&mut project, FIRST, "Again");
    project.run(&mut list, SECOND, status("Done", ""));
    list.catch_up(&mut project.store, &project.registry)
        .unwrap();

    let resumed = list.since(cursor).expect("a cursor the list issued");
    let changes: Vec<(&str, &str)> = resumed
        .iter()
        .map(|d| match &d.change {
            Change::Upsert(t) => (t.harp_id.as_str(), t.text.as_str()),
            Change::Remove { harp_id } => (harp_id.as_str(), "removed"),
        })
        .collect();
    assert_eq!(changes, [(SECOND, "Second"), (FIRST, "Again")]);
    assert_eq!(
        resumed.iter().map(|d| d.sequence).collect::<Vec<_>>(),
        [3, 4]
    );
}

#[test]
fn test_a_pruned_stream_read_again_before_its_purge_changes_nothing() {
    // A long-lived list that saw the prune's removal reads the stream
    // again before the purge lands: the same stream, already projected.
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(&mut list, FIRST, pruned());
    assert_eq!(list.sequence(), 2);
    list.catch_up(&mut project.store, &project.registry)
        .unwrap();
    assert_eq!(list.sequence(), 2, "nothing projected again");
    assert_eq!(unpurged(&list), [FIRST], "still pruned, not purged");
}

#[test]
fn test_a_redelivered_event_past_the_first_changes_nothing() {
    // Read back, the stream's middle event's state is not the task's now:
    // projected again, it would list the task as it was.
    let mut project = Project::new();
    let mut list = every_task();
    project.run(&mut list, FIRST, add(FIRST, "First"));
    project.run(&mut list, FIRST, status("In Progress", ""));
    project.run(&mut list, FIRST, status("Done", ""));
    list.catch_up(&mut project.store, &project.registry)
        .unwrap();
    assert_eq!(list.sequence(), 3);
    assert_eq!(list.entries()[0].status, "Done");
}
