# world: base
# Ported from cli/session.feature
# Ported from journeys/j001300_closeout.feature
# NOT ported: j001300's doctor rows (DOCTOR-CHECK-FOREIGN-WORKTREES,
#   gitignore, harp durability): doctor's checks are S12's. Ported: the
#   worktree rows of "Each population owns its destroyer" and j001300's
#   Step 2, the worktree rows, and the purge row that spares uncommitted work.
#   plan supersedes Go: the listing names each owner by the state its lock
#   proves (alive, dead, indeterminate), not by a pid; the session lock
#   carries none (plan 5.5 / session-ports Liveness).
Feature: session worktrees - the scratch checkouts a session left behind

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

    Scenario: Purging worktrees reports before it removes
      Given an initialized ctxloom project
      And a finished session "amber-swift-owl" with a transcript
      When I run "ctxloom session worktrees purge amber-swift-owl"
      Then the command succeeds
      And the output contains "removed nothing"
      And the output contains "ctxloom session worktrees purge amber-swift-owl --yes"

    Scenario: The retired reap switch is gone, not hidden
      Given an initialized ctxloom project
      When I run "ctxloom session worktrees --reap --yes"
      Then the command fails
      And the output contains "unknown flag"

  Rule: Two populations, two verbs: ctxloom lists and removes only its own

  # ---- Step 2: worktrees, two populations, two verbs ---------------------
  # The inspect half. Read-only by default on every leaf, so the first thing
  # this leaf ever does is show its work.
  #
  # `ctxloom session worktrees` lists ctxloom-owned scratch trees with harp,
  # owner pid and verdict — isolation.ClassifyOrphanedWorktrees, split out of
  # isolation.ReapOrphanedWorktrees so a caller can show its work before
  # anything is removed.
  Scenario: The scratch worktrees are listed before anything is removed
    Given a finished session "amber-quiet-heron" whose transcript retention has passed
    And session "amber-quiet-heron" left a clean scratch worktree
    And session "amber-quiet-heron" left a scratch worktree holding uncommitted work
    When I run "ctxloom session worktrees"
    Then the report names each scratch worktree with its harp, its owner and its verdict

  # THE SAFETY SCENARIO, and the one that must never be weakened. Only the
  # clean tree of a session that can be PROVEN ended may go. Uncommitted work
  # is spared IN PLACE — not stashed, not branched, not "recoverable from
  # reflog": still sitting where its author left it.
  #
  # This project has lost work to a force-removed worktree before. That is why
  # the assertion reads the WIP file's own bytes off disk rather than checking
  # that a directory still exists.
  #
  # ONE SESSION PER OWNER-STATE, and the split is forced by the product rather
  # than chosen: liveness is the SESSION's (an exclusive lock held for its
  # lifetime — internal/shared/sessionlock), so one harp can no longer hold a
  # dead-owned and a live-owned worktree at once. Clean-vs-dirty still varies
  # WITHIN the ended session, because that is a property of the checkout. The
  # running and unprovable populations keep their own sessions, and this
  # scenario proves a harp-scoped purge does not stray outside its harp; the
  # two scenarios below pin what happens when they are the TARGET.
  #
  # `session worktrees purge <harp> --yes` drives isolation.ReapWorktrees over
  # exactly what isolation.ClassifyOrphanedWorktrees just classified, and
  # reports its outcome taxonomy (reaped/spared/skipped).
  Scenario: Purging removes only what it can prove is safe, and says why it left the rest
    Given a finished session "amber-quiet-heron" whose transcript retention has passed
    And session "amber-quiet-heron" left a clean scratch worktree
    And session "amber-quiet-heron" left a scratch worktree holding uncommitted work
    And a session "teal-running-wren" that is still running
    And session "teal-running-wren" left a scratch worktree of its own
    And a session "slate-unproven-vole" nothing can prove the liveness of
    And session "slate-unproven-vole" left a scratch worktree of its own
    When I run "ctxloom session worktrees purge amber-quiet-heron --yes"
    Then only the clean, provably-orphaned worktree is gone from disk
    And the uncommitted work is still there, spared in place
    And the report says why each spared worktree was left alone

  # THE LIVE-SESSION ARM, and the incident this whole design guards against:
  # reaping out from under a session that is still running. The tree here is
  # genuinely CLEAN — every other signal would call it reapable — and it
  # survives purely because its owning session still holds its lock.
  Scenario: Purging a session that is still running removes nothing
    Given a session "teal-running-wren" that is still running
    And session "teal-running-wren" left a clean scratch worktree
    When I run "ctxloom session worktrees purge teal-running-wren --yes"
    Then no worktree of "teal-running-wren" is removed, and ctxloom says it could prove nothing safe

  # THE UNPROVABLE ARM. A session with no lock file at all cannot be proven
  # ended, and "I cannot tell" and "yes, someone is using this" have the same
  # correct answer. A MISSING lock must never read as a free one — that is the
  # single inversion that would turn this sweep into data loss.
  Scenario: Purging a session nothing can prove the liveness of removes nothing
    Given a session "slate-unproven-vole" nothing can prove the liveness of
    And session "slate-unproven-vole" left a clean scratch worktree
    When I run "ctxloom session worktrees purge slate-unproven-vole --yes"
    Then no worktree of "slate-unproven-vole" is removed, and ctxloom says it could prove nothing safe

  # The population split, pinned so it cannot erode. Foreign worktrees are
  # invisible to the candidate finder by construction — they live outside the
  # sessions root — and the absence of a reaping verb for them is DELIBERATE.
  # This scenario exists so that a later "helpful" change that starts listing
  # them under a verb which also reaps has to fail a test first.
  #
  # `session worktrees` excludes foreign trees structurally:
  # isolation.findWorkWorktrees only ever scans under
  # ~/.ctxloom/sessions/, so this population is never even candidate-listed.
  Scenario: Her own long-lived worktrees are not this verb's business
    Given a finished session "amber-quiet-heron" whose transcript retention has passed
    And session "amber-quiet-heron" left a clean scratch worktree
    And a long-lived worktree "stale-feature" of her own, outside the sessions root, with unmerged work
    When I run "ctxloom session worktrees purge amber-quiet-heron --yes"
    Then her own long-lived worktree is untouched and was never listed

  # THE SWEEP INHERITS THE SAFETY RULES, it does not outrank them. Emptying a
  # session covers the scratch worktrees living inside it, and covering them
  # means reaching exactly the verdicts the worktree leaf reaches on its own: a
  # tree holding uncommitted work is spared IN PLACE — not stashed, not
  # branched, not "recoverable from reflog" — still sitting where its author
  # left it and still registered with git. A sweep that recursively removed a
  # harp directory instead would take that work with it without ever consulting
  # an owner marker.
  #
  # This project has lost work to a force-removed worktree before. That is why
  # the assertion reads the WIP file's own bytes off disk rather than checking
  # that a directory still exists.
  Scenario: Emptying a session spares the uncommitted work living inside it
    Given a finished session "amber-quiet-heron" whose transcript retention has passed
    And session "amber-quiet-heron" left a scratch worktree holding uncommitted work
    When I run "ctxloom session purge amber-quiet-heron --yes"
    Then the uncommitted work is still there, spared in place
    And the scratch worktree is still registered with git

  # The other half of the same rule: emptying a session reaches the verdicts the
  # worktree leaf reaches, so the clean tree of an ended session goes and the
  # one holding uncommitted work is spared in place. (Go's `session purge`
  # reaped through isolation.ReapWorktrees; the Rust purge once left every
  # worktree behind.)
  Scenario: Emptying a session reaps the clean scratch worktree and spares the dirty one
    # new: the worktree half of `session purge` (Go's purge reaped through isolation.ReapWorktrees; the Rust purge once left every worktree behind), which no Go scenario states on its own.
    Given a finished session "amber-quiet-heron" whose transcript retention has passed
    And session "amber-quiet-heron" left a clean scratch worktree
    And session "amber-quiet-heron" left a scratch worktree holding uncommitted work
    When I run "ctxloom session purge amber-quiet-heron --yes"
    Then only the clean, provably-orphaned worktree is gone from disk
    And the uncommitted work is still there, spared in place
    And the scratch worktree is still registered with git
