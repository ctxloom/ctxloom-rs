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

    fn layer(&self, _scope: Scope) -> Result<Option<LayerText>, Failure> {
        Ok(None)
    }

    fn layer_path(&self, _scope: Scope) -> String {
        String::new()
    }

    fn write_layer(&self, _scope: Scope, _text: &str) -> Result<(), Failure> {
        Ok(())
    }
}

// a plain file is not executable
#[test]
fn test_a_put_file_is_plain() {
    let put = Put::file("a/b.md", "x");
    assert_eq!(
        (put.path.as_str(), put.bytes.as_slice(), put.executable),
        ("a/b.md", b"x".as_slice(), false)
    );
}

// configload: an adapter that never ignores a `CTXLOOM_ROOT` says nothing of
// one.
#[test]
fn test_a_config_source_ignores_no_root_unless_it_says_so() {
    assert_eq!(NoRootIgnored.ignored_root(), None);
}

#[test]
fn test_a_plain_relative_name_has_only_plain_segments() {
    for ok in ["a", "a/b", "a.b/c-d", ".hidden/x", "a..b"] {
        assert!(is_plain_relative(ok), "{ok:?}");
    }
    for bad in [
        "", "/", "/a", "a/", "a//b", ".", "..", "a/./b", "a/../b", "../a",
    ] {
        assert!(!is_plain_relative(bad), "{bad:?}");
    }
}
