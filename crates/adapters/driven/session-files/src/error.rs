//! Why a session's files could not be read or removed.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FilesError {
    /// The filesystem refused; `ext_err_txt` is its own text, verbatim.
    Io {
        op: String,
        path: String,
        ext_err_txt: String,
    },
    /// A path to remove that leaves the session's directory.
    Outside { rel: String },
    /// An output directory cannot move to where something already is.
    OutputExists { path: String },
}

impl FilesError {
    pub(crate) fn io(op: &str, path: &std::path::Path, error: &std::io::Error) -> FilesError {
        FilesError::Io {
            op: op.to_string(),
            path: path.display().to_string(),
            ext_err_txt: error.to_string(),
        }
    }
}

impl fmt::Display for FilesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for FilesError {
    fn code(&self) -> &str {
        match self {
            FilesError::Io { .. } => code!("io"),
            FilesError::Outside { .. } => code!("outside"),
            FilesError::OutputExists { .. } => code!("output-exists"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            FilesError::Io {
                op,
                path,
                ext_err_txt,
            } => vec![
                ("op", op.as_str().into()),
                ("path", path.as_str().into()),
                ("ext_err_txt", ext_err_txt.as_str().into()),
            ],
            FilesError::Outside { rel } => vec![("rel", rel.as_str().into())],
            FilesError::OutputExists { path } => vec![("path", path.as_str().into())],
        }
    }
}

/// This crate's en-US messages, its name first.
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// One value of every error, for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(FilesError::Io {
            op: "read".to_string(),
            path: "/home/u/.ctxloom/sessions/x".to_string(),
            ext_err_txt: "permission denied".to_string(),
        }),
        Box::new(FilesError::Outside {
            rel: "../x".to_string(),
        }),
        Box::new(FilesError::OutputExists {
            path: "/home/u/Documents/ctxloom/p/new-name".to_string(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
