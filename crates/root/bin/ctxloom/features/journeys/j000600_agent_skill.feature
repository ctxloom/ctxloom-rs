# world: surfaces
# Ported from journeys/j000600_agent_skill.feature
# NOT ported: nothing.


Feature: A skill you author once, and your assistant simply has

  Alice writes a "ctxloom-doctor" skill: guidance for validating a ctxloom
  setup, with a small precheck script beside it. She wants what anyone wants
  from an Agent Skill — that her assistant picks it up on its own, reads the
  guidance she actually wrote, and can run the script she shipped with it.

  A command is something you invoke; a skill is something the assistant reaches
  for unprompted. So the thing worth proving here is not that a file appeared —
  it is that what arrived is her work, whole and usable.

  # WHAT THIS JOURNEY DELIBERATELY DOES NOT CARRY. Which directory each of the
  # each engine expects, whether it registers the folder explicitly, the
  # per-engine path table — all of that is the skill noun's own surface and
  # lives in cli/skill.feature. This file asserts what ALICE can see. The spec
  # asserts it exhaustively, engine by engine, which is why the claim below is
  # made once here rather than four times.

  Background:
    Given Alice's project has a directory-form bundle "ops"
    And Alice starts a ctxloom-doctor skill in "ops" described as "DOCTOR-SKILL-MARKER-7d4e21"
    And Alice authors the ctxloom-doctor skill's full body in "ops#skills/ctxloom-doctor"
    And Alice adds an executable scripts/run.sh carrying the marker "DOCTOR-SCRIPT-MARKER-9c2f" to the skill "ops#skills/ctxloom-doctor"
    And a profile "clinic" with bundle "ops"
    And profile "clinic" curates skill "ops#skills/ctxloom-doctor"

  # The whole journey in one scenario, and every line of it is something Alice
  # would notice was missing. The body markers are the guidance she wrote — a
  # skill delivered as a correctly-named file with a truncated body is worse
  # than one that never arrived, because nothing looks wrong. The exec bit is
  # the difference between a script her assistant can run and one that fails
  # the first time it tries.
  Scenario: Alice's assistant gets the skill she wrote, guidance and script both
    When Carol materializes the "clinic" profile for claude-code
    Then the claude-code skill surface carries the doctor skill's marker "DOCTOR-SKILL-MARKER-7d4e21"
    And the claude-code skill surface carries the doctor skill's marker "DOCTOR-CHECK-DEPS-a1"
    And the claude-code skill surface carries the doctor skill's marker "DOCTOR-CHECK-HOOKS-TRUST-d4"
    And "out-claude-code/.claude/skills/ctxloom-doctor/scripts/run.sh" is executable
    And "out-claude-code/.claude/skills/ctxloom-doctor/scripts/run.sh" carries the marker "DOCTOR-SCRIPT-MARKER-9c2f"
