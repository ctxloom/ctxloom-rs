# Ported from journeys/j002300_cross_engine_delegation.feature (rev f36e8dfb2)
# NOT ported: the scenarios tagged @pending: they wait for the coordinator.

@pending
Feature: Delegation — each child sees only its own context, over a real two-way bus

  Alice's session hands work to other agents: a "librarian" that knows where
  the team's documents live, and a "cartographer" that knows the codebase's
  layout. Each child works from its own profile, never the orchestrator's or
  its sibling's, and the orchestrator and its children talk to each other as
  the work goes on: the orchestrator sends a child a message, a child reports
  back without being asked, and a child that cannot start says why.

  It holds on whichever engine a child runs on, on Alice's machine or in a
  container, in the project or in a checkout of its own.

  # Each child, asked to start, reports from the guidance in its own profile
  # and not its sibling's.
  Scenario: Two children delegated to the same engine each report guidance found only in their OWN composed profile
    # substitution: said as what each child reports
    Given Alice's orchestrator can delegate to two agents, "librarian" and "cartographer", each carrying its own distinct guidance in its own profile
    And Alice's session is running
    When Alice's orchestrator delegates to "librarian"
    Then "librarian" reports its own guidance, not "cartographer"'s
    When Alice's orchestrator delegates to "cartographer"
    Then "cartographer" reports its own guidance, not "librarian"'s

  # A child on opencode works from the guidance its session home delivered to
  # it, as a child on Claude Code does.
  @opencode
  Scenario: A child delegated to an opencode label runs on opencode's runner and reports what its delivered context says
    # substitution: an agent on the opencode engine; what the child reports is what Alice sees
    Given Alice's orchestrator can delegate to "qwen-worker", an agent on opencode, carrying its own guidance
    And Alice's session is running
    When Alice's orchestrator delegates to "qwen-worker"
    Then "qwen-worker" reports its own guidance

  # The orchestrator sends a child a message, and the child's next turn takes
  # it up.
  Scenario: A message the orchestrator sends via agent_send reaches its child, verified in the child's own next reported turn
    # new: told as the user's flow.
    Given Alice's orchestrator can delegate to two agents, "librarian" and "cartographer", each carrying its own distinct guidance in its own profile
    And Alice's session is running
    And Alice's orchestrator has delegated to "librarian"
    When Alice's orchestrator sends "librarian" "J002300-ROUNDTRIP-ECHO-TOKEN-6d2e73"
    Then "librarian"'s next turn carries "J002300-ROUNDTRIP-ECHO-TOKEN-6d2e73"

  # The orchestrator hands out the work and waits, asking nothing more. The
  # child's report wakes it, and its next turn starts with the report.
  Scenario: An idle orchestrator is woken by its child's report, and its turn-start hook delivers it, with no receive
    # new: told as the user's flow.
    Given Alice's orchestrator can delegate to two agents, "librarian" and "cartographer", each carrying its own distinct guidance in its own profile
    And Alice's session is running
    When Alice's orchestrator delegates to "librarian" and waits
    Then the orchestrator is woken, and its next turn starts with "librarian"'s report of its own guidance, not "cartographer"'s

  # A child that cannot start tells its orchestrator so, with the reason it
  # failed, and records no turn.
  Scenario: A child that cannot start tells its orchestrator why
    # new: the orchestrator's view of a child that fails to launch
    Given Alice's orchestrator can delegate to two agents, "librarian" and "cartographer", each carrying its own distinct guidance in its own profile
    And Alice's session is running
    And "librarian" cannot start
    When Alice's orchestrator delegates to "librarian"
    Then the orchestrator hears from "librarian" that it could not start, and why
    And "librarian" recorded no turn

  # On a real engine, the child reads a phrase that exists only in its own
  # profile and sends it to its orchestrator itself. The orchestrator then
  # sends the child a fresh value mid-session, and the child sends it back.
  @live-only @every-runtime @delegation
  Scenario Outline: A delegated child on a real <engine> reports back a marker only its own composed context could supply
    # substitution: one live scenario for the child's own report by agent_send and the orchestrator's mid-session message, on every runtime lane
    Given a real "<engine>" engine is available for a delegated child carrying marker "<marker>"
    And Alice's session is running
    When Alice's orchestrator delegates to "delegate", asking it to send back the marker phrase it finds in its context
    Then the orchestrator receives "<marker>" from "delegate", sent by "delegate" itself
    When Alice's orchestrator sends "delegate" a fresh value and asks for it back
    Then the orchestrator receives that value from "delegate"

    Examples:
      | engine      | marker                                       |
      | claude-code | J002300-DELEGATE-MARKER-CLAUDE-CODE-1d4c07ab |
      | opencode    | J002300-DELEGATE-MARKER-OPENCODE-6e1b3f52    |

  # A child busy with one long turn still sees its orchestrator's message
  # while that turn is running, and acts on it before the turn ends.
  @live-only @every-runtime @midturn-mail
  Scenario Outline: A delegated child on a real <engine> in one long multi-tool turn acts on its parent's message before that turn ends
    # substitution: said as the orchestrator's message reaching a child mid-turn, on every runtime lane
    Given a real "<engine>" engine is available for a mid-turn-mail child
    And Alice's session is running
    When Alice's orchestrator delegates one long task to "delegate"
    And "delegate" tells the orchestrator it has started
    And Alice's orchestrator sends "delegate" a code
    Then "delegate" acknowledges the code before its long turn ends

    Examples:
      | engine      |
      | claude-code |
