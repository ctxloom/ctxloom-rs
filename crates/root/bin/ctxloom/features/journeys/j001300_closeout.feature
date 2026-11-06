# world: base
# Ported from journeys/j001300_closeout.feature
# NOT ported: the journey's worktree scenarios (they are in
#   cli/session_worktrees.feature) and its doctor scenarios but one (the
#   foreign-worktree and harp-durability checks; the gitignore check is
#   ported below). `session sweep` is retired: the close-out's one command is
#   `session purge --older-than <age>` (vocabulary §8). The two `session purge
#   --older-than` scenarios are written for that design and wait in
#   j001300_closeout.pending.feature for product plan step 4.
Feature: closeout — purging what the workstream left behind

  Background:
    # substitution: Go's Background step ("the feature shipped on Friday and Alice is closing the workstream out") is spelled `Given an initialized ctxloom project`, which is all the Rust step does.
    Given an initialized ctxloom project

  # The routine will not start against a repo that cannot commit its own
  # content: doctor names the superseded blanket ignore rule and the command
  # that retires it.
  Scenario: The routine will not start against a repo that cannot commit its own content
    Given the project still carries the superseded blanket ctxloom ignore rule
    When I run "ctxloom doctor"
    Then the checks name the ignore rule and the command that retires it
