# Ported from journeys/j002100_delegation.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   j002100_delegation.feature with its steps (gherkin_lint.test.rs).

@doc
@pending
Feature: Coordinator delegates isolated work

  A coordinator that fans work out to delegated children is only as safe as
  the boundary between them. If a child agent Alice spawns to fix one thing
  can quietly reach the MCP server she gave to a DIFFERENT child — or the one
  she reserved for herself — then "delegation" is just a shared blast radius
  wearing a different name. ctxloom's answer is that each child's privileges
  are its OWN, resolved from its OWN configured profiles, never a sibling's or
  the coordinator's — and that grant is not just true in memory, it is
  journaled at the moment the child is enqueued, so an operator auditing
  later sees exactly what a live run was actually given, not what today's
  config now says.

  # NOTE ON WHAT THIS JOURNEY CAN AND CANNOT SEE: tests/acceptance drives a
  # `ctxloom` SUBPROCESS over MCP stdio — it can only observe what crosses the
  # wire or lands on disk, never a captured in-process Go struct. The
  # coordinator is hosted by the session owner this journey stands up (a
  # mock-engine `ctxloom run`; see session_owner_fixture.go), and the
  # harness's `ctxloom mcp` shim FORWARDS to that owner's runner — the same
  # runner-terminated surface a real engine's shim reaches. Every assertion
  # below still reads the coordinator's own durable journal, runs.jsonl,
  # directly off disk: the SAME data roster is backed by (consumer.go's
  # listRunsSnapshot reads this exact fold), and a genuinely external,
  # disk-durable observable — not a Go struct capture, and not faked.
  #
  # For the same reason, three things a NAIVE version of this journey might
  # have tried to assert are deliberately NOT here: a child's assembled
  # CONTEXT contents (the mock backend never echoes fragments back, and no
  # tool here returns a child's raw transcript), child WORKSPACE/filesystem
  # isolation (the mock engine never touches its WorkDir), and artifact
  # publish/fetch/tamper-refusal (agent_report/agent_fetch_artifact are
  # registered only on a spawned session's OWN per-cell runner socket, never
  # reachable from an external MCP client like this one). All three need a
  # different harness than this one to prove honestly; faking them here would
  # be exactly the overclaim this project has already been caught making once.

  Background:
    Given Alice's coordinator can delegate to two agents, "reviewer" and "fixer", each with its own profile, its own MCP server, and its own permission mode
    And a session owner is standing

  # LOCKED — the sharpest claim this journey makes, and the reason it exists.
  # Verified: prodSpawner.childMCPServers (spawner.go) composes a child's MCP
  # set strictly from ITS OWN resolved profiles; runEnqueued.MCPServers
  # (facts.go) journals exactly that set, names only. Break-point verified by
  # unioning all resolved profiles in childMCPServers: the scenario goes RED
  # because "reviewer"'s journaled grant then contains "deploy-tool".
  Scenario: Each child is granted only its own MCP servers, never a sibling's or the coordinator's
    When the agent calls tool "agent_run" with:
      | role         | reviewer |
      | input.prompt | go       |
    Then the tool call succeeds
    When the agent calls tool "agent_run" with:
      | role         | fixer |
      | input.prompt | go    |
    Then the tool call succeeds
    And "reviewer"'s journaled grant carries its own MCP server and not "fixer"'s
    And "fixer"'s journaled grant carries its own MCP server and not "reviewer"'s

  # LOCKED — the grant is auditable, not implicit: an operator reading the
  # journal sees the ACTUAL resolved permission mode a child ran with, not a
  # guess. The two children here are given genuinely different modes
  # (reviewer: plan; fixer: bypass) precisely so this cannot be satisfied by
  # one hard-coded value showing up twice.
  Scenario: Each child's permission mode is recorded, not just implied
    When the agent calls tool "agent_run" with:
      | role         | reviewer |
      | input.prompt | go       |
    Then the tool call succeeds
    When the agent calls tool "agent_run" with:
      | role         | fixer |
      | input.prompt | go    |
    Then the tool call succeeds
    And "reviewer"'s journaled grant records its own permission mode
    And "fixer"'s journaled grant records its own permission mode, different from "reviewer"'s

  # LOCKED — DURABILITY. Verified: runEnqueued.Permission/MCPServers are
  # written once, at enqueue (children.go's enqueueRun), the same journaling
  # discipline applied wherever a grant is journaled, for the identical
  # reason: a later config edit must not retroactively rewrite what a LIVE
  # run was actually granted. This is not a tautology about append-only
  # files — the second spawn below proves the edit genuinely took effect
  # (fixer's NEXT run reflects it), while the run already on record does not
  # move.
  Scenario: A child's grant is journaled at enqueue and survives a later config edit unchanged
    When the agent calls tool "agent_run" with:
      | role         | fixer |
      | input.prompt | go    |
    Then the tool call succeeds
    And "fixer"'s current journaled grant is remembered as "before the edit"
    When Alice edits "fixer" to use "reviewer"'s profile and permission mode instead
    And the agent calls tool "agent_run" with:
      | role         | fixer    |
      | input.prompt | go again |
    Then the tool call succeeds
    And "fixer"'s current journaled grant is remembered as "after the edit"
    And the grant remembered as "before the edit" still shows the original MCP server and permission mode
    And the grant remembered as "after the edit" shows the newly edited MCP server and permission mode

  # LOCKED — names, never secrets, deliberately. Verified: runEnqueued.MCPServers
  # (facts.go) carries mcp.ChatMCPServer NAMES only — never command, args, or
  # env — the same boundary CredHash already draws for the bearer token
  # (identity recorded, never the credential). Both fixture bundles here ship
  # an MCP command carrying a plausible secret-shaped argument specifically so
  # this scenario has something real to prove was never written.
  Scenario: The audit trail carries a server's name, never the command that can carry a secret
    When the agent calls tool "agent_run" with:
      | role         | reviewer |
      | input.prompt | go       |
    Then the tool call succeeds
    When the agent calls tool "agent_run" with:
      | role         | fixer |
      | input.prompt | go    |
    Then the tool call succeeds
    And the journal carries both children's MCP server names
    But the journal never carries the command or arguments that launch them

  # FAILURE PATH — every scenario above is happy-path (spawn, succeed, read
  # the journal back). agent_stop is one of this flow's own coordinator
  # verbs and had NO acceptance coverage at all before this scenario
  # (completeness_test.go's census was the only place the string appeared).
  # U024-F04 (this batch) fixed a real bug on exactly this shutdown edge: a
  # stop landing on an already-ended run must still cancel any armed
  # relaunch, not just report "already ended" as a no-op — the 2026-07-24
  # incident shape. This is the tip-to-tail proof that the STOP verb
  # behaves like every other verb in this flow: idempotent on a real run
  # (never errors on a second stop), and refused — never silently
  # accepted — on a run id that was never spawned at all.
  Scenario: Stopping a child is idempotent, and stopping a run that never existed is refused
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

  # FAILURE PATH — a message's `kind` is a security boundary, not a label.
  # user_injected is COORDINATOR-RESERVED and reads to a recipient as the
  # human speaking; steer and exited are the coordinator's own notices. Until
  # this scenario existed, `kind` was whatever the sender said it was: a
  # delegated child could queue a reserved kind into its coordinator's mailbox
  # and borrow that authority, complete with a plausible body.
  #
  # WHAT THIS HARNESS CAN AND CANNOT SEE (same limit as the note at the top):
  # it drives the COORDINATOR's own agent_send, never a child's — no tool here
  # reaches a spawned child's runner socket. The refusal is identity-
  # independent by construction (one ingress guard for both sender surfaces),
  # and the child-identity case is pinned in the unit suite
  # (TestAgentSend_RefusesSpoofedUserInjection). What this proves at the
  # real MCP surface is that the vocabulary is CLOSED: a RESERVED kind is
  # refused by the coordinator's ingress, which tells the sender what it may
  # use instead; a spelling outside the vocabulary altogether never reaches
  # the coordinator — the wire's closed enum refuses it at the tool schema,
  # naming the rejected value. And, in the same scenario, that it is not a
  # blanket refusal: a documented kind still goes through.
  Scenario: A sender cannot claim a coordinator-reserved message kind
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

  # An ask does not hold the asker: agent_ask returns an ask_id at once, and
  # the answer is whatever the child itself later sends back quoting that id,
  # reaching the asker as mail that starts its next turn. A delivered inbox
  # file is deleted, so delivery is read from the coordinator's audit of the
  # write (which records the in_reply_to) joined to the owner's delivered
  # record.
  #
  # WHAT THIS HARNESS CAN AND CANNOT SEE: a mock-engine child has no tools, so
  # it cannot choose to answer. Its runner's automatic turn report DOES quote
  # the ask id, and is deliberately not the answer. So the harness stands in
  # for the child's deliberate reply at the one place a child's agent_send
  # lands — a file in the child's OWN out/ spool, whose sender identity is the
  # directory, never the file — and the coordinator routes it from there
  # exactly as it routes any child send. The answer's own words, not the
  # correlation alone, select it from the turn report that shares its id.
  Scenario: An ask returns at once, and the child's answer arrives later as mail quoting it
    When the agent calls tool "agent_run" with:
      | role         | fixer |
      | input.prompt | go    |
    Then the tool call succeeds
    And "fixer"'s spawned session is remembered
    When the agent calls tool "agent_ask" for "fixer"'s remembered session with the question "which migration path?"
    Then the tool call succeeds
    And the tool result carries the ask's id
    When "fixer" answers that ask from its own outbox with "the reversible one"
    Then the coordinator's reader delivers "fixer"'s answer "the reversible one", quoting the ask's id

  # DURABLE HOLDS — a child parked on its rate limit, and a child the human
  # paused, are the coordinator's own state: the session's coordinator dying
  # mid-hold must not free either. The owner's `ctxloom run` (which hosts the
  # coordinator) is KILLED, not shut down, and the session is resumed with
  # `run --session`. A HOST child's runner dies with its host (the runner is
  # armed with a parent-death signal so a crash never orphans one), so the
  # children's runs end at the restart — and the holds stay on the CHILDREN:
  # neither is relaunched before its hold lets it. What a user sees is read
  # where they would see it: the roster's hold on each child, with the same
  # deadline, and the hold releasing itself at the engine's reset time, not
  # earlier and not never — while the human's pause, with no deadline, stays.
  @durable-holds
  Scenario: A held child's hold and a paused child's pause survive the session's coordinator restart
    When the agent calls tool "agent_run" for "reviewer" with a briefing that hits a rate limit resetting in 75 seconds
    Then the tool call succeeds
    And "reviewer"'s spawned session is remembered
    When the agent calls tool "agent_run" with:
      | role            | fixer |
      | input.prompt    | go    |
      | input.workspace | none  |
    Then the tool call succeeds
    And "fixer"'s spawned session is remembered
    And the roster comes to show "reviewer" held with kind "rate_limited"
    And "reviewer"'s hold deadline is remembered
    When the human pauses "fixer" from the overlay
    Then the roster comes to show "fixer" held with kind "human"
    When the session's coordinator dies and the session is resumed
    Then the roster comes to show "reviewer" held with kind "rate_limited"
    And "reviewer"'s hold deadline is unchanged
    And the roster shows "fixer" held with kind "human"
    And the roster comes to show "reviewer" released on time
    And the journal records "reviewer"'s hold released by its own backoff
    And the roster shows "fixer" held with kind "human"

  # A REFUSED CREDENTIAL has no deadline: nothing about it changes on its own,
  # so its hold outlives a restart until the human acts. (The mock engine
  # declares no auth, so its child carries no credential a restart could
  # re-resolve and compare: the hold stays, as it does when the human restarts
  # from an environment with the same refused credential.)
  @durable-holds
  Scenario: A child whose credential is refused stays held across the session's coordinator restart
    When the agent calls tool "agent_run" for "reviewer" with a briefing whose credential is refused
    Then the tool call succeeds
    And "reviewer"'s spawned session is remembered
    And the roster comes to show "reviewer" held with kind "credential_rejected"
    When the session's coordinator dies and the session is resumed
    Then the roster comes to show "reviewer" held with kind "credential_rejected"

  # REMOVED with the orchestrator-routed escalation ladder (2026-08-31). The
  # scenario's own Given configured a ladder ("whose escalation ladder relays to
  # a parent that never answers"), so its subject no longer exists.
  #
  # WHAT IT PROTECTED, recorded because the property outlived the mechanism:
  # ctxloom must never file a refusal under the OPERATOR'S name. The engine used
  # to be handed a reject_once option, which the engine adapter reported to the model
  # as {behavior:"deny", message:"User refused permission to run tool"} -- a
  # refusal the operator never made and cannot correct.
  #
  # The ruling that replaced the ladder goes FURTHER than this scenario did:
  # ctxloom must NEVER answer a permission request at all. The first-party LLM client
  # handles them natively, and a human reaches that client's own prompt in the
  # terminal the run is on. Nothing here should be reintroduced to re-assert a
  # verdict on the operator's behalf.
