# Ported from journeys/j000600_agent_skill.feature (rev f36e8dfb2)
# NOT ported: any of it yet; the scenario waits for `ctxloom run` to launch an
#   engine. Each engine's skill folder, and the skill's full body and script
#   reaching it, are cli/skill.feature's.

@pending
Feature: A skill you author once, and your assistant simply has

  Alice writes a "ctxloom-doctor" skill: guidance for validating a ctxloom
  setup, with a small precheck script beside it. She wants what anyone wants
  from an Agent Skill: that her assistant picks it up on its own, reads the
  guidance she actually wrote, and can run the script she shipped with it.

  A command is something you invoke; a skill is something the assistant
  reaches for unprompted. So what matters to Alice is not that a file
  appeared, but that what arrived is her work, whole and usable.

  # Alice writes the skill, adds her script, puts the skill in her profile and
  # starts a session. A skill that arrives with half its guidance, or a script
  # her assistant cannot run, is worse than none, because nothing looks wrong
  # until it is needed.
  Scenario: Alice's assistant gets the skill she wrote, guidance and script both
    # substitution: the authoring steps are the scenario's own, in the order Alice takes them, and she starts a session rather than materializing
    Given Alice's project has a directory-form bundle "ops"
    And Alice starts a ctxloom-doctor skill in "ops" described as "DOCTOR-SKILL-MARKER-7d4e21"
    And Alice authors the ctxloom-doctor skill's full body in "ops#skills/ctxloom-doctor"
    And Alice adds an executable scripts/run.sh carrying the marker "DOCTOR-SCRIPT-MARKER-9c2f" to the skill "ops#skills/ctxloom-doctor"
    And a profile "clinic" with bundle "ops"
    And profile "clinic" curates skill "ops#skills/ctxloom-doctor"
    When Alice starts a session on claude-code with the "clinic" profile
    Then Alice's session has the ctxloom-doctor skill, with the guidance she wrote
    And her assistant can run the skill's script as she shipped it
    And the project holds nothing the session wrote
