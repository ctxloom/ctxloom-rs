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
