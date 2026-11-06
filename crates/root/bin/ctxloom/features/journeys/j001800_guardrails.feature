# world: surfaces
# Ported from journeys/j001800_guardrails.feature
# NOT ported: "Alice's assistant receives the task-runner and
#   reuse-before-you-write guidance" and "ltk's own guidance is honest about
#   what it does — a redirect, not a block". Both need a registered
#   companion's loadout to reach the materialized context (ltk's guidance
#   fragment and its PreToolUse hook wiring in .claude/settings.json;
#   reprise's fragment). It does not in this port: a fake `ltk` and
#   `reprise` (made with fixture-program::install_program and registered
#   with `companion add`) are probed and recorded, and `materialize` writes
#   neither their fragments nor their hooks. Go's
#   Prober.ProbeCompanionLoadouts has no counterpart here, and ltk's loadout
#   text is not in this repository. Ported: Bob, who has neither companion.
@doc
Feature: Guardrails — when the assistant does not listen

  Every other journey in this suite proves DELIVERY: the right context reaches
  the right assistant, current. This journey proves ctxloom's half of the
  companions' promise: that the loadout reaches the assistant whether or not
  either binary happens to be on the machine, honestly described either way.

  # LOCKED — graceful degradation, the other direction. Bob's machine is not a
  # clone of Alice's: he may have neither companion installed. The team's own
  # context (first-party, no review — see J000800) must still arrive intact, and
  # nothing about his session may fail because two optional binaries are
  # absent. Reuses steps_j000800_onboarding.go's own installed/not-installed
  # companion fixture (reprise) verbatim rather than re-deriving it, and adds
  # the matching ltk fixture alongside it.
  Scenario: Bob, without either companion installed, still gets the team's context and nothing fails
    Given the team's project carries the context Carol has standardized on
    And Bob clones the project
    And the "reprise" companion is not installed on Bob's machine
    And the "ltk" companion is not installed on Bob's machine
    When Bob starts a session
    Then his assistant receives the team's standardized context
    And nothing fails because of the companion's presence or absence
