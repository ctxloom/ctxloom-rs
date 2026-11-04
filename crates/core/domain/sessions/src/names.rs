//! The name-claim aggregate: one stream per (project, harp text), holding
//! which session has the name (D5 Q1, owner-approved).

use angzarr_embedded::markup::{Context, Failed, PageContext};
use angzarr_embedded_macros::{aggregate, applies, handles_cmd};

use crate::{
    ClaimName, Claimed, NameClaimCommand, NameClaimEvent, NameClaimState, ReleaseName, Released,
    SessionRejection,
};

/// The namespace every project's name-claim namespace derives from: the
/// UUIDv5 of `ctxloom:project:` and the project id's text, in the URL
/// namespace. OWNER-APPROVED and FIXED: imported session ids and claim roots
/// are derived from it, so changing it orphans every imported stream.
pub fn project_namespace(project_id: &str) -> uuid::Uuid {
    uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_URL,
        format!("ctxloom:project:{project_id}").as_bytes(),
    )
}

/// The root of the claim stream for the name `harp` in project `project_id`:
/// UUIDv5 of the harp text in [`project_namespace`], so a name is unique in a
/// project because a stream is (plan 3.1). FIXED, as above.
pub fn claim_root(project_id: &str, harp: &str) -> angzarr_embedded::v1::Uuid {
    let id = uuid::Uuid::new_v5(&project_namespace(project_id), harp.as_bytes());
    angzarr_embedded::v1::Uuid::from_bytes(*id.as_bytes())
}

/// The claim aggregate: one name, folded from its stream.
#[derive(Clone, Debug, Default)]
pub struct NameClaimAggregate {
    claim: NameClaimState,
}

impl NameClaimAggregate {
    /// The folded claim.
    pub fn state(&self) -> &NameClaimState {
        &self.claim
    }
}

// The domain is NAMES_DOMAIN, spelled as the markup takes it, a literal.
#[aggregate(domain = "session-names", command = NameClaimCommand, event = NameClaimEvent)]
impl NameClaimAggregate {
    /// Take the name for a session; `None` when its holder asks again.
    #[handles_cmd(ClaimName)]
    fn claim(
        &self,
        cmd: ClaimName,
        ctx: &Context,
    ) -> Result<Option<Claimed>, Failed<SessionRejection>> {
        let _ = ctx;
        if self.claim.session == cmd.session {
            return Ok(None);
        }
        if !self.claim.session.is_empty() {
            return Err(SessionRejection::NamesTaken { harp: cmd.harp }.into());
        }
        Ok(Some(Claimed {
            project_id: cmd.project_id,
            harp: cmd.harp,
            session: cmd.session,
        }))
    }

    /// Give the name back.
    #[handles_cmd(ReleaseName)]
    fn release(
        &self,
        cmd: ReleaseName,
        ctx: &Context,
    ) -> Result<Released, Failed<SessionRejection>> {
        let _ = ctx;
        if self.claim.session.is_empty() {
            return Err(SessionRejection::NoClaim.into());
        }
        if self.claim.session != cmd.session {
            return Err(SessionRejection::NotHolder {
                harp: self.claim.harp.clone(),
            }
            .into());
        }
        Ok(Released {})
    }

    #[applies(Claimed)]
    fn claimed(&mut self, event: Claimed, page: &PageContext) {
        let _ = page;
        self.claim = NameClaimState {
            project_id: event.project_id,
            harp: event.harp,
            session: event.session,
            claimed: true,
        };
    }

    #[applies(Released)]
    fn released(&mut self, event: Released, page: &PageContext) {
        let _ = (event, page);
        self.claim.session.clear();
    }
}

#[cfg(test)]
#[path = "names.test.rs"]
mod tests;
