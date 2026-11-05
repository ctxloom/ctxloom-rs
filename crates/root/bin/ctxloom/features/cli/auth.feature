# Ported from Go ctxloom's tests/acceptance/features/cli/auth.feature
# (revision f36e8dfb21, release/0.7), all three scenarios. Rust has two engines
# with an environment credential (claude-code's token, opencode's optional
# endpoint key); a claude login shared in place has no row, as in Go.
@doc
Feature: auth — how ctxloom-launched engines authenticate

  Covers: `ctxloom auth status`.

  Every agent ctxloom spawns authenticates with ONE credential: a token the
  human mints with the engine's own flow (claude: `claude setup-token`) and
  exports — or keeps in a secret manager that exports it. Only the human's
  own session may share their login instead (the top-level `auth: login`).
  ctxloom never collects, stores or mints a credential: it reads the token
  from the environment it is launched in. `auth status` reports, per engine,
  whether that credential is present, never its value.

  Everything here runs against the scenario's own temporary home and
  environment, so the credential exported is a fixture string and nothing
  real is read.

  Rule: status reports whether each credential is in the environment, never its value

    # THE SECRET MUST APPEAR IN NO OUTPUT: a status that echoed the value it
    # found would leak the credential into logs and transcripts, silently.
    # Rows are per engine and mode, so the claim is read off claude-code's own
    # row: other engines report their own credentials (opencode's is an
    # endpoint API key). A second claude-code row (an api-key mode) would make
    # the selector ambiguous and fail it, which keeps claude token-only.
    Scenario: An exported token is reported present without revealing it
      Given an initialized ctxloom project
      And the environment variable "CLAUDE_CODE_OAUTH_TOKEN" is set to "sk-ant-oat01-acceptance-fixture-not-real"
      And the environment variable "ANTHROPIC_API_KEY" is set to ""
      When I run "ctxloom auth status --format json"
      Then the command succeeds
      And the output does not contain "sk-ant-oat01-acceptance-fixture-not-real"
      And the output reports "[engine=claude-code].mode" as "token"
      And the output reports "[engine=claude-code].present" as "true"

    Scenario: With nothing exported, status names how to mint and export the token
      Given an initialized ctxloom project
      And the environment variable "CLAUDE_CODE_OAUTH_TOKEN" is set to ""
      And the environment variable "ANTHROPIC_API_KEY" is set to ""
      When I run "ctxloom auth status --format text"
      Then the command succeeds
      And the output contains "claude-code token: missing — run `claude setup-token` and export CLAUDE_CODE_OAUTH_TOKEN"

  Rule: ctxloom takes no credential

    # Anthropic does not allow a third party to collect, store or intermediate
    # Claude.ai credentials, so there is no command that hands ctxloom one,
    # and nothing is ever written under the home for one.
    Scenario: There is no command that stores a credential
      Given an initialized ctxloom project
      When I run "ctxloom auth set --engine claude-code --mode token" with input:
        """
        sk-ant-oat01-acceptance-fixture-not-real
        """
      Then the command fails
      And the output does not contain "sk-ant-oat01-acceptance-fixture-not-real"
      And exactly 0 home files match ".ctxloom/auth/*"
