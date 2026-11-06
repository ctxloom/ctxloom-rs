# Ported from cli/mcp_resources.feature (rev f36e8dfb2)
# NOT ported: any of it yet (the Feature is @pending).

@pending
Feature: MCP resources
  The agent reads ctxloom state through read-only resources served by the
  SESSION's endpoint: the catalog the session was assembled from (fragments,
  commands, skills, each by qualified ref) and the items its package carries.
  Each returns a typed payload. Listings that are not catalog items —
  profiles, remotes, sessions, registered servers — are the CLI's, not
  resources: a running session holds its package and its catalog, not the
  project's configuration.

  Scenario: The help resource documents the resource surface
    Given an initialized ctxloom project
    When the agent reads resource "ctxloom://help"
    Then the resource MIME type is "text/markdown"
    And the resource contains "ctxloom://"

  Scenario: The fragments resource reflects created fragments
    Given an initialized ctxloom project
    And a bundle "demo" exists
    And a fragment "testing" in bundle "demo" exists
    When the agent reads resource "ctxloom://fragments"
    Then the resource MIME type is "application/yaml"
    And the resource contains "testing"

  Scenario: The commands resource reflects created prompts
    Given an initialized ctxloom project
    And a bundle "demo" exists
    And a command "review" in bundle "demo" exists
    When the agent reads resource "ctxloom://commands"
    Then the resource MIME type is "application/yaml"
    And the resource contains "review"
