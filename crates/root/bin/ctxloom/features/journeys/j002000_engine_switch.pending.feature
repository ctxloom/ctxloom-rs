# Ported from journeys/j002000_engine_switch.feature (rev f36e8dfb2)
# NOT ported: any of it yet. These scenarios wait for the engine-swap preview,
#   the capability-loss report, and `ctxloom run` launching an engine.

@doc
@pending
Feature: Engine switch day

  A pricing change, a policy change, a vendor doing something nobody liked:
  the team is moving to a different engine. This is the day ctxloom's premise
  is tested, because the premise is that the team's context is the team's.
  The fragments, the profiles and the history belong to the team, not to
  whichever assistant was in fashion when they were written.

  Alice switches the team's agent to the new engine. Before she does, she
  sees what the team would give up. After she does, the next session starts
  with the same guidance as before, and anyone who checks the agent later is
  told what its engine cannot do.

  Background:
    Given Alice's team has standardised on one engine and is moving to another

  # The question a migrating team asks first is not "will it work" but "what
  # do we give up". The new engine has no hook for the end of a session, and
  # the team built on that hook. Alice sees it before anything changes.
  Scenario: Before she switches, Alice sees what the team would give up
    # new: the engine-swap preview, the switch's confirmation
    When Alice asks to switch the team's agent to the new engine, without confirming
    Then ctxloom shows what the new engine cannot carry that the old one did
    And the team's agent is still on the old engine

  # Once she confirms, the team's next session runs on the new engine and
  # starts with the same guidance, word for word. Its surfaces are in its own
  # session home; the project is left as it was.
  Scenario: The next session on the new engine starts with the same guidance
    # new: the switch, seen from the team's next session
    Given the team's last session on the old engine started with the team's guidance
    When Alice switches the team's agent to the new engine, and confirms
    And Alice starts a session with the team's agent
    Then the session runs on the new engine
    And it starts with the same guidance the old engine's session had, unchanged
    And the project holds nothing the session wrote

  # Weeks later, a teammate wonders why something stopped happening at the
  # end of each session. Wherever they look, at the agent or at the project's
  # health, ctxloom names what the agent's engine cannot do.
  Scenario: After the switch, ctxloom names what the new engine cannot do
    # new: the capability-loss report, after the switch
    When Alice switches the team's agent to the new engine, and confirms
    And a teammate later checks the team's agent and the project's health
    Then each of them names the session-end hook the new engine cannot carry
