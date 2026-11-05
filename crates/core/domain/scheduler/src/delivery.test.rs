use angzarr_embedded::v1::Uuid;
use angzarr_embedded::{Any, Payload, Timestamp};

use crate::{delivered, AlarmArmed, AlarmFired, AlarmReached, SchedulerEvent, Stream};

fn target(domain: &str, root: Vec<u8>) -> Stream {
    Stream {
        domain: domain.to_string(),
        root,
        edition: None,
    }
}

fn at() -> Timestamp {
    Timestamp {
        seconds: 9,
        nanos: 1,
    }
}

fn fired(target: Stream) -> Any {
    SchedulerEvent::from(AlarmFired {
        target,
        alarm_id: "deadline".to_string(),
        at: at(),
    })
    .to_any()
}

#[test]
fn delivery_answers_alarm_fired_with_alarm_reached_fact_to_its_target_root() {
    let got = delivered(&fired(target("hold", vec![5; 16])), "hold").unwrap();
    assert_eq!(
        got,
        Some((
            Uuid::from_bytes([5; 16]),
            AlarmReached {
                alarm_id: "deadline".to_string(),
                at: at(),
            }
        ))
    );
}

#[test]
fn delivery_ignores_another_domains_firing() {
    assert_eq!(
        delivered(&fired(target("approval", vec![5; 16])), "hold").unwrap(),
        None
    );
}

#[test]
fn delivery_ignores_events_that_are_not_a_firing() {
    let armed = SchedulerEvent::from(AlarmArmed {
        target: target("hold", vec![5; 16]),
        alarm_id: "deadline".to_string(),
        at: at(),
    })
    .to_any();
    assert_eq!(delivered(&armed, "hold").unwrap(), None);
}

#[test]
fn delivery_to_a_root_that_is_not_a_uuid_delivers_nothing() {
    assert_eq!(
        delivered(&fired(target("hold", vec![5; 3])), "hold").unwrap(),
        None
    );
}

#[test]
fn delivery_of_an_event_the_scheduler_cannot_read_is_a_bug() {
    let stranger = Any {
        type_url: "/toy.v1.Stranger".to_string(),
        value: Vec::new(),
    };
    assert!(delivered(&stranger, "hold").is_err());
}
