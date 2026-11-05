# world: surfaces
# Ported from cli/mcp.feature
# NOT ported: every other scenario of the Go file (server show/set/remove,
#   the bare noun's refusal, the session endpoint). Ported: a team bundle's
#   shared server in the listing, and in each engine's own configuration,
#   with no ctxloom entry at rest.
@doc
Feature: mcp — the MCP servers ctxloom hands to every engine

  Rule: Composing a bundle that declares a server makes it reachable from the listing

    # "toolserver" is the bundle's key and the listed name alike, so the
    # command's own executable is asserted beside the name.
    Scenario: Alice's team bundle registers a shared MCP server
      Given Carol's team profile carries a shared fragment, command, MCP server, and hook
      And I run "ctxloom agent create dev --profiles team"
      And I run "ctxloom agent default dev"
      When I run "ctxloom mcp server list"
      Then the command succeeds
      And the output reports "servers[name=toolserver].command" as "j000400-mcp-tool-9e1b52"

  Rule: A server registered once reaches every engine in its own native configuration file

    # PAYLOAD, NOT EXISTENCE: the actual command under the actual server name.
    Scenario Outline: A shared MCP server materializes into <engine>'s own configuration file
      Given Carol's team profile carries a shared fragment, command, MCP server, and hook
      When Alice materializes the team profile for <engine>
      Then the materialized <engine> MCP configuration carries the shared server's command, in its own native shape

      Examples:
        | engine      |
        | claude-code |

  Rule: A materialized registry launches ctxloom as nothing — the session serves ctxloom's own server

    # ABSENCE UNDER THE KEY, NOT PRESENCE OF ANYTHING.
    Scenario Outline: <engine>'s materialized configuration registers no ctxloom server
      Given Carol's team profile carries a shared fragment, command, MCP server, and hook
      When Alice materializes the team profile for <engine>
      Then the materialized <engine> MCP configuration registers no server under ctxloom's own name

      Examples:
        | engine      |
        | claude-code |
