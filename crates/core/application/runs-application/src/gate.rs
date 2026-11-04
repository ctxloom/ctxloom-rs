//! The LOCAL shape of the upstream gate (embedded-listener.plan.md section 4):
//! `identify`, the allowlist, `owns`. When angzarr-coordinator
//! provides its `Gate`, this implements that and the local trait goes
//! (review question 7). STUBS (S5 red).

use std::future::Future;
use std::sync::Arc;

use angzarr_embedded::v1::{Cover, Uuid};
use run_ports::{Edge, Token};

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

/// The closed set of what ONE principal may SEND as a fact: the `AgentFact`
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
        todo!("S5 stub")
    }

    /// What the MODEL may send: the claim facts, and none of the runner's.
    pub fn model() -> Allowlist {
        todo!("S5 stub")
    }

    /// The list of `principal`.
    pub fn of(principal: Principal) -> Allowlist {
        match principal {
            Principal::Runner => Allowlist::runner(),
            Principal::Model => Allowlist::model(),
        }
    }

    pub fn permits_fact(&self, type_url: &str) -> bool {
        let _ = (&self.facts, type_url);
        todo!("S5 stub")
    }

    /// Every URL permitted as a fact.
    pub fn facts(&self) -> Vec<String> {
        todo!("S5 stub")
    }
}

/// Whether `identity` owns the stream `cover` names: its own run, its own
/// session (inbox included). A pure predicate (listener plan 4.5).
pub fn owns(identity: &Identity, cover: &Cover) -> bool {
    let _ = (identity, cover);
    todo!("S5 stub")
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
        let _ = (&self.edge, credential, Why::Mismatch);
        todo!("S5 stub")
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
