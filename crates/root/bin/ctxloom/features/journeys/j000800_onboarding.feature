# world: surfaces
# Ported from journeys/j000800_onboarding.feature
# NOT ported: the clone-and-start scenarios (they start a session on an
#   engine, and pinned versions and remotes). Ported: the materialize report
#   naming the hook an engine cannot carry, on this port's engine of that kind.
@doc
Feature: A new engineer clones the repo and is already set up

  # A backend that cannot carry hooks loses the team's guardrail; this asks
  # for the materialize report naming what it could not deliver.
  #
  # It asserts on the RIGHT thing: it is not sensitive to the eventual wording,
  # only to whether the loss is reported at all.
  Scenario: Materializing for an engine that cannot carry hooks says so
    # substitution: this port registers no `mock-lossy`; `mock-launch` is its engine that carries no hooks (its home-keyed surfaces are reported, not written), so the scenario names it.
    Given Carol's team profile carries a shared fragment, command, MCP server, and hook
    When Alice materializes the team profile for mock-launch
    Then the materialize report names the hook it could not deliver to mock-launch
