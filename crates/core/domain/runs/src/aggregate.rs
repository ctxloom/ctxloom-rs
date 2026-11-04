//! The run aggregate on angzarr-embedded's markup. STUBS (S5 red): every
//! handler and applier is `todo!`; the tests hold the behaviour.

use angzarr_embedded::markup::{Context, Failed, PageContext, Record};
use angzarr_embedded_macros::{aggregate, applies, handles_cmd, handles_fact};

use crate::{
    EnqueueRun, LaunchFailed, QueueTurn, RequestStop, RunCommand, RunEnded, RunEnqueued, RunEvent,
    RunExited, RunRejection, RunState, RunnerAttached, RunnerAttachedRecorded, StopRequested,
    TurnCompleted, TurnCompletedRecorded, TurnFailed, TurnFailedRecorded, TurnQueued, TurnStarted,
    TurnStartedRecorded,
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

#[aggregate(domain = "runs", command = RunCommand, event = RunEvent)]
impl RunAggregate {
    #[handles_cmd(EnqueueRun)]
    fn enqueue(&self, cmd: EnqueueRun, ctx: &Context) -> Result<RunEnqueued, Failed<RunRejection>> {
        let _ = (cmd, ctx);
        todo!("S5 stub")
    }

    #[handles_cmd(QueueTurn)]
    fn queue_turn(
        &self,
        cmd: QueueTurn,
        ctx: &Context,
    ) -> Result<TurnQueued, Failed<RunRejection>> {
        let _ = (cmd, ctx);
        todo!("S5 stub")
    }

    #[handles_cmd(RequestStop)]
    fn request_stop(
        &self,
        cmd: RequestStop,
        ctx: &Context,
    ) -> Result<Option<StopRequested>, Failed<RunRejection>> {
        let _ = (cmd, ctx);
        todo!("S5 stub")
    }

    #[handles_fact(RunnerAttached)]
    fn runner_attached(&self, fact: RunnerAttached, ctx: &Context) -> Record<Self> {
        let _ = (fact, ctx);
        todo!("S5 stub")
    }

    #[handles_fact(TurnStarted)]
    fn turn_started(&self, fact: TurnStarted, ctx: &Context) -> Record<Self> {
        let _ = (fact, ctx);
        todo!("S5 stub")
    }

    #[handles_fact(TurnCompleted)]
    fn turn_completed(&self, fact: TurnCompleted, ctx: &Context) -> Record<Self> {
        let _ = (fact, ctx);
        todo!("S5 stub")
    }

    #[handles_fact(TurnFailed)]
    fn turn_failed(&self, fact: TurnFailed, ctx: &Context) -> Record<Self> {
        let _ = (fact, ctx);
        todo!("S5 stub")
    }

    #[handles_fact(RunExited)]
    fn run_exited(&self, fact: RunExited, ctx: &Context) -> Record<Self> {
        let _ = (fact, ctx);
        todo!("S5 stub")
    }

    #[handles_fact(LaunchFailed)]
    fn launch_failed(&self, fact: LaunchFailed, ctx: &Context) -> Record<Self> {
        let _ = (fact, ctx);
        todo!("S5 stub")
    }

    #[applies(RunEnqueued)]
    fn run_enqueued(&mut self, event: RunEnqueued, page: &PageContext) {
        let _ = (event, page);
        todo!("S5 stub")
    }

    #[applies(RunnerAttachedRecorded)]
    fn runner_attached_recorded(&mut self, event: RunnerAttachedRecorded, page: &PageContext) {
        let _ = (event, page);
        todo!("S5 stub")
    }

    #[applies(TurnQueued)]
    fn turn_queued(&mut self, event: TurnQueued, page: &PageContext) {
        let _ = (event, page);
        todo!("S5 stub")
    }

    #[applies(TurnStartedRecorded)]
    fn turn_started_recorded(&mut self, event: TurnStartedRecorded, page: &PageContext) {
        let _ = (event, page);
        todo!("S5 stub")
    }

    #[applies(TurnCompletedRecorded)]
    fn turn_completed_recorded(&mut self, event: TurnCompletedRecorded, page: &PageContext) {
        let _ = (event, page);
        todo!("S5 stub")
    }

    #[applies(TurnFailedRecorded)]
    fn turn_failed_recorded(&mut self, event: TurnFailedRecorded, page: &PageContext) {
        let _ = (event, page);
        todo!("S5 stub")
    }

    #[applies(StopRequested)]
    fn stop_requested(&mut self, event: StopRequested, page: &PageContext) {
        let _ = (event, page);
        todo!("S5 stub")
    }

    #[applies(RunEnded)]
    fn run_ended(&mut self, event: RunEnded, page: &PageContext) {
        let _ = (event, page);
        todo!("S5 stub")
    }
}

#[cfg(test)]
#[path = "aggregate.test.rs"]
mod tests;
