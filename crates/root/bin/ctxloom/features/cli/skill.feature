# world: surfaces
# Ported from cli/skill.feature (rev f36e8dfb2)
# NOT ported: the Rules tagged @pending: authoring and removal, and the export
#   and import round trip (the zip import of a skill package).
@doc
Feature: skill — authoring an Agent Skill package, curating it, and shipping it

  A "command" is a user-invoked slash template. An Agent Skill is a different
  thing: a directory carrying SKILL.md (frontmatter name + description, an
  instructions body) plus optional bundled scripts and assets, which an engine
  loads on its own by progressive disclosure.

  This is the comprehensive per-noun spec for `ctxloom skill` — authoring a
  package, listing and showing it, curating which
  skills a profile exports, and round-tripping one through export/import.
  The narrative version is journeys/j000600_agent_skill.feature, which asserts what
  a PERSON gets: one authored skill arriving whole in every engine's own skill
  folder.

  # WHERE THE BOUNDARY FALLS, and it is not where it first looks. MATERIALIZING
  # a skill into an engine's native directory is not a `skill` verb at all — it
  # is `ctxloom materialize`, and the per-engine matrix that proves it lives in
  # j000600_agent_skill.feature across all five engines. A claude-only copy of that
  # matrix used to sit in this file (D6 in the suite-unification plan); it was
  # strictly weaker than j000600's, asserted nothing j000600 does not, and is gone. What
  # remains here is what the noun itself owns.

  @pending
  Rule: Authoring a package records what is actually on disk

    `skill create` scaffolds a package directory, and the directory IS the
    skill: nothing records its files anywhere else. Listing and showing a
    skill must reflect that real tree — every file, its sha256 and its POSIX
    mode — rather than a name someone declared.


    Scenario: Alice authors a skill package and its listing and show reflect the real tree
      Given Alice's project has a directory-form bundle "vault"
      When I run "ctxloom skill create vault reviewer -d SKILL-MARKER-reviewer-9f3c21"
      And Alice adds an executable scripts/run.sh carrying the marker "SCRIPT-MARKER-reviewer-2b7e40" to the skill "vault#skills/reviewer"
      When I run "ctxloom skill show vault#skills/reviewer --format json"
      Then the skill show output lists "SKILL.md" and an executable "scripts/run.sh"
      When I run "ctxloom skill list"
      Then the skill list output includes "reviewer" from bundle "vault"
      When I run "ctxloom skill show vault#skills/reviewer"
      Then the skill show output carries the frontmatter description "SKILL-MARKER-reviewer-9f3c21"
      # The session endpoint's catalog lists the skill by ref; a skill's
      # files reach an engine by delivery (its SKILL.md is materialized),
      # not as a resource body.
      When the agent reads resource "ctxloom://skills"
      Then the skill resource contains "reviewer"

  @pending
  Rule: A skill package can be created and removed

    Before `skill remove`, a skill package could be created and never
    removed at all. Bare invocation reports what would be removed and
    removes nothing; --yes applies it — the same posture every other
    `remove` leaf in this CLI shares.


    Scenario Outline: Removing a skill drops its on-disk directory
      Given Alice's project has a directory-form bundle "vault"
      And I run "ctxloom skill create vault reviewer -d SKILL-MARKER-reviewer-9f3c21"
      When I run "ctxloom skill remove vault#skills/reviewer <flags>"
      Then the command succeeds
      And the output reports "applied" as "<reports nothing removed>"
      And the output reports "apply" as "<names the apply command>"
      When I run "ctxloom skill list"
      Then the skill list output includes "reviewer" from bundle "vault"
      When I run "ctxloom skill remove vault#skills/reviewer --yes"
      Then the command succeeds
      And the output reports "name" as "reviewer"
      And the output reports "bundle" as "vault"
      And the file ".ctxloom/content/bundles/v2/vault/skills/reviewer" does not exist
      When I run "ctxloom skill list"
      Then the output does not contain "reviewer"

    @json-default
    Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
      | flags         | reports nothing removed | names the apply command                          |
      |               | false                   | ctxloom skill remove vault#skills/reviewer --yes |
      | --format json | false                   | ctxloom skill remove vault#skills/reviewer --yes |
      | --format text | Nothing was removed     | ctxloom skill remove vault#skills/reviewer --yes |

    Scenario: Removing a skill nobody created is refused, not silently reported as done
      Given Alice's project has a directory-form bundle "vault"
      When I run "ctxloom skill remove vault#skills/never-created --yes"
      Then the command fails

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

  @pending
  Rule: A package round-trips through export and import byte for byte

    Export packs the package's tree into an archive and import lands it in
    another bundle; the files arrive exactly as authored, modes included.

    Scenario: A skill's files survive export and import byte-for-byte
      Given Alice's project has a directory-form bundle "vault"
      And I run "ctxloom skill create vault reviewer -d SKILL-MARKER-reviewer-9f3c21"
      And Alice adds an executable scripts/run.sh carrying the marker "SCRIPT-MARKER-reviewer-2b7e40" to the skill "vault#skills/reviewer"
      And a directory-form bundle "landed" exists
      When I run "ctxloom skill export vault#skills/reviewer -o reviewer.zip"
      And I run "ctxloom skill import reviewer.zip --bundle landed --format json"
      Then the command succeeds
      And the imported files under "landed#skills/reviewer" match the originally-authored "vault#skills/reviewer", byte for byte

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
