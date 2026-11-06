//! A stand-in companion for the companion protocol's tests: a real compiled
//! program that answers what ctxloom asks of a companion, so a test exercises
//! the protocol through a genuine exec rather than a shell shim.
//!
//! It answers from files beside a prefix, so one built binary stands in for
//! any number of companions: a test registers
//! `[stub, --files, <dir>/reprise]` as a companion's command and writes the
//! files `<dir>/reprise.loadout` (and, to misbehave, `.exit` or `.hang`). With
//! no `--files` the prefix is the path the program was invoked by.
//!
//! - `version [--format json]`: `{"name":..,"version":"0.0.0-stub"}`, the
//!   name being the prefix's last segment.
//! - `loadout --format yaml`: the `.loadout` file, verbatim; nothing if there
//!   is none. A `.exit` file holding a number makes it exit with that code
//!   instead; a `.hang` file makes it never answer.
//!
//! It depends on nothing but std, and knows nothing of ctxloom.

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use file_system::{Fs, Os};

/// What the program does about an invocation.
#[derive(Debug, PartialEq, Eq)]
struct Reply {
    stdout: String,
    code: i32,
    /// Never answer: the caller's time limit is what ends it.
    hang: bool,
}

impl Reply {
    fn said(stdout: impl Into<String>) -> Reply {
        Reply {
            stdout: stdout.into(),
            code: 0,
            hang: false,
        }
    }

    fn exited(code: i32) -> Reply {
        Reply {
            stdout: String::new(),
            code,
            hang: false,
        }
    }
}

/// The last segment of the path the program was invoked by.
fn program_name(invoked: &str) -> &str {
    invoked.rsplit(['/', '\\']).next().unwrap_or(invoked)
}

/// The reply to `args` for the program invoked as `invoked`; `read` reads the
/// file at a path. The files are beside `--files <prefix>` when the arguments
/// open with it, else beside `invoked`.
fn answer(invoked: &str, args: &[String], read: &dyn Fn(&str) -> Option<String>) -> Reply {
    let (prefix, args) = match args {
        [flag, prefix, rest @ ..] if flag == "--files" => (prefix.as_str(), rest),
        _ => (invoked, args),
    };
    let beside = |suffix: &str| read(&format!("{prefix}.{suffix}"));
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["version", ..] => Reply::said(format!(
            "{{\"name\":\"{}\",\"version\":\"0.0.0-stub\"}}\n",
            program_name(prefix)
        )),
        ["loadout", "--format", "yaml"] => {
            if beside("hang").is_some() {
                return Reply {
                    stdout: String::new(),
                    code: 0,
                    hang: true,
                };
            }
            if let Some(code) = beside("exit") {
                return Reply::exited(code.trim().parse().unwrap_or(1));
            }
            Reply::said(beside("loadout").unwrap_or_default())
        }
        _ => Reply::exited(2),
    }
}

// The process edge: argv, the files beside the program, stdout, the exit
// code. Everything it decides is `answer`, which the tests reach directly.
fn main() -> ExitCode {
    let mut argv = std::env::args();
    let invoked = argv.next().unwrap_or_default();
    let args: Vec<String> = argv.collect();
    let host = os::host();
    let fs = Os::new(host.clone(), Path::new(host.flavor().top())).ok();
    let read = |path: &str| fs.as_ref()?.read_to_string(Path::new(path)).ok();
    let reply = answer(&invoked, &args, &read);
    if reply.hang {
        loop {
            std::thread::park();
        }
    }
    let _ = std::io::stdout().write_all(reply.stdout.as_bytes());
    ExitCode::from(u8::try_from(reply.code).unwrap_or(1))
}

#[cfg(test)]
#[path = "main.test.rs"]
mod tests;
