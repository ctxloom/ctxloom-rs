# Ported from cli/coordination_contract.feature (rev f36e8dfb2)
# Ported from journeys/j002100_delegation.feature (rev f36e8dfb2) (the last Rule)
# NOT ported: any of it yet (the Feature is @pending).

@pending
Feature: The coordination tools advertise a closed message-kind vocabulary

  An agent only knows what a mailbox message IS because the tool surface tells
  it. That used to be a free-text convention — put a `kind` key in the
  `structured` object, spelled however you like — and "however you like"
  included the coordinator's OWN kinds. A child could name one, and the
  coordinator interpolated the name into the notice the receiving model saw: a
  forged coordinator notice, assembled out of a string nobody validated.

  So `kind` is now a field with a closed vocabulary. The value an agent may
  send is enumerated in the tool's own schema, the coordinator's own kinds are
  refused from a sender, and there is nowhere left to write a kind that means
  something the vocabulary does not.

  These scenarios are what keeps that closed — a silent reversion to the
  free-string convention, or an accidental widening of the vocabulary, comes
  back red here.

  # WHAT THIS FEATURE CAN AND CANNOT SEE (see steps_coordination_contract.go):
  # it reads the runner-terminated MCP surface — the proto-canonical one a real
  # harness gets through `ctxloom run`, enumerated over a real
  # MCP transport. That surface is the ONLY place a coordination tool's RESULT
  # shape is advertised, and no external MCP client in this suite can reach a
  # spawned session's per-cell runner socket. What these scenarios claim is
  # about the advertised CONTRACT. They deliberately do not claim anything
  # about a delivered message's runtime payload: that needs the runner
  # topology, and asserting it from here would be an overclaim.

  Background:
    Given the coordination tool surface a harness receives

  Scenario: agent_send advertises the kind field, not a kind convention buried in structured
    When I read the "agent_send" tool's input contract
    Then it advertises "MESSAGE_KIND_RESULT"
    And it advertises "MESSAGE_KIND_QUESTION"
    # The retired convention: `structured` no longer tells an agent to name the
    # kind inside it, and the old lowercase spellings are gone from the surface.
    And it does not advertise "envelope whose `kind` names the message kind"
    And it does not advertise "result | question | error"

  # The vocabulary is CLOSED, and closed means enumerated: a value that is not
  # on this list cannot be named at all, and one that is added to the proto
  # without a decision shows up here rather than in a model's context.
  Scenario: the advertised vocabulary is exactly the declared one
    When I read the "agent_send" tool's input contract
    Then the kind vocabulary it advertises is exactly:
      | MESSAGE_KIND_UNSPECIFIED   |
      | MESSAGE_KIND_MESSAGE       |
      | MESSAGE_KIND_RESULT        |
      | MESSAGE_KIND_ERROR         |
      | MESSAGE_KIND_QUESTION      |
      | MESSAGE_KIND_USER_INJECTED |
      | MESSAGE_KIND_USER_CONTROL  |
      | MESSAGE_KIND_EXITED        |
      | MESSAGE_KIND_STEER         |
      | MESSAGE_KIND_REPORT        |
      | MESSAGE_KIND_SUMMARIZE     |

  # A sender is told, in the schema it reads before choosing an argument, that
  # the coordinator's own kinds are not its to name AND that a wrong value is
  # REFUSED rather than accepted-and-ignored. An agent that learns either of
  # those only from a rejection has already spent a turn on it — and one that
  # believes an unknown kind passes through will keep sending it.
  Scenario: the surface states the rejection behaviour, not just the vocabulary
    When I read the "agent_send" tool's input contract
    Then it advertises "REQUIRED"
    And it advertises "CLOSED vocabulary of exactly four values"
    And it advertises "is REJECTED from a sender, not quietly downgraded"
    And it advertises "An unrecognised value is likewise REJECTED rather than passed through"
    # The four sender-legal values, named as such — `message` included, which
    # the retired free-text doc never listed.
    And it advertises "MESSAGE_KIND_MESSAGE (plain prose"
    And it advertises "MESSAGE_KIND_ERROR"

  # The tool DESCRIPTION carries it too, not only the per-field schema: some
  # harnesses surface the description and elide argument docs, and an agent that
  # sees only the description would otherwise learn nothing about the closure.
  Scenario: the tool description itself states the vocabulary is closed
    When I read the "agent_send" tool's description
    Then it advertises "`kind` is REQUIRED and its vocabulary is CLOSED"
    And it advertises "is REFUSED rather than accepted-and-ignored"

  Rule: The coordinator's verbs refuse what is not theirs to accept

    Alice's orchestrator is a session owner (a mock-engine `ctxloom run`)
    with two agents it can delegate to, "reviewer" and "fixer", each with its
    own profile, MCP server and permission mode.

    # A message's kind is a security boundary, not a label: user_injected is
    # coordinator-reserved and reads to a recipient as the human speaking, and
    # steer and exited are the coordinator's own notices. A reserved kind is
    # refused at the coordinator's ingress, naming the kinds a sender may use;
    # a spelling outside the vocabulary is refused at the tool schema, naming
    # the value; a sendable kind still goes through. This drives the
    # orchestrator's own agent_send: no tool here reaches a child's runner
    # socket. The refusal is the same ingress guard for both sender surfaces,
    # and the child-identity case is the unit suite's.
    Scenario: A sender cannot claim a coordinator-reserved message kind
      # substitution: the journey's Background is the scenario's first Givens
      Given Alice's orchestrator can delegate to two agents, "reviewer" and "fixer", each with its own profile, its own MCP server, and its own permission mode
      And a session owner is standing
      When the agent calls tool "agent_run" with:
        | role         | fixer |
        | input.prompt | go    |
      Then the tool call succeeds
      And "fixer"'s spawned session is remembered
      When the agent sends "fixer"'s remembered session a message of kind "user_injected"
      Then the tool call fails
      And the tool failure message contains "reserved for the coordinator"
      And the tool failure message contains "message | result | error | question"
      When the agent sends "fixer"'s remembered session a message of kind "made_up_kind"
      Then the tool call fails
      And the tool failure message contains "made_up_kind"
      When the agent sends "fixer"'s remembered session a message of kind "result"
      Then the tool call succeeds

    # agent_stop is idempotent on a real run: a second stop succeeds and says
    # the run already ended (a stop landing on an ended run still cancels any
    # armed relaunch). A run id that was never spawned is refused, never
    # silently accepted.
    Scenario: Stopping a child is idempotent, and stopping a run that never existed is refused
      # substitution: the journey's Background is the scenario's first Givens
      Given Alice's orchestrator can delegate to two agents, "reviewer" and "fixer", each with its own profile, its own MCP server, and its own permission mode
      And a session owner is standing
      When the agent calls tool "agent_run" with:
        | role         | fixer |
        | input.prompt | go    |
      Then the tool call succeeds
      And "fixer"'s spawned session is remembered
      When the agent calls tool "agent_stop" for "fixer"'s remembered session
      Then the tool call succeeds
      When the agent calls tool "agent_stop" for "fixer"'s remembered session
      Then the tool call succeeds
      And the tool result contains "already ended"
      When the agent calls tool "agent_stop" with:
        | run_id | not-a-real-run-at-all |
      Then the tool call fails
