# Ported from probes/remote_content_live.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   remote_content_live.feature with its steps (gherkin_lint.test.rs).

@live
@pending
Feature: Remote content reaches a real agent
  The hermetic @remote scenarios (remote_content.feature) prove the mechanics
  of fetching and serving changed content, all the way up to the assembled
  context. This is the one link those scenarios cannot cover: that the
  assembled context is actually what a real backend receives and acts on, not
  just what ctxloom thinks it sent. Self-skips without credentials, same as
  the other @live scenarios.

  Scenario Outline: A real <agent> agent echoes an injected fragment's content
    Given a real <agent> agent is available
    And a bundle "handshake" with a fragment "secret" containing "The codeword for this session is PONYTAIL-7734. If asked for the codeword, respond with only PONYTAIL-7734 and no other text."
    And a profile "dev" with bundle "handshake"
    When I run "ctxloom run --one-shot --profile dev What is the codeword? Reply with only the codeword and nothing else."
    Then the command succeeds
    And the output contains "PONYTAIL-7734"

    Examples:
      | agent    |
      | Claude   |
      | Opencode |
