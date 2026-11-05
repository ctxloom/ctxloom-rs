# world: surfaces
# Ported from cli/profile.feature
# NOT ported: every other scenario of the Go file. Ported: the uncarried
#   surfaces reaching the prose report.
@doc
Feature: profile — the composition that decides what an agent actually receives

  Rule: Export publishes the definition; materialize writes the runnable surface

    # The uncarried surfaces have to reach the PROSE, not only the JSON: the
    # person who needs this line is reading a terminal.
    Scenario: The uncarried surfaces are stated in the prose report, not only in the JSON
      Given Carol's team profile carries a shared fragment, command, MCP server, and hook
      When I run "ctxloom materialize team --target out-launch-text --backend mock-launch --format text"
      Then the command succeeds
      And the materialize report names each surface mock-launch does not carry, with a reason
