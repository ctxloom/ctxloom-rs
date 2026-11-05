//! The scheduler aggregate, driven through its kernel as the binary drives it
//! (`Harness`, over an in-memory store).

use angzarr_diagnostic::{Arg, Diagnostic};
use angzarr_embedded::markup::DomainMessage;
use angzarr_embedded::testing::{Harness, Samples};
use angzarr_embedded::v1::{command_page, CommandBook, CommandPage, Cover, Fact, FactOrigin};
use angzarr_embedded::{Any, Failure, Handled, Payload, Registry, Timestamp};

use crate::{
    alarm_external_id, stream_for, AlarmArmed, AlarmCancelled, AlarmDue, AlarmFired, ArmAlarm,
    CancelAlarm, SchedulerAggregate, SchedulerCommand, SchedulerEvent, Stream, SCHEDULER_DOMAIN,
};

fn target() -> Stream {
    Stream {
        domain: "toy".to_string(),
        root: vec![7; 16],
        edition: None,
    }
}

fn at(seconds: i64) -> Timestamp {
    Timestamp { seconds, nanos: 0 }
}

fn cover_of(target: &Stream) -> Cover {
    Cover {
        domain: SCHEDULER_DOMAIN.to_string(),
        root: Some(stream_for(target)),
        edition: target.edition.clone(),
    }
}

fn arm(alarm_id: &str, seconds: i64) -> ArmAlarm {
    ArmAlarm {
        target: target(),
        alarm_id: alarm_id.to_string(),
        at: at(seconds),
    }
}

fn due(alarm_id: &str, seconds: i64) -> AlarmDue {
    AlarmDue {
        alarm_id: alarm_id.to_string(),
        at: at(seconds),
    }
}

/// A refusal's code and args.
type Refusal = (String, Vec<(String, String)>);

struct Scheduler {
    harness: Harness<SchedulerAggregate>,
}

impl Scheduler {
    fn for_target(target: &Stream) -> Scheduler {
        Scheduler::on(cover_of(target))
    }

    fn on(cover: Cover) -> Scheduler {
        let mut registry = Registry::new();
        registry
            .aggregate::<SchedulerAggregate>(SchedulerAggregate::default)
            .unwrap();
        Scheduler {
            harness: Harness::new(registry, cover).with_samples(Samples::starting_at(100)),
        }
    }

    fn new() -> Scheduler {
        Scheduler::for_target(&target())
    }

    /// The events a command recorded, or its refusal's code and args.
    fn command(
        &mut self,
        command: impl Into<SchedulerCommand>,
    ) -> Result<Vec<SchedulerEvent>, Refusal> {
        match self.harness.when(command).result() {
            Ok(events) => Ok(events),
            Err(Failure::Rejected(rejected)) => Err((
                rejected.code().to_string(),
                rejected
                    .args()
                    .into_iter()
                    .map(|(name, arg)| {
                        let text = match arg {
                            Arg::Str(text) => text,
                            other => format!("{other:?}"),
                        };
                        (name.to_string(), text)
                    })
                    .collect(),
            )),
            Err(other) => panic!("not a rejection: {other:?}"),
        }
    }

    fn fact(&mut self, fact: AlarmDue, external_id: &str) -> Handled {
        self.harness
            .when_book(fact_book(self.harness.cover().clone(), fact, external_id))
            .handled()
            .unwrap()
    }

    fn pages(&self) -> usize {
        self.harness.pages().len()
    }
}

fn fact_book(cover: Cover, fact: AlarmDue, external_id: &str) -> CommandBook {
    CommandBook {
        cover: Some(cover),
        page: Some(CommandPage {
            body: Some(command_page::Body::Fact(Fact {
                payload: Some(Any::from_msg(&fact.to_wire()).unwrap()),
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

fn armed(alarm_id: &str, seconds: i64) -> SchedulerEvent {
    AlarmArmed {
        target: target(),
        alarm_id: alarm_id.to_string(),
        at: at(seconds),
    }
    .into()
}

fn fired(alarm_id: &str, seconds: i64) -> SchedulerEvent {
    AlarmFired {
        target: target(),
        alarm_id: alarm_id.to_string(),
        at: at(seconds),
    }
    .into()
}

fn cancelled(alarm_id: &str) -> SchedulerEvent {
    AlarmCancelled {
        alarm_id: alarm_id.to_string(),
    }
    .into()
}

/// What a command recorded, as plain values to compare.
fn shown(events: &[SchedulerEvent]) -> Vec<String> {
    events.iter().map(|e| format!("{e:?}")).collect()
}

fn same(left: &[SchedulerEvent], right: &[SchedulerEvent]) -> bool {
    shown(left) == shown(right)
}

#[test]
fn arm_records_the_alarm_for_its_target() {
    let mut s = Scheduler::new();
    let events = s.command(arm("deadline", 10)).unwrap();
    assert!(same(&events, &[armed("deadline", 10)]));
    assert_eq!(s.harness.state().state().target, Some(target()));
}

#[test]
fn arm_addressed_to_another_targets_stream_is_refused_target_mismatch() {
    let mut other = target();
    other.root = vec![8; 16];
    // The stream is the scheduler stream for `target()`; the command names another.
    let mut s = Scheduler::new();
    let cmd = ArmAlarm {
        target: other.clone(),
        ..arm("deadline", 10)
    };
    let (code, args) = s.command(cmd).unwrap_err();
    assert_eq!(code, "scheduler-target-mismatch");
    assert_eq!(
        args,
        vec![
            (
                "stream".to_string(),
                crate::cover_text(&cover_of(&target()))
            ),
            ("target".to_string(), crate::stream_text(&other)),
        ]
    );
    assert_eq!(s.pages(), 0);
}

/// `arm` decided directly, in a stream at `cover`: an edition needs a fork
/// the kernel's store insists on, which this rule does not.
fn arm_in(cover: Cover, cmd: ArmAlarm) -> Result<Option<AlarmArmed>, String> {
    use angzarr_embedded::markup::{CommandContext, Context};
    let ctx = Context::new(at(1)).with_command(CommandContext {
        cover: Some(cover),
        ..CommandContext::default()
    });
    SchedulerAggregate::default()
        .arm(cmd, &ctx)
        .map_err(|failed| match failed {
            angzarr_embedded::markup::Failed::Rejected { rejection, .. } => {
                rejection.code().to_string()
            }
            other => format!("{other:?}"),
        })
}

#[test]
fn arm_in_another_edition_than_the_stream_is_refused_target_mismatch() {
    // The right root for the target, in an edition; the target is the main timeline's.
    let in_edition = Cover {
        edition: Some("what-if".to_string()),
        ..cover_of(&target())
    };
    assert_eq!(
        arm_in(in_edition, arm("deadline", 10)).unwrap_err(),
        "scheduler-target-mismatch"
    );
}

#[test]
fn arm_in_the_targets_own_edition_is_recorded() {
    let mut edition = target();
    edition.edition = Some("what-if".to_string());
    let cmd = ArmAlarm {
        target: edition.clone(),
        ..arm("deadline", 10)
    };
    assert!(arm_in(cover_of(&edition), cmd).unwrap().is_some());
}

#[test]
fn arm_with_an_empty_alarm_id_is_refused() {
    let mut s = Scheduler::new();
    let (code, args) = s.command(arm("", 10)).unwrap_err();
    assert_eq!(code, "scheduler-alarm-id-empty");
    assert!(args.is_empty());
}

#[test]
fn re_arm_moves_the_alarm_and_the_old_instant_is_never_fired() {
    let mut s = Scheduler::new();
    s.command(arm("deadline", 10)).unwrap();
    let moved = s.command(arm("deadline", 20)).unwrap();
    assert!(same(&moved, &[armed("deadline", 20)]));
    let old = s.fact(due("deadline", 10), "old");
    assert!(matches!(old, Handled::Applied(r) if r.books().iter().all(|b| b.pages.is_empty())));
    assert_eq!(s.pages(), 2);
    let new = s.fact(due("deadline", 20), "new");
    assert!(matches!(new, Handled::Applied(r) if r.books().iter().any(|b| b.pages.len() == 1)));
}

#[test]
fn re_arm_at_the_same_instant_records_nothing() {
    let mut s = Scheduler::new();
    s.command(arm("deadline", 10)).unwrap();
    assert!(s.command(arm("deadline", 10)).unwrap().is_empty());
}

#[test]
fn re_arm_of_a_reached_alarm_at_its_reached_instant_records_nothing() {
    let mut s = Scheduler::new();
    s.command(arm("deadline", 10)).unwrap();
    s.fact(due("deadline", 10), "a");
    assert!(s.command(arm("deadline", 10)).unwrap().is_empty());
}

#[test]
fn re_arm_of_a_reached_alarm_at_a_new_instant_arms_it() {
    let mut s = Scheduler::new();
    s.command(arm("deadline", 10)).unwrap();
    s.fact(due("deadline", 10), "a");
    let events = s.command(arm("deadline", 20)).unwrap();
    assert!(same(&events, &[armed("deadline", 20)]));
}

#[test]
fn re_arm_back_to_an_instant_already_reached_records_nothing() {
    // Reached at 10, moved to 20, moved back to 10: the clock's fact for
    // (deadline, 10) is already recorded, so a second would only be a
    // duplicate and the alarm would sit armed for ever.
    let mut s = Scheduler::new();
    s.command(arm("deadline", 10)).unwrap();
    s.fact(due("deadline", 10), "a");
    s.command(arm("deadline", 20)).unwrap();
    assert!(s.command(arm("deadline", 10)).unwrap().is_empty());
}

#[test]
fn re_arm_of_a_cancelled_alarm_arms_it_again() {
    let mut s = Scheduler::new();
    s.command(arm("deadline", 10)).unwrap();
    s.command(CancelAlarm {
        alarm_id: "deadline".to_string(),
    })
    .unwrap();
    let events = s.command(arm("deadline", 10)).unwrap();
    assert!(same(&events, &[armed("deadline", 10)]));
}

#[test]
fn cancel_of_an_armed_alarm_records_it() {
    let mut s = Scheduler::new();
    s.command(arm("deadline", 10)).unwrap();
    let events = s
        .command(CancelAlarm {
            alarm_id: "deadline".to_string(),
        })
        .unwrap();
    assert!(same(&events, &[cancelled("deadline")]));
}

#[test]
fn cancel_of_an_unarmed_alarm_records_nothing() {
    let mut s = Scheduler::new();
    let never = CancelAlarm {
        alarm_id: "deadline".to_string(),
    };
    assert!(s.command(never.clone()).unwrap().is_empty());
    s.command(arm("deadline", 10)).unwrap();
    s.command(never.clone()).unwrap();
    assert!(
        s.command(never.clone()).unwrap().is_empty(),
        "cancelled already"
    );
    s.command(arm("deadline", 10)).unwrap();
    s.fact(due("deadline", 10), "a");
    assert!(s.command(never).unwrap().is_empty(), "reached");
}

#[test]
fn alarm_due_at_the_armed_instant_records_alarm_fired() {
    let mut s = Scheduler::new();
    s.command(arm("deadline", 10)).unwrap();
    s.fact(due("deadline", 10), "a");
    let pages = s.harness.pages();
    let event = SchedulerEvent::from_any(pages[1].payload.as_ref().unwrap()).unwrap();
    assert!(same(&[event], &[fired("deadline", 10)]));
}

#[test]
fn alarm_due_for_a_moved_cancelled_or_reached_alarm_records_nothing() {
    let mut s = Scheduler::new();
    // Never armed.
    s.fact(due("deadline", 10), "never");
    assert_eq!(s.pages(), 0);
    s.command(arm("deadline", 10)).unwrap();
    // Moved.
    s.fact(due("deadline", 11), "moved");
    assert_eq!(s.pages(), 1);
    // Reached: a second, distinct fact for the same instant.
    s.fact(due("deadline", 10), "first");
    assert_eq!(s.pages(), 2);
    s.fact(due("deadline", 10), "second");
    assert_eq!(s.pages(), 2);
    // Cancelled.
    s.command(arm("other", 30)).unwrap();
    s.command(CancelAlarm {
        alarm_id: "other".to_string(),
    })
    .unwrap();
    let before = s.pages();
    s.fact(due("other", 30), "cancelled");
    assert_eq!(s.pages(), before);
}

#[test]
fn a_resent_alarm_due_is_duplicate() {
    let mut s = Scheduler::new();
    s.command(arm("deadline", 10)).unwrap();
    let id = alarm_external_id(&target(), "deadline", &at(10));
    assert!(matches!(
        s.fact(due("deadline", 10), &id),
        Handled::Applied(_)
    ));
    assert!(matches!(
        s.fact(due("deadline", 10), &id),
        Handled::Duplicate(..)
    ));
    assert_eq!(s.pages(), 2);
}
