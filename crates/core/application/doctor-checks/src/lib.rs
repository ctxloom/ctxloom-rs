//! `ctxloom doctor` (S12): the deterministic setup checks, one row each,
//! every row prefixed with a `DOCTOR-CHECK-*` marker shared with the
//! "ctxloom-doctor" skill. Of the Go doctor's 26 checks these ten are ported
//! (some reshaped); the rest are left to their slices:
//!
//! | marker | the check |
//! |---|---|
//! | `SETUP-MARKER-e5` | the `.ctxloom` marker and the config's validity |
//! | `DEPS-a1` | `git` on the PATH and a git identity |
//! | `AGENTS-b2` | every configured agent names an llm |
//! | `CREDENTIALS-m5` | the credential each configured engine needs is exported (presence only, never the value); a missing one is a `fail` |
//! | `VERSION-c3` | the binary's version |
//! | `HOOKS-TRUST-d4` | ctxloom's hooks registered with the engine (the trust store half is dead since ADR 0037) |
//! | `GITIGNORE-f6` | no blanket `.ctxloom` ignore rule |
//! | `MCP-INVOCATION-g7` | no settings file launches a ctxloom MCP server that does not exist |
//! | `STALE-HOOKS-n5` | no hook entry runs a `ctxloom hook` subcommand this ctxloom lacks |
//! | `INGESTION-q7` | the stated limit: whether the engine reads what ctxloom wrote |
//!
//! Left out here: the run-end and
//! runner/MCP-state checks (S6), the container runtime (S11), and the
//! lockstep check against the installed taskloom (unruled: D15/D3b).
//!
//! A row is `ok`, `warn`, `fail` or `info`. A warning is advisory; a `fail`
//! guarantees `ctxloom run` refuses, and makes doctor exit 3.

mod checks;
mod doctor;
mod hooks;

pub use checks::{Check, Status};
pub use doctor::{Doctor, Removed, Report};
pub use hooks::LIVE_HOOKS;

#[cfg(test)]
#[path = "checks.test.rs"]
mod checks_tests;

#[cfg(test)]
#[path = "hooks.test.rs"]
mod hooks_tests;

#[cfg(test)]
#[path = "doctor.test.rs"]
mod doctor_tests;
