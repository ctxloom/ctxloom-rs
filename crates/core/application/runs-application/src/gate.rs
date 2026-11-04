//! The LOCAL shape of the upstream gate (embedded-listener.plan.md section 4):
//! `identify`, the allowlist, `owns`. When angzarr-coordinator
//! provides its `Gate`, this implements that and the local trait goes
//! (review question 7).

use std::future::Future;
use std::sync::Arc;

use angzarr_embedded::markup::DomainMessage;
use angzarr_embedded::v1::{Cover, EventPage, Uuid};
use angzarr_embedded::{Payload, StoreError};
use run_ports::{Edge, Token, TokenHash};

use crate::error::{AppError, Why};
use crate::identity::{Identity, Principal};

/// What a caller presents: the bearer and the run id (two metadata pairs on the
/// wire; plan 5.3: "the bearer plus the run id find the run's stream"). The
/// bearer is the runner's token or the model's; which one it is, is decided by
/// the hash that matches, never by the caller.
#[derive(Clone, Debug)]
pub struct Credential {
    pub token: Token,
    pub run: Uuid,
}

/// The type URL of a domain message's wire form.
fn url<M: DomainMessage>() -> String {
    <M::Wire as prost::Name>::type_url()
}

/// A page's event as `E`, if it is one.
fn decode<E: Payload>(page: &EventPage) -> Option<E> {
    E::from_any(page.payload.as_ref()?).ok()
}

/// A store that could not be read.
pub(crate) fn failed(error: StoreError) -> AppError {
    AppError::Failed {
        reason: error.to_string(),
    }
}

/// The closed set of what ONE principal may SEND as a fact: the fact
/// type URLs it authors and nothing else. The two principals' lists are
/// disjoint (owner ruling 2026-11-04, item 16). Commands have no entry: the agent listener accepts no
/// generic Command envelope of any type (owner ruling 2026-11-04, S5 review Q3);
/// an agent's asks are the closed `AgentRequest` type, its own service.
#[derive(Clone, Debug)]
pub struct Allowlist {
    facts: Vec<String>,
}

impl Allowlist {
    /// What the RUNNER may send: the observation facts.
    pub fn runner() -> Allowlist {
        Allowlist {
            facts: vec![
                url::<runs::RunnerAttached>(),
                url::<runs::TurnStarted>(),
                url::<runs::TurnCompleted>(),
                url::<runs::TurnFailed>(),
                url::<runs::RunExited>(),
                url::<sessions::ClaimMessage>(),
                url::<sessions::DeliverMessage>(),
                url::<sessions::FailMessage>(),
                url::<sessions::BindEngineSession>(),
            ],
        }
    }

    /// What the MODEL may send: the claim facts, and none of the runner's.
    pub fn model() -> Allowlist {
        Allowlist {
            facts: vec![url::<runs::SummaryReported>(), url::<runs::FinalReported>()],
        }
    }

    /// The list of `principal`.
    pub fn of(principal: Principal) -> Allowlist {
        match principal {
            Principal::Runner => Allowlist::runner(),
            Principal::Model => Allowlist::model(),
        }
    }

    pub fn permits_fact(&self, type_url: &str) -> bool {
        self.facts.iter().any(|permitted| permitted == type_url)
    }

    /// Every URL permitted as a fact.
    pub fn facts(&self) -> Vec<String> {
        self.facts.clone()
    }
}

/// Whether `identity` owns the stream `cover` names: its own run, its own
/// session (inbox included). A pure predicate (listener plan 4.5).
pub fn owns(identity: &Identity, cover: &Cover) -> bool {
    *cover == identity.run_cover() || *cover == identity.session_cover()
}

/// The duties of the gate (listener plan 4.1) that remain on the fact path,
/// against local shapes; `stamp` left with the Command envelope (Q3 ruling).
pub trait Gate: Send + Sync {
    /// Credential to identity, reading current state: revocation is automatic
    /// because this runs per request and per stream open.
    fn identify(
        &self,
        credential: &Credential,
    ) -> impl Future<Output = Result<Identity, AppError>> + Send;

    /// What this identity's principal may send.
    fn allowlist(&self, identity: &Identity) -> Allowlist;

    /// Whether the identity owns `cover`.
    fn owns(&self, identity: &Identity, cover: &Cover) -> bool;
}

/// The gate for runs: identifies by the run's own stream and its session.
pub struct RunsGate<E: Edge> {
    edge: Arc<E>,
}

impl<E: Edge> RunsGate<E> {
    pub fn new(edge: Arc<E>) -> RunsGate<E> {
        RunsGate { edge }
    }
}

impl<E: Edge> Gate for RunsGate<E> {
    async fn identify(&self, credential: &Credential) -> Result<Identity, AppError> {
        // The run: its stream, folded as the aggregate folds it.
        let run_cover = Cover {
            domain: runs::RUN_DOMAIN.to_string(),
            root: Some(credential.run.clone()),
            edition: None,
        };
        let pages = self.edge.events(&run_cover, 0).await.map_err(failed)?;
        let mut run = runs::RunState::default();
        for page in &pages {
            if let Some(event) = decode::<runs::RunEvent>(page) {
                runs::apply(&mut run, &event, page.sequence);
            }
        }
        if run.mode.is_none() {
            return Err(AppError::Unauthenticated(Why::UnknownRun));
        }
        // The principal is the one whose journaled hash the token matches;
        // checked before the end, so a wrong token learns nothing of it.
        let principal = [
            (&run.runner_credential_hash, Principal::Runner),
            (&run.model_credential_hash, Principal::Model),
        ]
        .into_iter()
        .find(|(hash, _)| {
            TokenHash::from_slice(hash).is_some_and(|hash| hash.matches(&credential.token))
        })
        .map(|(_, principal)| principal)
        .ok_or(AppError::Unauthenticated(Why::Mismatch))?;
        if run.credential_revoked {
            return Err(AppError::Unauthenticated(Why::Revoked));
        }
        // The lineage: the session's, folded as its aggregate folds it.
        let session = uuid::Uuid::parse_str(&run.session)
            .map(|id| Uuid::from_bytes(*id.as_bytes()))
            .map_err(|_| AppError::Unauthenticated(Why::UnknownRun))?;
        let session_cover = Cover {
            domain: sessions::SESSION_DOMAIN.to_string(),
            root: Some(session.clone()),
            edition: None,
        };
        let pages = self.edge.events(&session_cover, 0).await.map_err(failed)?;
        let mut held = sessions::SessionState::default();
        for page in &pages {
            if let Some(event) = decode::<sessions::SessionEvent>(page) {
                let at = page.meta.as_ref().and_then(|meta| meta.recorded_at);
                sessions::apply(&mut held, &event, at.unwrap_or_default());
            }
        }
        if held.harp.is_empty() {
            return Err(AppError::Unauthenticated(Why::UnknownRun));
        }
        Ok(Identity {
            run: credential.run.clone(),
            session,
            incarnation: run.incarnation,
            harp: held.harp,
            depth: held.depth,
            principal,
        })
    }

    fn allowlist(&self, identity: &Identity) -> Allowlist {
        Allowlist::of(identity.principal)
    }

    fn owns(&self, identity: &Identity, cover: &Cover) -> bool {
        owns(identity, cover)
    }
}

#[cfg(test)]
#[path = "gate.test.rs"]
mod tests;
