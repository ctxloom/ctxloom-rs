# world: surfaces
# Ported from cli/session_hooks.feature
# NOT ported: these Go scenarios, and why:
#   - session-bind, hud and the retired turn-changed: S4's, tested in
#     hook.test.rs.
#   - stamp-plan: taskloom's (D17).


Feature: The machine callbacks every session fires — do they deliver, and do they stay on their channel?

  Four hidden commands run on every ctxloom session, invoked by the host
  engine rather than by a person: `hook session-start` hands the engine the
  session-start notices (never the project's context, which the session
  already has as its system prompt),
  `hook session-bind` records
  which backend session belongs to which harp, `hook stamp-plan` marks a plan
  file with the session that edited it, and `hook hud` renders the statusline.
  Nobody types any of them, so nobody notices when one stops working.

  They share a contract that makes them unusually easy to break quietly. A
  hook must NEVER fail the host engine's startup, so every one of them warns
  and exits 0 on any failure it can foresee — an unparseable payload, an
  absent harp. Exit code therefore says nothing at
  all here, and a hook that delivers nothing is indistinguishable from a hook
  that delivered, unless something reads what it actually produced.

  The second half of the contract is the CHANNEL: stdout is the engine's
  input, and a diagnostic printed there is not a warning the user sees but
  corruption of the payload the engine parses. So the stdout assertions below
  are made by parsing stdout ALONE as JSON, never the combined stream — a
  test reading combined output would pass happily while ctxloom fed an engine
  a warning line spliced into its context envelope.

  These were invisible to the coverage gate itself until recently: registered
  Hidden, they failed the walk that discovers leaves, so nothing could even
  report them as uncovered. They are listed explicitly as required leaves now,
  and this feature is what answers that requirement.

  Background:
    Given an initialized ctxloom project

  # The project's context reaches a session ONCE, as its system prompt. The
  # SessionStart hook delivering it too is the duplication the hook was cut
  # down to remove, so the claim here is an ABSENCE — and an absence is only
  # worth asserting where the thing could be found: the project's assembled
  # context is on disk, in the cache an older hook read, and the marker text
  # exists nowhere else. The envelope must still parse (stdout is the
  # engine's input), and nothing in it — context or notice — may carry the
  Scenario: The SessionStart hook never hands the engine the project's context
    Given the project already has the file ".ctxloom/cache/context/abc123.md":
      """
      HOOK-MUST-NOT-DELIVER-THIS-EXACT-TEXT and nothing else in this project says so.
      """
    When I run "ctxloom hook session-start --engine claude-code" with input:
      """
      {"session_id":"vendor-session-1","hook_event_name":"SessionStart","source":"startup"}
      """
    Then the command succeeds
    And the hook's additionalContext is empty
    And the output does not contain "HOOK-MUST-NOT-DELIVER-THIS-EXACT-TEXT"
