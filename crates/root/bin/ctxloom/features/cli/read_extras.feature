# world: surfaces
# Ported from cli/read_extras.feature
# NOT ported: nothing.
Feature: Additional read and configuration commands
  Coverage for the remaining show/registration commands.

  # "the output contains <name>" asserted nothing about the CONTENT: `show`
  # prints the item name as a header, so the fixture seeds a marker that
  # exists nowhere but inside the item's bytes.
  Scenario: Show a fragment's content
    Given an initialized ctxloom project
    And a bundle "demo" exists
    And a fragment "testing" in bundle "demo" exists
    When I run "ctxloom fragment show demo#fragments/testing"
    Then the command succeeds
    And the output contains "testing"
    And the output contains "FRAGMENT-BODY-testing"

  Scenario: Show a prompt's content
    Given an initialized ctxloom project
    And a bundle "demo" exists
    And a command "review" in bundle "demo" exists
    When I run "ctxloom command show demo#commands/review"
    Then the command succeeds
    And the output contains "review"
    And the output contains "COMMAND-BODY-review"

  # ctxloom's own MCP server is served by the running session's endpoint and
  # written into the SESSION's registry at start; nothing at rest carries it.
  Scenario: An explicit materialize registers no ctxloom MCP server at rest
      Given an initialized ctxloom project
      When I run "ctxloom materialize --yes"
      Then the command succeeds
      And the file ".mcp.json" registers no MCP server named "ctxloom"
