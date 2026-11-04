//! The tree aggregate on angzarr-embedded's markup. STUBS (S5 red).

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
    #[handles_cmd(FoundTree)]
    fn found(&self, cmd: FoundTree, ctx: &Context) -> Result<TreeFounded, Failed<TreeRejection>> {
        let _ = (cmd, ctx);
        todo!("S5 stub")
    }

    #[handles_cmd(OccupyTree)]
    fn occupy(
        &self,
        cmd: OccupyTree,
        ctx: &Context,
    ) -> Result<Option<TreeOccupied>, Failed<TreeRejection>> {
        let _ = (cmd, ctx);
        todo!("S5 stub")
    }

    #[applies(TreeFounded)]
    fn tree_founded(&mut self, event: TreeFounded, page: &PageContext) {
        let _ = (event, page);
        todo!("S5 stub")
    }

    #[applies(TreeOccupied)]
    fn tree_occupied(&mut self, event: TreeOccupied, page: &PageContext) {
        let _ = (event, page);
        todo!("S5 stub")
    }
}

#[cfg(test)]
#[path = "aggregate.test.rs"]
mod tests;
