//! Why a remote or dependency command did not complete, and what it noticed
//! on the way. Every message is in `locales/en-US/messages.ftl`; a read-model
//! or adapter error passes through as itself.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic, Report};

/// This crate's en-US messages (`locales/en-US/messages.ftl`, ids under the
/// crate's name).
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// A command that did not complete.
#[derive(Debug)]
pub enum AppError {
    /// An error of the read model or an adapter, as itself.
    Cause(Box<dyn Diagnostic>),
    /// A file could not be read or written.
    File { what: String, detail: String },
    /// A name or an address left out.
    Required { what: String },
    /// A repository no remote is registered for.
    NotRegistered { url: String },
    /// The same item referenced at differing commits.
    Conflict { text: String },
    /// Dependencies that could not be installed.
    PullFailed { count: u64, refs: String },
    /// A remote that could not be browsed.
    Browse { remote: String, detail: String },
    /// A remote preview or removal of one that is not there.
    NoRemoteNamed { name: String },
    /// `remote default` without a name or `--clear`.
    DefaultNeedsName,
    /// A name that is neither a local bundle nor a locked dependency.
    NothingToHold { name: String, verb: String },
    /// A bundle of this project that is not there.
    BundleNotFound { name: String },
    /// A move to a place that is neither a remote nor a directory.
    BadDestination { to: String, known: String },
    /// A move onto a bundle already there.
    DestinationExists { dest: String },
    /// A move onto the bundle's own directory.
    DestinationItself { dest: String },
    /// A publish that did not land.
    Publish { remote: String, detail: String },
    /// A pull request, which needs a forge's API.
    PullRequestNeedsForge,
    /// `remote discover`, which needs a forge's API.
    DiscoverNeedsForge,
    /// A single-reference check whose repository could not be fetched.
    CloneNotRefreshed { url: String, detail: String },
    /// A reference that does not name a remote bundle.
    NotRemoteReference { reference: String },
    /// A clone that could not be read for a bundle.
    Unreadable { what: String, detail: String },
}

impl AppError {
    /// An error of the read model or an adapter, as itself.
    pub fn cause(error: impl Diagnostic + 'static) -> AppError {
        AppError::Cause(Box::new(error))
    }

    /// The exit code: 1 failed, 2 refused before any state was read.
    pub fn exit_code(&self) -> u8 {
        match self {
            AppError::Required { .. }
            | AppError::DefaultNeedsName
            | AppError::PullRequestNeedsForge
            | AppError::DiscoverNeedsForge => 2,
            _ => 1,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl std::error::Error for AppError {}

impl Diagnostic for AppError {
    fn code(&self) -> &str {
        match self {
            AppError::Cause(cause) => cause.code(),
            AppError::File { .. } => code!("file"),
            AppError::Required { .. } => code!("required"),
            AppError::NotRegistered { .. } => code!("not-registered"),
            AppError::Conflict { .. } => code!("conflict"),
            AppError::PullFailed { .. } => code!("pull-failed"),
            AppError::Browse { .. } => code!("browse"),
            AppError::NoRemoteNamed { .. } => code!("no-remote-named"),
            AppError::DefaultNeedsName => code!("default-needs-name"),
            AppError::NothingToHold { .. } => code!("nothing-to-hold"),
            AppError::BundleNotFound { .. } => code!("bundle-not-found"),
            AppError::BadDestination { .. } => code!("bad-destination"),
            AppError::DestinationExists { .. } => code!("destination-exists"),
            AppError::DestinationItself { .. } => code!("destination-itself"),
            AppError::Publish { .. } => code!("publish"),
            AppError::PullRequestNeedsForge => code!("pull-request-needs-forge"),
            AppError::DiscoverNeedsForge => code!("discover-needs-forge"),
            AppError::CloneNotRefreshed { .. } => code!("clone-not-refreshed"),
            AppError::NotRemoteReference { .. } => code!("not-remote-reference"),
            AppError::Unreadable { .. } => code!("unreadable"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            AppError::Cause(cause) => cause.args(),
            AppError::File { what, detail } | AppError::Unreadable { what, detail } => vec![
                ("what", what.as_str().into()),
                ("detail", detail.as_str().into()),
            ],
            AppError::Required { what } => vec![("what", what.as_str().into())],
            AppError::NotRegistered { url } => vec![("url", url.as_str().into())],
            AppError::Conflict { text } => vec![("text", text.as_str().into())],
            AppError::PullFailed { count, refs } => {
                vec![("count", (*count).into()), ("refs", refs.as_str().into())]
            }
            AppError::Browse { remote, detail } | AppError::Publish { remote, detail } => vec![
                ("remote", remote.as_str().into()),
                ("detail", detail.as_str().into()),
            ],
            AppError::NoRemoteNamed { name } | AppError::BundleNotFound { name } => {
                vec![("name", name.as_str().into())]
            }
            AppError::DefaultNeedsName
            | AppError::PullRequestNeedsForge
            | AppError::DiscoverNeedsForge => Vec::new(),
            AppError::NothingToHold { name, verb } => vec![
                ("name", name.as_str().into()),
                ("verb", verb.as_str().into()),
            ],
            AppError::BadDestination { to, known } => {
                vec![("to", to.as_str().into()), ("known", known.as_str().into())]
            }
            AppError::DestinationExists { dest } | AppError::DestinationItself { dest } => {
                vec![("dest", dest.as_str().into())]
            }
            AppError::CloneNotRefreshed { url, detail } => vec![
                ("url", url.as_str().into()),
                ("detail", detail.as_str().into()),
            ],
            AppError::NotRemoteReference { reference } => {
                vec![("reference", reference.as_str().into())]
            }
        }
    }
}

/// Something a command noticed and did not stop for: said on the error stream
/// as a warning, with what to do about it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Notice {
    /// In plain words, and what to do about it (empty for nothing).
    Text { text: String, remedy: String },
    /// A part of the closure that could not be read, and why.
    Unexpanded { what: String, cause: Report },
    /// What a pin brings in, which could not be listed, and why.
    Unlisted {
        identity: String,
        sha: String,
        cause: Report,
    },
}

impl Notice {
    /// A warning in plain words.
    pub fn text(text: impl Into<String>) -> Notice {
        Notice::Text {
            text: text.into(),
            remedy: String::new(),
        }
    }

    /// A warning with what to do about it.
    pub fn with_fix(text: impl Into<String>, remedy: impl Into<String>) -> Notice {
        Notice::Text {
            text: text.into(),
            remedy: remedy.into(),
        }
    }

    /// What a pin brings in, which could not be listed.
    pub fn unlisted(identity: &str, sha: &str, cause: &dyn Diagnostic) -> Notice {
        Notice::Unlisted {
            identity: identity.to_string(),
            sha: sha.get(..7).unwrap_or(sha).to_string(),
            cause: Report::of(cause),
        }
    }

    /// A part of the closure that could not be read.
    pub fn unexpanded(what: impl Into<String>, cause: &dyn Diagnostic) -> Notice {
        Notice::Unexpanded {
            what: what.into(),
            cause: Report::of(cause),
        }
    }
}

impl fmt::Display for Notice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for Notice {
    fn code(&self) -> &str {
        match self {
            Notice::Text { .. } => code!("notice"),
            Notice::Unexpanded { .. } => code!("notice-unexpanded"),
            Notice::Unlisted { .. } => code!("notice-unlisted"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            Notice::Text { text, remedy } => vec![
                ("text", text.as_str().into()),
                ("remedy", remedy.as_str().into()),
            ],
            Notice::Unexpanded { what, cause } => vec![
                ("what", what.as_str().into()),
                ("cause", Arg::Nested(Box::new(cause.clone()))),
            ],
            Notice::Unlisted {
                identity,
                sha,
                cause,
            } => vec![
                ("identity", identity.as_str().into()),
                ("sha", sha.as_str().into()),
                ("cause", Arg::Nested(Box::new(cause.clone()))),
            ],
        }
    }
}

/// One value of every error here, for a catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let s = |text: &str| text.to_string();
    vec![
        Box::new(AppError::File {
            what: s("lock.yaml"),
            detail: s("permission denied"),
        }),
        Box::new(AppError::Required { what: s("name") }),
        Box::new(AppError::NotRegistered {
            url: s("file:///srv/r.git"),
        }),
        Box::new(AppError::Conflict {
            text: s("a is referenced at 1 and 2"),
        }),
        Box::new(AppError::PullFailed {
            count: 1,
            refs: s("ctxloom+git://h/r//bundles/a"),
        }),
        Box::new(AppError::Browse {
            remote: s("origin"),
            detail: s("could not read"),
        }),
        Box::new(AppError::NoRemoteNamed { name: s("origin") }),
        Box::new(AppError::DefaultNeedsName),
        Box::new(AppError::NothingToHold {
            name: s("typo"),
            verb: s("hold"),
        }),
        Box::new(AppError::BundleNotFound { name: s("demo") }),
        Box::new(AppError::BadDestination {
            to: s("nowhere"),
            known: s(" (configured remotes: origin)"),
        }),
        Box::new(AppError::DestinationExists {
            dest: s("other/.ctxloom/content/bundles/v2/demo"),
        }),
        Box::new(AppError::DestinationItself { dest: s("demo") }),
        Box::new(AppError::Publish {
            remote: s("origin"),
            detail: s("rejected"),
        }),
        Box::new(AppError::PullRequestNeedsForge),
        Box::new(AppError::DiscoverNeedsForge),
        Box::new(AppError::CloneNotRefreshed {
            url: s("file:///srv/r.git"),
            detail: s("unreachable"),
        }),
        Box::new(AppError::NotRemoteReference {
            reference: s("demo"),
        }),
        Box::new(AppError::Unreadable {
            what: s("demo"),
            detail: s("no bundle.yaml"),
        }),
        Box::new(Notice::with_fix("fetch failed", "retry")),
        Box::new(Notice::unlisted(
            "ctxloom+git://h/r//bundles/a",
            "0123456789",
            &AppError::NoRemoteNamed { name: s("origin") },
        )),
        Box::new(Notice::unexpanded(
            "ctxloom+git://h/r//bundles/a",
            &AppError::NoRemoteNamed { name: s("origin") },
        )),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
