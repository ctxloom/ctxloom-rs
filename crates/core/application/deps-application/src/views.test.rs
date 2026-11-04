use super::*;

// The default pointer is said only when an edit moved it.
#[test]
fn test_an_edit_says_the_default_followed_only_when_it_did() {
    let state = |name: &str| RemoteState {
        name: name.to_string(),
        url: "file:///a.git".to_string(),
        forge: String::new(),
    };
    let edited = |followed: bool| EditedRemote {
        status: "edited",
        name: "up".to_string(),
        before: state("origin"),
        after: state("up"),
        changed: vec!["name".to_string()],
        default_pointer_updated: followed,
    };
    let said = |followed: bool| serde_json::to_value(edited(followed)).expect("json");
    assert_eq!(said(true)["default_pointer_updated"], true);
    assert!(said(false).get("default_pointer_updated").is_none());
}
