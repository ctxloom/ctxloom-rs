# Ported from journeys/j000600_agent_skill.feature (rev f36e8dfb2)
# NOT ported: the scenarios tagged @pending: it waits for `ctxloom run` to
#   launch an engine. The skill's body and script reaching each engine's skill
#   folder are cli/skill.feature's.

@pending
Feature: A skill you author once, and your assistant simply has

  Alice writes a "deploy-doctor" skill: guidance for diagnosing a broken
  deploy, with a small precheck script beside it. She wants what anyone wants
  from an Agent Skill: that her assistant picks it up on its own, reads the
  guidance she actually wrote, and can run the script she shipped with it.

  A skill is a directory in the Agent Skills shape that Claude Code and the
  other engines have converged on: a `SKILL.md` whose front matter names the
  skill, says when to reach for it, and lists the commands the assistant may
  run on her behalf, with the scripts it needs beside it. ctxloom keeps the
  skill in that shape and hands it to every engine as it is.

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
    When Alice creates the deploy-doctor skill in "ops", with a description, and writes its full body in her editor:
      """
      ctxloom skill create ops#skills/deploy-doctor -d "Diagnoses a broken deploy"
      """
    And the skill's front matter, as she writes it, lets her assistant run its precheck on her behalf:
      """
      ---
      name: deploy-doctor
      description: Diagnoses a broken deploy
      allowed-tools: Bash(scripts/run.sh)
      ---
      """
    And Alice adds an executable precheck script, scripts/run.sh, to the skill "ops#skills/deploy-doctor"
    And Alice puts the skill in a "clinic" profile:
      """
      ctxloom profile create clinic --add-bundle ops --add-skill ops#skills/deploy-doctor
      """
    And Alice starts a session on claude-code with the "clinic" profile:
      """
      ctxloom run --engine claude-code --profile clinic
      """
    Then Alice's session has the deploy-doctor skill, with the guidance she wrote
    And her assistant can run the skill's script as she shipped it, without asking her first
    And the project holds nothing the session wrote
