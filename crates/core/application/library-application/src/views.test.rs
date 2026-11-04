//! The predicates that leave an empty field out of a view's JSON.

use std::collections::BTreeMap;

use super::{is_false, is_zero, no_entries, no_items, no_text};

// Go's `omitempty`: an empty string, slice, map, false and zero are left out.
#[test]
fn test_omitempty_predicates_say_empty_only_for_empty() {
    assert!(no_text(""));
    assert!(!no_text("x"));
    assert!(no_items::<u8>(&[]));
    assert!(!no_items(&[1]));
    assert!(no_entries::<u8, u8>(&BTreeMap::new()));
    assert!(!no_entries(&BTreeMap::from([(1, 2)])));
    assert!(is_false(&false));
    assert!(!is_false(&true));
    assert!(is_zero(&0));
    assert!(!is_zero(&1));
}
