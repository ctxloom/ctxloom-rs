//! The session aggregate on angzarr-embedded's markup: thin handlers over
//! the rules in `lib.rs`.
//!
//! - the instance holds the folded [`SessionState`], read by
//!   [`SessionAggregate::state`];
//! - one `#[handles_cmd]` per command, `ctx.command.had_prior_events` saying
//!   whether the stream has history and `ctx.recorded_at()` the instant its
//!   events are recorded at: no clock;
//! - a bind, a keep, a rename, an adopt return `Option`: a change that
//!   changes nothing records nothing;
//! - every refusal is message-only, a [`SessionRejection`] as a
//!   `Failed::Rejected`;
//! - one `#[applies]` per event, folding at its page's `recorded_at`;
//! - no snapshot: a session's stream is short, and every load replays it.

use angzarr_embedded::markup::{Context, Failed, PageContext};
use angzarr_embedded_macros::{aggregate, applies, handles_cmd};

use crate::{
    adopted, end_recorded, engine_bound, forgotten, kept_changed, purged, renamed, session_minted,
    AdoptRotations, BindEngineSession, EndRecorded, EngineSessionBound, ForgetSession, KeptChanged,
    MintSession, RecordEnd, RecordPurge, RenameSession, RenamedTo, SessionAdopted, SessionCommand,
    SessionEvent, SessionForgotten, SessionMinted, SessionPurged, SessionRejection, SessionState,
    SetKept,
};

/// The session aggregate: one session, folded from its stream.
#[derive(Clone, Debug, Default)]
pub struct SessionAggregate {
    session: SessionState,
}

impl SessionAggregate {
    /// The folded session.
    pub fn state(&self) -> &SessionState {
        &self.session
    }
}

/// `event`, folded into the session at its page's `recorded_at`
/// ([`crate::apply`]).
fn fold(session: &mut SessionState, event: SessionEvent, page: &PageContext) {
    let recorded_at = page.meta.and_then(|meta| meta.recorded_at);
    crate::apply(session, &event, recorded_at.unwrap_or_default());
}

// The domain is SESSION_DOMAIN, spelled as the markup takes it, a literal
// (`aggregate.test.rs` holds them to each other).
#[aggregate(domain = "sessions", command = SessionCommand, event = SessionEvent)]
impl SessionAggregate {
    /// Create the session in the stream the caller addressed: refused on a
    /// stream with history, recorded at `ctx.recorded_at()`.
    #[handles_cmd(MintSession)]
    fn mint(
        &self,
        cmd: MintSession,
        ctx: &Context,
    ) -> Result<SessionMinted, Failed<SessionRejection>> {
        let taken = ctx.command.had_prior_events;
        Ok(session_minted(&self.session, taken, &cmd)?)
    }

    /// Bind the engine's session and transcript; `None` when it changes
    /// nothing.
    #[handles_cmd(BindEngineSession)]
    fn bind(
        &self,
        cmd: BindEngineSession,
        ctx: &Context,
    ) -> Result<Option<EngineSessionBound>, Failed<SessionRejection>> {
        let _ = ctx;
        Ok(engine_bound(&self.session, &cmd)?)
    }

    /// Mark or unmark the transcript kept; `None` when it changes nothing.
    #[handles_cmd(SetKept)]
    fn set_kept(
        &self,
        cmd: SetKept,
        ctx: &Context,
    ) -> Result<Option<KeptChanged>, Failed<SessionRejection>> {
        let _ = ctx;
        Ok(kept_changed(&self.session, &cmd)?)
    }

    /// Rename the session; `None` when the name is the one held.
    #[handles_cmd(RenameSession)]
    fn rename(
        &self,
        cmd: RenameSession,
        ctx: &Context,
    ) -> Result<Option<RenamedTo>, Failed<SessionRejection>> {
        let _ = ctx;
        Ok(renamed(&self.session, &cmd)?)
    }

    /// Record that a purge destroyed the transcript.
    #[handles_cmd(RecordPurge)]
    fn record_purge(
        &self,
        cmd: RecordPurge,
        ctx: &Context,
    ) -> Result<SessionPurged, Failed<SessionRejection>> {
        let _ = ctx;
        Ok(purged(&self.session, &cmd)?)
    }

    /// Forget the session, keeping its stream.
    #[handles_cmd(ForgetSession)]
    fn forget(
        &self,
        cmd: ForgetSession,
        ctx: &Context,
    ) -> Result<SessionForgotten, Failed<SessionRejection>> {
        let _ = (cmd, ctx);
        Ok(forgotten(&self.session)?)
    }

    /// Adopt rotations the lineage lacks; `None` when it lacks none.
    #[handles_cmd(AdoptRotations)]
    fn adopt(
        &self,
        cmd: AdoptRotations,
        ctx: &Context,
    ) -> Result<Option<SessionAdopted>, Failed<SessionRejection>> {
        let _ = ctx;
        Ok(adopted(&self.session, &cmd)?)
    }

    /// Record when the last run ended.
    #[handles_cmd(RecordEnd)]
    fn record_end(
        &self,
        cmd: RecordEnd,
        ctx: &Context,
    ) -> Result<EndRecorded, Failed<SessionRejection>> {
        let _ = ctx;
        Ok(end_recorded(&self.session, &cmd)?)
    }

    #[applies(SessionMinted)]
    fn session_minted(&mut self, event: SessionMinted, page: &PageContext) {
        fold(&mut self.session, event.into(), page);
    }

    #[applies(EngineSessionBound)]
    fn engine_session_bound(&mut self, event: EngineSessionBound, page: &PageContext) {
        fold(&mut self.session, event.into(), page);
    }

    #[applies(KeptChanged)]
    fn kept_changed(&mut self, event: KeptChanged, page: &PageContext) {
        fold(&mut self.session, event.into(), page);
    }

    #[applies(RenamedTo)]
    fn renamed_to(&mut self, event: RenamedTo, page: &PageContext) {
        fold(&mut self.session, event.into(), page);
    }

    #[applies(SessionPurged)]
    fn session_purged(&mut self, event: SessionPurged, page: &PageContext) {
        fold(&mut self.session, event.into(), page);
    }

    #[applies(SessionForgotten)]
    fn session_forgotten(&mut self, event: SessionForgotten, page: &PageContext) {
        fold(&mut self.session, event.into(), page);
    }

    #[applies(SessionAdopted)]
    fn session_adopted(&mut self, event: SessionAdopted, page: &PageContext) {
        fold(&mut self.session, event.into(), page);
    }

    #[applies(EndRecorded)]
    fn end_recorded(&mut self, event: EndRecorded, page: &PageContext) {
        fold(&mut self.session, event.into(), page);
    }
}

#[cfg(test)]
#[path = "aggregate.test.rs"]
mod tests;
