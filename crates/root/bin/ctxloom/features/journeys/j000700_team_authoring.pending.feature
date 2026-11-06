# Ported from journeys/j000700_team_authoring.feature (rev f36e8dfb2): the scenarios not yet ported, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   j000700_team_authoring.feature with its steps (gherkin_lint.test.rs).
#   The ported scenarios are in the files that say `Ported from journeys/j000700_team_authoring.feature`.

@doc
@pending
Feature: A team lead shares a command with the team

  A team's standards only help if they reach every engineer's assistant the same
  way. When the lead writes down how the team works — a commit convention, a
  review checklist, a house pattern — it should land in every teammate's
  assistant automatically, just by being part of the project. No one copies
  prompts around; authoring it once, in the project, is enough.

  # Every Then here names something a TEAMMATE can see: the command reaches
  # his assistant, the compact form is
  # what he receives, the stale version is gone once he pulls again. The
  # byte-level proofs — that create writes real content and not just a name,
  # how a single command is read over CLI and MCP, what deleting one actually
  # removes, how --no-distill and an empty editor buffer are refused — belong
  # to cli/command.feature, which owns that noun.
  #
  # This journey deliberately authors and edits the command by writing the
  # bundle manifest directly rather than through `command create`/`command
  # edit`: the mechanism under test here is propagation across a real git
  # pull and distillation-serving — not the editor
  # round-trip, which cli/command.feature already covers on its own terms.

  # LOCKED — distillation: Carol shortens verbose guidance, and a teammate on
  # distilled context receives the compact form, not the original. NOTE:
  # distillation applies to COMMANDS as well as fragments; a fragment is used
  # here as the common case, but the same compact-form-served behavior holds
  # for commands.
  Scenario: Carol distills verbose guidance and teammates receive the compact form
    Given Carol has authored a verbose fragment in the project
    When Carol distills the fragment
    And she commits both forms
    And Bob pulls the project with distilled context enabled
    Then Bob's assistant receives the distilled guidance
    And it does not receive the verbose original

  # FUTURE — deferred, NOT in the green run. Whether Carol's OWN active assistant
  # picks up a change she just made is engine-dependent and complex: some engines
  # live-reload context, others need a restart (cf. J000200's two-phase restart), and it
  # varies by engine and how the session was launched. Pin per-engine live-reload
  # behavior first. Tracked as a task.
  @future
  Scenario: Carol's own active assistant picks up the change she just made
    Given Carol has an active session in the project
    When Carol edits a fragment
    Then her own assistant reflects the change without her copying anything
