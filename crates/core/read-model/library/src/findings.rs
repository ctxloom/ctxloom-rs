//! What a read noticed on the way and did not stop for: a profile's parent
//! that is not there, a fragment that did not load, a variable nobody
//! defined. The caller says them (as warnings, with the remedy) and, for the
//! ones that fail a launch, refuses the run unless `--degraded`.

/// Whether a finding fails a launch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    /// Fatal to a launch (and to its preview) unless `--degraded`; Go's
    /// `strictness.Fail`.
    Fail,
    /// A warning only (Go's `clidiag.Warn`).
    Warn,
}

/// One finding.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Finding {
    /// What kind of thing was wrong: `bundle`, `ref`, ...
    pub kind: String,
    pub text: String,
    /// What to do about it; empty for none.
    pub remedy: String,
    pub class: Class,
}

impl Finding {
    /// A finding that fails a launch.
    pub fn fail(kind: &str, text: impl Into<String>, remedy: impl Into<String>) -> Finding {
        Finding {
            kind: kind.to_string(),
            text: text.into(),
            remedy: remedy.into(),
            class: Class::Fail,
        }
    }

    /// A finding that only warns.
    pub fn warn(kind: &str, text: impl Into<String>) -> Finding {
        Finding {
            kind: kind.to_string(),
            text: text.into(),
            remedy: String::new(),
            class: Class::Warn,
        }
    }
}
