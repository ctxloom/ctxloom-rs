# Written for the new design; awaits product plan step 4 (docs/plans/ctxloom-features.md).
# Ported from cli/session.feature
# Ported from journeys/j001300_closeout.feature
# NOT ported: Go's other session destroyers. `session remove`, `session sweep`,
#   `session transcript purge` and `session worktrees purge` are retired:
#   `session purge` is the one destroyer, and a purged session is gone, from
#   disk and from the listing (vocabulary §8). Go's "the sweep empties a
#   session but does not unlist it" is superseded by that rule. `session
#   adopt` is dropped.
@pending
Feature: session purge — the one destroyer, and a purged session is gone

  `ctxloom session purge <harp>` purges one session; `ctxloom session purge
  --older-than <age>` purges every session past that age. Purging takes the
  session's index entry, its transcript and its scratch worktrees: afterwards
  it is not on disk and not in `session list`. The session's output dir (its
  plans, any summary) is the human's and is never touched.

  Purge takes only what ctxloom wrote. A session still holding anything the
  human wrote (uncommitted work in a scratch worktree, notes nobody filed) is
  refused whole, and the report names each thing that kept it.

  Without `--yes` purge only reports: it changes nothing, names the exact
  command that would, and exits 4 when no one is at a terminal to confirm.

  Background:
    # new: Rust's own Background, stating the Given that Go repeats in each scenario (see their substitutions).
    Given an initialized ctxloom project

  Rule: Purge reports first, and with --yes the session is gone

    Scenario: The sweep reports before it removes
      # substitution: the project Given is the Background's; the report-only run exits 4 (confirmation required off a terminal) where Go's exited 0.
      Given a finished session "amber-swift-owl" with a transcript
      When I run "ctxloom session purge amber-swift-owl"
      Then the command exits with code 4
      And the output contains "removed nothing"
      And the output contains "ctxloom session purge amber-swift-owl --yes"
      And the home file ".ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl" exists

    Scenario: --yes removes the index entry and the transcript
      # substitution: the project Given is the Background's; the verb is `session purge` (Go's `session remove` is retired), and the session's whole directory goes with it.
      Given a finished session "amber-swift-owl" with a transcript and a summary an older ctxloom left
      When I run "ctxloom session purge amber-swift-owl --yes"
      Then the command succeeds
      And the home file ".ctxloom/sessions/amber-swift-owl" does not exist
      And the output file "essence.md" of session "amber-swift-owl" exists
      When I run "ctxloom session list --all"
      Then the output does not contain "amber-swift-owl"

    # The session lock only ever refuses. A free lock proves the owner dead; a
    # held lock, or no lock at all, refuses: "cannot determine" is never
    # permission. The refusal names the exact command that purges it deliberately.
    Scenario: A session the lock cannot prove finished is refused, and the escape is named
      # substitution: the project Given is the Background's; the escape is `--force` (Go's `--even-if-live`).
      Given a finished session "amber-swift-owl" with a transcript but no session lock
      When I run "ctxloom session purge amber-swift-owl --yes"
      Then the command fails
      And the output contains "cannot be proven dead"
      And the output contains "ctxloom session purge amber-swift-owl --yes --force"
      And the home file ".ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl" exists

    Scenario: --force purges it anyway
      # new: Go's "--even-if-live sweeps it anyway", with `--force` overriding the unprovable-liveness refusal; the session is gone from the listing too.
      Given a finished session "amber-swift-owl" with a transcript but no session lock
      When I run "ctxloom session purge amber-swift-owl --yes --force"
      Then the command succeeds
      And the home file ".ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl" does not exist
      When I run "ctxloom session list --all"
      Then the output does not contain "amber-swift-owl"

    # The index says ended; the lock says running. The lock wins.
    Scenario: A session running again is refused by the transcript's own destroyer
      # substitution: the project Given is the Background's; the destroyer is `session purge` (`session transcript purge` is retired), and the escape is `--force`.
      Given a session "amber-swift-owl" with a transcript that is running again
      When I run "ctxloom session purge amber-swift-owl --yes"
      Then the command fails
      And the output contains "owner is alive"
      And the output contains "--force"
      And the home file ".ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl" exists

  Rule: A transcript is the session's record, kept until its retention passes

    Engines compact their own sessions, so a session's transcript is its only
    record. Purge refuses a session that ended within
    session_transcript_retention (30 days unless configured), and one marked
    keep at any age. No flag overrides it: `ctxloom session keep <harp> --undo`
    releases a kept one, and time releases the rest.

    Scenario: A transcript within its retention is refused by every destroyer
      # substitution: the project Given is the Background's; `session purge` is the one destroyer, so Go's outline over three destroyers is this one scenario.
      Given a session "brisk-copper-moth" with a transcript that finished an hour ago
      When I run "ctxloom session purge brisk-copper-moth --yes"
      Then the command fails
      And the output contains "retention"
      And the home file ".ctxloom/sessions/brisk-copper-moth/transcripts/transcript.jsonl" exists

    Scenario: A kept transcript is refused at any age, and the listing says it is kept
      # substitution: the project Given is the Background's; the destroyer is `session purge` (`session transcript purge` is retired).
      Given a finished session "amber-swift-owl" with a transcript
      When I run "ctxloom session keep amber-swift-owl"
      Then the command succeeds
      When I run "ctxloom session list --all --format json"
      Then the JSON output array "$" contains an object whose "harp" is "amber-swift-owl" and whose "kept" is "true"
      When I run "ctxloom session purge amber-swift-owl --yes"
      Then the command fails
      And the output contains "ctxloom session keep amber-swift-owl --undo"
      And the home file ".ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl" exists
      When I run "ctxloom session keep amber-swift-owl --undo"
      Then the command succeeds
      When I run "ctxloom session purge amber-swift-owl --yes"
      Then the command succeeds
      And the home file ".ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl" does not exist

  Rule: Purge takes one harp or an age, never both and never neither

    `--older-than <age>` purges every session of the project that ended longer
    ago than that. Without it, a configured `session_purge_age` is the age; with
    neither a harp, an age nor a configured age, purge is refused rather than
    guessing.

    Scenario: A harp and an age together are refused
      # new: purge takes one harp or one age.
      Given a finished session "amber-swift-owl" with a transcript
      When I run "ctxloom session purge amber-swift-owl --older-than 90d --yes"
      Then the command exits with code 2
      And the home file ".ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl" exists

    Scenario: Neither a harp nor an age is refused, naming both forms
      # new: purge with nothing to select refuses rather than purging everything.
      Given a finished session "amber-swift-owl" with a transcript
      When I run "ctxloom session purge --yes"
      Then the command exits with code 2
      And the output contains "ctxloom session purge <harp>"
      And the output contains "--older-than"
      And the home file ".ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl" exists

  Rule: A configured purge age stands where no age is given

    Background:
      # new: Rust's own.
      Given the project already has the file ".ctxloom/config.yaml":
        """
        schema_version: 8
        session_purge_age: 90d
        """
      And a finished session "amber-quiet-heron" whose transcript retention has passed
      And every session has been idle for 120 days

    Scenario: Without an age the configured purge age is purge's
      # new: the configured age selects when no --older-than is given.
      When I run "ctxloom session purge --yes"
      Then the command succeeds
      And the machine-written bulk of "amber-quiet-heron" is gone
      When I run "ctxloom session list --all"
      Then the output does not contain "amber-quiet-heron"

    Scenario: The flag beats the configured purge age
      # new: --older-than overrides the configured age.
      When I run "ctxloom session purge --older-than 200d --yes"
      Then the command succeeds
      And the machine-written bulk of "amber-quiet-heron" is still there

    Scenario: A configured purge age that is no age refuses before it touches anything
      # new: a malformed configured age is refused, not read as zero.
      Given the project already has the file ".ctxloom/config.yaml":
        """
        schema_version: 8
        session_purge_age: soon
        """
      When I run "ctxloom session purge --yes"
      Then the command fails
      And the machine-written bulk of "amber-quiet-heron" is still there
