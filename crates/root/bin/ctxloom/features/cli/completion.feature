Feature: completion — the always-scriptable utility command with no noun of its own
  Smoke coverage: the completion script must generate for a supported shell
  and actually contain a completion directive, not just exit zero.

  Scenario: Generate a shell completion script
    Given an initialized ctxloom project
    When I run "ctxloom completion bash"
    Then the command succeeds
    And the output contains "complete"
