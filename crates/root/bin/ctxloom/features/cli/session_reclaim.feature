# world: base
# Ported from cli/session.feature
# NOT ported: the worktree rows (S11) and `session adopt` (needs the engines'
#   transcript readers, S9). Ported: the destroyers (purge, transcript purge)
#   and the retention rule.
Feature: session purge — each population owns its destroyer, and every one reports first

  Background:
    # new: Rust's own Background, stating the Given that Go repeats in each scenario (see their substitutions).
    Given an initialized ctxloom project


  Rule: Each population owns its destroyer, and every one reports first

    A session holds two separable populations: the conversation it recorded
    and the scratch git checkouts it left behind. Each is destroyed by the
    sub-noun that understands it, and `session purge` sweeps both. The
    session's output dir — its plans, and any summary an older ctxloom left
    there — is the human's, and no destroyer reaches it.

    None of them removes anything without `--yes`. A run without it prints
    the plan and then says outright that it removed nothing, naming the exact
    command that would — a plan and an outcome otherwise render identically,
    and the difference is the whole point.

    Scenario: Purging a transcript reports before it removes
      # substitution: Go repeats `Given an initialized ctxloom project` in each scenario; this file states it once, in its Background.
      Given a finished session "amber-swift-owl" with a transcript
      When I run "ctxloom session transcript purge amber-swift-owl"
      Then the command succeeds
      And the output contains "removed nothing"
      And the output contains "ctxloom session transcript purge amber-swift-owl --yes"
      And the home file ".ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl" exists

    Scenario: --yes destroys the transcript and leaves the output dir
      # substitution: Go repeats `Given an initialized ctxloom project` in each scenario; this file states it once, in its Background.
      Given a finished session "amber-swift-owl" with a transcript and a summary an older ctxloom left
      When I run "ctxloom session transcript purge amber-swift-owl --yes"
      Then the command succeeds
      And the home file ".ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl" does not exist
      And the output file "essence.md" of session "amber-swift-owl" exists

    Scenario: The sweep reports before it removes
      # substitution: Go repeats `Given an initialized ctxloom project` in each scenario; this file states it once, in its Background.
      Given a finished session "amber-swift-owl" with a transcript
      When I run "ctxloom session purge amber-swift-owl"
      Then the command succeeds
      And the output contains "removed nothing"
      And the output contains "ctxloom session purge amber-swift-owl --yes"
      And the home file ".ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl" exists

    Scenario: The sweep empties a session but does not unlist it
      # substitution: Go repeats `Given an initialized ctxloom project` in each scenario; this file states it once, in its Background.
      Given a finished session "amber-swift-owl" with a transcript and a summary an older ctxloom left
      When I run "ctxloom session purge amber-swift-owl --yes"
      Then the command succeeds
      And the home file ".ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl" does not exist
      And the output file "essence.md" of session "amber-swift-owl" exists
      When I run "ctxloom session list --all"
      Then the output contains "amber-swift-owl"

    # The session lock only ever REFUSES. A free lock proves the owner dead; a
    # held lock, or no lock at all, refuses — "cannot determine" is never
    # permission. Every session from before the lock existed has no lock, so
    # the refusal names the exact command that destroys it deliberately.
    Scenario: A session the lock cannot prove finished is refused, and the escape is named
      # substitution: Go repeats `Given an initialized ctxloom project` in each scenario; this file states it once, in its Background.
      Given a finished session "amber-swift-owl" with a transcript but no session lock
      When I run "ctxloom session purge amber-swift-owl --yes"
      Then the command fails
      And the output contains "cannot be proven dead"
      And the output contains "ctxloom session purge amber-swift-owl --yes --even-if-live"
      And the home file ".ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl" exists

    Scenario: --even-if-live sweeps it anyway
      # substitution: Go repeats `Given an initialized ctxloom project` in each scenario; this file states it once, in its Background.
      Given a finished session "amber-swift-owl" with a transcript but no session lock
      When I run "ctxloom session purge amber-swift-owl --yes --even-if-live"
      Then the command succeeds
      And the home file ".ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl" does not exist

    # The index says ended; the lock says running. The lock wins, on the leaf
    # as much as on the sweep.
    Scenario: A session running again is refused by the transcript's own destroyer
      # substitution: Go repeats `Given an initialized ctxloom project` in each scenario; this file states it once, in its Background.
      Given a session "amber-swift-owl" with a transcript that is running again
      When I run "ctxloom session transcript purge amber-swift-owl --yes"
      Then the command fails
      And the output contains "owner is alive"
      And the output contains "--even-if-live"
      And the home file ".ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl" exists

  Rule: A transcript is the session's record, kept until its retention passes

    Engines compact their own sessions, so a session's transcript is its only
    record. Every destroyer of transcripts refuses one whose session ended
    within session_transcript_retention (30 days unless configured), and one
    marked keep at any age. No flag overrides it: `ctxloom session keep
    <harp> --undo` releases a kept one, and time releases the rest.

    Scenario Outline: A transcript within its retention is refused by every destroyer
      # substitution: Go repeats `Given an initialized ctxloom project` in each scenario; this file states it once, in its Background.
      Given a session "brisk-copper-moth" with a transcript that finished an hour ago
      When I run "ctxloom <destroyer> brisk-copper-moth --yes"
      Then the command fails
      And the output contains "retention"
      And the home file ".ctxloom/sessions/brisk-copper-moth/transcripts/transcript.jsonl" exists

      Examples:
        | destroyer                  |
        | session transcript purge   |
        | session purge              |
        | session remove             |

    Scenario: A kept transcript is refused at any age, and the listing says it is kept
      # substitution: Go repeats `Given an initialized ctxloom project` in each scenario; this file states it once, in its Background.
      Given a finished session "amber-swift-owl" with a transcript
      When I run "ctxloom session keep amber-swift-owl"
      Then the command succeeds
      When I run "ctxloom session list --all --format json"
      Then the JSON output array "$" contains an object whose "harp" is "amber-swift-owl" and whose "kept" is "true"
      When I run "ctxloom session transcript purge amber-swift-owl --yes"
      Then the command fails
      And the output contains "ctxloom session keep amber-swift-owl --undo"
      And the home file ".ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl" exists
      When I run "ctxloom session keep amber-swift-owl --undo"
      Then the command succeeds
      When I run "ctxloom session transcript purge amber-swift-owl --yes"
      Then the command succeeds
      And the home file ".ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl" does not exist
