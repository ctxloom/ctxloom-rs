//! Why a bundle's files could not be read. A third party's text (the OS's)
//! is the arg `ext_err_txt`, verbatim.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

/// This crate's en-US messages (`locales/en-US/messages.ftl`, ids under the
/// crate's name).
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ContentError {
    /// The bundles directory could not be listed.
    ListRoot { path: String, detail: String },
    /// A directory inside a bundle could not be listed.
    ListDir { path: String, detail: String },
    /// A file could not be read.
    ReadFile { path: String, detail: String },
    /// A name that is no bundle's path (it climbs, or is absolute).
    BadName { name: String },
    /// A path inside a bundle that is absolute, climbs out, or goes through a link.
    BadPath { path: String },
    /// A file could not be written.
    WriteFile { path: String, detail: String },
    /// A file or a directory could not be taken away.
    RemovePath { path: String, detail: String },
    /// A directory could not be made.
    MakeDir { path: String, detail: String },
}

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl std::error::Error for ContentError {}

impl Diagnostic for ContentError {
    fn code(&self) -> &str {
        match self {
            ContentError::ListRoot { .. } => code!("list-root"),
            ContentError::ListDir { .. } => code!("list-dir"),
            ContentError::ReadFile { .. } => code!("read-file"),
            ContentError::BadName { .. } => code!("bad-name"),
            ContentError::BadPath { .. } => code!("bad-path"),
            ContentError::WriteFile { .. } => code!("write-file"),
            ContentError::RemovePath { .. } => code!("remove-path"),
            ContentError::MakeDir { .. } => code!("make-dir"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            ContentError::ListRoot { path, detail }
            | ContentError::ListDir { path, detail }
            | ContentError::ReadFile { path, detail }
            | ContentError::WriteFile { path, detail }
            | ContentError::RemovePath { path, detail }
            | ContentError::MakeDir { path, detail } => vec![
                ("path", path.as_str().into()),
                ("ext_err_txt", detail.as_str().into()),
            ],
            ContentError::BadName { name } => vec![("name", name.as_str().into())],
            ContentError::BadPath { path } => vec![("path", path.as_str().into())],
        }
    }
}

/// One value of every error here, for a catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let (path, detail) = (
        "/p/.ctxloom/content/bundles/v2".to_string(),
        "permission denied".to_string(),
    );
    vec![
        Box::new(ContentError::ListRoot {
            path: path.clone(),
            detail: detail.clone(),
        }),
        Box::new(ContentError::ListDir {
            path: path.clone(),
            detail: detail.clone(),
        }),
        Box::new(ContentError::ReadFile {
            path: path.clone(),
            detail: detail.clone(),
        }),
        Box::new(ContentError::BadName {
            name: "../x".to_string(),
        }),
        Box::new(ContentError::BadPath {
            path: "../x".to_string(),
        }),
        Box::new(ContentError::WriteFile {
            path: path.clone(),
            detail: detail.clone(),
        }),
        Box::new(ContentError::RemovePath {
            path: path.clone(),
            detail: detail.clone(),
        }),
        Box::new(ContentError::MakeDir { path, detail }),
    ]
}
