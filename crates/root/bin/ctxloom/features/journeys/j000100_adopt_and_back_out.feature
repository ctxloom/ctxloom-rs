# Go ctxloom's tests/acceptance/features/journeys/j000100_adopt_and_back_out.feature at f36e8dfb21 (release/0.7), kept
# as written except for the scenarios left out below.
# Left out:
#   - "Alice can check the harness deterministically ...": `ctxloom doctor` is S12's and is covered by doctor.feature.


Feature: Adopting ctxloom without regret — the first hour, and the exit verified before committing

  Alice has a live repo and five assistants configured five different ways. She
  is willing to try one more tool, on one condition she has learned the hard
  way: she wants to know she can get back out. Adoption is cheap to reverse or
  it is not really a trial.

  So her first hour has two halves. She wires ctxloom in, gets suspicious ten
  minutes later that it did anything at all — and then, before she commits any
  of it, she takes it back out again and checks that what she wrote is still
  hers.

  # Every Then here names something ALICE can see: her assistant's sessions
  # start through ctxloom, the tool tells her what it wired, her own files
  # survive. (The project's context itself reaches a session that
  # `ctxloom run` starts, as its system prompt; an assistant started by hand
  # gets ctxloom's session-start hook and no project context.)
  # The byte-level proofs — which key in which settings file, how the session-start
  # hook is told apart from a statusline entry, each engine's own config
  # surfaces — belong to cli/manage.feature, which owns that noun.

  Background:
    Given an empty project directory

  # Installing is the subject here, so the commands are spelled out: the
  # scaffold, then the explicit wiring of her assistant's own files into the
  # project (install alone writes none — a `ctxloom run` session carries its
  # own; ruled 2026-09-21: sessions carry their surfaces). Alice's question
  # ten minutes in is not "did it report success" — that is the thing she is
  # suspicious of — but whether a fresh assistant session actually starts
  # through ctxloom. (--no-pull and --degraded: the scaffold's profile
  # inherits ctxloom's published default, which install would pull, and this
  # cell never touches the network.)
  Scenario: Alice wires ctxloom in and her assistant's sessions start through it
    When Alice wires ctxloom into her project:
      """
      ctxloom manage install --engine claude-code --no-pull
      ctxloom materialize --yes --degraded
      """
    Then the command succeeds
    And ctxloom's session-start hook is wired into her assistant
    And ctxloom's own working state is kept out of source control

  # The tool's report must agree with the disk. A status command that says
  # "wired" over a project where nothing was written is worse than no status
  # command, because it answers exactly the question being asked.
  Scenario: What ctxloom reports it wired is what is actually there
    Given ctxloom is already wired into her project
    When Alice asks ctxloom what it has configured:
      """
      ctxloom manage check
      """
    Then the command succeeds
    And ctxloom reports the engine it wired her project for

  # The half that decides whether she adopts. Exiting cleanly means two things
  # at once: what ctxloom put in is gone, and what she authored is untouched.
  #
  # The wired state is asserted BEFORE the removal, so the scenario starts from
  # a project that demonstrably has something to remove — otherwise it would
  # pass just as well against an uninstall that does nothing.
  Scenario: Alice verifies she can leave before she commits
    Given ctxloom is already wired into her project
    And ctxloom's session-start hook is wired into her assistant
    When Alice takes ctxloom back out:
      """
      ctxloom manage uninstall
      """
    Then the command succeeds
    And nothing ctxloom wired into her assistant is left behind
    And the context she authored is still hers
