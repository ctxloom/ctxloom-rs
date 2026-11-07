# Ported from journeys/j002100_delegation.feature (rev f36e8dfb2)
# NOT ported: the scenarios tagged @pending: they wait for the coordinator.
#   The reserved-kind refusal and agent_stop's idempotence are
#   cli/coordination_contract.feature's.

@doc
@pending
Feature: Orchestrator delegates isolated work

  Alice runs a session that hands work out to other agents: a "reviewer" to
  read the change and a "fixer" to repair what the reviewer finds. Each has
  its own profile, its own MCP server and its own permissions. She trusts the
  arrangement only if each child gets what she gave it and nothing more: the
  fixer must not reach the reviewer's tools, and neither may reach the ones
  she kept for herself.

  So Alice watches. She sees what each child was granted, she asks a child a
  question and gets its answer later, she pauses a child, and when a child is
  waiting on a rate limit she sees when it will go again. If her session dies
  and she resumes it, every hold is where she left it. When she changes an
  agent's setup, the record of what an earlier run was granted does not
  change with it.

  Background:
    # substitution: the session owner is Alice's own session
    Given Alice's orchestrator can delegate to two agents, "reviewer" and "fixer", each with its own profile, its own MCP server, and its own permission mode
    And Alice's session "amber-quiet-heron" is running

  # Alice's orchestrator hands out both pieces of work. In the record of each
  # run, Alice sees the MCP servers and permissions that child was given: its
  # own, never its sibling's or hers, and the servers by name, never the
  # commands that launch them and the secrets those can carry.
  Scenario: Each child is granted only its own MCP servers, never a sibling's or the orchestrator's
    # new: told as the user's flow.
    When Alice's orchestrator delegates the review to "reviewer" and the fix to "fixer"
    Then Alice sees "reviewer" was granted its own MCP server and permissions, and not "fixer"'s
    And Alice sees "fixer" was granted its own MCP server and permissions, and not "reviewer"'s
    And the record names each child's MCP servers, and never the commands that launch them

  # The orchestrator asks the fixer which way to go and carries on at once.
  # The fixer's answer arrives later, as mail that starts the orchestrator's
  # next turn, saying which question it answers.
  Scenario: An ask returns at once, and the child's answer arrives later as mail quoting it
    # substitution: said as Alice's orchestrator asking, and the answer arriving
    Given Alice's orchestrator has delegated the fix to "fixer"
    When Alice's orchestrator asks "fixer" "which migration path?"
    Then the orchestrator is not kept waiting for the answer
    When "fixer" answers "the reversible one"
    Then the orchestrator's next turn starts with "fixer"'s answer, quoting the question

  # The reviewer hits its rate limit and is held until the engine's reset;
  # Alice pauses the fixer herself. Her session dies and she resumes it.
  # Alice's view of her children shows both holds as they were: the reviewer
  # goes again when its limit resets, not before and not never, and the fixer
  # stays paused until she says otherwise.
  @durable-holds
  Scenario: A held child's hold and a paused child's pause survive the session's coordinator restart
    # substitution: said as what Alice sees of her children
    Given Alice's orchestrator has delegated the review to "reviewer", which hits a rate limit resetting in 75 seconds
    And Alice's orchestrator has delegated the fix to "fixer", in session "brisk-copper-moth"
    And Alice sees "reviewer" held on its rate limit, until its reset
    When Alice pauses "fixer":
      """
      ctxloom session pause brisk-copper-moth
      """
    Then Alice sees "fixer" paused by her
    When Alice's session dies and she resumes it:
      """
      ctxloom run --session amber-quiet-heron
      """
    Then Alice sees "reviewer" still held on its rate limit, until the same reset
    And Alice sees "fixer" still paused by her
    And Alice sees "reviewer" go again when its limit resets
    And Alice sees "fixer" still paused by her

  # A child whose credential was refused waits for Alice: nothing about it
  # changes on its own, so it is still held after she resumes her session.
  @durable-holds
  Scenario: A child whose credential is refused stays held across the session's coordinator restart
    # substitution: said as what Alice sees of her children
    Given Alice's orchestrator has delegated the review to "reviewer", whose credential is refused
    And Alice sees "reviewer" held on its refused credential
    When Alice's session dies and she resumes it:
      """
      ctxloom run --session amber-quiet-heron
      """
    Then Alice sees "reviewer" still held on its refused credential

  # Alice gives the fixer the reviewer's profile and permissions. The fixer's
  # next run gets the new setup; the record of its earlier run still shows
  # what that run was actually given.
  Scenario: A child's grant is journaled at enqueue and survives a later config edit unchanged
    # substitution: said as what Alice sees of each run's grant
    Given Alice's orchestrator has delegated the fix to "fixer"
    When Alice gives "fixer" the reviewer's profile and permissions:
      """
      ctxloom agent edit fixer --add-profile reviewing --permissions plan
      """
    And Alice's orchestrator delegates the next fix to "fixer"
    Then Alice sees "fixer"'s first run was granted its original MCP server and permissions
    And Alice sees "fixer"'s second run was granted the reviewer's MCP server and permissions

  # When the work is done, Alice stops the fixer, and sees it has ended.
  Scenario: Alice stops a child she no longer needs
    # new: the operator's stop
    Given Alice's orchestrator has delegated the fix to "fixer", in session "brisk-copper-moth"
    When Alice stops "fixer":
      """
      ctxloom session stop brisk-copper-moth
      """
    Then Alice sees "fixer" ended
