use angzarr_embedded::markup::DomainMessage;
use angzarr_embedded::Timestamp;
use scheduler_proto::v1 as wire;

use crate::AlarmReached;

#[test]
fn the_fact_round_trips_through_its_wire_message() {
    let reached = AlarmReached {
        alarm_id: "deadline".to_string(),
        at: Timestamp {
            seconds: 5,
            nanos: 6,
        },
    };
    assert_eq!(AlarmReached::from_wire(reached.to_wire()).unwrap(), reached);
}

#[test]
fn a_fact_without_its_instant_is_malformed() {
    let wire = wire::AlarmReached {
        alarm_id: "deadline".to_string(),
        at: None,
    };
    assert!(AlarmReached::from_wire(wire).is_err());
}
