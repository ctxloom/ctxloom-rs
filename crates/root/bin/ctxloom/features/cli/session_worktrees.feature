# Written for the new design; awaits product plan step 4 (docs/plans/ctxloom-features.md).
# Ported from cli/session.feature
# Ported from journeys/j001300_closeout.feature
# NOT ported: `session worktrees purge` and the `--reap` switch: a session's
#   worktrees go only with the session, through `session purge` (vocabulary
#   §8). j001300's doctor rows (foreign worktrees, gitignore, harp durability)
#   are doctor's. The listing names each owner by the state its lock proves
#   (alive, dead, indeterminate), not by a pid.
@pending
Feature: session worktrees - the scratch checkouts a session left behind

  A session's scratch worktrees can be listed on their own, and go only with
  their session. Purging a session reaches the same verdicts the listing
  shows: a clean tree of a provably ended session goes; a tree holding
  uncommitted work keeps its whole session, and the work stays where its
  author left it, still registered with git.

  Background:
    Given the feature shipped on Friday and Alice is closing the workstream out

  Rule: The scratch worktrees are a population with their own listing

    Scenario: The scratch worktrees are a population with their own listing
      Given an initialized ctxloom project
      And a finished session "amber-swift-owl" with a transcript
      When I run "ctxloom session worktrees"
      Then the command succeeds
      When I run "ctxloom session worktrees list"
      Then the command succeeds
      When I run "ctxloom session worktrees list amber-swift-owl"
      Then the command succeeds

    Scenario: The scratch worktrees are listed before anything is removed
      Given a finished session "amber-quiet-heron" whose transcript retention has passed
      And session "amber-quiet-heron" left a clean scratch worktree
      And session "amber-quiet-heron" left a scratch worktree holding uncommitted work
      When I run "ctxloom session worktrees"
      Then the report names each scratch worktree with its harp, its owner and its verdict

  Rule: Purging a session takes its worktrees only when it can prove that safe

    # A harp-scoped purge does not stray outside its harp: the running and the
    # unprovable sessions beside it keep their worktrees.
    Scenario: Purging removes only what it can prove is safe, and says why it left the rest
      # substitution: the verb is `session purge` (`session worktrees purge` is retired); the purged session holds only a clean tree, since uncommitted work keeps its whole session (next scenario).
      Given a finished session "amber-quiet-heron" whose transcript retention has passed
      And session "amber-quiet-heron" left a clean scratch worktree
      And a session "teal-running-wren" that is still running
      And session "teal-running-wren" left a scratch worktree of its own
      And a session "slate-unproven-vole" nothing can prove the liveness of
      And session "slate-unproven-vole" left a scratch worktree of its own
      When I run "ctxloom session purge amber-quiet-heron --yes"
      Then the command succeeds
      And the machine-written bulk of "amber-quiet-heron" is gone
      And the machine-written bulk of "teal-running-wren" is still there
      And the machine-written bulk of "slate-unproven-vole" is still there

    # The safety scenario, and the one that must never be weakened. Uncommitted
    # work is never taken: not stashed, not branched, not left to the reflog. It
    # stays in place, read back by its own bytes, and its session is refused
    # whole, so a purged session is always entirely gone.
    Scenario: Emptying a session spares the uncommitted work living inside it
      # substitution: purge is all or nothing, so a session holding uncommitted work is refused whole (its clean sibling stays too) and the report names the worktree.
      Given a finished session "amber-quiet-heron" whose transcript retention has passed
      And session "amber-quiet-heron" left a clean scratch worktree
      And session "amber-quiet-heron" left a scratch worktree holding uncommitted work
      When I run "ctxloom session purge amber-quiet-heron --yes"
      Then the command fails
      And the output contains "ctxloom-wt-wip"
      And the uncommitted work is still there, spared in place
      And the scratch worktree is still registered with git
      And the machine-written bulk of "amber-quiet-heron" is still there

    # A clean tree survives purely because its session still holds its lock.
    Scenario: Purging a session that is still running removes nothing
      # substitution: the verb is `session purge` (`session worktrees purge` is retired).
      Given a session "teal-running-wren" that is still running
      And session "teal-running-wren" left a clean scratch worktree
      When I run "ctxloom session purge teal-running-wren --yes"
      Then no worktree of "teal-running-wren" is removed, and ctxloom says it could prove nothing safe

    # A missing lock never reads as a free one.
    Scenario: Purging a session nothing can prove the liveness of removes nothing
      # substitution: the verb is `session purge` (`session worktrees purge` is retired).
      Given a session "slate-unproven-vole" nothing can prove the liveness of
      And session "slate-unproven-vole" left a clean scratch worktree
      When I run "ctxloom session purge slate-unproven-vole --yes"
      Then no worktree of "slate-unproven-vole" is removed, and ctxloom says it could prove nothing safe

    # Worktrees outside the sessions root are never listed and never purged.
    Scenario: Her own long-lived worktrees are not this verb's business
      # substitution: the verb is `session purge` (`session worktrees purge` is retired).
      Given a finished session "amber-quiet-heron" whose transcript retention has passed
      And session "amber-quiet-heron" left a clean scratch worktree
      And a long-lived worktree "stale-feature" of her own, outside the sessions root, with unmerged work
      When I run "ctxloom session purge amber-quiet-heron --yes"
      Then her own long-lived worktree is untouched and was never listed
