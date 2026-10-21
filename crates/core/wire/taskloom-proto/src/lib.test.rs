//! The wire types carry the names annals routes by, and round-trip.

use super::v1;
use prost::{Message, Name};

// annals routes and upcasts by type URL, so each message must carry its
// own full name with angzarr's "/" prefix.
#[test]
fn test_type_url_names_each_message_by_full_name() {
    assert_eq!(v1::TaskAdded::type_url(), "/ctxloom.tasks.v1.TaskAdded");
    assert_eq!(v1::AddTask::type_url(), "/ctxloom.tasks.v1.AddTask");
    assert_eq!(v1::TaskState::type_url(), "/ctxloom.tasks.v1.TaskState");
}

#[test]
fn test_event_round_trips_through_its_encoding() {
    let event = v1::Retagged {
        added: vec!["urgent".to_string()],
        removed: vec!["later".to_string()],
    };
    let decoded = v1::Retagged::decode(event.encode_to_vec().as_slice()).expect("decode");
    assert_eq!(decoded, event);
}
