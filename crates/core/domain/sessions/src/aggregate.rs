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

use angzarr_embedded::markup::{Context, Failed, PageContext, Record};
use angzarr_embedded_macros::{aggregate, applies, handles_cmd, handles_fact};

use crate::{
    adopted, end_recorded, engine_bound, forgotten, inbox, kept_changed, lifeline, purged, renamed,
    session_minted, AdoptRotations, BindEngineSession, ClaimMessage, DeliverMessage, EndRecorded,
    EngineSessionBound, FailMessage, ForgetSession, IncarnationEnded, IncarnationOwed, KeptChanged,
    MessageArrived, MessageClaimed, MessageDelivered, MessageFailed, MintSession, RecordEnd,
    RecordIncarnationEnded, RecordMessageArrived, RecordPurge, RenameSession, RenamedTo,
    SessionAdopted, SessionCommand, SessionEvent, SessionForgotten, SessionMinted, SessionPurged,
    SessionRejection, SessionState, SetKept, SteerWithdrawn, StopOrdered, StopSession,
    WithdrawSteer,
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
    ) -> Result<(SessionMinted, IncarnationOwed), Failed<SessionRejection>> {
        let taken = ctx.command.had_prior_events;
        let minted = session_minted(&self.session, taken, &cmd)?;
        // Owner ruling 2026-11-04 (S5 review Q1 (b)): a new session owes its
        // first incarnation in the same decision.
        Ok((minted, lifeline::first_owed()))
    }

    /// Bind the engine's session and transcript: a FACT (owner ruling
    /// 2026-11-04, S5 review item 8), so it is never refused; records nothing
    /// when it changes nothing, or when the stream holds no live session.
    #[handles_fact(BindEngineSession)]
    fn bind(&self, fact: BindEngineSession, ctx: &Context) -> Record<Self> {
        let _ = ctx;
        Record::of(engine_bound(&self.session, &fact).ok().flatten())
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

    /// Order the session's runs to stop; refused from a child.
    #[handles_cmd(StopSession)]
    fn stop_session(
        &self,
        cmd: StopSession,
        ctx: &Context,
    ) -> Result<Option<StopOrdered>, Failed<SessionRejection>> {
        let _ = ctx;
        Ok(lifeline::stop_ordered(&self.session, &cmd)?)
    }

    /// Withdraw a queued steer.
    #[handles_cmd(WithdrawSteer)]
    fn withdraw_steer(
        &self,
        cmd: WithdrawSteer,
        ctx: &Context,
    ) -> Result<Option<SteerWithdrawn>, Failed<SessionRejection>> {
        let _ = ctx;
        Ok(inbox::withdrawn(&self.session, &cmd)?)
    }

    /// A run's end, told by the policy run to session (a fact).
    #[handles_fact(RecordIncarnationEnded)]
    fn record_incarnation_ended(
        &self,
        fact: RecordIncarnationEnded,
        ctx: &Context,
    ) -> Record<Self> {
        let _ = ctx;
        lifeline::ended(&self.session, &fact)
    }

    #[handles_fact(RecordMessageArrived)]
    fn message_arrived(&self, fact: RecordMessageArrived, ctx: &Context) -> Record<Self> {
        let _ = ctx;
        inbox::arrived(&self.session, &fact)
    }

    #[handles_fact(ClaimMessage)]
    fn claim_message(&self, fact: ClaimMessage, ctx: &Context) -> Record<Self> {
        let _ = ctx;
        inbox::claimed(&self.session, &fact)
    }

    #[handles_fact(DeliverMessage)]
    fn deliver_message(&self, fact: DeliverMessage, ctx: &Context) -> Record<Self> {
        let _ = ctx;
        inbox::delivered(&self.session, &fact)
    }

    #[handles_fact(FailMessage)]
    fn fail_message(&self, fact: FailMessage, ctx: &Context) -> Record<Self> {
        let _ = ctx;
        inbox::failed(&self.session, &fact)
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

    #[applies(IncarnationOwed)]
    fn incarnation_owed(&mut self, event: IncarnationOwed, page: &PageContext) {
        fold(&mut self.session, event.into(), page);
    }

    #[applies(IncarnationEnded)]
    fn incarnation_ended(&mut self, event: IncarnationEnded, page: &PageContext) {
        fold(&mut self.session, event.into(), page);
    }

    #[applies(StopOrdered)]
    fn stop_ordered(&mut self, event: StopOrdered, page: &PageContext) {
        fold(&mut self.session, event.into(), page);
    }

    #[applies(SteerWithdrawn)]
    fn steer_withdrawn(&mut self, event: SteerWithdrawn, page: &PageContext) {
        fold(&mut self.session, event.into(), page);
    }

    #[applies(MessageArrived)]
    fn message_arrived_applied(&mut self, event: MessageArrived, page: &PageContext) {
        fold(&mut self.session, event.into(), page);
    }

    #[applies(MessageClaimed)]
    fn message_claimed(&mut self, event: MessageClaimed, page: &PageContext) {
        fold(&mut self.session, event.into(), page);
    }

    #[applies(MessageDelivered)]
    fn message_delivered(&mut self, event: MessageDelivered, page: &PageContext) {
        fold(&mut self.session, event.into(), page);
    }

    #[applies(MessageFailed)]
    fn message_failed(&mut self, event: MessageFailed, page: &PageContext) {
        fold(&mut self.session, event.into(), page);
    }
}

#[cfg(test)]
#[path = "aggregate.test.rs"]
pub(crate) mod tests;
