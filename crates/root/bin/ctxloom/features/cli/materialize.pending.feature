# Ported from cli/materialize.feature (rev f36e8dfb2): the scenarios not yet ported, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   materialize.feature with its steps (gherkin_lint.test.rs).
#   The ported scenarios are in the files that say `Ported from cli/materialize.feature`.

@doc
@pending
Feature: materialize — the profiles' surfaces written at rest, any subset of them

  `ctxloom materialize` writes the assembled profiles into a directory as
  each engine's NATIVE files — its context file, MCP registry, settings and
  hooks, commands and skills — so an engine launched there with ctxloom out of
  the loop inherits them. Each engine's kinds are owned separately, so
  re-delivering one kind, or one engine, never takes another's files out, and
  `--release` takes back exactly what it names.

  Rule: A run says so when the project holds materialized context

    Scenario: A run in a project with materialized context warns and names the remedy
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      And the mock LLM responds "MOCK-REPLY"
      When I run "ctxloom materialize dev --backend mock --surface context --yes"
      Then the command succeeds
      When I run "ctxloom run --one-shot --profile dev unicorn-prompt"
      Then the command succeeds
      And the output contains "MOCK-REPLY"
      And the output contains "ctxloom materialize --release --surface context"
