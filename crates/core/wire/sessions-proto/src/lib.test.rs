//! The wire types carry the names the kernel routes by, and round-trip.

use super::v1;
use prost::{Message, Name};

#[test]
fn test_type_url_names_each_message_by_full_name() {
    assert_eq!(
        v1::SessionMinted::type_url(),
        "/ctxloom.sessions.v1.SessionMinted"
    );
    assert_eq!(
        v1::BindEngineSession::type_url(),
        "/ctxloom.sessions.v1.BindEngineSession"
    );
    assert_eq!(v1::Claimed::type_url(), "/ctxloom.sessions.v1.Claimed");
}

#[test]
fn test_event_round_trips_through_its_encoding() {
    let event = v1::EngineSessionBound {
        native_id: "second".to_string(),
        transcript_path: "/t2".to_string(),
        engine_version: String::new(),
        displaced: Some(v1::Rotation {
            native_id: "first".to_string(),
            transcript_path: "/t1".to_string(),
            rotated_at: None,
        }),
    };
    let decoded = v1::EngineSessionBound::decode(event.encode_to_vec().as_slice()).expect("decode");
    assert_eq!(decoded, event);
}
