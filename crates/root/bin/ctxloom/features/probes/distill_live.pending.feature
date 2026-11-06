# Ported from probes/distill_live.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   distill_live.feature with its steps (gherkin_lint.test.rs).

@live
@pending
Feature: Live distillation with a real agent
  Distillation delegates compression to a real backend. The assertions are
  loose about wording but strict about behavior: the output must be a genuine
  compression (not a passthrough, not a stub) and must keep the load-bearing
  facts. The same scenarios run against every supported agent via the Examples
  table; each row skips when no credentials for that agent are present.

  Scenario Outline: Distilling a fragment compresses it while preserving the key fact
    Given a real <agent> agent is available
    And a bundle "lore" with a long fragment "rules"
    When I run "ctxloom fragment distill lore#fragments/rules -f"
    Then the command succeeds
    And the distilled fragment "rules" in bundle "lore" is a real compression
    And the distilled fragment "rules" in bundle "lore" preserves the domain

    Examples:
      | agent  |
      | Claude |

    # @wip — the provider, not ctxloom: a distillation is tool-less now
    # (marauding-magnolia, decision #5), so it launches on opencode, and
    # opencode sends the model no tool at all. The free hosted model these
    # rows run on (liveOpencodeModel, opencode/big-pickle) refuses exactly
    # that request. Measured 2026-10-13 (opencode 1.18.35): 403 FreeTierError,
    # "OpenCode's free tier can only be used from within OpenCode", recorded
    # in the run's own opencode store. Untag when these rows run on an
    # opencode label whose provider serves a tool-less request.
    @opencode @wip
    Examples:
      | agent    |
      | Opencode |

  Scenario Outline: Distilling a prompt records a compressed rendering
    Given a real <agent> agent is available
    And a bundle "lore" with a long fragment "rules"
    When I run "ctxloom command distill lore#commands/guidance -f"
    Then the command succeeds
    And the bundle "lore" records a distillation

    Examples:
      | agent  |
      | Claude |

    # @wip — the provider, not ctxloom: a distillation is tool-less now
    # (marauding-magnolia, decision #5), so it launches on opencode, and
    # opencode sends the model no tool at all. The free hosted model these
    # rows run on (liveOpencodeModel, opencode/big-pickle) refuses exactly
    # that request. Measured 2026-10-13 (opencode 1.18.35): 403 FreeTierError,
    # "OpenCode's free tier can only be used from within OpenCode", recorded
    # in the run's own opencode store. Untag when these rows run on an
    # opencode label whose provider serves a tool-less request.
    @opencode @wip
    Examples:
      | agent    |
      | Opencode |

  Scenario Outline: Distilling a whole bundle distills its fragment
    Given a real <agent> agent is available
    And a bundle "lore" with a long fragment "rules"
    When I run "ctxloom bundle distill .ctxloom/content/bundles/v2/lore/bundle.yaml -f"
    Then the command succeeds
    And the distilled fragment "rules" in bundle "lore" is a real compression
    And the distilled fragment "rules" in bundle "lore" preserves the domain

    Examples:
      | agent  |
      | Claude |

    # @wip — the provider, not ctxloom: a distillation is tool-less now
    # (marauding-magnolia, decision #5), so it launches on opencode, and
    # opencode sends the model no tool at all. The free hosted model these
    # rows run on (liveOpencodeModel, opencode/big-pickle) refuses exactly
    # that request. Measured 2026-10-13 (opencode 1.18.35): 403 FreeTierError,
    # "OpenCode's free tier can only be used from within OpenCode", recorded
    # in the run's own opencode store. Untag when these rows run on an
    # opencode label whose provider serves a tool-less request.
    @opencode @wip
    Examples:
      | agent    |
      | Opencode |
