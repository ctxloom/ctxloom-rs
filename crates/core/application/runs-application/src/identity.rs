//! Who a credential identifies. Resolved by the listener from the bearer,
//! NEVER from a payload (plan 4.2).

use angzarr_embedded::v1::{Cover, Uuid};

/// Owner (depth 0) or child (depth above 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Owner,
    Child,
}

/// WHICH credential of a run spoke (owner ruling 2026-11-04, S5 review item
/// 16): a run has two principals, each with its own token and its own
/// allowlist, so a model that can read its own token file cannot forge what the
/// runner observed. Resolved by `identify` from the hash that matched, never
/// from a payload; an unmatched token is no principal (fail closed).
///
/// There is deliberately NO decider variant. Accepting or rejecting a model's
/// claim is a separate refusable command by a DECIDER role (an orchestrating
/// agent, or a human via the CLI or TUI), which is S10's and S14's: it is not
/// built here and nothing in S5 hard-wires who the decider is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Principal {
    /// The runner: may record authoritative observations (an attach, a turn's
    /// start and end, an exit, an engine session bound, the inbox's claim,
    /// delivery and failure).
    Runner,
    /// The model: may record only CLAIM facts (advisory evidence) and send the
    /// closed `AgentRequest`s.
    Model,
}

/// `Identity{run, session, harp, depth, role}` of plan 5.3, less `tree`
/// (the tree's occupant arrives with S10), plus the credential's principal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    /// The run stream's root: UUIDv5(session, incarnation).
    pub run: Uuid,
    /// The session stream's root.
    pub session: Uuid,
    pub incarnation: u32,
    pub harp: String,
    pub depth: u32,
    /// Which of the run's two credentials this identity came from.
    pub principal: Principal,
}

impl Identity {
    pub fn role(&self) -> Role {
        if self.depth == 0 {
            Role::Owner
        } else {
            Role::Child
        }
    }

    /// The caller's own run stream.
    pub fn run_cover(&self) -> Cover {
        Cover {
            domain: runs::RUN_DOMAIN.to_string(),
            root: Some(self.run.clone()),
            edition: None,
        }
    }

    /// The caller's own session stream (its inbox included).
    pub fn session_cover(&self) -> Cover {
        Cover {
            domain: sessions::SESSION_DOMAIN.to_string(),
            root: Some(self.session.clone()),
            edition: None,
        }
    }
}
