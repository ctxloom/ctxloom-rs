//! Why a command to a run was refused.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RunRejection {
    /// An enqueue on a stream that already holds a run.
    AlreadyEnqueued,
    /// A command to a stream holding no run.
    NoRun,
    /// A turn queued while one is in flight.
    TurnInFlight { turn: u64 },
    /// A command to a run that has ended (the end fence).
    Ended,
    /// A credential hash that is not a SHA-256 digest (32 bytes).
    CredentialHashInvalid { length: u64 },
    /// A stream whose root is not the run's derived root.
    NotItsStream,
    /// The runner's and the model's credential hashes are the same, so a
    /// token would not name one principal (fail closed).
    CredentialsNotDistinct,
}

impl fmt::Display for RunRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for RunRejection {
    fn code(&self) -> &str {
        match self {
            RunRejection::AlreadyEnqueued => code!("already-enqueued"),
            RunRejection::NoRun => code!("no-run"),
            RunRejection::TurnInFlight { .. } => code!("turn-in-flight"),
            RunRejection::Ended => code!("ended"),
            RunRejection::CredentialHashInvalid { .. } => code!("credential-hash-invalid"),
            RunRejection::NotItsStream => code!("not-its-stream"),
            RunRejection::CredentialsNotDistinct => code!("credentials-not-distinct"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            RunRejection::TurnInFlight { turn } => vec![("turn", (*turn).into())],
            RunRejection::CredentialHashInvalid { length } => vec![("length", (*length).into())],
            RunRejection::AlreadyEnqueued
            | RunRejection::NoRun
            | RunRejection::Ended
            | RunRejection::NotItsStream
            | RunRejection::CredentialsNotDistinct => Vec::new(),
        }
    }
}

/// This crate's en-US messages, its name first.
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// One value of every rejection, for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(RunRejection::AlreadyEnqueued),
        Box::new(RunRejection::NoRun),
        Box::new(RunRejection::TurnInFlight { turn: 1 }),
        Box::new(RunRejection::Ended),
        Box::new(RunRejection::CredentialHashInvalid { length: 7 }),
        Box::new(RunRejection::NotItsStream),
        Box::new(RunRejection::CredentialsNotDistinct),
    ]
}
