//! Why a hook did not complete, and what it warned of.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum HookError {
    /// A `--engine` naming no engine whose hooks are read (Go's
    /// `UnknownHookEngineError`): guessing would decode one engine's wire
    /// with another's codec.
    UnknownEngine { engine: String, known: String },
    /// A session_start payload the engine's codec could not read: the bind
    /// did not happen.
    Payload { harp: String, detail: String },
}

impl fmt::Display for HookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for HookError {
    fn code(&self) -> &str {
        match self {
            HookError::UnknownEngine { .. } => code!("unknown-engine"),
            HookError::Payload { .. } => code!("payload"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            HookError::UnknownEngine { engine, known } => vec![
                ("engine", engine.as_str().into()),
                ("known", known.as_str().into()),
            ],
            HookError::Payload { harp, detail } => vec![
                ("harp", harp.as_str().into()),
                ("detail", detail.as_str().into()),
            ],
        }
    }
}

/// What a hook says on its diagnostic channel and still succeeds: stdout is
/// the engine's input and a diagnostic printed there would corrupt it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum HookWarning {
    /// There is no harp to name: nothing truthful to emit on stdout.
    NoUsableHarp { harp: String },
}

impl fmt::Display for HookWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for HookWarning {
    fn code(&self) -> &str {
        match self {
            HookWarning::NoUsableHarp { .. } => code!("no-usable-harp"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            HookWarning::NoUsableHarp { harp } => vec![("harp", harp.as_str().into())],
        }
    }
}

/// This crate's en-US messages, its name first.
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// One value of every error and warning, for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(HookError::UnknownEngine {
            engine: "nope".to_string(),
            known: "claude-code".to_string(),
        }),
        Box::new(HookError::Payload {
            harp: "amber-swift-owl".to_string(),
            detail: "expected value".to_string(),
        }),
        Box::new(HookWarning::NoUsableHarp {
            harp: String::new(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
