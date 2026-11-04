//! The session and the name claim, folded from their events, and their wire
//! forms.

use angzarr_embedded::{Timestamp, WireError};

use crate::{Origin, Rotation};

/// The sessions domain's name: every session stream's `Cover.domain`.
pub const SESSION_DOMAIN: &str = "sessions";

/// The name claims' domain: every claim stream's `Cover.domain`.
pub const NAMES_DOMAIN: &str = "session-names";

/// The session, folded from its events. Derived values (its title, its
/// state now) are computed when read, never stored.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SessionState {
    pub harp: String,
    pub project_id: String,
    pub project_dir: String,
    pub output_dir: String,
    pub engine: String,
    pub origin: Option<Origin>,
    pub native_id: String,
    pub transcript_path: String,
    pub engine_version: String,
    /// Every binding displaced, oldest first.
    pub rotations: Vec<Rotation>,
    pub kept: bool,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    pub purged_at: Option<Timestamp>,
    pub forgotten: bool,
    /// The run lifeline (plan 3.1): the highest incarnation the session has
    /// owed; 0 until the first.
    pub incarnation: u32,
    /// The incarnation owed or running and not yet ended, if any.
    pub open_incarnation: Option<u32>,
    /// Relaunches spent since the last mail was consumed (the budget is the
    /// SESSION's, per harp, not per run).
    pub relaunches: u32,
    /// Set by a stop order; no incarnation is owed after it.
    pub stopped: bool,
    /// The inbox, oldest first.
    pub inbox: Vec<InboxEntry>,
}

/// Where a message is in the session's inbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InboxStatus {
    Queued,
    Claimed,
    Delivered,
    Withdrawn,
    Failed,
}

/// One message in the session's inbox (the former mailbox, D5 Q4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InboxEntry {
    pub message_id: String,
    pub from: String,
    pub kind: String,
    pub body_ref: String,
    pub status: InboxStatus,
    pub claimant: String,
    pub at_turn: Option<u64>,
}

impl From<SessionState> for sessions_proto::v1::SessionState {
    fn from(value: SessionState) -> Self {
        sessions_proto::v1::SessionState {
            harp: value.harp,
            project_id: value.project_id,
            project_dir: value.project_dir,
            output_dir: value.output_dir,
            engine: value.engine,
            origin: value.origin.map_or(0, i32::from),
            native_id: value.native_id,
            transcript_path: value.transcript_path,
            engine_version: value.engine_version,
            rotations: value
                .rotations
                .into_iter()
                .map(sessions_proto::v1::Rotation::from)
                .collect(),
            kept: value.kept,
            started_at: Some(value.started_at),
            ended_at: value.ended_at,
            purged_at: value.purged_at,
            forgotten: value.forgotten,
        }
    }
}

impl TryFrom<sessions_proto::v1::SessionState> for SessionState {
    type Error = WireError;

    fn try_from(value: sessions_proto::v1::SessionState) -> Result<Self, WireError> {
        let origin = match value.origin {
            0 => None,
            other => Some(
                Origin::try_from(other).map_err(|value| WireError::UnknownEnum {
                    message: "ctxloom.sessions.v1.SessionState",
                    field: "origin",
                    value,
                })?,
            ),
        };
        Ok(SessionState {
            harp: value.harp,
            project_id: value.project_id,
            project_dir: value.project_dir,
            output_dir: value.output_dir,
            engine: value.engine,
            origin,
            native_id: value.native_id,
            transcript_path: value.transcript_path,
            engine_version: value.engine_version,
            rotations: value
                .rotations
                .into_iter()
                .map(Rotation::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            kept: value.kept,
            started_at: value.started_at.ok_or(WireError::Missing {
                message: "ctxloom.sessions.v1.SessionState",
                field: "started_at",
            })?,
            ended_at: value.ended_at,
            purged_at: value.purged_at,
            forgotten: value.forgotten,
            ..SessionState::default()
        })
    }
}

/// A name's claim, folded from its events: held by a session, or free.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NameClaimState {
    pub project_id: String,
    pub harp: String,
    /// The stable id of the holder; empty while the name is free.
    pub session: String,
    /// Whether the stream has history.
    pub claimed: bool,
}

impl From<NameClaimState> for sessions_proto::v1::NameClaimState {
    fn from(value: NameClaimState) -> Self {
        sessions_proto::v1::NameClaimState {
            project_id: value.project_id,
            harp: value.harp,
            session: value.session,
            claimed: value.claimed,
        }
    }
}

impl TryFrom<sessions_proto::v1::NameClaimState> for NameClaimState {
    type Error = WireError;

    fn try_from(value: sessions_proto::v1::NameClaimState) -> Result<Self, WireError> {
        Ok(NameClaimState {
            project_id: value.project_id,
            harp: value.harp,
            session: value.session,
            claimed: value.claimed,
        })
    }
}
