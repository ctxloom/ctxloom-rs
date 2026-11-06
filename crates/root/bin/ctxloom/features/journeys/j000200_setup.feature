# world: surfaces
# Ported from journeys/j000200_setup.feature
# NOT ported: "Setup configures the agents, then a restart delivers their
#   context" (it launches a mock engine, which waits on the host runner), the
#   @live scenario (a real assistant) and the held-content scenario. Ported:
#   the two that need no engine, as Go runs them. The source repositories are
#   bare git repositories made in the scenario's temporary directory; "setup"
#   writes the project's configuration and its seed bundle and default
#   profile directly (as Go does, to stay off the network), and "starts a
#   session" is the materialize Go calls that.
@doc
Feature: Setting up ctxloom on a project

  A developer's assistant is only as good as the context it is handed.
  Adding a repository is the trust act: what the developer adds is what
  reaches the assistant.

  Scenario Outline: After setup, added sources are part of the configuration
    Given Alice has a fresh project directory
    And Alice has a personal ctxloom repository
    And her company has a ctxloom repository
    When Alice runs the ctxloom setup for <engine>
    And she adds her personal repository as a source
    And she adds her company's repository as a source
    Then her project is configured for <engine>
    And her personal repository's context is part of her configuration
    And her company repository's context is part of her configuration

    Examples:
      | engine      |
      | claude-code |

  Scenario: Content from a repository Alice adds reaches her assistant
    Given a third-party ctxloom repository
    When Alice adds it as a source
    And Alice starts a session
    Then her assistant receives that repository's content
