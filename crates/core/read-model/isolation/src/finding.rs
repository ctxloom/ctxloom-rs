//! The fatal findings (KindIsolation): exit 3, in both modes.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

use crate::Ownership;

/// Why a run's isolation cannot be had. Never downgraded by `--degraded`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Finding {
    /// A container was demanded and no runtime is reachable.
    NoRuntime { demanded: Ownership },
    /// A runtime is reachable, but not with the demanded ownership. The other
    /// mode is never substituted.
    OwnershipMismatch {
        demanded: Ownership,
        offered: Vec<Ownership>,
    },
    /// The runtime serves the ownership, and the agent image can be neither
    /// found nor built.
    ImageUnavailable { demanded: Ownership },
}

impl Finding {
    /// The process exit code: fatal findings are 3.
    pub fn exit_code(&self) -> u8 {
        3
    }
}

pub(crate) fn word(ownership: Ownership) -> &'static str {
    match ownership {
        Ownership::Rootless => "container-rootless",
        Ownership::Rootful => "container-rootful",
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for Finding {
    fn code(&self) -> &str {
        match self {
            Finding::NoRuntime { .. } => code!("no-runtime"),
            Finding::OwnershipMismatch { .. } => code!("ownership-mismatch"),
            Finding::ImageUnavailable { .. } => code!("image-unavailable"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            Finding::NoRuntime { demanded } | Finding::ImageUnavailable { demanded } => {
                vec![("runtime", word(*demanded).into())]
            }
            Finding::OwnershipMismatch { demanded, offered } => {
                let offered: Vec<&str> = offered.iter().map(|o| word(*o)).collect();
                vec![
                    ("runtime", word(*demanded).into()),
                    ("offered", offered.join(", ").as_str().into()),
                ]
            }
        }
    }
}

/// This crate's en-US messages, its name first.
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// One value of every finding, for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(Finding::NoRuntime {
            demanded: Ownership::Rootless,
        }),
        Box::new(Finding::OwnershipMismatch {
            demanded: Ownership::Rootless,
            offered: vec![Ownership::Rootful],
        }),
        Box::new(Finding::ImageUnavailable {
            demanded: Ownership::Rootful,
        }),
    ]
}
