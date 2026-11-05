use super::*;

fn path(segments: &[&str]) -> Option<CachePath> {
    CachePath::new(segments.iter().map(|s| s.to_string()).collect())
}

#[test]
fn test_a_cache_path_keeps_its_segments_in_order() {
    let p = path(&["example.test", "o", "r"]).expect("plain names");
    assert_eq!(p.segments(), ["example.test", "o", "r"]);
}

#[test]
fn test_a_cache_path_needs_at_least_one_segment() {
    assert_eq!(path(&[]), None);
}

#[test]
fn test_a_cache_path_refuses_a_segment_that_is_not_a_plain_name() {
    for bad in ["", ".", "..", "a/b", "a\\b", "/", "\\"] {
        assert_eq!(path(&["ok", bad]), None, "{bad:?}");
        assert_eq!(path(&[bad]), None, "{bad:?}");
    }
    assert!(path(&["..a", "a..", ".git", "a.b"]).is_some());
}
