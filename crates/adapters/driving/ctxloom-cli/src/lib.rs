//! The ctxloom command line (D28): arguments parsed into commands, answers
//! written as json or text, errors as words or as the envelope.
//!
//! - Noun-verb, with `-h` at every depth and no `help` command; an unknown
//!   verb is an error that says "Did you mean" and lists the verbs there are.
//! - `--format` is `json` or `text` (angzarr-presentation's [`Format`]),
//!   json off a terminal; any other value is refused, whatever it once was.
//! - Under json an error is the envelope `{code, args, message}` on stderr;
//!   under text it is `ctxloom: <message>`, then `  fix: <remedy>` where its
//!   catalog has one. Stdout holds answers only.
//! - Exit codes: 0, 1 failed, 2 refused, 3 fatal findings.
//! - `-v` is accepted everywhere and changes no output.
//! - With the `test-support` feature, the hidden `test` commands.

mod commands;
mod error;
mod extension;
mod run;
#[cfg(feature = "test-support")]
mod test_commands;

pub use angzarr_presentation::Catalog;
pub use commands::PROG;
#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{CliError, Fix, EN_US};
pub use extension::{Call, Extension, Failure, NoExtension};
pub use run::{run, Context, Services};

#[cfg(test)]
#[path = "run.test.rs"]
mod tests;
