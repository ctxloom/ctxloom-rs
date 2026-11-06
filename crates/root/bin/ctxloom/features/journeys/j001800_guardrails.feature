# world: surfaces
# Ported from journeys/j001800_guardrails.feature
# NOT ported: "Alice's assistant receives the task-runner and
#   reuse-before-you-write guidance" and "ltk's own guidance is honest about
#   what it does — a redirect, not a block".
#   The first needs the companions' PreToolUse hook wiring in
#   .claude/settings.json as well as their fragments; a companion's fragments
#   now reach the materialized context (virtuous-word, companions slice 1),
#   its hooks do not yet (slice 2), and it is Go's ltk and reprise it names.
#   The second asserts ltk's OWN wording ("ltk is a cooperative redirect, not
#   a sandbox"), which is ltk's loadout text, not ctxloom's: the stub
#   companion every companion test uses (the compiled
#   ctxloom-companion-stub) can deliver a marker but cannot prove what ltk
#   says about itself, and a stand-in that repeats the sentence would assert
#   nothing. It waits for ltk's real loadout. Ported: Bob, who has neither
#   companion (registered with a command whose program is not there).
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
