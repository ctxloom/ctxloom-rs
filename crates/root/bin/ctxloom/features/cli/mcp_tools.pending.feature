# Ported from cli/mcp_tools.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   mcp_tools.feature with its steps (gherkin_lint.test.rs).

@pending
Feature: MCP tools
  The agent drives ctxloom through tools. These exercise the context and search
  tools end to end, and the host-relayed task tool, evaluate_triggers.

  Every tool here can answer successfully while delivering nothing, so each
  scenario names a marker that exists ONLY in its own fixture, and asserts
  that marker in the result. An envelope with the right fields is not
  evidence.

  # `fragments_loaded` has no omitempty, so the KEY is present even when the
  # value is null: asserting it proved only that the envelope had a field,
  # which assembly delivering zero fragments and an empty context satisfies
  # perfectly. The assertions name the fragment that must have been resolved
  # and the marker from inside its stored body, so an empty assembly cannot
  # pass. The `contains "fragments_loaded"` line those two replaced is gone:
  # it could not fail independently of them, and a line that reads as an
  # assertion while being incapable of failing is worse than no line. (An
  # exact-list assertion on the field was considered and rejected — the
  # always-on builtin companion fragments make its contents depend on what the
  # machine has installed, which is how a green-for-the-wrong-reason scenario
  # is born.)
  #
  # The tool is served by the SESSION's endpoint over the package its launch
  # carried: a fragment is asked for by its qualified ref (what the
  # ctxloom://fragments catalog lists), and a profile or tag ask is refused
  # by name — profile resolution is the originator's, made when the launch
  # was resolved, not something a running session re-does against a catalog
  # it does not hold. Both halves are pinned, or a session that quietly
  # re-resolved profiles would pass the first.
  Scenario: Assemble context from a fragment the session's package carries
    Given an initialized ctxloom project
    And a bundle "demo" exists
    And a fragment "testing" in bundle "demo" exists
    And a profile "dev" with bundle "demo"
    And a session owner is standing
    When the agent reads resource "ctxloom://fragments"
    Then the resource contains "ctxloom+local:session-owner#fragments/session-owner"
    When the agent calls tool "assemble_context" with:
      | bundles[] | ctxloom+local:session-owner#fragments/session-owner |
    Then the tool call succeeds
    And the tool result contains "ctxloom+local:session-owner#fragments/session-owner"
    And the tool result contains "the session owner's own context"
    When the agent calls tool "assemble_context" with:
      | profile | dev |
    Then the tool call fails
    And the tool failure message contains "cannot be resolved inside a running session"

  # SearchContentResult.Query has no omitempty, so the QUERY is echoed into
  # the envelope whether or not the search matched anything: `the tool result
  # contains "testing"` was satisfied by {"results":null,"count":0,
  # "query":"testing"} — a search that found nothing. count is the
  # load-bearing half (SearchResult carries Type/Name/Source but no body), so
  # this asserts it, exactly as the sibling search_library scenario below does.
  Scenario: Search content over MCP
    Given an initialized ctxloom project
    And a bundle "demo" exists
    And a fragment "testing" in bundle "demo" exists
    When the agent calls tool "search_content" with:
      | query | testing |
    Then the tool call succeeds
    And the tool result field "count" equals "1"
    And the tool result contains "demo"

  # "Degrades gracefully" means it RAN and found nothing, not that it exited 0.
  # A handler that never called SearchRemotes at all — and so never surfaced an
  # error either — satisfied the bare exit-code assertion. The decoded result
  # fields can only be there if a real SearchRemotesResult came back.
  Scenario: Search the installable library degrades gracefully with no remotes
    Given an initialized ctxloom project
    When the agent calls tool "search_library" with:
      | query | anything |
    Then the tool call succeeds
    And the tool result field "query" equals "anything"
    And the tool result field "count" equals "0"

  # evaluate_triggers asks a model whether each Deferred task's revive
  # condition has fired, with the repository's recent commits as evidence. Its
  # result is mostly COUNTERS — evaluated, cache_hits, cache_misses, omitted —
  # and a run that produced no usable verdict fills those in exactly as
  # convincingly as one that worked. The tool's own `omitted` field exists
  # because a model chunk can come back well-formed while silently not
  # mentioning a task, so the claim here is the verdict's ATTRIBUTION, not the
  # count.
  #
  # Single-round only, deliberately: a "needs-investigation" answer carries
  # follow-up queries that ctxloom runs before asking again, and a canned
  # response would hand back the same array in round two — testing an
  # escalation loop against a stub that cannot escalate. That path needs a
  # fixture that can answer differently the second time.
  Scenario: A deferred task's trigger verdict reaches the caller, attributed to it
    Given an initialized ctxloom project
    And the mock LLM responds "placeholder, replaced once the task's harp exists"
    And a fresh taskloom store
    And a deferred task "revisit the cache eviction policy" waiting on "the v2 API ships"
    And the trigger model answers "fired" for that task
    When the agent calls tool "evaluate_triggers"
    Then the tool call succeeds
    And the tool result field "evaluated" equals "1"
    And the verdict is attributed to that task, with outcome "fired"
    And the tool result contains "TRIGGER-VERDICT-REACHED-THE-CALLER"

  # The empty case, which must NOT reach the model at all: with nothing
  # deferred there is nothing to judge, and a run that called the LLM anyway
  # would burn a request per invocation on a backlog that asks no questions.
  # evaluated=0 is the observable for that.
  Scenario: With nothing deferred, evaluate_triggers judges nothing
    Given an initialized ctxloom project
    And a fresh taskloom store
    When the agent calls tool "evaluate_triggers"
    Then the tool call succeeds
    And the tool result field "evaluated" equals "0"

  # The approval hook (`ctxloom hook permission`) is the engine's half of the
  # approval route: it posts the engine's ask to the runner's hook path and
  # writes back only a decision the runner returned. A session that routes
  # no approvals (the standing owner is interactive: its human answers in the
  # engine's own UI) refuses, and the hook says so on stderr while writing
  # NOTHING to stdout — no decision, which an engine nobody sits at denies.
  # The refusal's text can only have come back over the wire, under the
  # endpoint's bearer.
  Scenario: The approval hook relays a runner that routes no approvals as no decision
    Given an initialized ctxloom project
    And a session owner is standing
    And the approval hook reaches the session owner's endpoint
    When I run "ctxloom hook permission --event PermissionRequest" with input:
      """
      {"hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"ls"}}
      """
    Then the command succeeds
    And the hook writes nothing to stdout
    And the output contains "routes no approvals"
