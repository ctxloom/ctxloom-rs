//! Why the cache could not be measured or removed.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CacheError {
    /// The filesystem refused; `ext_err_txt` is its own text, verbatim.
    Io {
        op: String,
        path: String,
        ext_err_txt: String,
    },
    /// A path that leaves the app dir.
    Outside { rel: String },
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for CacheError {
    fn code(&self) -> &str {
        match self {
            CacheError::Io { .. } => code!("io"),
            CacheError::Outside { .. } => code!("outside"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            CacheError::Io {
                op,
                path,
                ext_err_txt,
            } => vec![
                ("op", op.as_str().into()),
                ("path", path.as_str().into()),
                ("ext_err_txt", ext_err_txt.as_str().into()),
            ],
            CacheError::Outside { rel } => vec![("rel", rel.as_str().into())],
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
        Box::new(CacheError::Io {
            op: "remove".to_string(),
            path: "/work/.ctxloom/cache/repos".to_string(),
            ext_err_txt: "permission denied".to_string(),
        }),
        Box::new(CacheError::Outside {
            rel: "../x".to_string(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
