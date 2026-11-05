//! Why a command to a scheduler stream was refused. A refusal here is always
//! a bug in the asking domain's policy (scheduler.plan.md OD-3).

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SchedulerRejection {
    /// An ArmAlarm with no alarm id.
    AlarmIdEmpty,
    /// An ArmAlarm whose target is not the one this stream belongs to: the
    /// stream is not `stream_for(target)`, or is in another edition than the
    /// target's. `stream` is the stream commanded, `target` the target asked for.
    TargetMismatch { stream: String, target: String },
}

impl fmt::Display for SchedulerRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for SchedulerRejection {
    fn code(&self) -> &str {
        match self {
            SchedulerRejection::AlarmIdEmpty => code!("alarm-id-empty"),
            SchedulerRejection::TargetMismatch { .. } => code!("target-mismatch"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            SchedulerRejection::AlarmIdEmpty => Vec::new(),
            SchedulerRejection::TargetMismatch { stream, target } => vec![
                ("stream", stream.as_str().into()),
                ("target", target.as_str().into()),
            ],
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
        Box::new(SchedulerRejection::AlarmIdEmpty),
        Box::new(SchedulerRejection::TargetMismatch {
            stream: "scheduler/s".to_string(),
            target: "toy/t".to_string(),
        }),
    ]
}
