//! The tasks domain's wire types: prost messages generated from
//! `proto/ctxloom/tasks/v1/tasks.proto`. They are the wire form only; domain
//! code works on domain types converted from these at the boundary.

/// `ctxloom.tasks.v1`. One module, not one per package segment: the package
/// refers to no other package's types except angzarr's, which prost reaches
/// by absolute path (`extern_path`).
pub mod v1 {
    include!(concat!(env!("OUT_DIR"), "/ctxloom.tasks.v1.rs"));
}

#[cfg(test)]
mod tests {
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
}
