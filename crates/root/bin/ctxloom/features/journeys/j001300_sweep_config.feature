# world: base
# NOT ported: nothing is ported (no Go source): every scenario here is new
#   (see its `# new:`) and needs human review. They cover the sweep's
#   purge-age rule, which Go's journey states in `session sweep --help`
#   ("--purge-older-than ... overriding session_purge_age; without either
#   nothing is purged") and in Go's purgeCutoff: the flag beats the
#   configured age, and the configured age stands where no flag is given.
Feature: closeout — the sweep reads session_purge_age

  Background:
    # new: Rust's own.
    Given an initialized ctxloom project
    And the project already has the file ".ctxloom/config.yaml":
      """
      schema_version: 8
      session_purge_age: 90d
      """
    And a finished session "amber-quiet-heron" whose transcript retention has passed
    And every session has been idle for 120 days

  Scenario: Without the flag the configured purge age is the sweep's
    # new: no Go scenario states the purge-age rule (see the header).
    When I run "ctxloom session sweep --yes"
    Then the command succeeds
    And the machine-written bulk of "amber-quiet-heron" is gone

  Scenario: The flag beats the configured purge age
    # new: no Go scenario states the purge-age rule (see the header).
    When I run "ctxloom session sweep --purge-older-than 200d --yes"
    Then the command succeeds
    And the machine-written bulk of "amber-quiet-heron" is still there

  Scenario: A configured purge age that is no age refuses the sweep before it touches anything
    # new: no Go scenario states the purge-age rule (see the header).
    Given the project already has the file ".ctxloom/config.yaml":
      """
      schema_version: 8
      session_purge_age: soon
      """
    When I run "ctxloom session sweep --yes"
    Then the command fails
    And the machine-written bulk of "amber-quiet-heron" is still there
