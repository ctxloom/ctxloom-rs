//! Why a config file could not be read. A third party's text (the OS's) is
//! the arg `ext_err_txt`, verbatim.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

/// This crate's en-US messages (`locales/en-US/messages.ftl`, ids under the
/// crate's name).
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConfigFileError {
    /// A config file that is there and could not be read.
    Read { path: String, detail: String },
    /// A config file that is not UTF-8.
    NotText { path: String },
    /// A config file that could not be written.
    Write { path: String, detail: String },
}

impl fmt::Display for ConfigFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl std::error::Error for ConfigFileError {}

impl Diagnostic for ConfigFileError {
    fn code(&self) -> &str {
        match self {
            ConfigFileError::Read { .. } => code!("read"),
            ConfigFileError::NotText { .. } => code!("not-text"),
            ConfigFileError::Write { .. } => code!("write"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            ConfigFileError::Read { path, detail } | ConfigFileError::Write { path, detail } => {
                vec![
                    ("path", path.as_str().into()),
                    ("ext_err_txt", detail.as_str().into()),
                ]
            }
            ConfigFileError::NotText { path } => vec![("path", path.as_str().into())],
        }
    }
}

/// One value of every error here, for a catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(ConfigFileError::Read {
            path: "/p/.ctxloom/config.yaml".to_string(),
            detail: "permission denied".to_string(),
        }),
        Box::new(ConfigFileError::NotText {
            path: "/p/.ctxloom/config.yaml".to_string(),
        }),
        Box::new(ConfigFileError::Write {
            path: "/p/.ctxloom/config.yaml".to_string(),
            detail: "permission denied".to_string(),
        }),
    ]
}
