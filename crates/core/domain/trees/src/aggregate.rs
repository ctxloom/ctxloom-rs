//! The tree aggregate on angzarr-embedded's markup.

use angzarr_embedded::markup::{Context, Failed, PageContext};
use angzarr_embedded_macros::{aggregate, applies, handles_cmd};

use crate::{
    FoundTree, OccupyTree, TreeCommand, TreeEvent, TreeFounded, TreeOccupied, TreeRejection,
    TreeState,
};

#[derive(Clone, Debug, Default)]
pub struct TreeAggregate {
    tree: TreeState,
}

impl TreeAggregate {
    /// The folded tree.
    pub fn state(&self) -> &TreeState {
        &self.tree
    }
}

#[aggregate(domain = "trees", command = TreeCommand, event = TreeEvent)]
impl TreeAggregate {
    /// Found the tree, once, snapshotting its limits; its owner occupies it.
    #[handles_cmd(FoundTree)]
    fn found(&self, cmd: FoundTree, ctx: &Context) -> Result<TreeFounded, Failed<TreeRejection>> {
        if ctx.command.had_prior_events {
            return Err(TreeRejection::AlreadyFounded.into());
        }
        Ok(TreeFounded {
            owner_session: cmd.owner_session,
            ephemeral: cmd.ephemeral,
            concurrency: cmd.concurrency,
            depth: cmd.depth,
        })
    }

    /// Change the occupant; the occupant occupying again records nothing.
    #[handles_cmd(OccupyTree)]
    fn occupy(
        &self,
        cmd: OccupyTree,
        ctx: &Context,
    ) -> Result<Option<TreeOccupied>, Failed<TreeRejection>> {
        let _ = ctx;
        if !self.tree.founded {
            return Err(TreeRejection::NoTree.into());
        }
        let changed = self.tree.occupant != cmd.session;
        Ok(changed.then_some(TreeOccupied {
            session: cmd.session,
        }))
    }

    #[applies(TreeFounded)]
    fn tree_founded(&mut self, event: TreeFounded, page: &PageContext) {
        let _ = page;
        self.tree = TreeState {
            occupant: event.owner_session.clone(),
            owner_session: event.owner_session,
            ephemeral: event.ephemeral,
            concurrency: event.concurrency,
            depth: event.depth,
            founded: true,
        };
    }

    #[applies(TreeOccupied)]
    fn tree_occupied(&mut self, event: TreeOccupied, page: &PageContext) {
        let _ = page;
        self.tree.occupant = event.session;
    }
}

#[cfg(test)]
#[path = "aggregate.test.rs"]
mod tests;
