# world: surfaces
# Ported from cli/skill.feature
# NOT ported: authoring and removal, export and import round-trip scenarios
#   (the zip import of a skill package). Ported: profile curation and the
#   curated skill landing in the engine's skill folder, exec bit included.
@doc
Feature: skill — authoring an Agent Skill package, curating it, and shipping it

  Rule: A profile exports only the skills it curates

    Shipping a skill in a bundle does not push it at every project. Curation is
    the choice, so a bundle can offer more than any one profile takes.

    Scenario: Profile skill curation exports only the curated skill, not the bundle's other one
      Given Alice's project has a directory-form bundle "vault"
      And I run "ctxloom skill create vault reviewer -d SKILL-MARKER-reviewer-9f3c21"
      And I run "ctxloom skill create vault planner -d SKILL-MARKER-planner-1a2b3c"
      And a profile "studio" with bundle "vault"
      And profile "studio" curates skill "vault#skills/reviewer"
      When I run "ctxloom materialize studio --target out-curated --backend claude-code"
      Then "out-curated/.claude/skills/reviewer/SKILL.md" carries the marker "SKILL-MARKER-reviewer-9f3c21"
      And "out-curated/.claude/skills/planner" does not exist

  Rule: A curated skill lands in every engine's own skill folder

    Background:
      Given Alice's project has a directory-form bundle "ops"
      And Alice starts a ctxloom-doctor skill in "ops" described as "DOCTOR-SKILL-MARKER-7d4e21"
      And Alice authors the ctxloom-doctor skill's full body in "ops#skills/ctxloom-doctor"
      And Alice adds an executable scripts/run.sh carrying the marker "DOCTOR-SCRIPT-MARKER-9c2f" to the skill "ops#skills/ctxloom-doctor"
      And a profile "clinic" with bundle "ops"
      And profile "clinic" curates skill "ops#skills/ctxloom-doctor"

    # LOCKED — the core claim: every engine gets the doctor skill's real body,
    # not just a file with the right name.
    Scenario Outline: The ctxloom-doctor skill's full body reaches <engine>'s native skill surface
      When Carol materializes the "clinic" profile for <engine>
      Then the <engine> skill surface carries the doctor skill's marker "DOCTOR-SKILL-MARKER-7d4e21"
      And the <engine> skill surface carries the doctor skill's marker "DOCTOR-CHECK-DEPS-a1"
      And the <engine> skill surface carries the doctor skill's marker "DOCTOR-CHECK-AGENTS-b2"
      And the <engine> skill surface carries the doctor skill's marker "DOCTOR-CHECK-VERSION-c3"
      And the <engine> skill surface carries the doctor skill's marker "DOCTOR-CHECK-HOOKS-TRUST-d4"

      Examples:
        | engine      |
        | claude-code |

    # Exec bit survives materialization for a REAL surface, proving the doctor
    # skill's bundled precheck script is actually runnable once it lands.
    Scenario: The doctor skill's script is materialized executable
      When Carol materializes the "clinic" profile for claude-code
      Then "out-claude-code/.claude/skills/ctxloom-doctor/scripts/run.sh" is executable
      And "out-claude-code/.claude/skills/ctxloom-doctor/scripts/run.sh" carries the marker "DOCTOR-SCRIPT-MARKER-9c2f"
