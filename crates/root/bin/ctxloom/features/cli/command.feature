# world: surfaces
# Ported from cli/command.feature
# NOT ported: every other scenario of the Go file: create, MCP resource
#   reads (`the agent reads resource` has no MCP session in this port),
#   remove, the --no-distill edit and distill refusals (they need an editor
#   and the distiller), and the misspelled --bundle filter. Ported: the
#   engine-native landing and the CLI show.
@doc
Feature: command — authoring reusable prompt templates for AI coding assistants

  A "command" is a user-invoked slash template. This file keeps the two
  scenarios of Go's per-noun spec that need only the CLI and materialize.

  Rule: A materialized command lands in each engine's own native surface

    # ONE command, PARSED in its own native shape, never a bare file-exists.
    Scenario Outline: A team's command lands in each engine's own command surface
      Given Carol's team profile carries a shared fragment, command, MCP server, and hook
      When Alice materializes the team profile for <engine>
      Then the materialized <engine> command file carries the shared command's body, in its own native shape

      Examples:
        | engine      |
        | claude-code |

  Rule: A command's content reaches a person and an agent byte for byte

    # `show` prints the item's name as a header, echoed straight from the
    # argument: the body marker exists only inside the command's own bytes.
    Scenario: Showing a command's content over the CLI
      Given an initialized ctxloom project
      And a bundle "demo" exists
      And a command "review" in bundle "demo" exists
      When Alice reads it herself:
        """
        ctxloom command show demo#commands/review
        """
      Then the command succeeds
      And the output contains "review"
      And the output contains "COMMAND-BODY-review"
