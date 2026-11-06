# Ported from journeys/j002000_engine_switch.feature (rev f36e8dfb2): the scenarios not yet ported, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   j002000_engine_switch.feature with its steps (gherkin_lint.test.rs).
#   The ported scenarios are in the files that say `Ported from journeys/j002000_engine_switch.feature`.

@doc
@pending
Feature: Engine switch day

  A pricing change, a policy change, a vendor doing something nobody liked —
  and the team is moving to a different engine. This is the day ctxloom's whole
  premise gets tested, because the premise is that your context is yours: the
  fragments, the profiles, the trust decisions and the history belong to the
  team, not to whichever assistant happened to be fashionable when they were
  written.

  On paper it is one command. `agent edit dev --llm <new>` swaps the engine
  under the binding; profiles are engine-neutral by construction, so the same
  content composes for the new engine's own native surfaces; the canonical
  transcript keeps the history readable; and reaching for a session the tee
  never covered still rescues whatever the abandoned vendor's private store
  holds.

  In practice Alice discovers the pieces exist and the FLOW does not. No
  migration story narrates the order, nothing verifies the result, and the one
  thing she most wants to know — what does the new engine NOT do that the old
  one did — has no surface at all. Nothing here is structural for the
  mainstream engines, which is exactly the finding: the portability story is
  sellable and untold.

  # NOTE ON SCOPE. J000400 (j000400_multi_engine.feature) is the axis's REFERENCE
  # feature — one profile, many engines, a matrix row per engine — and it is
  # deliberately not a journey. This file does not turn it into one. What it
  # asserts is what a matrix cannot express: that the swap is a SEQUENCE with a
  # before and an after, and that the context surviving it is the SAME context
  # rather than merely a valid one.
  #
  # NOTE ON THE CENTRAL ASSERTION. The comparison is between two real
  # materializations, captured on either side of the swap, compared as bytes.
  # A migration that delivers different-but-valid content is worse than one
  # that fails, because the team's assistant quietly knows something different
  # on Monday and nobody is told what.
  #
  # NOTE ON TAGS. Each scenario carries its own note: an untagged one records
  # the mutation that proves it can still fail, a @wip one the product surface
  # that does not exist yet.

  Background:
    Given Alice's team has standardised on one engine and is moving to another

  # THE MISSING FLOW, stated as the question a migrating team actually asks.
  # Not "did it work" — they can see that — but "what did I just give up?"
  # The concrete, verified loss on this exact engine pair: the new engine
  # declares no hook surface at all. A team moving onto it loses a hook event
  # they may well have built on, and no surface anywhere mentions it, before
  # or after the switch. They will find out when something stops happening.
  #
  # This is the same shape as U3's missing per-engine capability-loss report,
  # arriving through a different door: there, a new hire silently inherits less
  # than his deskmate; here, a whole team silently loses something it had.
  #
  # UNTAGGED (trusting-ambiguity): all three probed surfaces now report it —
  # `doctor` (DOCTOR-CHECK-CAPABILITY-LOSS-u1), `manage check` (its "Capability
  # loss" section) and `agent show <name>`. The Then step requires EVERY one of
  # them, not any one: while only `agent show` was wired, the any-of form left
  # this row green over a gap that was half present, and a user who ran
  # `doctor` still learned nothing.
  #
  # The mechanism behind all three — operations.CapabilityLoss wraps the SAME
  # backends.UncarriedSurfaces read `ctxloom materialize` already performed
  # (whiny-exclusive), against the resolved agent's ACTUAL backend and
  # profiles rather than a materialize target, read once per configured agent.
  # A whole-mechanism noHooksReason alone cannot catch a PER-EVENT gap, which
  # is why agentDescriptor also carries unsupportedHookKinds — the SAME string
  # addUnifiedHooks' route uses to warn at write time — and UncarriedSurfaces
  # now checks it too.
  Scenario: Nothing tells her what the team just gave up
    When Alice swaps the engine under the binding
    And Alice asks ctxloom what the switch changed and what to verify
    Then ctxloom names what the new engine cannot do that the old one could
