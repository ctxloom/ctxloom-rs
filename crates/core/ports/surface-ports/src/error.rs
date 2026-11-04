//! Why a surface could not be delivered, released or read. Every message is
//! in `locales/en-US/messages.ftl`.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

/// This crate's en-US messages (`locales/en-US/messages.ftl`, ids under the
/// crate's name).
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// A surface that could not be delivered, released or read.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SurfaceError {
    /// A write that was refused, with nothing changed: the file is not
    /// something ctxloom may edit.
    Refused { file: String, reason: String },
    /// A file that could not be read or written.
    Io { file: String, detail: String },
    /// The ownership record could not be read.
    Record { file: String, detail: String },
    /// A request an engine cannot carry out.
    Unsupported { engine: String, what: String },
    /// A hook payload the engine's codec could not read.
    Payload { engine: String, detail: String },
}

impl fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl std::error::Error for SurfaceError {}

impl Diagnostic for SurfaceError {
    fn code(&self) -> &str {
        match self {
            SurfaceError::Refused { .. } => code!("refused"),
            SurfaceError::Io { .. } => code!("io"),
            SurfaceError::Record { .. } => code!("record"),
            SurfaceError::Unsupported { .. } => code!("unsupported"),
            SurfaceError::Payload { .. } => code!("payload"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            SurfaceError::Refused { file, reason } => vec![
                ("file", file.as_str().into()),
                ("reason", reason.as_str().into()),
            ],
            SurfaceError::Io { file, detail } | SurfaceError::Record { file, detail } => vec![
                ("file", file.as_str().into()),
                ("detail", detail.as_str().into()),
            ],
            SurfaceError::Unsupported { engine, what } => vec![
                ("engine", engine.as_str().into()),
                ("what", what.as_str().into()),
            ],
            SurfaceError::Payload { engine, detail } => vec![
                ("engine", engine.as_str().into()),
                ("detail", detail.as_str().into()),
            ],
        }
    }
}

/// One value of every error, for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(SurfaceError::Refused {
            file: "settings.json".into(),
            reason: "its statusLine cannot be read".into(),
        }),
        Box::new(SurfaceError::Io {
            file: "CLAUDE.md".into(),
            detail: "permission denied".into(),
        }),
        Box::new(SurfaceError::Record {
            file: "records/x.claims.yaml".into(),
            detail: "not a record".into(),
        }),
        Box::new(SurfaceError::Unsupported {
            engine: "claude-code".into(),
            what: "a context file in the session home".into(),
        }),
        Box::new(SurfaceError::Payload {
            engine: "claude-code".into(),
            detail: "expected value".into(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
