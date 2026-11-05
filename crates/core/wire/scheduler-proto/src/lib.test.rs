//! The wire messages carry the names the kernel routes by, in their final
//! package: a stored event's type URL must never change at the lift.

use super::v1;
use prost::Name;

#[test]
fn every_message_is_in_the_final_package() {
    for url in [
        v1::ArmAlarm::type_url(),
        v1::CancelAlarm::type_url(),
        v1::AlarmReached::type_url(),
        v1::AlarmDue::type_url(),
        v1::AlarmArmed::type_url(),
        v1::AlarmCancelled::type_url(),
        v1::AlarmFired::type_url(),
    ] {
        assert!(url.starts_with("/angzarr.embedded.scheduler.v1."), "{url}");
    }
}
