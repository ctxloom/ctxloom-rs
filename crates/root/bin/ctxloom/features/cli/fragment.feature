# world: surfaces
# Ported from cli/fragment.feature
# NOT ported: every other scenario of the Go file (create, show, remove,
#   the MCP resource reads, the hand-authored-file outline that j000400
#   already carries, the editor and distill scenarios). Ported: the native
#   context file each engine reads.
@doc
Feature: fragment — reusable context units, and the engine surface each one reaches

  Rule: A fragment authored once reaches every engine's own native context surface

    # The team writes guidance in one place; each engine reads it from the
    # file that engine actually looks at, without anyone hand-translating it.
    Scenario Outline: The same fragment materializes into each engine's own native context file
      Given Carol's team profile carries a shared fragment, command, MCP server, and hook
      When Alice materializes the team profile for <engine>
      Then the materialized <engine> context carries the shared fragment's marker, in its own native shape

      Examples:
        | engine      |
        | claude-code |
        | mock        |
