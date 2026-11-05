//! The due model on the projector port, caught up: each command runs through
//! angzarr-embedded's kernel over angzarr-ports' in-memory store, and a
//! model kept across commands catches up after each of them; a fresh model
//! catches up from the store, as another process's does.

use angzarr_embedded::angzarr_ports::memory::MemoryStore;
use angzarr_embedded::markup::DomainMessage;
use angzarr_embedded::testing::block_on_ready as ready;
use angzarr_embedded::v1::{
    command_page, Command, CommandBook, CommandPage, Cover, EventBook, EventMeta, EventPage, Fact,
    FactOrigin, Uuid,
};
use angzarr_embedded::{
    Any, CatchUp, EventStore, Kernel, OnRetryable, Payload, Purge, Registry, Sampled, StreamRef,
    Timestamp,
};
use scheduler::{
    alarm_external_id, stream_for, AlarmArmed, AlarmDue, AlarmFired, ArmAlarm, CancelAlarm,
    SchedulerAggregate, SchedulerEvent, Stream, SCHEDULER_DOMAIN,
};

use crate::{catch_up, Due, DueAlarm, DueModel};

type Model = CatchUp<DueModel>;

fn at(seconds: i64) -> Timestamp {
    Timestamp { seconds, nanos: 0 }
}

fn target(id: u8) -> Stream {
    Stream {
        domain: "toy".to_string(),
        root: vec![id; 16],
        edition: None,
    }
}

fn cover(target: &Stream) -> Cover {
    Cover {
        domain: SCHEDULER_DOMAIN.to_string(),
        root: Some(stream_for(target)),
        edition: None,
    }
}

struct Project {
    store: MemoryStore,
    registry: Registry,
    seconds: i64,
}

impl Project {
    fn new() -> Project {
        let mut registry = Registry::new();
        registry
            .aggregate::<SchedulerAggregate>(SchedulerAggregate::default)
            .unwrap();
        Project {
            store: MemoryStore::new(),
            registry,
            seconds: 100,
        }
    }

    fn run(&mut self, model: &mut Model, book: CommandBook) {
        self.seconds += 1;
        let sampled = Sampled {
            now: at(self.seconds),
            command_id: Uuid::from_bytes([self.seconds as u8; 16]),
        };
        ready(
            Kernel::new(&self.registry)
                .on_retryable(OnRetryable {
                    max_retries: 0,
                    ..OnRetryable::default()
                })
                .handle::<SchedulerAggregate, _>(&self.store, book, &sampled),
        )
        .unwrap_or_else(|failure| panic!("{failure:?}"));
        self.catch_up(model);
    }

    fn arm(&mut self, model: &mut Model, target_id: u8, alarm_id: &str, seconds: i64) {
        let target = target(target_id);
        let cmd = ArmAlarm {
            target: target.clone(),
            alarm_id: alarm_id.to_string(),
            at: at(seconds),
        };
        self.run(model, command(cover(&target), cmd));
    }

    fn cancel(&mut self, model: &mut Model, target_id: u8, alarm_id: &str) {
        let cmd = CancelAlarm {
            alarm_id: alarm_id.to_string(),
        };
        self.run(model, command(cover(&target(target_id)), cmd));
    }

    fn fire(&mut self, model: &mut Model, target_id: u8, alarm_id: &str, seconds: i64) {
        let target = target(target_id);
        let due = AlarmDue {
            alarm_id: alarm_id.to_string(),
            at: at(seconds),
        };
        let id = alarm_external_id(&target, alarm_id, &at(seconds));
        self.run(model, fact(cover(&target), due, &id));
    }

    fn catch_up(&self, model: &mut Model) {
        let caught = ready(model.step(&self.store, self.registry.catalog())).unwrap();
        assert!(caught.failures().is_empty(), "{:?}", caught.failures());
    }

    fn rebuilt(&self) -> Model {
        let mut model = catch_up();
        self.catch_up(&mut model);
        model
    }
}

fn command(cover: Cover, message: impl DomainMessage) -> CommandBook {
    CommandBook {
        cover: Some(cover),
        page: Some(CommandPage {
            body: Some(command_page::Body::Command(Command {
                payload: Some(Any::from_msg(&message.to_wire()).unwrap()),
            })),
            ..CommandPage::default()
        }),
        handling: None,
    }
}

fn fact(cover: Cover, message: impl DomainMessage, external_id: &str) -> CommandBook {
    CommandBook {
        cover: Some(cover),
        page: Some(CommandPage {
            body: Some(command_page::Body::Fact(Fact {
                payload: Some(Any::from_msg(&message.to_wire()).unwrap()),
            })),
            fact_origin: Some(FactOrigin {
                external_id: external_id.to_string(),
                description: String::new(),
            }),
            ..CommandPage::default()
        }),
        handling: None,
    }
}

/// The alarms due at `now`, as (target id, alarm id, seconds).
fn due(model: &Model, now: i64) -> Vec<(u8, String, i64)> {
    model
        .projector()
        .due()
        .due_at_or_before(&at(now))
        .into_iter()
        .map(|a| (a.target.root[0], a.alarm_id, a.at.seconds))
        .collect()
}

#[test]
fn due_lists_armed_alarms_at_or_before_now_in_at_order() {
    let mut p = Project::new();
    let mut m = catch_up();
    p.arm(&mut m, 2, "b", 30);
    p.arm(&mut m, 1, "late", 99);
    p.arm(&mut m, 1, "z", 20);
    p.arm(&mut m, 1, "a", 20);
    p.arm(&mut m, 3, "c", 10);
    assert_eq!(
        due(&m, 30),
        vec![
            (3, "c".to_string(), 10),
            (1, "a".to_string(), 20),
            (1, "z".to_string(), 20),
            (2, "b".to_string(), 30),
        ]
    );
    assert_eq!(due(&m, 9), vec![]);
    assert_eq!(due(&m, 10).len(), 1, "an alarm is due AT its instant");
}

#[test]
fn due_breaks_an_instant_tie_by_target_text_then_alarm_id() {
    let mut p = Project::new();
    let mut m = catch_up();
    p.arm(&mut m, 2, "a", 10);
    p.arm(&mut m, 1, "b", 10);
    p.arm(&mut m, 1, "a", 10);
    let order: Vec<(u8, String)> = due(&m, 10).into_iter().map(|(t, a, _)| (t, a)).collect();
    assert_eq!(
        order,
        vec![
            (1, "a".to_string()),
            (1, "b".to_string()),
            (2, "a".to_string())
        ]
    );
}

#[test]
fn due_forgets_cancelled_reached_and_moved_instants() {
    let mut p = Project::new();
    let mut m = catch_up();
    p.arm(&mut m, 1, "cancelled", 10);
    p.arm(&mut m, 1, "reached", 10);
    p.arm(&mut m, 1, "moved", 10);
    p.arm(&mut m, 1, "kept", 10);
    p.cancel(&mut m, 1, "cancelled");
    p.fire(&mut m, 1, "reached", 10);
    p.arm(&mut m, 1, "moved", 50);
    assert_eq!(
        due(&m, 10),
        vec![(1, "kept".to_string(), 10)],
        "only the kept one is due at 10"
    );
    assert_eq!(
        due(&m, 50),
        vec![(1, "kept".to_string(), 10), (1, "moved".to_string(), 50)]
    );
}

#[test]
fn due_next_after_is_the_earliest_future_at() {
    let mut p = Project::new();
    let mut m = catch_up();
    assert_eq!(m.projector().due().next_after(&at(0)), None);
    p.arm(&mut m, 1, "a", 40);
    p.arm(&mut m, 2, "b", 20);
    p.arm(&mut m, 3, "c", 10);
    let next = |now: i64| m.projector().due().next_after(&at(now)).map(|t| t.seconds);
    assert_eq!(next(0), Some(10));
    assert_eq!(next(10), Some(20), "an instant reached is not 'next'");
    assert_eq!(next(25), Some(40));
    assert_eq!(next(40), None);
}

#[test]
fn a_fresh_model_rebuilt_from_the_store_holds_what_a_kept_one_does() {
    let mut p = Project::new();
    let mut kept = catch_up();
    p.arm(&mut kept, 1, "a", 10);
    p.arm(&mut kept, 1, "b", 20);
    p.fire(&mut kept, 1, "a", 10);
    p.arm(&mut kept, 2, "c", 5);
    let fresh = p.rebuilt();
    assert_eq!(due(&fresh, 100), due(&kept, 100));
    assert_eq!(due(&fresh, 100).len(), 2);
}

#[test]
fn a_purged_stream_leaves_the_model() {
    let mut p = Project::new();
    let mut m = catch_up();
    p.arm(&mut m, 1, "a", 10);
    p.arm(&mut m, 2, "b", 10);
    let purged = cover(&target(1));
    let stream = StreamRef::try_from(&purged).unwrap();
    let head = ready(EventStore::head(&p.store, stream)).unwrap();
    ready(Purge::purge(&p.store, stream, head.next_sequence)).unwrap();
    p.catch_up(&mut m);
    assert_eq!(due(&m, 10), vec![(2, "b".to_string(), 10)]);
}

#[test]
fn due_keeps_the_correlation_of_the_arming_event() {
    let mut p = Project::new();
    let mut m = catch_up();
    p.arm(&mut m, 1, "a", 10);
    let mut alarms = m.projector().due().due_at_or_before(&at(10));
    let alarm: DueAlarm = alarms.remove(0);
    // The kernel sets an uncorrelated command's correlation to its command id.
    assert_eq!(alarm.correlation, Some(Uuid::from_bytes([101; 16])));
    assert_eq!(alarm.target, target(1));
}

fn page(sequence: u64, event: SchedulerEvent, correlation: u8) -> EventPage {
    EventPage {
        sequence,
        payload: Some(event.to_any()),
        meta: Some(EventMeta {
            correlation_id: Some(Uuid::from_bytes([correlation; 16])),
            ..EventMeta::default()
        }),
    }
}

fn book(pages: Vec<EventPage>, next: u64) -> EventBook {
    EventBook {
        cover: Some(cover(&target(1))),
        pages,
        next_sequence: next,
        snapshot: None,
    }
}

fn armed(alarm_id: &str, seconds: i64) -> SchedulerEvent {
    AlarmArmed {
        target: target(1),
        alarm_id: alarm_id.to_string(),
        at: at(seconds),
    }
    .into()
}

#[test]
fn a_book_from_sequence_zero_starts_the_stream_afresh() {
    let mut due = Due::new();
    due.project(&book(vec![page(0, armed("old", 10), 1)], 1))
        .unwrap();
    due.project(&book(vec![page(0, armed("new", 20), 2)], 1))
        .unwrap();
    let alarms = due.due_at_or_before(&at(100));
    assert_eq!(alarms.len(), 1);
    assert_eq!(alarms[0].alarm_id, "new");
}

#[test]
fn a_later_book_adds_to_the_stream() {
    let mut due = Due::new();
    due.project(&book(vec![page(0, armed("a", 10), 1)], 1))
        .unwrap();
    due.project(&book(vec![page(1, armed("b", 20), 2)], 2))
        .unwrap();
    assert_eq!(due.due_at_or_before(&at(100)).len(), 2);
}

#[test]
fn an_empty_book_at_zero_forgets_the_stream_and_an_empty_one_after_it_does_not() {
    let mut due = Due::new();
    due.project(&book(vec![page(0, armed("a", 10), 1)], 1))
        .unwrap();
    due.project(&book(vec![], 1)).unwrap();
    assert_eq!(
        due.due_at_or_before(&at(100)).len(),
        1,
        "nothing new is not gone"
    );
    due.project(&book(vec![], 0)).unwrap();
    assert_eq!(due.due_at_or_before(&at(100)).len(), 0);
}

#[test]
fn a_firing_removes_the_alarm_it_names() {
    let mut due = Due::new();
    let fired = AlarmFired {
        target: target(1),
        alarm_id: "a".to_string(),
        at: at(10),
    };
    due.project(&book(
        vec![page(0, armed("a", 10), 1), page(1, fired.into(), 1)],
        2,
    ))
    .unwrap();
    assert!(due.due_at_or_before(&at(100)).is_empty());
}

#[test]
fn an_event_the_scheduler_cannot_read_is_the_projectors_bug() {
    let mut due = Due::new();
    let stranger = EventPage {
        sequence: 0,
        payload: Some(Any {
            type_url: "/toy.v1.Stranger".to_string(),
            value: vec![],
        }),
        meta: None,
    };
    assert!(due.project(&book(vec![stranger], 1)).is_err());
}

#[test]
fn the_model_is_named_and_reads_the_scheduler_domain() {
    use angzarr_embedded::ProjectorHandler;
    assert_eq!(DueModel::default().name(), "scheduler-due");
    let _: Model = catch_up();
}

#[test]
fn an_unreadable_event_fails_the_projector_with_a_code_and_the_type_url() {
    use angzarr_embedded::ProjectorHandler;
    let model = DueModel::default();
    let stranger = EventPage {
        sequence: 0,
        payload: Some(Any {
            type_url: "/toy.v1.Stranger".to_string(),
            value: vec![],
        }),
        meta: None,
    };
    let error = ready(model.dispatch(&book(vec![stranger], 1))).unwrap_err();
    assert!(!error.code.is_empty());
    assert!(
        error.extras.values().any(|v| v.contains("toy.v1.Stranger")),
        "{:?}",
        error.extras
    );
}
