# Ported from journeys/j000400_multi_engine.feature (rev f36e8dfb2)
# NOT ported: any of it yet; these scenarios wait for `ctxloom run` to launch
#   an engine. What one materialize delivers to each engine is
#   cli/materialize.feature's.

@doc
@pending
Feature: One shared profile, reaching every engine in its own native format

  A team does not standardize on one assistant. Carol's team writes one shared
  profile, with a fragment, a command, an MCP server and a hook, once. Alice
  uses Claude Code; Bob, working in the same checkout, uses opencode. Each of
  them starts a session on their own engine, and each session starts with the
  team's profile, in whatever shape that engine reads. Nobody forks the
  profile per engine, nobody translates a fragment by hand, and the checkout
  they share is left as it was: what each session needs is in its own session
  home.

  Background:
    # new: the team's profile is the journey's starting point
    Given Carol's team profile carries a shared fragment, command, MCP server, and hook

  # Alice keeps her own notes for Claude Code in the project. Her session
  # starts with the team's guidance and her notes both; Bob's session on his
  # engine starts with the same guidance. Neither session writes anything into
  # the checkout they share.
  Scenario: Alice and Bob each start a session on their own engine with the team's profile
    # new: the journey's flow, two teammates on two engines in one checkout
    Given Alice keeps her own notes for Claude Code in the project
    When Alice starts a session on claude-code with the team profile
    Then Alice's session starts with the team's guidance and her own notes
    And Alice's session offers the team's command, MCP server and hook
    When Bob starts a session on opencode with the team profile
    Then Bob's session starts with the same team guidance
    And Bob's session offers the team's command, MCP server and hook
    And the project holds nothing either session wrote, and Alice's notes are as she wrote them

  # On a real engine, the assistant itself can see the team's guidance: asked
  # for a phrase that is only in the team's fragment, it answers with it.
  @live-only
  Scenario Outline: A real engine actually receives the shared context and can use it
    # substitution: the team's profile is the Background's, and the fragment's marker is the sentinel
    Given a real <engine> agent is available
    When Alice asks her <engine> assistant to repeat the sentinel it can see
    Then its reply contains the sentinel marker

    Examples:
      | engine      |
      | Claude      |
      | Opencode    |
