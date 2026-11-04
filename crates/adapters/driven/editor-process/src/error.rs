//! Why the editor could not be run.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

/// This crate's en-US messages (`locales/en-US/messages.ftl`, ids under the
/// crate's name).
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EditorError {
    /// The editor could not be started.
    Spawn { program: String, detail: String },
    /// The editor ran and did not succeed: how it ended.
    Failed { program: String, status: String },
    /// The temporary file could not be made or written.
    Temp { detail: String },
    /// What the editor left could not be read as text.
    Read { path: String, detail: String },
    /// A file name that is a path.
    BadName { name: String },
}

impl fmt::Display for EditorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl std::error::Error for EditorError {}

impl Diagnostic for EditorError {
    fn code(&self) -> &str {
        match self {
            EditorError::Spawn { .. } => code!("spawn"),
            EditorError::Failed { .. } => code!("failed"),
            EditorError::Temp { .. } => code!("temp"),
            EditorError::Read { .. } => code!("read"),
            EditorError::BadName { .. } => code!("bad-name"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            EditorError::Spawn { program, detail } => vec![
                ("program", program.as_str().into()),
                ("ext_err_txt", detail.as_str().into()),
            ],
            EditorError::Failed { program, status } => vec![
                ("program", program.as_str().into()),
                ("status", status.as_str().into()),
            ],
            EditorError::Temp { detail } => vec![("ext_err_txt", detail.as_str().into())],
            EditorError::Read { path, detail } => vec![
                ("path", path.as_str().into()),
                ("ext_err_txt", detail.as_str().into()),
            ],
            EditorError::BadName { name } => vec![("name", name.as_str().into())],
        }
    }
}

/// One value of every error here, for a catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(EditorError::Spawn {
            program: "nano".to_string(),
            detail: "not found".to_string(),
        }),
        Box::new(EditorError::Failed {
            program: "nano".to_string(),
            status: "exit status: 1".to_string(),
        }),
        Box::new(EditorError::Temp {
            detail: "no space".to_string(),
        }),
        Box::new(EditorError::Read {
            path: "/tmp/x.md".to_string(),
            detail: "not UTF-8".to_string(),
        }),
        Box::new(EditorError::BadName {
            name: "a/b".to_string(),
        }),
    ]
}
