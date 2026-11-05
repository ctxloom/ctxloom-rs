//! Why an isolation use case did not complete.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};
use isolation_ports::Failure;

#[derive(Debug)]
pub enum IsolationError {
    /// A worktree command named a session that has no directory.
    NoSession { harp: String },
    /// `session worktrees purge --yes` removed nothing: an action that
    /// changed nothing refuses, so an unattended run cannot be mistaken for
    /// one that cleaned up.
    NothingRemoved {
        harp: String,
        spared: u64,
        skipped: u64,
    },
    /// A backend that is none of the engines.
    UnknownBackend { backend: String, available: String },
    /// A `--runtime` that is none of the runtimes ctxloom knows.
    UnknownRuntime { runtime: String, known: String },
    /// `container prune` on a machine with no container runtime: its own
    /// exit status (3), distinct from a removal that failed (1).
    NoPruneRuntime,
    /// `container build` on a machine with no container runtime.
    NoBuildRuntime,
    /// A scaffold over a project that already has a devcontainer.
    DevcontainerExists { path: String },
    /// A command whose work needs a live container runtime, which this port
    /// does not do yet.
    NotPorted { command: String },
    /// An adapter's own failure.
    Adapter(Failure),
}

impl IsolationError {
    /// The process exit code.
    pub fn exit_code(&self) -> u8 {
        match self {
            IsolationError::NothingRemoved { .. }
            | IsolationError::UnknownBackend { .. }
            | IsolationError::UnknownRuntime { .. } => 2,
            IsolationError::NoPruneRuntime | IsolationError::NoBuildRuntime => 3,
            IsolationError::NoSession { .. }
            | IsolationError::DevcontainerExists { .. }
            | IsolationError::NotPorted { .. }
            | IsolationError::Adapter(_) => 1,
        }
    }
}

impl fmt::Display for IsolationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for IsolationError {
    fn code(&self) -> &str {
        match self {
            IsolationError::NoSession { .. } => code!("no-session"),
            IsolationError::NothingRemoved { .. } => code!("nothing-removed"),
            IsolationError::UnknownBackend { .. } => code!("unknown-backend"),
            IsolationError::UnknownRuntime { .. } => code!("unknown-runtime"),
            IsolationError::NoPruneRuntime => code!("no-prune-runtime"),
            IsolationError::NoBuildRuntime => code!("no-build-runtime"),
            IsolationError::DevcontainerExists { .. } => code!("devcontainer-exists"),
            IsolationError::NotPorted { .. } => code!("not-ported"),
            IsolationError::Adapter(cause) => cause.code(),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            IsolationError::NoSession { harp } => vec![("harp", harp.as_str().into())],
            IsolationError::NothingRemoved {
                harp,
                spared,
                skipped,
            } => vec![
                ("harp", harp.as_str().into()),
                ("spared", (*spared).into()),
                ("skipped", (*skipped).into()),
            ],
            IsolationError::UnknownBackend { backend, available } => vec![
                ("backend", backend.as_str().into()),
                ("available", available.as_str().into()),
            ],
            IsolationError::UnknownRuntime { runtime, known } => vec![
                ("runtime", runtime.as_str().into()),
                ("known", known.as_str().into()),
            ],
            IsolationError::NoPruneRuntime | IsolationError::NoBuildRuntime => Vec::new(),
            IsolationError::DevcontainerExists { path } => vec![("path", path.as_str().into())],
            IsolationError::NotPorted { command } => vec![("command", command.as_str().into())],
            IsolationError::Adapter(cause) => cause.args(),
        }
    }
}

/// This crate's en-US messages, its name first.
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// Every catalog this crate's diagnostics are rendered from: its own and the
/// decision table's findings'.
pub const CATALOGS: [(&str, &str); 2] = [EN_US, isolation::EN_US];

/// One value of every error this crate defines (not the wrapper, whose code
/// is its cause's), for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let mut all: Vec<Box<dyn Diagnostic>> = vec![
        Box::new(IsolationError::NoSession {
            harp: "amber-swift-owl".to_string(),
        }),
        Box::new(IsolationError::NothingRemoved {
            harp: "amber-swift-owl".to_string(),
            spared: 1,
            skipped: 2,
        }),
        Box::new(IsolationError::UnknownBackend {
            backend: "bogus".to_string(),
            available: "claude-code, mock".to_string(),
        }),
        Box::new(IsolationError::UnknownRuntime {
            runtime: "rkt".to_string(),
            known: "docker|podman".to_string(),
        }),
        Box::new(IsolationError::NoPruneRuntime),
        Box::new(IsolationError::NoBuildRuntime),
        Box::new(IsolationError::DevcontainerExists {
            path: ".devcontainer".to_string(),
        }),
        Box::new(IsolationError::NotPorted {
            command: "container build".to_string(),
        }),
    ];
    all.extend(isolation::diagnostic_samples());
    all
}
