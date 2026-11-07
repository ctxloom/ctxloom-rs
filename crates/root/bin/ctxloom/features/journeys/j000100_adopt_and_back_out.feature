# world: base
# Ported from journeys/j000100_adopt_and_back_out.feature
# NOT ported: the wiring scenarios; cli/manage.feature covers wiring.
#   The scenarios tagged @pending: `ctxloom run` is not built (no run-launcher,
#   host-launcher or session directory writer yet), and both start a session.

Feature: Adopting ctxloom without regret — the first hour, and the exit verified before committing

  Alice has a live repo and five assistants configured five different ways. She
  is willing to try one more tool, on one condition she has learned the hard
  way: she wants to know she can get back out. Adoption is cheap to reverse or
  it is not really a trial.

  So her first hour has two halves. She sets ctxloom up, gets suspicious ten
  minutes later that it did anything at all — and then, before she commits any
  of it, she takes it back out again and checks that her project is as she
  left it.

  Leaving is deleting `.ctxloom` and `~/.ctxloom`, and nothing more. Every session ctxloom
  starts for her carries its hooks, its MCP server and her project's context
  in a session home of its own, so nothing is written into her project or
  into her assistant's own configuration. Started on its own, her assistant
  is exactly the assistant it was before she tried ctxloom. Her sessions, with
  their homes, their records and the checkouts her agents worked in, live in
  `.ctxloom` too, where she can look at an agent's edits before she takes
  them, and where git does not see them; deleting `.ctxloom` takes them all.
  What ctxloom keeps for her across projects, such as the companions she has
  added and the content it has fetched, is in `~/.ctxloom`, and deleting that takes the rest.

  Background:
    # substitution: Alice's setting.
    Given an empty project directory
    And Alice's assistant is Claude Code
    And Alice has exported her assistant's token
    And ctxloom's published default content is within reach

  Scenario: Alice sets ctxloom up, and it keeps to its own two directories
    # new: her first step.
    When Alice sets ctxloom up in her project:
      """
      ctxloom init
      """
    Then the command succeeds
    And nothing in her project outside .ctxloom was written
    And nothing in her home directory outside ~/.ctxloom was written

  @pending
  Scenario: Ten minutes in, a session started through ctxloom has her project's context
    # new: her second step.
    Given Alice has set ctxloom up in her project
    When Alice starts her assistant through ctxloom and asks it something:
      """
      ctxloom run --one-shot "what does this project expect of me?"
      """
    Then the command succeeds
    And her assistant was handed ctxloom's published default content
    And nothing in her project outside .ctxloom was written
    And nothing in her home directory outside ~/.ctxloom was written

  @json-default
  Scenario: Alice asks ctxloom whether everything is in order
    # new: her check.
    Given Alice has set ctxloom up in her project
    When Alice asks ctxloom whether everything is in order:
      """
      ctxloom doctor
      """
    Then she is shown a health check for every part of the harness

  @pending
  Scenario: Alice verifies she can leave before she commits
    # substitution: she leaves by deleting .ctxloom and ~/.ctxloom.
    Given Alice has set ctxloom up in her project
    And Alice has started her assistant through ctxloom
    And her project has a .ctxloom directory
    When Alice takes ctxloom back out by deleting .ctxloom and ~/.ctxloom
    Then her project is exactly as it was before she set ctxloom up
    And her home directory is exactly as it was before she set ctxloom up
    And her assistant, started on its own, is exactly as it was before she set ctxloom up
