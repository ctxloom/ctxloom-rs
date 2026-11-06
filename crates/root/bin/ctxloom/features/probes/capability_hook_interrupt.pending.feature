# Ported from probes/capability_hook_interrupt.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   capability_hook_interrupt.feature with its steps (gherkin_lint.test.rs).

@live @probe-p15-hook-interrupt
@pending
Feature: P15 — an interrupted claude turn takes the PermissionRequest hook it was waiting on with it

  ctxloom interrupts a child's turn by sending claude SIGINT and killing it
  after a grace, and treats the interrupted turn as an ordinary boundary: the
  turn's approval slots are cancelled and the run lives. That is sound only
  if the approval hook claude was waiting on dies with the turn. An orphaned
  hook goes on holding a request the runner already cancelled.

    just capability-probe p15-hook-interrupt probes/capability_hook_interrupt.feature claude-code host none

  HOW THE CELL WORKS. The vendor binary is run directly, the way the driver
  runs it — stream-json in and out, the prompt on stdin, its own process
  group — in a throwaway CLAUDE_CONFIG_DIR and HOME with a fresh git repo as
  cwd. A --settings file outside the repo registers one PermissionRequest hook
  on Bash that records its pid and then holds far longer than the cell waits.
  Once the hook is blocked the cell sends SIGINT, gives claude the driver's
  grace, and reads the hook's process from /proc. Whether claude wrote a
  result frame on its way out is recorded on the evidence line.

  Self-skips LOUDLY when claude is absent or no credential is exported.
  One paid haiku turn.
  Scenario Outline: A <engine> turn interrupted while its hook blocks
    Given the hook-interrupt probe targets "<engine>" under runtime "<runtime>" and workspace "<workspace>"
    When it interrupts the engine while its PermissionRequest hook is blocked
    Then the blocked hook dies with the engine

    @claude-code @host @ws-none
    Examples:
      | engine      | runtime | workspace |
      | claude-code | host    | none      |
