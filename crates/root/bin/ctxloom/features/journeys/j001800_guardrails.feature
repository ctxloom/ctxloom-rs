# world: surfaces
# Ported from journeys/j001800_guardrails.feature
# NOT ported: "ltk's own guidance is honest about what it does — a redirect,
#   not a block". It asserts ltk's OWN wording ("ltk is a cooperative
#   redirect, not a sandbox"), which is ltk's loadout text, not ctxloom's: the
#   stub companion every companion test uses (the compiled
#   ctxloom-companion-stub) can deliver a marker but cannot prove what ltk
#   says about itself, and a stand-in that repeats the sentence would assert
#   nothing. It waits for an owner decision (ltk's real loadout). Ported:
#   Alice's delivery (below) and Bob, who has neither companion (registered
#   with a command whose program is not there).
# substitution: "Alice's assistant receives the task-runner and
#   reuse-before-you-write guidance" runs against the stub companion
#   (ctxloom-companion-stub), registered once as "ltk" and once as "reprise",
#   each answering a generic loadout: a fragment with a marker, and for "ltk"
#   two `pre_tool` hooks (`tool: shell` and `tool: file_edit`) running a stub
#   command. The stub stands in for ltk and reprise; neither's own wording or
#   command is asserted. What is asserted is ctxloom's half: the fragment
#   reaches CLAUDE.md and each declared tool class reaches
#   .claude/settings.json as its own PreToolUse entry whose matcher covers the
#   engine's tool names (a hand-written superset of Bash, PowerShell, Edit,
#   Write, MultiEdit, NotebookEdit).
@doc
Feature: Guardrails — when the assistant does not listen

  Every other journey in this suite proves DELIVERY: the right context reaches
  the right assistant, current. This journey proves ctxloom's half of the
  companions' promise: that the loadout reaches the assistant whether or not
  either binary happens to be on the machine, honestly described either way.

  # LOCKED — delivery, for TWO independent companions at once: the
  # registered-companion loadout reaching both the assembled CLAUDE.md AND the
  # generated .claude/settings.json — the hook wiring, not just prose.
  Scenario: Alice's assistant receives the task-runner and reuse-before-you-write guidance
    Given Alice's project exists
    And ltk and reprise are installed on Alice's machine
    When Alice starts a session
    Then her assistant receives ltk's task-runner guidance
    And her assistant receives reprise's reuse-before-you-write guidance
    And her assistant's engine is configured to run ltk on every shell command and file edit

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
