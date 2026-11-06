# Ported from journeys/j001300_closeout.feature (rev f36e8dfb2): the scenarios not yet ported, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   j001300_closeout.feature with its steps (gherkin_lint.test.rs).
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

  The one thing she runs at the end is `ctxloom session sweep`, and it is
  deterministic: a fixed table decides every ended session of the project, and
  each action goes through the same leaf, with the same refusals, that this
  file specifies one at a time — worktrees, then reclaim, then retention. It
  removes what it can prove is safe and reports the rest; the same sessions
  give the same report. There is deliberately no top-level `cleanup` verb; the
  sweep lives under the noun it tidies.

  # COMMAND SURFACES THIS FILE COVERS: `ctxloom doctor`, `session worktrees`
  # and its `purge` leaf, `session purge` and `session transcript purge`, and
  # `session sweep`.
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

  # ---- Step 4: retention -------------------------------------------------
  # Read-only default, the confirmation line's first rule. Inspect always. The
  # payload assertion is that the report DESTROYED NOTHING — a "report" that
  # deletes as a side effect is the worst possible version of this leaf.
  #
  # `ctxloom session purge <harp>` reports without destroying: with no `--yes`,
  # nothing on disk or in the session index changes, on a TTY or not.
  Scenario: Purge shows its work before it destroys anything
    Given a finished session "amber-quiet-heron" whose transcript retention has passed
    And a session "brisk-copper-moth" that finished an hour ago
    When I run "ctxloom session purge amber-quiet-heron"
    Then the report lists what would be destroyed and what would be kept
    And every byte of every session is still on disk

  # WHAT "EMPTY" MEANS, and the one thing it may never take. Emptying a session
  # sweeps the machine-written bulk ctxloom wrote into it — the recorded
  # conversation. That is the whole point of the verb, and it sets the
  # deadline this whole flywheel runs against: a lesson still sitting in a
  # session directory when retention reaches it is a lesson lost.
  #
  # Two things survive regardless. The index entry stays, marked purged, because
  # a session that vanishes from the index is indistinguishable from one that
  # never existed. And human-authored artifacts are never negotiable — they are
  # NAMED in the report rather than silently skipped, because a file kept but
  # never mentioned is a file nobody will ever file.
  #
  # The session's output dir — its plans, and the summary an older ctxloom
  # left there — is the human's, and no destroyer reaches it. What this row
  # owns is the authored file planted in a real harp directory, which a wrong
  # implementation would really have destroyed.
  Scenario: Emptying a session takes everything ctxloom wrote, and nothing Alice did
    Given a finished session "amber-quiet-heron" carrying design notes nobody filed
    When I run "ctxloom session purge amber-quiet-heron --yes"
    Then the machine-written bulk of "amber-quiet-heron" is gone
    And its output dir is untouched, and its index entry survives
    And her unfiled design notes are still there, and were named in the report

  # Maximal friction on the irreversible case. Engines compact their own
  # sessions, so a session's transcript is its only record — for a structured
  # run the canonical transcript is the ONLY copy, and nothing can re-derive
  # it once the engine's own store has rotated. So a transcript is kept until
  # the session ended more than session_transcript_retention ago (30 days by
  # default), and the refusal says that is why. No flag overrides it.
  Scenario: Emptying a session that ended within its retention refuses, and says why
    Given a session "brisk-copper-moth" that finished an hour ago
    When I run "ctxloom session purge brisk-copper-moth --yes"
    Then ctxloom refuses, naming the session and that its transcript is within its retention
    And every byte of every session is still on disk
