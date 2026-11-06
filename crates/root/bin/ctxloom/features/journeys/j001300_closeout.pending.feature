# Ported from journeys/j001300_closeout.feature
# NOT ported: any of it yet. Elsewhere: purging one session is cli/session_purge.pending.feature's, its worktrees cli/session_worktrees.pending.feature's, and the check for notes left in a session's directory cli/doctor.pending.feature's.

@doc
@pending
Feature: The close-out — the end of a workstream

  The feature shipped on Friday. Monday the debris is real, and none of it is
  interesting enough that anyone will deal with it voluntarily: stale
  worktrees, several holding work that was never merged; a pile of finished
  sessions; and design notes nobody filed.

  Alice closes the workstream out with one command, `ctxloom session purge
  --older-than <age>`. First she sees what it would do: every old session's
  fate, decided the same way every time, and nothing touched. Then she lets
  it run. It takes what it can prove is safe, a purged session is gone, and
  it says why it left each of the rest. What it left is hers to deal with:
  work she has not saved, notes she has not filed, a session she marked to
  keep. Once she has dealt with them, the same command takes the rest. Her own
  long-lived worktrees were never ctxloom's to take; doctor tells her which
  are merged and how to remove them herself.

  Background:
    Given the feature shipped on Friday and Alice is closing the workstream out

  # She looks before anything is destroyed.
  Scenario: The sweep reports every session's fate before it changes anything
    # substitution: the verb is `session purge --older-than`; without --yes off a terminal it exits 4.
    Given a finished session "amber-quiet-heron" whose transcript retention has passed
    And a finished session "brisk-copper-moth" whose transcript retention has passed
    And every session has been idle for 120 days
    When I run "ctxloom session purge --older-than 90d"
    Then the command exits with code 4
    And the sweep report names "amber-quiet-heron" and "brisk-copper-moth"
    And every byte of every session is still on disk

  # She lets it run. A running session, and one nothing can prove has ended,
  # are left alone. A session holding work she has not saved is left whole,
  # the work where she left it. A session she marked to keep is left, and she
  # is told how to release it. The rest are gone, and gone from the listing.
  Scenario: The sweep removes only what it can prove is safe, and says why it left the rest
    # substitution: the verb is `session purge --older-than`; a purged session is gone from the listing, and a session holding uncommitted work keeps its clean worktree too.
    Given a finished session "amber-quiet-heron" whose transcript retention has passed
    And a finished session "brisk-copper-moth" marked keep
    And an internal one-shot session "gray-oneshot-wisp" that finished an hour ago
    And a finished session "olive-dirty-finch" whose transcript retention has passed
    And session "olive-dirty-finch" left a clean scratch worktree
    And session "olive-dirty-finch" left a scratch worktree holding uncommitted work
    And a session "teal-running-wren" that is still running
    And session "teal-running-wren" left a scratch worktree of its own
    And a session "slate-unproven-vole" nothing can prove the liveness of
    And session "slate-unproven-vole" left a scratch worktree of its own
    And every session has been idle for 120 days
    When I run "ctxloom session purge --older-than 90d --yes"
    Then the sweep skipped "teal-running-wren" and "slate-unproven-vole" without touching either
    And the uncommitted work is still there, spared in place
    And the report says why each spared worktree was left alone
    And the machine-written bulk of "amber-quiet-heron" is gone
    And the machine-written bulk of "gray-oneshot-wisp" is gone
    And the sweep kept the transcript of "brisk-copper-moth" and named "ctxloom session keep brisk-copper-moth --undo"
    And the sweep kept the transcript of "olive-dirty-finch" and named "ctxloom-wt-wip"
    When I run "ctxloom session list --all"
    Then the output does not contain "amber-quiet-heron"
    And the output does not contain "gray-oneshot-wisp"
    And the output contains "olive-dirty-finch"

  # She deals with what it named: she saves the work she wants, files her
  # notes, and runs the same command again.
  Scenario: Once she has dealt with what it named, the same command takes the rest
    # new: the close-out's second pass, after Alice has acted on the first one's report.
    Given a finished session "olive-dirty-finch" whose transcript retention has passed
    And session "olive-dirty-finch" left a scratch worktree holding uncommitted work
    And a finished session "amber-quiet-heron" carrying design notes nobody filed, whose transcript retention has passed
    And every session has been idle for 120 days
    And Alice has run "ctxloom session purge --older-than 90d --yes" and been told what it left
    When Alice saves the work she wants out of the scratch worktree of "olive-dirty-finch"
    And Alice files her design notes into the output dir of "amber-quiet-heron"
    And I run "ctxloom session purge --older-than 90d --yes"
    Then the command succeeds
    And the machine-written bulk of "olive-dirty-finch" is gone
    And the machine-written bulk of "amber-quiet-heron" is gone
    And her filed design notes are in the output dir of "amber-quiet-heron"

  # Her own long-lived worktrees were never ctxloom's to take. Doctor tells her
  # which are unmerged or hold work, and the commands she would run to remove
  # them herself.
  Scenario: Worktrees ctxloom did not create are reported, never touched
    Given a long-lived worktree "stale-feature" of her own, outside the sessions root, with unmerged work
    When I run "ctxloom doctor"
    Then the checks name the foreign worktree, that it is unmerged and dirty, and the exact commands to remove it
