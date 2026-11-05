//! The scheduler's loop, end to end and in process: a dummy domain records a
//! deadline, its policy arms an alarm, `tick(now)` fires it, and the delivery
//! policy hands the dummy `AlarmReached`, which it records. A fake clock, an
//! in-memory store, and nothing sleeps.

use std::time::Duration;

use angzarr_embedded::angzarr_ports::memory::MemoryStore;
use angzarr_embedded::markup::pack_message;
use angzarr_embedded::testing::{block_on_ready as ready, Samples};
use angzarr_embedded::v1::{
    command_page, CommandBook, CommandPage, Cover, EventPage, Fact, FactOrigin, Uuid,
};
use angzarr_embedded::{
    CommandBookExt as _, Handled, Kernel, OnRetryable, Payload, Registry, StreamRef, Timestamp,
};
use scheduler::{
    alarm_external_id, stream_for, AlarmDue, SchedulerAggregate, SchedulerEvent, Stream,
    SCHEDULER_DOMAIN,
};
use scheduler_fixture::{
    ArmsDeadline, Close, DeliverToDummy, DummyAggregate, DummyCommand, DummyEvent, SetDeadline,
    DEADLINE_ALARM, DUMMY_DOMAIN,
};
use scheduler_ports::{Clock as _, FakeClock, RecordingWait};

use crate::{diagnostic_samples, pause, LoopError, SchedulerLoop, Tick, EN_US};

fn at(seconds: i64) -> Timestamp {
    Timestamp { seconds, nanos: 0 }
}

fn dummy_root(n: u8) -> Uuid {
    Uuid::from_bytes([n; 16])
}

fn dummy_cover(n: u8) -> Cover {
    Cover {
        domain: DUMMY_DOMAIN.to_string(),
        root: Some(dummy_root(n)),
        edition: None,
    }
}

fn target(n: u8) -> Stream {
    Stream {
        domain: DUMMY_DOMAIN.to_string(),
        root: vec![n; 16],
        edition: None,
    }
}

struct World {
    registry: Registry,
    store: MemoryStore,
    samples: Samples,
}

impl World {
    fn new() -> World {
        let mut registry = Registry::new();
        registry
            .aggregate::<DummyAggregate>(DummyAggregate::default)
            .unwrap()
            .aggregate::<SchedulerAggregate>(SchedulerAggregate::default)
            .unwrap()
            .policy(ArmsDeadline)
            .unwrap()
            .policy(DeliverToDummy)
            .unwrap();
        World {
            registry,
            store: MemoryStore::new(),
            samples: Samples::starting_at(1_000),
        }
    }

    fn kernel(&self) -> Kernel<'_> {
        Kernel::new(&self.registry).on_retryable(OnRetryable {
            max_retries: 0,
            ..OnRetryable::default()
        })
    }

    /// A command to dummy `n`.
    fn send(&mut self, n: u8, command: impl Into<DummyCommand>) {
        self.send_correlated(n, command, None);
    }

    fn send_correlated(
        &mut self,
        n: u8,
        command: impl Into<DummyCommand>,
        correlation: Option<Uuid>,
    ) {
        let mut book = CommandBook::command_of(dummy_cover(n), &command.into());
        if let Some(page) = book.page.as_mut() {
            page.correlation_id = correlation;
        }
        let sampled = self.samples.next_sample();
        ready(
            self.kernel()
                .handle::<DummyAggregate, _>(&self.store, book, &sampled),
        )
        .unwrap_or_else(|failure| panic!("{failure:?}"));
    }

    fn set_deadline(&mut self, n: u8, seconds: i64) {
        self.send(
            n,
            SetDeadline {
                deadline: at(seconds),
            },
        );
    }

    fn tick(&self, scheduler: &mut SchedulerLoop, now: i64) -> Tick {
        ready(scheduler.tick(&self.kernel(), &self.store, at(now))).unwrap()
    }

    fn pages(&self, cover: &Cover) -> Vec<EventPage> {
        let stream = StreamRef::try_from(cover).unwrap();
        ready(angzarr_embedded::EventStore::read(
            &self.store,
            stream,
            0,
            None,
        ))
        .unwrap()
        .pages
    }

    /// Dummy `n`'s events, in order.
    fn dummy(&self, n: u8) -> Vec<DummyEvent> {
        self.pages(&dummy_cover(n))
            .iter()
            .map(|p| DummyEvent::from_any(p.payload.as_ref().unwrap()).unwrap())
            .collect()
    }

    /// The seconds of each `DeadlinePassed` dummy `n` recorded.
    fn passed(&self, n: u8) -> Vec<i64> {
        self.dummy(n)
            .into_iter()
            .filter_map(|e| match e {
                DummyEvent::DeadlinePassed(passed) => Some(passed.at.seconds),
                _ => None,
            })
            .collect()
    }

    fn scheduler_cover(&self, n: u8) -> Cover {
        Cover {
            domain: SCHEDULER_DOMAIN.to_string(),
            root: Some(stream_for(&target(n))),
            edition: None,
        }
    }

    fn scheduler_events(&self, n: u8) -> Vec<SchedulerEvent> {
        self.pages(&self.scheduler_cover(n))
            .iter()
            .map(|p| SchedulerEvent::from_any(p.payload.as_ref().unwrap()).unwrap())
            .collect()
    }
}

fn fired_of(tick: &Tick) -> Vec<(u8, i64)> {
    tick.fired()
        .iter()
        .map(|alarm| (alarm.target.root[0], alarm.at.seconds))
        .collect()
}

#[test]
fn tick_before_at_fires_nothing() {
    let mut w = World::new();
    let mut scheduler = SchedulerLoop::new();
    w.set_deadline(1, 100);
    let tick = w.tick(&mut scheduler, 99);
    assert!(tick.sent.is_empty() && tick.failed.is_empty());
    assert_eq!(tick.next, Some(at(100)));
    assert_eq!(w.passed(1), Vec::<i64>::new());
}

#[test]
fn tick_at_at_fires_once_and_the_target_records_the_deadline_it_asked_for() {
    let mut w = World::new();
    let mut scheduler = SchedulerLoop::new();
    w.set_deadline(1, 100);
    let tick = w.tick(&mut scheduler, 100);
    assert_eq!(fired_of(&tick), vec![(1, 100)]);
    assert_eq!(tick.next, None);
    assert_eq!(w.passed(1), vec![100]);
    // The scheduler's own record: armed, then fired.
    assert!(matches!(
        w.scheduler_events(1).as_slice(),
        [SchedulerEvent::AlarmArmed(_), SchedulerEvent::AlarmFired(_)]
    ));
    // Nothing fires twice, however many ticks follow.
    for now in [100, 101, 5_000] {
        let again = w.tick(&mut scheduler, now);
        assert!(again.sent.is_empty(), "now={now}");
    }
    assert_eq!(w.passed(1), vec![100]);
}

#[test]
fn an_alarm_armed_in_the_past_fires_on_the_next_tick() {
    let mut w = World::new();
    let mut scheduler = SchedulerLoop::new();
    w.set_deadline(1, 100);
    assert_eq!(fired_of(&w.tick(&mut scheduler, 100_000)), vec![(1, 100)]);
    assert_eq!(
        w.passed(1),
        vec![100],
        "the armed instant, not the clock's reading"
    );
}

#[test]
fn alarms_fire_in_instant_order_each_with_its_own_target() {
    let mut w = World::new();
    let mut scheduler = SchedulerLoop::new();
    w.set_deadline(1, 300);
    w.set_deadline(2, 100);
    w.set_deadline(3, 200);
    let tick = w.tick(&mut scheduler, 1_000);
    assert_eq!(fired_of(&tick), vec![(2, 100), (3, 200), (1, 300)]);
    assert_eq!(
        (w.passed(1), w.passed(2), w.passed(3)),
        (vec![300], vec![100], vec![200])
    );
}

fn due_book(n: u8, seconds: i64) -> (CommandBook, String) {
    let id = alarm_external_id(&target(n), DEADLINE_ALARM, &at(seconds));
    let book = CommandBook {
        cover: Some(World::new().scheduler_cover(n)),
        page: Some(CommandPage {
            body: Some(command_page::Body::Fact(Fact {
                payload: Some(pack_message(&AlarmDue {
                    alarm_id: DEADLINE_ALARM.to_string(),
                    at: at(seconds),
                })),
            })),
            fact_origin: Some(FactOrigin {
                external_id: id.clone(),
                description: String::new(),
            }),
            ..CommandPage::default()
        }),
        handling: None,
    };
    (book, id)
}

#[test]
fn a_resent_alarm_due_is_deduplicated_by_its_id_and_delivers_nothing_more() {
    let mut w = World::new();
    let mut scheduler = SchedulerLoop::new();
    w.set_deadline(1, 100);
    w.tick(&mut scheduler, 100);
    let before = (w.scheduler_events(1).len(), w.dummy(1).len());
    // The same fact again, as a second process or a replay would send it.
    let (book, id) = due_book(1, 100);
    assert_eq!(id, format!("alarm:dummy/01010101-0101-0101-0101-010101010101/deadline@1970-01-01T00:01:40.000000000Z"));
    let sampled = w.samples.next_sample();
    let handled = ready(w.kernel().handle_any(&w.store, book, &sampled)).unwrap();
    assert!(matches!(handled, Handled::Duplicate(..)));
    assert_eq!((w.scheduler_events(1).len(), w.dummy(1).len()), before);
}

#[test]
fn a_fact_for_an_alarm_that_is_not_armed_records_nothing_and_is_not_a_firing() {
    let mut w = World::new();
    w.set_deadline(1, 100);
    // Due at an instant it was never armed at.
    let (book, _) = due_book(1, 7);
    let sampled = w.samples.next_sample();
    let handled = ready(w.kernel().handle_any(&w.store, book, &sampled)).unwrap();
    assert!(matches!(handled, Handled::Applied(_)));
    assert_eq!(w.scheduler_events(1).len(), 1);
    assert_eq!(w.passed(1), Vec::<i64>::new());
}

#[test]
fn a_restarted_loop_fires_nothing_twice() {
    let mut w = World::new();
    let mut first = SchedulerLoop::new();
    w.set_deadline(1, 100);
    w.tick(&mut first, 100);
    drop(first);
    // A fresh model over the same store.
    let mut second = SchedulerLoop::new();
    let tick = w.tick(&mut second, 200);
    assert!(tick.sent.is_empty());
    assert_eq!(w.passed(1), vec![100]);
}

#[test]
fn fires_missed_while_down_fire_once_each_in_at_order_with_the_armed_at() {
    let mut w = World::new();
    let mut before_the_outage = SchedulerLoop::new();
    w.set_deadline(1, 300);
    w.set_deadline(2, 100);
    w.set_deadline(3, 200);
    // Up: nothing is due yet.
    assert!(w.tick(&mut before_the_outage, 50).sent.is_empty());
    drop(before_the_outage);
    // Down through all three instants; back up long after.
    let mut after = SchedulerLoop::new();
    let tick = w.tick(&mut after, 10_000);
    assert_eq!(fired_of(&tick), vec![(2, 100), (3, 200), (1, 300)]);
    assert_eq!(
        (w.passed(1), w.passed(2), w.passed(3)),
        (vec![300], vec![100], vec![200])
    );
    // And once is all: another restart, another tick.
    let mut third = SchedulerLoop::new();
    assert!(w.tick(&mut third, 20_000).sent.is_empty());
    assert_eq!(
        (w.passed(1), w.passed(2), w.passed(3)),
        (vec![300], vec![100], vec![200])
    );
}

#[test]
fn a_cancel_before_due_prevents_the_fire() {
    let mut w = World::new();
    let mut scheduler = SchedulerLoop::new();
    w.set_deadline(1, 100);
    w.send(1, Close {});
    assert!(matches!(
        w.scheduler_events(1).as_slice(),
        [
            SchedulerEvent::AlarmArmed(_),
            SchedulerEvent::AlarmCancelled(_)
        ]
    ));
    let tick = w.tick(&mut scheduler, 1_000);
    assert!(tick.sent.is_empty());
    assert_eq!(tick.next, None);
    assert_eq!(w.passed(1), Vec::<i64>::new());
}

#[test]
fn re_arming_moves_the_alarm_and_the_old_instant_is_never_reached() {
    let mut w = World::new();
    let mut scheduler = SchedulerLoop::new();
    w.set_deadline(1, 100);
    w.set_deadline(1, 300);
    assert!(
        w.tick(&mut scheduler, 150).sent.is_empty(),
        "the old instant is gone"
    );
    assert_eq!(w.tick(&mut scheduler, 150).next, Some(at(300)));
    assert_eq!(fired_of(&w.tick(&mut scheduler, 300)), vec![(1, 300)]);
    assert_eq!(w.passed(1), vec![300]);
}

#[test]
fn the_target_rechecks_a_reached_alarm_against_its_own_deadline() {
    // The scheduler fires (deadline 100) but the dummy moved its deadline
    // after: the dummy's own state no longer matches, so it records nothing.
    let mut w = World::new();
    let mut scheduler = SchedulerLoop::new();
    w.set_deadline(1, 100);
    // Prime the loop's model on the 100 alarm, then move the deadline in the
    // dummy without the loop seeing it, and fire the stale instant directly.
    w.tick(&mut scheduler, 0);
    w.set_deadline(1, 500);
    let (book, _) = due_book(1, 100);
    let sampled = w.samples.next_sample();
    ready(w.kernel().handle_any(&w.store, book, &sampled)).unwrap();
    assert_eq!(w.passed(1), Vec::<i64>::new());
}

#[test]
fn the_arming_correlation_runs_through_the_timer_to_the_target() {
    let mut w = World::new();
    let mut scheduler = SchedulerLoop::new();
    let correlation = Uuid::from_bytes([0xc0; 16]);
    w.send_correlated(
        1,
        SetDeadline { deadline: at(100) },
        Some(correlation.clone()),
    );
    w.tick(&mut scheduler, 100);
    let correlation_of = |page: &EventPage| page.meta.as_ref().unwrap().correlation_id.clone();
    let armed = w.pages(&w.scheduler_cover(1));
    assert_eq!(armed.len(), 2);
    assert_eq!(
        correlation_of(&armed[0]),
        Some(correlation.clone()),
        "arming"
    );
    assert_eq!(
        correlation_of(&armed[1]),
        Some(correlation.clone()),
        "firing"
    );
    let dummy = w.pages(&dummy_cover(1));
    assert_eq!(correlation_of(&dummy[1]), Some(correlation), "delivered");
}

#[test]
fn an_alarm_whose_fact_fails_is_reported_and_stays_due() {
    // A fact the kernel cannot route fails: reported, not swallowed.
    let mut w = World::new();
    let mut scheduler = SchedulerLoop::new();
    w.set_deadline(1, 100);
    // A kernel with no scheduler registered: the fact's domain is unknown.
    let mut bare = Registry::new();
    bare.aggregate::<DummyAggregate>(DummyAggregate::default)
        .unwrap();
    let kernel = Kernel::new(&bare);
    // Catch the model up through the real registry first.
    w.tick(&mut scheduler, 0);
    let tick = ready(scheduler.tick(&kernel, &w.store, at(100))).unwrap();
    assert_eq!(tick.failed.len(), 1);
    assert!(!tick.failed[0].1.is_empty(), "the failure's code");
    assert!(tick.sent.is_empty());
    // Still due on the next tick, through the real registry.
    assert_eq!(fired_of(&w.tick(&mut scheduler, 100)), vec![(1, 100)]);
}

#[test]
fn run_waits_until_the_next_due_or_the_poll_whichever_is_sooner() {
    let mut w = World::new();
    let mut scheduler = SchedulerLoop::new();
    w.set_deadline(1, 100);
    let clock = FakeClock::at(0);
    let wait = RecordingWait::advancing(&clock);
    let mut ticks = 0;
    ready(scheduler.run(
        &w.kernel(),
        &w.store,
        &clock,
        &wait,
        Duration::from_secs(40),
        |tick| {
            ticks += 1;
            tick.fired().is_empty()
        },
    ))
    .unwrap();
    // 0 -> 40 -> 80 -> 100 (the next due is sooner than a poll) -> fired.
    assert_eq!(
        wait.waits(),
        vec![
            Duration::from_secs(40),
            Duration::from_secs(40),
            Duration::from_secs(20)
        ]
    );
    assert_eq!(ticks, 4);
    assert_eq!(clock.now(), at(100));
    assert_eq!(w.passed(1), vec![100]);
}

#[test]
fn run_polls_every_when_nothing_is_armed() {
    let w = World::new();
    let mut scheduler = SchedulerLoop::new();
    let clock = FakeClock::at(0);
    let wait = RecordingWait::advancing(&clock);
    let mut ticks = 0;
    ready(scheduler.run(
        &w.kernel(),
        &w.store,
        &clock,
        &wait,
        Duration::from_secs(7),
        |_| {
            ticks += 1;
            ticks < 3
        },
    ))
    .unwrap();
    assert_eq!(
        wait.waits(),
        vec![Duration::from_secs(7), Duration::from_secs(7)]
    );
}

#[test]
fn pause_is_the_time_to_the_next_instant_capped_at_the_poll() {
    let every = Duration::from_secs(30);
    let now = Timestamp {
        seconds: 10,
        nanos: 500_000_000,
    };
    assert_eq!(pause(&now, None, every), every);
    assert_eq!(
        pause(
            &now,
            Some(&Timestamp {
                seconds: 20,
                nanos: 250_000_000
            }),
            every
        ),
        Duration::new(9, 750_000_000)
    );
    assert_eq!(
        pause(
            &now,
            Some(&Timestamp {
                seconds: 20,
                nanos: 900_000_000
            }),
            every
        ),
        Duration::new(10, 400_000_000)
    );
    assert_eq!(pause(&now, Some(&at(1_000)), every), every);
    // Not after `now` (never from the model, which asks for strictly later):
    // no wait at all, not a negative one.
    assert_eq!(pause(&now, Some(&at(3)), every), Duration::ZERO);
}

#[test]
fn a_stream_the_model_cannot_read_is_reported_and_the_others_still_fire() {
    use angzarr_embedded::angzarr_ports::{EventStore as _, Write};
    use angzarr_embedded::v1::EventMeta;
    let mut w = World::new();
    let mut scheduler = SchedulerLoop::new();
    w.set_deadline(1, 100);
    // A scheduler stream holding an event the scheduler cannot read.
    let broken = w.scheduler_cover(2);
    let stranger = EventPage {
        sequence: 0,
        payload: Some(angzarr_embedded::Any {
            type_url: "/toy.v1.Stranger".to_string(),
            value: vec![],
        }),
        meta: Some(EventMeta::default()),
    };
    let stream = StreamRef::try_from(&broken).unwrap();
    ready(w.store.write(Write::append(stream, 0, &[stranger]))).unwrap();
    let tick = w.tick(&mut scheduler, 100);
    assert_eq!(tick.unread, vec![broken]);
    assert_eq!(fired_of(&tick), vec![(1, 100)]);
}

#[test]
fn a_crash_between_the_firing_and_its_delivery_is_delivered_once_by_resume() {
    // The scheduler records AlarmFired but no delivery policy is registered
    // (the process died before it ran, or the policy is new).
    let mut w = World::new();
    w.set_deadline(1, 100);
    let mut without = Registry::new();
    without
        .aggregate::<DummyAggregate>(DummyAggregate::default)
        .unwrap()
        .aggregate::<SchedulerAggregate>(SchedulerAggregate::default)
        .unwrap();
    let mut scheduler = SchedulerLoop::new();
    let fired = ready(scheduler.tick(&Kernel::new(&without), &w.store, at(100))).unwrap();
    assert_eq!(fired_of(&fired), vec![(1, 100)]);
    assert_eq!(w.passed(1), Vec::<i64>::new(), "fired, not yet delivered");
    // The next process: the full registry, resumed.
    let sampled = w.samples.next_sample();
    ready(w.kernel().resume(&w.store, &sampled)).unwrap();
    assert_eq!(w.passed(1), vec![100]);
    // Resuming again, and a fresh loop's tick, deliver nothing more.
    let sampled = w.samples.next_sample();
    ready(w.kernel().resume(&w.store, &sampled)).unwrap();
    assert!(w.tick(&mut SchedulerLoop::new(), 200).sent.is_empty());
    assert_eq!(w.passed(1), vec![100]);
}

// The crate's catalog test (the binary's holds the same for the whole graph).
#[test]
fn samples_are_not_empty_and_every_diagnostic_has_a_message_that_uses_exactly_its_args() {
    let (name, catalog) = EN_US;
    assert_eq!(name, "scheduler-loop");
    assert_eq!(diagnostic_samples().len(), 1);
    for sample in diagnostic_samples() {
        let code = sample.code();
        assert!(code.starts_with("scheduler-loop-"), "{code} is prefixed");
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
        assert_eq!(args, used, "{code} uses exactly its args");
    }
    assert_eq!(
        LoopError::CatchUpFailed {
            reason: "x".to_string()
        }
        .to_string(),
        "scheduler-loop-catch-up-failed reason=x"
    );
}
