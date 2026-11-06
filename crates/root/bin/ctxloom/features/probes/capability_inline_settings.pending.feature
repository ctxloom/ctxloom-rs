# Ported from probes/capability_inline_settings.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   capability_inline_settings.feature with its steps (gherkin_lint.test.rs).

@live @probe-p17-inline-settings
@pending
Feature: P17 — an inline JSON --settings applies alongside the user settings in claude's config home

  A child's settings come from two places on the same turn: ctxloom's hooks
  live in the session home, claude's user source, and the turn's posture —
  permission rules and defaultMode — arrives as an inline JSON document on
  --settings. If the inline document replaced the user settings, a child
  given a posture would lose ctxloom's hooks; if claude ignored inline JSON,
  the posture would never reach the child.

    just capability-probe p17-inline-settings probes/capability_inline_settings.feature claude-code host none

  HOW THE CELL WORKS. The vendor binary is run directly in a throwaway
  CLAUDE_CONFIG_DIR and HOME, with a fresh git repo as cwd and
  --setting-sources user, as an untrusted child runs. The config home's
  settings.json registers a SessionStart and a PreToolUse hook that write
  markers outside the repo, and allows one echo; the inline --settings JSON
  allows a different echo. The turn runs both echoes. Green means both
  echoes ran and both markers exist.

  Self-skips LOUDLY when claude is absent or no credential is exported.
  One paid haiku turn.
  Scenario Outline: A <engine> turn with config-home user settings and an inline --settings
    Given the inline-settings probe targets "<engine>" under runtime "<runtime>" and workspace "<workspace>"
    When it runs one turn with user settings in the config home and an inline JSON --settings
    Then both settings sources apply to the turn

    @claude-code @host @ws-none
    Examples:
      | engine      | runtime | workspace |
      | claude-code | host    | none      |
