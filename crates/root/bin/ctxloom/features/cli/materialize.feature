# world: surfaces
# Ported from cli/materialize.feature (rev f36e8dfb2)
# NOT ported: the Rule tagged @pending, "A run says so when the project holds
#   materialized context": it needs a running mock engine (S6).
@doc
Feature: materialize — the profiles' surfaces written at rest, any subset of them

  `ctxloom materialize` writes the assembled profiles into a directory as
  each engine's NATIVE files — its context file, MCP registry, settings and
  hooks, commands and skills — so an engine launched there with ctxloom out of
  the loop inherits them. Each engine's kinds are owned separately, so
  re-delivering one kind, or one engine, never takes another's files out, and
  `--release` takes back exactly what it names.

  Rule: A subset re-delivery leaves every other surface standing

    # The per-kind ownership is the whole point: a context-only run that took
    # the commands out with it would still satisfy "the context changed".
    Scenario: Re-materializing only the context keeps the commands and skills
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And I run "ctxloom skill create demo reviewer -d SKILL-MARKER-reviewer"
      And a profile "dev" with bundle "demo"
      When I run "ctxloom materialize dev --target out --backend mock"
      Then the command succeeds
      And the file "out/MOCK_CONTEXT.md" contains "FRAGMENT-BODY-testing"
      And the file "out/.mock/commands/demo/example.md" exists
      When I run "ctxloom materialize dev --target out --backend mock --surface context"
      Then the command succeeds
      And the file "out/MOCK_CONTEXT.md" contains "FRAGMENT-BODY-testing" exactly 1 times
      And the file "out/.mock/commands/demo/example.md" exists
      When I run "ctxloom materialize --target out --backend mock --release --surface commands"
      Then the command succeeds
      And the file "out/.mock/commands/demo/example.md" does not exist
      And the file "out/MOCK_CONTEXT.md" contains "FRAGMENT-BODY-testing"

  Rule: The context can land in any file inside the target, and leaves it as it was

    Scenario: Context written into the user's own file is released byte for byte
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      And the project already has the file "out/docs/AGENTS.md":
        """
        # Our agents

        Hand-written notes that stay ours.
        """
      When I run "ctxloom materialize dev --target out --backend mock --surface context=file:docs/AGENTS.md"
      Then the command succeeds
      And the file "out/docs/AGENTS.md" contains "Hand-written notes that stay ours."
      And the file "out/docs/AGENTS.md" contains "FRAGMENT-BODY-testing"
      And the file "out/MOCK_CONTEXT.md" does not exist
      When I run "ctxloom materialize --target out --backend mock --release --surface context"
      Then the command succeeds
      And the file "out/docs/AGENTS.md" does not contain "FRAGMENT-BODY-testing"
      And the file "out/docs/AGENTS.md" matches "\A# Our agents\n\nHand-written notes that stay ours.\n?\z"

  Rule: Two engines' files coexist in one directory

    Scenario: Releasing one engine leaves the other's files
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      When I run "ctxloom materialize dev --target out --backend mock --backend claude-code"
      Then the command succeeds
      And the file "out/MOCK_CONTEXT.md" contains "FRAGMENT-BODY-testing"
      And the file "out/CLAUDE.md" contains "FRAGMENT-BODY-testing"
      When I run "ctxloom materialize --target out --backend mock --release"
      Then the command succeeds
      And the file "out/MOCK_CONTEXT.md" does not exist
      And the file "out/CLAUDE.md" contains "FRAGMENT-BODY-testing"

  Rule: The project directory is written only when asked for in so many words

    Scenario: With no target, materialize warns and refuses unless --yes is given
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a fragment "testing" in bundle "demo" exists
      And a profile "dev" with bundle "demo"
      When I run "ctxloom materialize dev --backend mock"
      Then the command fails
      And the output contains "no --target given"
      And the file "MOCK_CONTEXT.md" does not exist
      When I run "ctxloom materialize dev --backend mock --yes"
      Then the command succeeds
      And the file "MOCK_CONTEXT.md" contains "FRAGMENT-BODY-testing"

  @pending
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
