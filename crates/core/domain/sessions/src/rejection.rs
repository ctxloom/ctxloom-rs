//! Why a command to a session, or to a name's claim, was refused.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SessionRejection {
    /// A mint on a stream that already holds a session.
    SessionExists { harp: String },
    /// A command to a stream holding no session.
    NoSession,
    /// A command to a session that was forgotten (`session remove --yes`).
    Forgotten { harp: String },
    /// A harp that cannot be a session's name; `reason` is one of `empty`,
    /// `directory`, `separator`, `control`, `whitespace` (Go's
    /// `harp.Validate`).
    HarpInvalid { harp: String, reason: String },
    /// A name a human chose that is past the bound (Go's `MaxNameLen`).
    HarpTooLong {
        harp: String,
        limit: u64,
        length: u64,
    },
    /// A claim on a name another session holds.
    NamesTaken { harp: String },
    /// A release of a name nobody holds.
    NoClaim,
    /// A release by a session that does not hold the name.
    NotHolder { harp: String },
    /// An incarnation owed while another is owed or running.
    IncarnationOpen { incarnation: u64 },
    /// An incarnation owed after a stop order.
    Stopped,
    /// A stop ordered by a child (Go's I2); `depth` is the requester's.
    ChildCannotStop { depth: u64 },
    /// A message the inbox does not hold.
    UnknownMessage { message_id: String },
    /// A steer withdrawn after it was delivered.
    AlreadyDelivered { message_id: String },
}

/// Its key and args, never English (angzarr decision 14.37).
impl fmt::Display for SessionRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for SessionRejection {
    fn code(&self) -> &str {
        match self {
            SessionRejection::SessionExists { .. } => code!("session-exists"),
            SessionRejection::NoSession => code!("no-session"),
            SessionRejection::Forgotten { .. } => code!("forgotten"),
            SessionRejection::HarpInvalid { .. } => code!("harp-invalid"),
            SessionRejection::HarpTooLong { .. } => code!("harp-too-long"),
            SessionRejection::NamesTaken { .. } => code!("names-taken"),
            SessionRejection::NoClaim => code!("no-claim"),
            SessionRejection::NotHolder { .. } => code!("not-holder"),
            SessionRejection::IncarnationOpen { .. } => code!("incarnation-open"),
            SessionRejection::Stopped => code!("stopped"),
            SessionRejection::ChildCannotStop { .. } => code!("child-cannot-stop"),
            SessionRejection::UnknownMessage { .. } => code!("unknown-message"),
            SessionRejection::AlreadyDelivered { .. } => code!("already-delivered"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            SessionRejection::SessionExists { harp }
            | SessionRejection::Forgotten { harp }
            | SessionRejection::NamesTaken { harp }
            | SessionRejection::NotHolder { harp } => vec![("harp", harp.as_str().into())],
            SessionRejection::HarpInvalid { harp, reason } => vec![
                ("harp", harp.as_str().into()),
                ("reason", reason.as_str().into()),
            ],
            SessionRejection::HarpTooLong {
                harp,
                limit,
                length,
            } => vec![
                ("harp", harp.as_str().into()),
                ("limit", (*limit).into()),
                ("length", (*length).into()),
            ],
            SessionRejection::IncarnationOpen { incarnation } => {
                vec![("incarnation", (*incarnation).into())]
            }
            SessionRejection::ChildCannotStop { depth } => vec![("depth", (*depth).into())],
            SessionRejection::UnknownMessage { message_id }
            | SessionRejection::AlreadyDelivered { message_id } => {
                vec![("message_id", message_id.as_str().into())]
            }
            SessionRejection::NoSession | SessionRejection::NoClaim | SessionRejection::Stopped => {
                Vec::new()
            }
        }
    }
}

/// This crate's en-US messages, its name first: what an edge's catalog
/// embeds to render its diagnostics.
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// One value of every rejection, for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let harp = || "amber-swift-owl".to_string();
    vec![
        Box::new(SessionRejection::SessionExists { harp: harp() }),
        Box::new(SessionRejection::NoSession),
        Box::new(SessionRejection::Forgotten { harp: harp() }),
        Box::new(SessionRejection::HarpInvalid {
            harp: "a/b".to_string(),
            reason: "separator".to_string(),
        }),
        Box::new(SessionRejection::HarpTooLong {
            harp: harp(),
            limit: 24,
            length: 30,
        }),
        Box::new(SessionRejection::NamesTaken { harp: harp() }),
        Box::new(SessionRejection::NoClaim),
        Box::new(SessionRejection::NotHolder { harp: harp() }),
        Box::new(SessionRejection::IncarnationOpen { incarnation: 1 }),
        Box::new(SessionRejection::Stopped),
        Box::new(SessionRejection::ChildCannotStop { depth: 1 }),
        Box::new(SessionRejection::UnknownMessage {
            message_id: "m1".to_string(),
        }),
        Box::new(SessionRejection::AlreadyDelivered {
            message_id: "m1".to_string(),
        }),
    ]
}
