use angzarr_embedded::markup::Policy;

use crate::{ArmsDeadline, DeliverToDummy};

#[test]
fn delivery_name_is_the_consumers_package_and_type() {
    // From the INVOKING crate's package, not the scheduler's: it does not
    // change when the scheduler is lifted.
    assert_eq!(
        <DeliverToDummy as Policy>::NAME,
        "scheduler-fixture::DummyAggregate::alarms"
    );
    assert_ne!(
        <DeliverToDummy as Policy>::NAME,
        <ArmsDeadline as Policy>::NAME
    );
}

// The dummy's own rules, through the kernel: the scheduler's tests prove the
// path, these pin the dummy's re-check so a wrong dummy cannot hide behind it.
mod dummy {
    use angzarr_embedded::markup::DomainMessage;
    use angzarr_embedded::testing::{Harness, Samples};
    use angzarr_embedded::v1::{
        command_page, CommandBook, CommandPage, Cover, Fact, FactOrigin, Uuid,
    };
    use angzarr_embedded::{Any, Registry, Timestamp};
    use scheduler::AlarmReached;

    use crate::{Close, DummyAggregate, DummyEvent, SetDeadline, DEADLINE_ALARM, DUMMY_DOMAIN};

    fn at(seconds: i64) -> Timestamp {
        Timestamp { seconds, nanos: 0 }
    }

    fn harness() -> Harness<DummyAggregate> {
        let mut registry = Registry::new();
        registry
            .aggregate::<DummyAggregate>(DummyAggregate::default)
            .unwrap();
        let cover = Cover {
            domain: DUMMY_DOMAIN.to_string(),
            root: Some(Uuid::from_bytes([1; 16])),
            edition: None,
        };
        Harness::new(registry, cover).with_samples(Samples::starting_at(10))
    }

    fn reach(h: &mut Harness<DummyAggregate>, alarm_id: &str, seconds: i64, id: &str) -> usize {
        let fact = AlarmReached {
            alarm_id: alarm_id.to_string(),
            at: at(seconds),
        };
        let book = CommandBook {
            cover: Some(h.cover().clone()),
            page: Some(CommandPage {
                body: Some(command_page::Body::Fact(Fact {
                    payload: Some(Any::from_msg(&fact.to_wire()).unwrap()),
                })),
                fact_origin: Some(FactOrigin {
                    external_id: id.to_string(),
                    description: String::new(),
                }),
                ..CommandPage::default()
            }),
            handling: None,
        };
        let before = h.pages().len();
        h.when_book(book).handled().unwrap();
        h.pages().len() - before
    }

    fn open(h: &mut Harness<DummyAggregate>, seconds: i64) {
        h.when(SetDeadline {
            deadline: at(seconds),
        })
        .events();
    }

    #[test]
    fn a_matching_reach_is_recorded_once() {
        let mut h = harness();
        open(&mut h, 100);
        assert_eq!(reach(&mut h, DEADLINE_ALARM, 100, "a"), 1);
        assert_eq!(reach(&mut h, DEADLINE_ALARM, 100, "b"), 0, "passed already");
        let last = h.pages().pop().unwrap();
        assert!(matches!(
            <DummyEvent as angzarr_embedded::Payload>::from_any(last.payload.as_ref().unwrap()),
            Ok(DummyEvent::DeadlinePassed(p)) if p.at == at(100)
        ));
    }

    #[test]
    fn a_reach_that_does_not_match_is_not_recorded() {
        let mut h = harness();
        assert_eq!(reach(&mut h, DEADLINE_ALARM, 100, "none-set"), 0);
        open(&mut h, 100);
        assert_eq!(reach(&mut h, "other", 100, "other-alarm"), 0);
        assert_eq!(reach(&mut h, DEADLINE_ALARM, 99, "other-instant"), 0);
        h.when(Close {}).events();
        assert_eq!(reach(&mut h, DEADLINE_ALARM, 100, "closed"), 0);
    }

    #[test]
    fn the_state_follows_the_events() {
        let mut h = harness();
        open(&mut h, 100);
        assert_eq!(h.state().deadline, Some(at(100)));
        assert!(!h.state().closed && !h.state().passed);
        reach(&mut h, DEADLINE_ALARM, 100, "a");
        assert!(h.state().passed);
        let mut h = harness();
        h.when(Close {}).events();
        assert!(h.state().closed);
    }
}
