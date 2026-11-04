//! The inbound fact port (plan 4.2): narrow by construction.
//!
//! S5 carries the facts whose aggregates exist: the run's (attached, turn
//! started / completed / failed, exited) and the session inbox's (claimed,
//! delivered, failed). The plan's other variants (`RunPaused`, `RunResumed`,
//! `ArtifactPublished`, `ApprovalAsked`) arrive with their aggregates (S6, S9,
//! S10). `EngineSessionBound` is in: S4's bind became a fact (item 8, RULED).

use std::sync::Arc;

use angzarr_embedded::markup::DomainMessage;
use angzarr_embedded::v1::{
    command_page, CommandBook, CommandPage, Cover, DeliveryOutcome, Fact as AFact, FactOrigin, Uuid,
};
use angzarr_embedded::{Answered, Any};
use run_ports::{Arrival, Edge, Unserved};

use crate::error::AppError;
use crate::identity::{Identity, Principal};

/// A fact of one principal: what [`Facts::record`] takes. Implemented by
/// [`RunnerFact`] and [`ModelFact`] only; each is authored by exactly one
/// principal, so a call site holding a model's credential cannot even
/// construct what only the runner may observe (owner ruling 2026-11-04).
pub trait Fact: Clone + std::fmt::Debug + Send + Sync {
    /// The only principal that may record this kind of fact.
    const AUTHOR: Principal;

    /// The fact as the `CommandBook` the edge takes.
    fn book(&self, who: &Identity) -> CommandBook;
}

/// What the RUNNER observes and may record: authoritative facts. Closed: no
/// command in it. Every variant must apply in every state: a fact handler
/// never rejects.
#[derive(Clone, Debug, PartialEq)]
pub enum RunnerFact {
    RunnerAttached(runs::RunnerAttached),
    TurnStarted(runs::TurnStarted),
    TurnCompleted(runs::TurnCompleted),
    TurnFailed(runs::TurnFailed),
    RunExited(runs::RunExited),
    MessageClaimed(sessions::ClaimMessage),
    MessageDelivered(sessions::DeliverMessage),
    MessageFailed(sessions::FailMessage),
    /// The SessionStart hook's bind, a fact since the owner ruling of
    /// 2026-11-04 (S5 review item 8): `bind:<session>:<native_id>`. The hook
    /// holds the RUNNER's credential (it is launched by the runner; owner
    /// ruling 2026-11-04), so it can emit this and no model claim.
    EngineSessionBound(sessions::BindEngineSession),
}

/// What the MODEL claims and may record: advisory evidence, nothing the
/// runner observed. The turn and ordinal are the listener's references (the
/// runner side derives them; the model supplies none, owner ruling
/// 2026-11-04), so the id is never a counter the model chooses.
#[derive(Clone, Debug, PartialEq)]
pub enum ModelFact {
    /// A model's summary or progress report: `report:<run>:<turn>:<ordinal>`.
    SummaryReported(runs::SummaryReported),
    /// A model's "my work is done": a CLAIM that notifies the parent and
    /// transitions nothing. `final:<run>:<turn>`.
    FinalReported(runs::FinalReported),
}

/// The stream a fact addresses.
enum Stream {
    Run,
    Session,
}

/// A wire message's `Any`, for a book's fact body.
fn any_of<M: DomainMessage>(message: &M) -> Any {
    Any::from_msg(&message.to_wire()).expect("a wire message encodes")
}

/// `uuid`'s hyphenated lowercase text, as the external ids spell a run or
/// session.
fn text(uuid: &Uuid) -> String {
    uuid::Uuid::from_slice(&uuid.value)
        .map(|u| u.hyphenated().to_string())
        .unwrap_or_default()
}

/// The book of a fact on `cover`.
fn book_of(cover: Cover, payload: Any, external_id: String) -> CommandBook {
    CommandBook {
        cover: Some(cover),
        page: Some(CommandPage {
            body: Some(command_page::Body::Fact(AFact {
                payload: Some(payload),
            })),
            fact_origin: Some(FactOrigin {
                external_id,
                description: String::new(),
            }),
            ..CommandPage::default()
        }),
        handling: None,
    }
}

fn cover_of(stream: Stream, who: &Identity) -> Cover {
    match stream {
        Stream::Run => who.run_cover(),
        Stream::Session => who.session_cover(),
    }
}

impl RunnerFact {
    fn payload(&self) -> Any {
        match self {
            RunnerFact::RunnerAttached(m) => any_of(m),
            RunnerFact::TurnStarted(m) => any_of(m),
            RunnerFact::TurnCompleted(m) => any_of(m),
            RunnerFact::TurnFailed(m) => any_of(m),
            RunnerFact::RunExited(m) => any_of(m),
            RunnerFact::MessageClaimed(m) => any_of(m),
            RunnerFact::MessageDelivered(m) => any_of(m),
            RunnerFact::MessageFailed(m) => any_of(m),
            RunnerFact::EngineSessionBound(m) => any_of(m),
        }
    }

    /// The type URL of the fact's wire message.
    pub fn type_url(&self) -> String {
        self.payload().type_url
    }

    /// The `external_id`, derived from the fact's cause, never from a counter
    /// (plan 4.3): the run and session are `who`'s, in hyphenated lowercase.
    pub fn external_id(&self, who: &Identity) -> String {
        let (run, session) = (text(&who.run), text(&who.session));
        match self {
            RunnerFact::RunnerAttached(m) => format!("attach:{run}:{}", m.runner_id),
            RunnerFact::TurnStarted(m) => format!("turn:{run}:{}:started", m.turn),
            RunnerFact::TurnCompleted(m) => format!("turn:{run}:{}:completed", m.turn),
            RunnerFact::TurnFailed(m) => format!("turn:{run}:{}:failed", m.turn),
            RunnerFact::RunExited(_) => format!("exit:{run}"),
            RunnerFact::MessageClaimed(m) => {
                format!("claim:{session}:{}:{}", m.message_id, m.claimant)
            }
            RunnerFact::MessageDelivered(m) => format!("deliver:{session}:{}", m.message_id),
            RunnerFact::MessageFailed(m) => format!("fail:{session}:{}", m.message_id),
            RunnerFact::EngineSessionBound(m) => format!("bind:{session}:{}", m.native_id),
        }
    }

    /// The one stream this fact may address: `who`'s own run or session.
    pub fn cover(&self, who: &Identity) -> Cover {
        cover_of(
            match self {
                RunnerFact::RunnerAttached(_)
                | RunnerFact::TurnStarted(_)
                | RunnerFact::TurnCompleted(_)
                | RunnerFact::TurnFailed(_)
                | RunnerFact::RunExited(_) => Stream::Run,
                RunnerFact::MessageClaimed(_)
                | RunnerFact::MessageDelivered(_)
                | RunnerFact::MessageFailed(_)
                | RunnerFact::EngineSessionBound(_) => Stream::Session,
            },
            who,
        )
    }

    /// The fact as the `CommandBook` the edge takes: a Fact body with its
    /// `fact_origin`.
    pub fn book(&self, who: &Identity) -> CommandBook {
        book_of(self.cover(who), self.payload(), self.external_id(who))
    }

    /// One fact of every variant (a test's way to enumerate the allowlist).
    #[cfg(any(test, feature = "test-support"))]
    pub fn samples() -> Vec<RunnerFact> {
        vec![
            RunnerFact::RunnerAttached(runs::RunnerAttached {
                runner_id: "runner-1".to_string(),
                protocol: "v1".to_string(),
            }),
            RunnerFact::TurnStarted(runs::TurnStarted { turn: 1 }),
            RunnerFact::TurnCompleted(runs::TurnCompleted {
                turn: 1,
                native_key: String::new(),
                answer_ref: String::new(),
                exit_code: 0,
            }),
            RunnerFact::TurnFailed(runs::TurnFailed {
                turn: 1,
                class: runs::TurnFailureClass::EngineError,
                detail: String::new(),
            }),
            RunnerFact::RunExited(runs::RunExited { code: 0, signal: 0 }),
            RunnerFact::MessageClaimed(sessions::ClaimMessage {
                message_id: "m1".to_string(),
                claimant: "runner".to_string(),
            }),
            RunnerFact::MessageDelivered(sessions::DeliverMessage {
                message_id: "m1".to_string(),
                at_turn: 1,
            }),
            RunnerFact::MessageFailed(sessions::FailMessage {
                message_id: "m1".to_string(),
                reason: "engine".to_string(),
            }),
            RunnerFact::EngineSessionBound(sessions::BindEngineSession {
                native_id: "native-1".to_string(),
                transcript_path: "/t".to_string(),
                engine_version: None,
            }),
        ]
    }
}

impl ModelFact {
    fn payload(&self) -> Any {
        match self {
            ModelFact::SummaryReported(m) => any_of(m),
            ModelFact::FinalReported(m) => any_of(m),
        }
    }

    /// The type URL of the fact's wire message.
    pub fn type_url(&self) -> String {
        self.payload().type_url
    }

    /// The `external_id`: the run and the runner-side references (plan 4.3).
    pub fn external_id(&self, who: &Identity) -> String {
        let run = text(&who.run);
        match self {
            ModelFact::SummaryReported(m) => format!("report:{run}:{}:{}", m.turn, m.ordinal),
            ModelFact::FinalReported(m) => format!("final:{run}:{}", m.turn),
        }
    }

    /// A claim addresses the caller's own run.
    pub fn cover(&self, who: &Identity) -> Cover {
        who.run_cover()
    }

    pub fn book(&self, who: &Identity) -> CommandBook {
        book_of(self.cover(who), self.payload(), self.external_id(who))
    }

    /// One claim of every variant.
    #[cfg(any(test, feature = "test-support"))]
    pub fn samples() -> Vec<ModelFact> {
        vec![
            ModelFact::SummaryReported(runs::SummaryReported {
                turn: 1,
                ordinal: 0,
                scope: "progress".to_string(),
                text_ref: "t:1".to_string(),
            }),
            ModelFact::FinalReported(runs::FinalReported {
                turn: 1,
                text_ref: "t:final".to_string(),
            }),
        ]
    }
}

impl Fact for RunnerFact {
    const AUTHOR: Principal = Principal::Runner;

    fn book(&self, who: &Identity) -> CommandBook {
        RunnerFact::book(self, who)
    }
}

impl Fact for ModelFact {
    const AUTHOR: Principal = Principal::Model;

    fn book(&self, who: &Identity) -> CommandBook {
        ModelFact::book(self, who)
    }
}

/// What the kernel did with a fact (the spec's `DeliveryOutcome`). Not the
/// host's `Recorded`/`Handled`: the two never meet in one module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recorded {
    Applied,
    Duplicate,
    /// At the race cap (D13): dead-lettered, alerted, nothing persisted. The
    /// caller treats it as FINAL for that fact and never retries it.
    Parked,
}

/// Records facts on the caller's own streams.
pub struct Facts<E: Edge> {
    edge: Arc<E>,
}

impl<E: Edge> Facts<E> {
    pub fn new(edge: Arc<E>) -> Facts<E> {
        Facts { edge }
    }

    /// Record `fact` on a stream `who` owns, if `who` is the fact's author
    /// ([`Fact::AUTHOR`]); another principal's fact is `Denied` (hidden)
    /// and nothing is sent (defence in depth: the types already keep a model
    /// call site from building a runner fact). The answer is the kernel's: a
    /// fact it dead-letters (a handler bug, a race lost to the cap, or a
    /// rejection, which the kernel since 7873207 treats as its handler's bug)
    /// is `Parked`, and the letter is the kernel's, kept in the store; this
    /// crate files none.
    pub async fn record<F: Fact>(&self, who: &Identity, fact: F) -> Result<Recorded, AppError> {
        let book = fact.book(who);
        if who.principal != F::AUTHOR {
            let domain = book.cover.map(|c| c.domain).unwrap_or_default();
            return Err(AppError::Denied(Unserved::Hidden { domain }));
        }
        let answered = self
            .edge
            .serve(book, Arrival::Loopback)
            .await
            .map_err(AppError::from_unserved)?;
        Recorded::of(&answered).ok_or_else(|| AppError::Failed {
            reason: "the kernel gave no delivery outcome".to_string(),
        })
    }
}

impl Recorded {
    /// The spec's delivery outcome of `answered`, if it names one.
    pub(crate) fn of(answered: &Answered) -> Option<Recorded> {
        let delivery = answered.outcome.as_ref()?.delivery;
        match DeliveryOutcome::try_from(delivery).ok()? {
            DeliveryOutcome::Applied => Some(Recorded::Applied),
            DeliveryOutcome::Duplicate => Some(Recorded::Duplicate),
            DeliveryOutcome::Parked => Some(Recorded::Parked),
            DeliveryOutcome::Unspecified => None,
        }
    }
}

#[cfg(test)]
#[path = "facts.test.rs"]
mod tests;
