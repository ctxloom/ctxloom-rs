//! The ports' default behaviour.

use super::*;

struct NoRootIgnored;

impl ConfigFiles for NoRootIgnored {
    fn source(&self) -> Source {
        Source::Home
    }

    fn app_dir(&self) -> String {
        "/h/.ctxloom".to_string()
    }

    fn layers(&self) -> Result<Vec<LayerText>, Failure> {
        Ok(Vec::new())
    }
}

// configload: an adapter that never ignores a `CTXLOOM_ROOT` says nothing of
// one.
#[test]
fn test_a_config_source_ignores_no_root_unless_it_says_so() {
    assert_eq!(NoRootIgnored.ignored_root(), None);
}
