# Ported from journeys/j000400_multi_engine.feature (rev f36e8dfb2): the scenarios not yet ported, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   j000400_multi_engine.feature with its steps (gherkin_lint.test.rs).
#   The ported scenarios are in the files that say `Ported from journeys/j000400_multi_engine.feature`.

@doc
@pending
Feature: One shared profile, reaching every engine in its own native format

  A team does not standardize on one assistant. Carol's team writes one shared
  profile — a fragment, a command, an MCP server, a hook — once. Alice's teammates
  use different engines, and every one of them needs that
  same profile to reach their own engine, in whatever native shape that engine
  actually reads. ctxloom's job is to be the one place the team's standard is
  authored, and to speak every engine's own dialect on the way out — nobody
  forks the profile per engine, and nobody hand-translates a fragment into
  different config formats.

  The engines do NOT converge on one shape, and this is not incidental —
  it is the whole point of proving it here rather than asserting it in prose.
  Verified straight from each engine's own surfaces declaration
  (each engine package's surfaces.go, plus backends/mock_surfaces.go):

    | engine      | context lands in            | MCP lands in   | hooks land in         | commands land in  |
    |-------------|-----------------------------|----------------|-----------------------|-------------------|
    | claude-code | CLAUDE.md (managed markers) | .mcp.json      | .claude/settings.json | .claude/commands/ |
    | mock        | MOCK_CONTEXT.md (managed markers) | .mock/mcp.json | .mock/settings.json | .mock/commands/   |

  ONE REAL VENDOR SHIPS TODAY, and the second row is the test double. That is
  stated rather than glossed: the mechanism below — one profile, each engine's
  own native shape — is genuinely exercised across two engines, but the
  stronger claim this journey was written for, that engines from DIFFERENT
  VENDORS diverge in shape, is not currently demonstrable. mock stands in for
  a second vendor and its surfaces are all project-local, so it cannot show
  the sharpest divergence there is: an engine whose hooks, MCP servers and
  prompts have no cwd-keyed home at all and are delivered per-session at
  launch. That engine's narrowed materialize is the parked scenario below.

  An absence a report states is a
  fact a team can plan around; an absence nothing mentions is the same tree with
  a lie on top of it.

  Proving Alice's bytes were WRITTEN in each engine's own shape is NOT the same
  claim as proving any engine READ them — that is what the second, much smaller
  table below exists to prove, for the engines where it can honestly be proven
  today.

  # @live: one row per real engine the live lane drives. Each row
  # self-skips without credentials, exactly like J000200's own @live scenario.
  # Adding an engine here is adding a ROW — no new Go and no new steps.
  @live
  Scenario Outline: A real engine actually receives the shared context and can use it
    Given a real <engine> agent is available
    And Carol's team profile carries a fragment with a sentinel marker
    When Alice asks her <engine> assistant to repeat the sentinel it can see
    Then its reply contains the sentinel marker

    Examples:
      | engine      |
      | Claude      |
      | Opencode    |
