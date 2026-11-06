# world: surfaces
# Ported from journeys/j002000_engine_switch.feature
# NOT ported: the rest of the journey (j002000_engine_switch.pending.feature).
#   The swap's own contract and the unknown-engine refusal are
#   cli/agent.feature's.
@doc
Feature: Engine switch day

  A pricing change, a policy change, a vendor doing something nobody liked:
  the team is moving to a different engine. This is the day ctxloom's premise
  is tested, because the premise is that the team's context is the team's.
  The fragments, the profiles and the history belong to the team, not to
  whichever assistant was in fashion when they were written.

  Alice switches the team's agent to the new engine. Before she does, she
  sees what the team would give up. After she does, the next session starts
  with the same guidance as before, and every session the team recorded on
  the old engine is still there to look back on.

  Background:
    Given Alice's team has standardised on one engine and is moving to another

  # History that dies with the engine that produced it is history rented
  # from a vendor. Nobody looks for an old session until they need one.
  Scenario: History recorded under the old engine survives the move
    Given a session was recorded under the old engine before the switch
    When Alice swaps the engine under the binding
    Then the sessions recorded under the old engine are still listed after the switch
