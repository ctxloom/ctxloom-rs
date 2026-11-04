//! The LOCAL shape of the upstream gate (embedded-listener.plan.md section 4):
//! `identify`, the allowlist, `owns`, `stamp`. When angzarr-coordinator
//! provides its `Gate`, this implements that and the local trait goes
//! (review question 7). STUBS (S5 red).

use std::future::Future;
use std::sync::Arc;

use angzarr_embedded::v1::{CommandBook, Cover, Uuid};
use run_ports::{Edge, Token};

use crate::error::{AppError, Why};
use crate::identity::Identity;

/// What a caller presents: the bearer and the run id (two metadata pairs on the
/// wire; plan 5.3: "the bearer plus the run id find the run's stream").
#[derive(Clone, Debug)]
pub struct Credential {
    pub token: Token,
    pub run: Uuid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    Command,
    Fact,
}

/// The closed set of what an agent may SEND as: (body kind, type URL) pairs.
#[derive(Clone, Debug)]
pub struct Allowlist {
    facts: Vec<String>,
    commands: Vec<String>,
}

impl Allowlist {
    /// What a runner may send: the closed `AgentFact` URLs as facts, the closed
    /// `AgentRequest` URLs as commands (listener plan 4.5).
    pub fn agent() -> Allowlist {
        todo!("S5 stub")
    }

    pub fn permits(&self, kind: BodyKind, type_url: &str) -> bool {
        let _ = (&self.facts, &self.commands, kind, type_url);
        todo!("S5 stub")
    }

    /// Every URL permitted as `kind`.
    pub fn urls(&self, kind: BodyKind) -> Vec<String> {
        let _ = kind;
        todo!("S5 stub")
    }
}

/// Whether `identity` owns the stream `cover` names: its own run, its own
/// session (inbox included). A pure predicate (listener plan 4.5).
pub fn owns(identity: &Identity, cover: &Cover) -> bool {
    let _ = (identity, cover);
    todo!("S5 stub")
}

/// The four duties of the gate (listener plan 4.1), against local shapes.
pub trait Gate: Send + Sync {
    /// Credential to identity, reading current state: revocation is automatic
    /// because this runs per request and per stream open.
    fn identify(
        &self,
        credential: &Credential,
    ) -> impl Future<Output = Result<Identity, AppError>> + Send;

    /// What this identity may send.
    fn allowlist(&self, identity: &Identity) -> Allowlist;

    /// Whether the identity owns `cover`.
    fn owns(&self, identity: &Identity, cover: &Cover) -> bool;

    /// The book with the caller's identity stamped over any sender field in the
    /// payload ("from the identity, never the payload").
    fn stamp(&self, identity: &Identity, book: CommandBook) -> CommandBook;
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
        let _ = identity;
        Allowlist::agent()
    }

    fn owns(&self, identity: &Identity, cover: &Cover) -> bool {
        owns(identity, cover)
    }

    fn stamp(&self, identity: &Identity, book: CommandBook) -> CommandBook {
        let _ = (identity, book);
        todo!("S5 stub")
    }
}

#[cfg(test)]
#[path = "gate.test.rs"]
mod tests;
