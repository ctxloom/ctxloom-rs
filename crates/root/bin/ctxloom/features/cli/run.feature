# Ported from Go ctxloom's tests/acceptance/features/cli/run.feature
# (revision f36e8dfb21, release/0.7), ONE scenario, the first of the parity POC.
# NOT ported: every other scenario of the Go file (the mock engine's recorded
# input, one-shot, dry-run, tag and agent selection, deny_tools, the opencode
# TUI): they need `run` to launch an engine. Plan override: clap's usage error
# says "unexpected argument" where cobra said "unknown flag"; the step maps one
# to the other (plan 8.2).
@doc
Feature: run — assembling a project's context and handing it to an engine

  `run` is the core operation, and the only one whose whole job is to put
  something in front of a model. It resolves which context this invocation
  gets — a profile, loose fragments, a tag selection, or a named agent binding
  — assembles it, and launches the configured engine with that context and a
  prompt.

  Surfaces covered here:

    ctxloom run [prompt...]              (interactive: a session)
    ctxloom run --one-shot [prompt...]   (one turn, then exit)
    ctxloom run --dry-run                (render what would be sent; send nothing)
    ctxloom run --profile/-p <name>
    ctxloom run --tag/-t <tag>
    ctxloom run --command/-r <name>      (a saved command as the prompt)
    ctxloom run --agent <name>

  A RUN IS EITHER A SESSION OR A SINGLE TURN, and `--one-shot` is how a caller
  says which. The flag names the MODE, not its output: every mode prints, and
  what actually differs is that a one-shot takes one turn and exits. Bare
  `ctxloom run` is the session — interactive and terminal-owning; the
  journeys drive it over a pty (j000200_setup.feature), and so does the one
  opencode scenario at the end, whose scripted engine records what it was
  launched with and exits.

  EVERY OTHER SCENARIO HERE DRIVES THE MOCK ENGINE, never a real one. That is not a
  compromise — it is what makes the full path OBSERVABLE. The mock records the
  setup payload that actually crossed the launch wire, so a scenario can assert
  what the model RECEIVED rather than that the command exited 0. `the mock
  recorded input contains` is the strongest assertion available on this
  surface, and the reason the deny_tools regression below is catchable at all:
  a per-hop test cannot see a field silently dropped crossing the proto wire,
  because the converter and its mirror struct read as a matched pair.

  Rule: The mode has one spelling, and asking for two at once is a refusal

    A caller reaching for a mode by a name it does not have must not get a run
    they did not ask for. An unknown flag is an error, and that error is what
    makes the real spelling discoverable; an alias quietly accepted alongside
    it would leave two names for one mode forever — in help, in scripts, and in
    every future reader's head.

    Scenario: A flag that is not the mode's name is refused rather than guessed at
      Given an initialized ctxloom project
      When Alice reaches for the mode by a name it does not have:
        """
        ctxloom run --print hello
        """
      Then the command fails
      And the output contains "unknown flag"

