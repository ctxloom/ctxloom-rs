//! Why a setup command did not complete. Every message is in
//! `locales/en-US/messages.ftl`; a library or surface error passes through as
//! itself.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

/// This crate's en-US messages (`locales/en-US/messages.ftl`, ids under the
/// crate's name).
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// A command that did not complete.
#[derive(Debug)]
pub enum SetupError {
    /// An error of the library or an adapter, as itself.
    Cause(Box<dyn Diagnostic>),
    /// `materialize` with no `--target` and no `--yes`.
    NoTarget { engines: String, dir: String },
    /// A `--target` that is a file.
    TargetNotDirectory { path: String },
    /// A request that contradicts itself.
    Request { detail: String },
    /// An engine ctxloom does not know.
    UnknownEngine { engine: String, known: String },
    /// `manage install --engine` over a project that already has its config.
    EngineNotApplied { engine: String },
    /// A selection that assembles to nothing.
    EmptyContext { profiles: String, engine: String },
    /// Findings a strict run refuses over; nothing was written.
    Refused { findings: String },
    /// A hook fired under an engine whose wire ctxloom does not read.
    UnknownHookEngine { engine: String, known: String },
    /// A hook event that is none of the lifecycle's.
    UnknownEvent { event: String, known: String },
    /// `util config-write` that would not write.
    ConfigWrite { detail: String },
}

impl SetupError {
    /// An error of the library or an adapter, as itself.
    pub fn cause(error: impl Diagnostic + 'static) -> SetupError {
        SetupError::Cause(Box::new(error))
    }

    /// The exit code: 1 failed, 2 refused before any state was read, 3 fatal
    /// findings.
    pub fn exit_code(&self) -> u8 {
        match self {
            SetupError::NoTarget { .. }
            | SetupError::Request { .. }
            | SetupError::UnknownEngine { .. }
            | SetupError::UnknownEvent { .. } => 2,
            SetupError::Refused { .. } => 3,
            _ => 1,
        }
    }
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl std::error::Error for SetupError {}

impl Diagnostic for SetupError {
    fn code(&self) -> &str {
        match self {
            SetupError::Cause(cause) => cause.code(),
            SetupError::NoTarget { .. } => code!("no-target"),
            SetupError::TargetNotDirectory { .. } => code!("target-not-directory"),
            SetupError::Request { .. } => code!("request"),
            SetupError::UnknownEngine { .. } => code!("unknown-engine"),
            SetupError::EngineNotApplied { .. } => code!("engine-not-applied"),
            SetupError::EmptyContext { .. } => code!("empty-context"),
            SetupError::Refused { .. } => code!("refused"),
            SetupError::UnknownHookEngine { .. } => code!("unknown-hook-engine"),
            SetupError::UnknownEvent { .. } => code!("unknown-event"),
            SetupError::ConfigWrite { .. } => code!("config-write"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            SetupError::Cause(cause) => cause.args(),
            SetupError::NoTarget { engines, dir } => {
                vec![
                    ("engines", engines.as_str().into()),
                    ("dir", dir.as_str().into()),
                ]
            }
            SetupError::TargetNotDirectory { path } => vec![("path", path.as_str().into())],
            SetupError::Request { detail } | SetupError::ConfigWrite { detail } => {
                vec![("detail", detail.as_str().into())]
            }
            SetupError::UnknownEngine { engine, known }
            | SetupError::UnknownHookEngine { engine, known } => vec![
                ("engine", engine.as_str().into()),
                ("known", known.as_str().into()),
            ],
            SetupError::EngineNotApplied { engine } => vec![("engine", engine.as_str().into())],
            SetupError::EmptyContext { profiles, engine } => vec![
                ("profiles", profiles.as_str().into()),
                ("engine", engine.as_str().into()),
            ],
            SetupError::Refused { findings } => vec![("findings", findings.as_str().into())],
            SetupError::UnknownEvent { event, known } => vec![
                ("event", event.as_str().into()),
                ("known", known.as_str().into()),
            ],
        }
    }
}

/// One value of every error, for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(SetupError::NoTarget {
            engines: "claude-code".into(),
            dir: "/work/project".into(),
        }),
        Box::new(SetupError::TargetNotDirectory {
            path: "/work/out".into(),
        }),
        Box::new(SetupError::Request {
            detail: "--release takes no profiles".into(),
        }),
        Box::new(SetupError::UnknownEngine {
            engine: "bogus".into(),
            known: "claude-code, mock".into(),
        }),
        Box::new(SetupError::EngineNotApplied {
            engine: "mock".into(),
        }),
        Box::new(SetupError::EmptyContext {
            profiles: "dev".into(),
            engine: "claude-code".into(),
        }),
        Box::new(SetupError::Refused {
            findings: "failed to load bundle".into(),
        }),
        Box::new(SetupError::UnknownHookEngine {
            engine: "bogus".into(),
            known: "claude-code, mock".into(),
        }),
        Box::new(SetupError::UnknownEvent {
            event: "pre_toll".into(),
            known: "pre_tool, post_tool".into(),
        }),
        Box::new(SetupError::ConfigWrite {
            detail: "vendor.json is not valid json".into(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
