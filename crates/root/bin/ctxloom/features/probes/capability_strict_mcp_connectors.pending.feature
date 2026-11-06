# Ported from probes/capability_strict_mcp_connectors.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   capability_strict_mcp_connectors.feature with its steps (gherkin_lint.test.rs).

@live @probe-p16-strict-mcp-connectors
@pending
Feature: P16 — --strict-mcp-config keeps claude.ai's connectors out of a claude -p session

  An untrusted repository's child is launched with --setting-sources user
  --strict-mcp-config, so the only MCP servers it reaches are the ones
  ctxloom passes on --mcp-config. claude.ai's connectors are fetched for the
  account rather than read from a settings file, so --setting-sources does
  not remove them. If --strict-mcp-config does not either, every child
  reaches the account's connectors whatever posture ctxloom chose.

    just capability-probe p16-strict-mcp-connectors probes/capability_strict_mcp_connectors.feature claude-code host none

  HOW THE CELL WORKS. The vendor binary is run directly in a throwaway
  CLAUDE_CONFIG_DIR and HOME with a fresh git repo as cwd, twice: once
  without the flag (the control, which must list at least one claude.ai
  connector, or there was nothing to suppress) and once with it (which must
  list none). Both are read from the init frame's mcp_servers.

  Self-skips LOUDLY when claude is absent or no credential is exported.
  Two paid one-word haiku turns.
  Scenario Outline: claude.ai connectors in a <engine> session with and without --strict-mcp-config
    Given the strict-mcp-connectors probe targets "<engine>" under runtime "<runtime>" and workspace "<workspace>"
    When it starts one turn without and one turn with --strict-mcp-config
    Then the claude.ai connectors load only without the flag

    @claude-code @host @ws-none
    Examples:
      | engine      | runtime | workspace |
      | claude-code | host    | none      |
