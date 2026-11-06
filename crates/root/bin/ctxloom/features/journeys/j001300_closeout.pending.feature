# Ported from journeys/j001300_closeout.feature (rev f36e8dfb2): the scenarios not yet ported, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   j001300_closeout.feature with its steps (gherkin_lint.test.rs).
#   The two `session purge --older-than` scenarios below (the sweep reports
#   every session's fate; the sweep removes only what it can prove is safe)
#   are new-design scenarios, not Go's, and await product plan step 4
#   (docs/plans/ctxloom-features.md); they replace Go's sweep scenarios.
#   The ported scenarios are in the files that say `Ported from journeys/j001300_closeout.feature`.

@doc
@pending
Feature: The close-out — the end of a workstream

  The feature shipped on Friday. Monday the debris is real, and none of it is
  interesting enough that anyone will deal with it voluntarily: stale worktrees,
  several holding commits that were never merged; thousands of session
  directories; and the only durable record of what the team actually learned
  sitting in one person's head, where it will stay.

  This is the leg of the flywheel that closes it. A session produces a lesson,
  the lesson becomes authored content, the content shapes the next session.
  Skip it and ctxloom is a delivery pipeline with an open loop: it can hand an
  assistant everything the team has ever written down, and nothing that
  Friday's work discovered.

  The one thing she runs at the end is `ctxloom session purge --older-than
  <age>`, and it is deterministic: a fixed table decides every ended session
  of the project, with the same refusals as purging one session. It removes
  what it can prove is safe and reports the rest; the same sessions give the
  same report. A purged session is gone, from disk and from the listing.

  # COMMAND SURFACES THIS FILE COVERS: `ctxloom doctor`, `session worktrees`
  # (listing) and `session purge`.
  #
  # NOTE ON WHAT THE FIXTURES BUILD. A close-out flow is defined almost
  # entirely by what it REFUSES to do, and a refusal cannot be tested against a
  # fixture with nothing to refuse. So these scenarios build the real debris:
  # genuine `git worktree add` checkouts inside a harp's own work/
  # directory beside real session liveness locks, foreign long-lived worktrees
  # outside the sessions root, uncommitted work, and harp directories carrying
  # machine-written bulk beside human-authored plan files. Every "spared",
  # "skipped" and "preserved" assertion reads a real file that a wrong
  # implementation would really have destroyed.
  #
  # NOTE ON THE CONFIRMATION LINE, which several scenarios enforce directly:
  # inspect always (read-only default on every leaf); additive writes proceed
  # on plain apply WITH payload assertions ("wrote 0 fragments" is
  # exit-nonzero, never success); destruction confirms against evidence shown
  # in the same invocation; and the refusal list is hard-coded — no
  # force-remove, no dirty or unmerged or unowned trees, no live session, no
  # destroying a transcript its retention keeps, no touching vendor stores.
  #
  # NOTE ON TAGS. Every scenario passes and says what closed it rather than
  # being left silent. A scenario added here as a wish carries @wip and states
  # its own untag condition — whether anything here is still a wish is the
  # first thing a reader needs, and a @wip tag is the only honest way to say
  # so.

  Background:
    Given the feature shipped on Friday and Alice is closing the workstream out

  # doctor check 5 (the foreign-worktree report). The population ctxloom did
  # NOT create: report-only, always, by design. The report has to carry enough
  # for a human to act — merged-ness, dirty state, and the exact commands —
  # because ctxloom deliberately will not run them.
  #
  # Closed: DOCTOR-CHECK-FOREIGN-WORKTREES-r8 reports long-lived worktrees
  # outside the sessions root with their merge state (git.Git gained
  # MergedBranches — no merged-ness primitive existed anywhere before this),
  # their dirty state as measured AT CHECK TIME (never assumed — the fixture's
  # own tree is clean by the time doctor runs, having committed over its
  # planted WIP; a wrong implementation that echoed the fixture's setup step
  # back would print "dirty" here and be lying), and the manual commands —
  # `git worktree remove` then `git branch -d`, never `-D`.
  Scenario: Worktrees ctxloom did not create are reported, never touched
    Given a long-lived worktree "stale-feature" of her own, outside the sessions root, with unmerged work
    When I run "ctxloom doctor"
    Then the checks name the foreign worktree, that it is unmerged and dirty, and the exact commands to remove it

  # doctor check 4 (B13, the session-dir durability contract). The session dir
  # holds machine state only, classified row by row; readable outputs belong in
  # the session's output dir. An authored file at the session dir's TOP LEVEL
  # is classified by no row: a containerized agent writing a design note there
  # writes into container-ephemeral space and loses it on exit.
  #
  # Closed: DOCTOR-CHECK-HARP-DURABILITY-s9 warns about authored files in the
  # unclassified top level and names the output dir they belong in. The walk is
  # two-level and guards IsDir() on the OUTER iteration: the sessions root holds
  # lock files beside the harp directories, which are never inside any one
  # harp dir at all.
  Scenario: Authored artifacts in the unclassified middle of a harp directory are flagged
    Given a finished session "amber-quiet-heron" carrying design notes nobody filed
    When I run "ctxloom doctor"
    Then the checks warn that the design notes sit in the harp directory's unclassified top level

  # Report-only by default: with no `--yes`, nothing on disk or in the
  # session index changes, and the report says what would go and what stays.
  Scenario: Purge shows its work before it destroys anything
    # substitution: the report-only run exits 4 (confirmation required off a terminal).
    Given a finished session "amber-quiet-heron" whose transcript retention has passed
    And a session "brisk-copper-moth" that finished an hour ago
    When I run "ctxloom session purge amber-quiet-heron"
    Then the command exits with code 4
    And the report lists what would be destroyed and what would be kept
    And every byte of every session is still on disk

  # Purge takes only what ctxloom wrote. Human-authored files planted in a harp
  # directory are never taken: the session is refused whole, and each file is
  # named in the report, because a file kept but never mentioned is a file
  # nobody will ever file. The output dir is the human's and is never touched.
  Scenario: Emptying a session takes everything ctxloom wrote, and nothing Alice did
    # substitution: purge is all or nothing, so a session holding authored files is refused whole and keeps its index entry until she files or deletes them.
    Given a finished session "amber-quiet-heron" carrying design notes nobody filed
    When I run "ctxloom session purge amber-quiet-heron --yes"
    Then the command fails
    And the machine-written bulk of "amber-quiet-heron" is still there
    And its output dir is untouched, and its index entry survives
    And her unfiled design notes are still there, and were named in the report

  # A session's transcript is its only record, so it is kept until the session
  # ended more than session_transcript_retention ago (30 days by default), and
  # the refusal says that is why. No flag overrides it.
  Scenario: Emptying a session that ended within its retention refuses, and says why
    Given a session "brisk-copper-moth" that finished an hour ago
    When I run "ctxloom session purge brisk-copper-moth --yes"
    Then ctxloom refuses, naming the session and that its transcript is within its retention
    And every byte of every session is still on disk

  # One command closes the workstream out, and nothing in it is decided by a
  # model: each session's facts decide its fate by a fixed table. Without
  # --yes it names each session's fate and destroys nothing.
  Scenario: The sweep reports every session's fate before it changes anything
    # substitution: the verb is `session purge --older-than` (`session sweep` is retired); the report-only run exits 4 (confirmation required off a terminal).
    Given a finished session "amber-quiet-heron" whose transcript retention has passed
    And a finished session "brisk-copper-moth" whose transcript retention has passed
    And every session has been idle for 120 days
    When I run "ctxloom session purge --older-than 90d"
    Then the command exits with code 4
    And the sweep report names "amber-quiet-heron" and "brisk-copper-moth"
    And every byte of every session is still on disk

  # Purging by age inherits every refusal of purging one session, over a mixed
  # fixture where each refusal has something real to refuse. A running session
  # and one whose liveness nothing can prove are skipped whole. A session
  # holding uncommitted work is skipped whole, its work in place. A kept
  # session is skipped, and the report names the command that releases it.
  # The rest are gone. An internal one-shot is nobody's only record of
  # anything, so it goes at any transcript age.
  Scenario: The sweep removes only what it can prove is safe, and says why it left the rest
    # substitution: the verb is `session purge --older-than` (`session sweep` is retired); a purged session is gone from the listing, and a session holding uncommitted work keeps its clean worktree too.
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
