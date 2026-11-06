# Ported from cli/context_status.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   context_status.feature with its steps (gherkin_lint.test.rs).

@pending
Feature: Measuring context-window occupancy
  An agent that suspects it is running out of room should measure instead of
  guessing. context_status turns "this conversation feels long" into a number:
  the latest recorded sample of this session's context occupancy, plus a short
  trend so the direction is visible.

  Both scenarios run in the default hermetic lane — no engine is involved. The
  samples come from ctxloom's own statusline capture, so the fixture writes
  them through the SAME writer the statusline uses (contextmetrics.Append),
  which is what keeps this suite honest about the on-disk format: a fixture
  that hand-rolled the JSONL could keep passing after the writer changed shape.

  # The failure this scenario forbids is the house defect in its most
  # dangerous form. A session with no samples has UNKNOWN occupancy, and the
  # tempting answer — 0% used — is the exact opposite of the truth an agent
  # would act on: it reads as "plenty of room". So the assertions are on the
  # payload's own words, and on the ABSENCE of a latest sample. An envelope
  # with available=false but a zero-valued sample beside it would pass a
  # bare-flag check and mislead any agent that read the number.
  Scenario: context_status reports an absent measurement rather than a zero one
    Given an initialized ctxloom project
    And the session harp is "swift-amber-falcon"
    When the agent calls tool "context_status"
    Then the tool call succeeds
    And the tool result field "available" equals "false"
    And the tool result contains "no samples yet"
    And the tool result contains "statusline integration not active"
    And the tool result contains "UNKNOWN"
    And the tool result contains "not the same as low"
    And the tool result carries no context measurement

  # The trend is asserted as an ORDERED list, not as a count. A count is
  # satisfied by the right number of samples in the wrong order — and the
  # newest sample is read off the END of that list, so an inverted series
  # would report the session's OLDEST occupancy as its current one, which is
  # the reading most likely to be believed and least likely to be checked.
  #
  # TWELVE samples against a default trend of ten, deliberately. A shorter
  # series never reaches the truncation branch at all, so the scenario would
  # pass just as happily against a trend that kept the OLDEST ten — the
  # measured stale reading this whole tool exists to prevent. Seeding past the
  # cap is what makes "which end gets kept" load-bearing here.
  # The caller the endpoint identifies is the STANDING owner, so the series
  # is seeded under its harp after it stands.
  Scenario: context_status measures a session that has recorded samples
    Given an initialized ctxloom project
    And a session owner is standing
    And the session has recorded context samples:
      | percent | tokens | window  |
      | 30      | 300000 | 1000000 |
      | 31      | 310000 | 1000000 |
      | 32      | 320000 | 1000000 |
      | 33      | 330000 | 1000000 |
      | 34      | 340000 | 1000000 |
      | 35      | 350000 | 1000000 |
      | 36      | 360000 | 1000000 |
      | 37      | 370000 | 1000000 |
      | 38      | 380000 | 1000000 |
      | 39      | 390000 | 1000000 |
      | 40      | 400000 | 1000000 |
      | 42      | 420000 | 1000000 |
    When the agent calls tool "context_status"
    Then the tool call succeeds
    And the tool result field "available" equals "true"
    And the latest context sample reads 42% used, 420000 of 1000000 tokens
    And the context trend reads "32,33,34,35,36,37,38,39,40,42" oldest first
    And the tool result contains "42% used"

  # A second session's series must never be able to answer for this one. The
  # tool takes no session argument at all, so the only thing that can go wrong
  # here is the identity it resolves — and reporting a stranger's occupancy as
  # your own is worse than reporting nothing, because it is actionable.
  Scenario: context_status answers for the calling session, not another one
    Given an initialized ctxloom project
    And the session harp is "bold-crimson-thunder"
    And the session "swift-amber-falcon" has recorded context samples:
      | percent | tokens | window  |
      | 91      | 910000 | 1000000 |
    When the agent calls tool "context_status"
    Then the tool call succeeds
    And the tool result field "available" equals "false"
    And the tool result carries no context measurement
    And the tool result names no samples from "swift-amber-falcon"
