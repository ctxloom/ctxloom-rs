# Ported from Go ctxloom's journeys/j001300_closeout.feature (revision
# f36e8dfb21, release/0.7): the sweep scenarios, worktree rows included
# (S11). The journey's worktree-leaf scenarios are in
# cli/session_worktrees.feature; its doctor scenarios are S12's.
Feature: closeout — the sweep

  Background:
    Given an initialized ctxloom project

  # ---- The sweep ----------------------------------------------------------
  # One command ties the legs together, and nothing in it is decided by a
  # model: `session sweep` turns each session's facts into rows by a fixed
  # table (operations.DecideSweep) and acts only through the leaves above,
  # each of which re-checks the session's lock under its own hold.
  #
  # Read-only default, the confirmation line's first rule: the report names
  # what it would do to each session, and destroys nothing.
  #
  # Closed: `ctxloom session sweep`, reporting without --yes.
  Scenario: The sweep reports every session's fate before it changes anything
    Given a finished session "amber-quiet-heron" whose transcript retention has passed
    And a finished session "brisk-copper-moth" whose transcript retention has passed
    And every session has been idle for 120 days
    When I run "ctxloom session sweep --purge-older-than 90d"
    Then the sweep report names "amber-quiet-heron" and "brisk-copper-moth"
    And every byte of every session is still on disk

  # THE SWEEP INHERITS EVERY REFUSAL ABOVE, over a mixed fixture where each
  # refusal has something real to refuse. A running session and one whose
  # liveness nothing can prove are skipped whole. A scratch worktree holding
  # uncommitted work is spared IN PLACE — read back by its own bytes — while
  # its clean sibling goes, and its session is spared from purge. A session
  # past the purge age whose transcript retention has passed has its
  # transcripts emptied — its output dir (plans, any summary an older ctxloom
  # left) is the human's and a sweep never takes it; a session marked keep is
  # not, and the report names the command that releases it. An internal
  # one-shot is nobody's only record of anything, so it is emptied at any
  # transcript age.
  #
  # Closed: `ctxloom session sweep --yes`.
  Scenario: The sweep removes only what it can prove is safe, and says why it left the rest
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
    When I run "ctxloom session sweep --purge-older-than 90d --yes"
    Then the sweep skipped "teal-running-wren" and "slate-unproven-vole" without touching either
    And only the clean, provably-orphaned worktree is gone from disk
    And the uncommitted work is still there, spared in place
    And the report says why each spared worktree was left alone
    And the machine-written bulk of "amber-quiet-heron" is gone
    And the machine-written bulk of "gray-oneshot-wisp" is gone
    And the sweep kept the transcript of "brisk-copper-moth" and named "ctxloom session keep brisk-copper-moth --undo"
