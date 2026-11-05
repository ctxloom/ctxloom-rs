# Ported from Go ctxloom's tests/acceptance/features/journeys/j002000_engine_switch.feature
# (revision f36e8dfb21, release/0.7), ONE scenario, the second of the parity POC.
# NOT ported: the other three scenarios. "The engine changes under the binding
# and the context does not" and "Nothing tells her what the team just gave up"
# need the mock-lossy engine, which this port does not register (the new
# engine here is `mock`); "History recorded under the old engine survives the
# move" needs the session-index seeding of J001200. The Background is the
# Go text unchanged; its step is Rust's.
@doc
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

  # FOLDED (Phase 3, D-row not previously numbered): "manage install --engine
  # <unknown>" used to have its own scenario here too, but cli/manage.feature's
  # "A misspelled engine is refused by name, and nothing is scaffolded" already
  # covers the identical claim with a STRONGER assertion — it checks that
  # .ctxloom/config.yaml and .mcp.json were never written, not just that the
  # error message named the engines. Nothing here was worth moving down; the
  # duplicate scenario is deleted and the noun's own spec is where that leaf's
  # coverage lives now. What remains below is the validation gap
  # cli/manage.feature does NOT cover: the same mistake one layer up, at the
  # `agent edit --llm` binding.
  #
  # The same validation question one layer up, at the binding. The two commands
  # take the same kind of argument on the same day and are asserted separately,
  # because they are separate code paths and a fix to one says nothing about
  # the other. cli/agent.feature explicitly defers this leaf's `--llm`
  # validation coverage here rather than duplicating it (see its own "create
  # and edit are NOT an upsert" Rule comment).
  #
  # MEASURED: RED, and worse than the command above. `ctxloom agent edit dev
  # --engine bogus-engine` exits 0 and WRITES the nonexistent engine into the
  # binding. Nothing validates the name at the moment it becomes the team's
  # configuration, so the failure surfaces later, somewhere else, as whatever a
  # missing engine happens to look like downstream.
  #
  # UNTAGGED: `agent edit --llm` now validates its argument and names the
  # engines it knows. The check lives in operations.SetAgent — `agent create`
  # and `agent edit`'s single shared body, so both verbs are covered — and
  # rejects before the write, so a typo'd edit cannot corrupt a live binding.
  # Membership is operations.AvailableLLMNames (registered backends UNION the
  # config's declared labels), the same set `llm default` accepts and offers,
  # because an agent's engine is a LABEL and `--engine claude-fast` is one of
  # this command's own documented examples.
  Scenario: Binding an agent to an engine that does not exist is refused
    When Alice types an engine name that does not exist while swapping the binding:
      """
      ctxloom agent edit dev --llm bogus-engine
      """
    Then ctxloom refuses and names the engines it knows
