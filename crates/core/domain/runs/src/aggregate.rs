//! The run aggregate on angzarr-embedded's markup: one run per incarnation.
//!
//! Commands are refusable (enqueue, queue a turn, request a stop). Facts are
//! what already happened and are never refused (owner ruling 2026-11-04, item
//! 16, totality): a fact the run cannot use records nothing. A run ends exactly
//! once; folding the end revokes every credential (D29).

use angzarr_embedded::markup::{Context, Failed, PageContext, Record};
use angzarr_embedded_macros::{aggregate, applies, handles_cmd, handles_fact};

use crate::{
    apply, run_root, EndCause, EnqueueRun, FinalReported, FinalReportedRecorded, LaunchFailed,
    QueueTurn, RequestStop, RunCommand, RunEnded, RunEnqueued, RunEvent, RunExited, RunRejection,
    RunState, RunnerAttached, RunnerAttachedRecorded, StopRequested, SummaryReported,
    SummaryReportedRecorded, TurnCompleted, TurnCompletedRecorded, TurnFailed, TurnFailedRecorded,
    TurnQueued, TurnStarted, TurnStartedRecorded,
};

/// The run aggregate: one run, folded from its stream.
#[derive(Clone, Debug, Default)]
pub struct RunAggregate {
    run: RunState,
}

impl RunAggregate {
    /// The folded run.
    pub fn state(&self) -> &RunState {
        &self.run
    }
}

/// A SHA-256 digest's length.
const DIGEST_LEN: usize = 32;

impl RunAggregate {
    /// Whether an enqueue has been folded.
    fn enqueued(&self) -> bool {
        self.run.mode.is_some()
    }

    /// Whether the run can still take an observation: enqueued, not ended.
    fn live(&self) -> bool {
        self.enqueued() && self.run.ended.is_none()
    }

    /// The end of the run, once: `None` when it is not live.
    fn end(&self, cause: EndCause, exit_code: Option<i32>, detail: String) -> Option<RunEnded> {
        self.live().then_some(RunEnded {
            cause,
            exit_code,
            detail,
        })
    }
}

/// A credential hash must be a SHA-256 digest.
fn digest(hash: &[u8]) -> Result<(), RunRejection> {
    match hash.len() == DIGEST_LEN {
        true => Ok(()),
        false => Err(RunRejection::CredentialHashInvalid {
            length: hash.len() as u64,
        }),
    }
}

#[aggregate(domain = "runs", command = RunCommand, event = RunEvent)]
impl RunAggregate {
    /// Enqueue the run: once, on its own derived stream, with a distinct
    /// SHA-256 digest for each principal's token.
    #[handles_cmd(EnqueueRun)]
    fn enqueue(&self, cmd: EnqueueRun, ctx: &Context) -> Result<RunEnqueued, Failed<RunRejection>> {
        if ctx.command.had_prior_events {
            return Err(RunRejection::AlreadyEnqueued.into());
        }
        digest(&cmd.runner_credential_hash)?;
        digest(&cmd.model_credential_hash)?;
        if cmd.runner_credential_hash == cmd.model_credential_hash {
            return Err(RunRejection::CredentialsNotDistinct.into());
        }
        if ctx.root() != Some(&run_root(&cmd.session, cmd.incarnation)) {
            return Err(RunRejection::NotItsStream.into());
        }
        Ok(RunEnqueued {
            session: cmd.session,
            incarnation: cmd.incarnation,
            mode: cmd.mode,
            runner_credential_hash: cmd.runner_credential_hash,
            model_credential_hash: cmd.model_credential_hash,
        })
    }

    /// Queue a turn: one at a time, and none after the end.
    #[handles_cmd(QueueTurn)]
    fn queue_turn(
        &self,
        cmd: QueueTurn,
        ctx: &Context,
    ) -> Result<TurnQueued, Failed<RunRejection>> {
        let _ = ctx;
        if !self.enqueued() {
            return Err(RunRejection::NoRun.into());
        }
        if self.run.ended.is_some() {
            return Err(RunRejection::Ended.into());
        }
        if let Some(turn) = self.run.turn_in_flight {
            return Err(RunRejection::TurnInFlight { turn }.into());
        }
        Ok(TurnQueued {
            prompt_ref: cmd.prompt_ref,
            resume: cmd.resume,
        })
    }

    /// Ask the run to stop: recorded once; the run ends later, by its exit.
    #[handles_cmd(RequestStop)]
    fn request_stop(
        &self,
        cmd: RequestStop,
        ctx: &Context,
    ) -> Result<Option<StopRequested>, Failed<RunRejection>> {
        let _ = ctx;
        if !self.enqueued() {
            return Err(RunRejection::NoRun.into());
        }
        // An ended run has nothing left to stop; a second request changes nothing.
        let stoppable = self.run.ended.is_none() && !self.run.stop_requested;
        Ok(stoppable.then_some(StopRequested { reason: cmd.reason }))
    }

    #[handles_fact(RunnerAttached)]
    fn runner_attached(&self, fact: RunnerAttached, ctx: &Context) -> Record<Self> {
        let _ = ctx;
        Record::of(self.live().then_some(RunnerAttachedRecorded {
            runner_id: fact.runner_id,
            protocol: fact.protocol,
        }))
    }

    #[handles_fact(TurnStarted)]
    fn turn_started(&self, fact: TurnStarted, ctx: &Context) -> Record<Self> {
        let _ = ctx;
        Record::of(
            self.live()
                .then_some(TurnStartedRecorded { turn: fact.turn }),
        )
    }

    #[handles_fact(TurnCompleted)]
    fn turn_completed(&self, fact: TurnCompleted, ctx: &Context) -> Record<Self> {
        let _ = ctx;
        Record::of(self.live().then_some(TurnCompletedRecorded {
            turn: fact.turn,
            native_key: fact.native_key,
            answer_ref: fact.answer_ref,
            exit_code: fact.exit_code,
        }))
    }

    #[handles_fact(TurnFailed)]
    fn turn_failed(&self, fact: TurnFailed, ctx: &Context) -> Record<Self> {
        let _ = ctx;
        Record::of(self.live().then_some(TurnFailedRecorded {
            turn: fact.turn,
            class: fact.class,
            detail: fact.detail,
        }))
    }

    #[handles_fact(RunExited)]
    fn run_exited(&self, fact: RunExited, ctx: &Context) -> Record<Self> {
        let _ = ctx;
        Record::of(self.end(EndCause::RunnerExit, Some(fact.code), String::new()))
    }

    #[handles_fact(LaunchFailed)]
    fn launch_failed(&self, fact: LaunchFailed, ctx: &Context) -> Record<Self> {
        let _ = ctx;
        Record::of(self.end(EndCause::LaunchFailed, None, fact.detail))
    }

    /// A model's claim: advisory evidence, recorded and nothing else (owner
    /// ruling 2026-11-04, item 16). Total: it applies in every state.
    #[handles_fact(SummaryReported)]
    fn summary_reported(&self, fact: SummaryReported, ctx: &Context) -> Record<Self> {
        let _ = ctx;
        Record::of(self.enqueued().then_some(SummaryReportedRecorded {
            turn: fact.turn,
            ordinal: fact.ordinal,
            scope: fact.scope,
            text_ref: fact.text_ref,
        }))
    }

    /// A model's final claim: notifies, transitions nothing (it neither ends
    /// the run nor revokes a credential). Total.
    #[handles_fact(FinalReported)]
    fn final_reported(&self, fact: FinalReported, ctx: &Context) -> Record<Self> {
        let _ = ctx;
        Record::of(self.enqueued().then_some(FinalReportedRecorded {
            turn: fact.turn,
            text_ref: fact.text_ref,
        }))
    }

    #[applies(RunEnqueued)]
    fn run_enqueued(&mut self, event: RunEnqueued, page: &PageContext) {
        apply(&mut self.run, &event.into(), page.sequence);
    }

    #[applies(RunnerAttachedRecorded)]
    fn runner_attached_recorded(&mut self, event: RunnerAttachedRecorded, page: &PageContext) {
        apply(&mut self.run, &event.into(), page.sequence);
    }

    #[applies(TurnQueued)]
    fn turn_queued(&mut self, event: TurnQueued, page: &PageContext) {
        apply(&mut self.run, &event.into(), page.sequence);
    }

    // A turn starting changes nothing the run decides on: its fold is empty, so
    // replacing it with nothing is the same function.
    #[cfg_attr(test, mutants::skip)]
    #[applies(TurnStartedRecorded)]
    fn turn_started_recorded(&mut self, event: TurnStartedRecorded, page: &PageContext) {
        apply(&mut self.run, &event.into(), page.sequence);
    }

    #[applies(TurnCompletedRecorded)]
    fn turn_completed_recorded(&mut self, event: TurnCompletedRecorded, page: &PageContext) {
        apply(&mut self.run, &event.into(), page.sequence);
    }

    #[applies(TurnFailedRecorded)]
    fn turn_failed_recorded(&mut self, event: TurnFailedRecorded, page: &PageContext) {
        apply(&mut self.run, &event.into(), page.sequence);
    }

    #[applies(StopRequested)]
    fn stop_requested(&mut self, event: StopRequested, page: &PageContext) {
        apply(&mut self.run, &event.into(), page.sequence);
    }

    #[applies(SummaryReportedRecorded)]
    fn summary_reported_recorded(&mut self, event: SummaryReportedRecorded, page: &PageContext) {
        apply(&mut self.run, &event.into(), page.sequence);
    }

    #[applies(FinalReportedRecorded)]
    fn final_reported_recorded(&mut self, event: FinalReportedRecorded, page: &PageContext) {
        apply(&mut self.run, &event.into(), page.sequence);
    }

    #[applies(RunEnded)]
    fn run_ended(&mut self, event: RunEnded, page: &PageContext) {
        apply(&mut self.run, &event.into(), page.sequence);
    }
}

#[cfg(test)]
#[path = "aggregate.test.rs"]
mod tests;
