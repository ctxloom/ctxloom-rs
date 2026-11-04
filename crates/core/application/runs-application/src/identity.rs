//! Who a credential identifies. Resolved by the listener from the bearer,
//! NEVER from a payload (plan 4.2).

use angzarr_embedded::v1::{Cover, Uuid};

/// Owner (depth 0) or child (depth above 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Owner,
    Child,
}

/// `Identity{run, session, harp, depth, role}` of plan 5.3, less `tree`
/// (the tree's occupant arrives with S10) .
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    /// The run stream's root: UUIDv5(session, incarnation).
    pub run: Uuid,
    /// The session stream's root.
    pub session: Uuid,
    pub incarnation: u32,
    pub harp: String,
    pub depth: u32,
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
