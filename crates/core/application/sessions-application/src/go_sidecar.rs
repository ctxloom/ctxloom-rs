//! Go ctxloom's session sidecar, `~/.ctxloom/sessions/<harp>/session.yaml`,
//! schema 1 to 3 (release/0.7): read through the one reader (angzarr-yaml's
//! versioned, strict decode) by `session import` alone. Go's sidecar is the
//! session's whole record; here its facts become events, once.
//!
//! - schema 2 added `keep` (a marker generation) and schema 3 dropped
//!   `source_entries` (session compaction is gone): both migrate on read;
//! - a sidecar with no `schema_version` is refused as too old, and a newer one
//!   as too new, naming both numbers (Go: `schemaver.ErrTooOld`, `ErrNewer`);
//! - decoding is strict: a key this build does not know is refused (owner
//!   ruling, 2026-10-31: strict for all files, Go-written ones included).

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic, Report};
use angzarr_embedded::Timestamp;
use angzarr_yaml::from_value;
use angzarr_yaml::versioned::{remove_key, LoadError, LoadProblem, Migrations};
use serde::Deserialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// A session as Go's sidecar records it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GoEntry {
    /// The engine's own session id for the current binding; empty until bound.
    pub session_id: String,
    pub backend: String,
    pub project_dir: String,
    pub output_dir: String,
    /// The epoch where the sidecar records none.
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    pub transcript_path: String,
    pub purged_at: Option<Timestamp>,
    pub keep: bool,
    pub engine_version: String,
    /// `session`, `agent` or `oneshot`; empty reads as a human's session.
    pub origin: String,
    /// Every binding displaced, oldest first.
    pub rotations: Vec<GoRotation>,
    pub run_ends: Vec<GoRunEnd>,
    pub end_reason: Option<GoEndReason>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GoRotation {
    pub session_id: String,
    pub transcript_path: String,
    pub rotated_at: Timestamp,
}

/// How one run ended (Go's `RunEnd`).
#[derive(Clone, Debug, PartialEq)]
pub struct GoRunEnd {
    pub run_id: String,
    pub at: Timestamp,
    pub cause: String,
    pub detail: String,
    pub exit_code: Option<i64>,
    pub failed: bool,
}

/// Why a session ended before its engine launched (Go's `EndReason`).
#[derive(Clone, Debug, PartialEq)]
pub struct GoEndReason {
    pub exit: i64,
    pub class: String,
    pub message: String,
    pub findings: Vec<String>,
}

/// Why a sidecar could not be read.
#[derive(Clone, PartialEq, Debug)]
pub enum GoSidecarError {
    /// Written by a newer build than this one reads.
    TooNew { found: u32, current: u32 },
    /// Older than the oldest generation this build reads; a sidecar that
    /// declares none is version 0.
    TooOld { found: u32, oldest: u32 },
    /// An instant that is not RFC 3339.
    Instant { field: String, value: String },
    /// Anything else: not YAML this reader reads, a key it does not know, a
    /// value of the wrong kind.
    Unreadable(Report),
}

impl fmt::Display for GoSidecarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for GoSidecarError {
    fn code(&self) -> &str {
        match self {
            GoSidecarError::TooNew { .. } => code!("go-sidecar-too-new"),
            GoSidecarError::TooOld { .. } => code!("go-sidecar-too-old"),
            GoSidecarError::Instant { .. } => code!("go-sidecar-instant"),
            GoSidecarError::Unreadable(_) => code!("go-sidecar-unreadable"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            GoSidecarError::TooNew { found, current } => vec![
                ("found", u64::from(*found).into()),
                ("current", u64::from(*current).into()),
            ],
            GoSidecarError::TooOld { found, oldest } => vec![
                ("found", u64::from(*found).into()),
                ("oldest", u64::from(*oldest).into()),
            ],
            GoSidecarError::Instant { field, value } => vec![
                ("field", field.as_str().into()),
                ("value", value.as_str().into()),
            ],
            GoSidecarError::Unreadable(cause) => {
                vec![("error", Arg::Nested(Box::new(cause.clone())))]
            }
        }
    }
}

/// The generation Go 0.7 writes.
pub const CURRENT: u32 = 3;

/// Read a sidecar's text, migrating it to the current generation in memory
/// (nothing is written back: Go's files are left exactly as they are).
pub fn decode(text: &str) -> Result<GoEntry, GoSidecarError> {
    let loader = Migrations::new("session sidecar", CURRENT)
        // 1 to 2 added `keep`, a marker generation: a generation-1 sidecar
        // carries no mark and needs no edit.
        .step(1, |_| Ok(()))
        // 2 to 3 dropped `source_entries`.
        .step(2, |doc| remove_key(doc, "source_entries").map(drop))
        .build()
        .map_err(|e| GoSidecarError::Unreadable(Report::of(&e)))?;
    let raw: Raw = loader
        .load_as(None, text, |doc| from_value::<Raw>(doc))
        .map_err(refused)?;
    raw.entry()
}

/// A load that failed, as the sidecar's own errors say it.
fn refused(error: LoadError) -> GoSidecarError {
    match error.problem {
        LoadProblem::TooNew { found, current } => GoSidecarError::TooNew { found, current },
        LoadProblem::TooOld { found, oldest } => GoSidecarError::TooOld { found, oldest },
        _ => GoSidecarError::Unreadable(Report::of(&error)),
    }
}

/// The sidecar as its current generation decodes: the instants still text.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    /// The stamp the loader leaves: the current generation.
    schema_version: u32,
    session_id: Option<String>,
    backend: Option<String>,
    project_dir: Option<String>,
    output_dir: Option<String>,
    started_at: Option<String>,
    ended_at: Option<String>,
    transcript_path: Option<String>,
    purged_at: Option<String>,
    keep: Option<bool>,
    engine_version: Option<String>,
    origin: Option<String>,
    rotations: Option<Vec<RawRotation>>,
    run_ends: Option<Vec<RawRunEnd>>,
    end_reason: Option<RawEndReason>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRotation {
    session_id: String,
    transcript_path: Option<String>,
    rotated_at: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRunEnd {
    run_id: String,
    at: String,
    cause: String,
    detail: Option<String>,
    exit_code: Option<i64>,
    failed: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEndReason {
    exit: i64,
    class: String,
    message: Option<String>,
    findings: Option<Vec<String>>,
}

impl Raw {
    fn entry(self) -> Result<GoEntry, GoSidecarError> {
        debug_assert_eq!(
            self.schema_version, CURRENT,
            "the loader stamps the current generation"
        );
        let instant = |field: &str, value: &str| {
            OffsetDateTime::parse(value, &Rfc3339)
                .ok()
                .and_then(|at| {
                    Some(Timestamp {
                        seconds: at.unix_timestamp(),
                        nanos: i32::try_from(at.nanosecond()).ok()?,
                    })
                })
                .ok_or_else(|| GoSidecarError::Instant {
                    field: field.to_string(),
                    value: value.to_string(),
                })
        };
        let optional = |field: &str, value: Option<&String>| {
            value.map(|text| instant(field, text)).transpose()
        };
        let mut rotations = Vec::new();
        for rotation in self.rotations.unwrap_or_default() {
            rotations.push(GoRotation {
                session_id: rotation.session_id,
                transcript_path: rotation.transcript_path.unwrap_or_default(),
                rotated_at: instant("rotations.rotated_at", &rotation.rotated_at)?,
            });
        }
        let mut run_ends = Vec::new();
        for end in self.run_ends.unwrap_or_default() {
            run_ends.push(GoRunEnd {
                at: instant("run_ends.at", &end.at)?,
                run_id: end.run_id,
                cause: end.cause,
                detail: end.detail.unwrap_or_default(),
                exit_code: end.exit_code,
                failed: end.failed.unwrap_or_default(),
            });
        }
        Ok(GoEntry {
            session_id: self.session_id.unwrap_or_default(),
            backend: self.backend.unwrap_or_default(),
            project_dir: self.project_dir.unwrap_or_default(),
            output_dir: self.output_dir.unwrap_or_default(),
            started_at: optional("started_at", self.started_at.as_ref())?.unwrap_or_default(),
            ended_at: optional("ended_at", self.ended_at.as_ref())?,
            transcript_path: self.transcript_path.unwrap_or_default(),
            purged_at: optional("purged_at", self.purged_at.as_ref())?,
            keep: self.keep.unwrap_or_default(),
            engine_version: self.engine_version.unwrap_or_default(),
            origin: self.origin.unwrap_or_default(),
            rotations,
            run_ends,
            end_reason: self.end_reason.map(|reason| GoEndReason {
                exit: reason.exit,
                class: reason.class,
                message: reason.message.unwrap_or_default(),
                findings: reason.findings.unwrap_or_default(),
            }),
        })
    }
}

/// One value of every error here, for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(GoSidecarError::TooNew {
            found: 4,
            current: 3,
        }),
        Box::new(GoSidecarError::TooOld {
            found: 0,
            oldest: 1,
        }),
        Box::new(GoSidecarError::Instant {
            field: "started_at".to_string(),
            value: "yesterday".to_string(),
        }),
        Box::new(GoSidecarError::Unreadable(Report::new("x-parse"))),
    ]
}

#[cfg(test)]
#[path = "go_sidecar.test.rs"]
mod tests;
