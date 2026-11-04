//! What a command noticed, said on the error stream: a warning and what to do
//! about it: a json line under json, words under text.

use std::fmt;
use std::io::Write;

use angzarr_diagnostic::{Arg, Diagnostic};
use angzarr_presentation::{write_json_line, Catalog, Format};
use deps_application::Notice;
use serde::Serialize;

#[derive(Serialize)]
struct Warning<'a> {
    prog: &'a str,
    warning: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    remedy: &'a str,
}

/// The remedy of a diagnostic: its code with `-fix`, and its args.
pub(crate) struct FixOf<'a>(pub(crate) &'a dyn Diagnostic, pub(crate) String);

impl fmt::Debug for FixOf<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FixOf({})", self.1)
    }
}

impl fmt::Display for FixOf<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for FixOf<'_> {
    fn code(&self) -> &str {
        &self.1
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        self.0.args()
    }
}

/// Say each notice on `err`.
pub fn say(
    notices: &[Notice],
    program: &str,
    format: Format,
    catalog: &Catalog,
    err: &mut dyn Write,
) {
    for notice in notices {
        let message = catalog.message(notice);
        let remedy = catalog
            .render(&FixOf(notice, format!("{}-fix", notice.code())))
            .unwrap_or_default();
        let _ = match format {
            Format::Json => write_json_line(
                err,
                &Warning {
                    prog: program,
                    warning: &message,
                    remedy: &remedy,
                },
            ),
            Format::Text => {
                let head = writeln!(err, "{program}: warning: {message}");
                if remedy.is_empty() {
                    head
                } else {
                    head.and_then(|()| writeln!(err, "  fix: {remedy}"))
                }
            }
        };
    }
}

#[cfg(test)]
#[path = "notices.test.rs"]
mod tests;
