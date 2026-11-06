# Ported from cli/content_distill.feature (rev f36e8dfb2)
# NOT ported: any of it yet (the Feature is @pending).

@pending
Feature: Distilling authored content — does the compression happen, is it kept, and is a bad one refused?

  Distillation is how an author keeps a bundle's guidance token-efficient: the
  raw item stays the source of truth and a compressed rendering is stored
  beside it for assembly to use. Three leaves do it — `fragment distill`,
  `command distill`, and `bundle distill` (every distillable item at once).

  A failed distillation — a backend that dies, an empty answer, an
  implausibly short answer — FAILS the command: it warns why, keeps whatever
  was stored before, and exits non-zero, so an exit status of success never
  sits over content nobody distilled. The exit status alone still cannot tell
  a refused answer that kept the good one from a failure that stamped
  something wrong, so every scenario below reads the stored PAYLOAD as well.

  These scenarios are the hermetic counterpart to distill_live.feature, which
  runs the same three leaves against real vendor engines and asserts what only
  a real engine can settle: that the output is a genuine compression in a
  plausible band and keeps the domain vocabulary. Those are judgments about an
  engine's OUTPUT. What is asserted here is the WIRING those judgments
  presuppose and that no live run isolates — the item's real content reaching
  the distiller, and the distiller's real answer being what gets stored. A
  canned answer is the right instrument for that and the wrong one for the
  other, so "is this a good summary" is deliberately not asked here.

  Both halves are asserted every time, because either alone can pass while the
  feature is broken: reading only the stored output would be satisfied by a
  distiller handed an empty item, and reading only what the distiller received
  would be satisfied by a run whose answer was thrown away.

  Background:
    Given an initialized ctxloom project
    And the mock LLM responds "DISTILLED-BY-THE-CONFIGURED-LLM: retries, tokens, tenants, caches, webhooks, vaults"
    And a bundle "lore" with a long fragment "rules"

  # The fixture fragment's content carries "dead-letter queue" and nothing else
  # in the project does, so finding it in what the distiller received proves
  # the REAL fragment was sent — not an empty request, not a stale one, not the
  # item's name alone.
  Scenario: Distilling a fragment sends its content and stores the answer
    When I run "ctxloom fragment distill lore#fragments/rules -f"
    Then the command succeeds
    And the mock recorded input contains "dead-letter queue"
    And the distilled fragment "rules" in bundle "lore" is the distiller's answer

  # Same contract, different item kind. Commands distill through the same seam
  # but carry their own content, so a regression that dropped command bodies
  # while still distilling fragments would show up here and nowhere else.
  Scenario: Distilling a command sends its content and stores the answer
    When I run "ctxloom command distill lore#commands/guidance -f"
    Then the command succeeds
    And the mock recorded input contains "Operational guidance for reviewers"
    And the distilled command "guidance" in bundle "lore" is the distiller's answer

  # `bundle distill` is the whole-manifest form: one invocation must reach
  # EVERY distillable item, not just the first one it finds. Asserting the
  # fragment AND the command is the point — a loop that stopped after its first
  # item would still satisfy either assertion alone.
  Scenario: Distilling a bundle stores an answer for every item in it
    When I run "ctxloom bundle distill .ctxloom/content/bundles/v2/lore/bundle.yaml -f"
    Then the command succeeds
    And the distilled fragment "rules" in bundle "lore" is the distiller's answer
    And the distilled command "guidance" in bundle "lore" is the distiller's answer

  # A model that returns a stub or a truncated answer must not be allowed to
  # REPLACE a good distillation with it — assembly serves whatever is stored,
  # so accepting the stub silently degrades every future session that reads
  # this bundle. The floor is proportional (under 2% of the source) rather than
  # absolute, because "too short" only means anything relative to what was sent.
  #
  # The scenario is written as a REPLACEMENT rather than a first distillation
  # on purpose: the claim is not merely that a stub is refused, but that
  # refusing it leaves the previous good answer intact. A guard that rejected
  # the stub and cleared the field would pass a first-distillation version of
  # this and still lose the author's content.
  Scenario: An implausibly short answer is refused, and the good distillation survives
    When I run "ctxloom fragment distill lore#fragments/rules -f"
    Then the command succeeds
    And the distilled fragment "rules" in bundle "lore" is the distiller's answer
    Given the mock LLM responds "tiny"
    When I run "ctxloom fragment distill lore#fragments/rules -f"
    Then the command fails
    # State before message, deliberately: godog stops a scenario at its first
    # failing step, so putting the warning check first would let it mask the
    # assertion that actually matters — a guard that warned and overwrote
    # anyway would fail on the message and never reach the payload.
    And the distilled fragment "rules" in bundle "lore" still contains "DISTILLED-BY-THE-CONFIGURED-LLM"
    And the output contains "rejecting as truncated"

  # The backend-failure path, which is the one an author actually hits (a
  # missing engine binary, an expired credential). Two claims, and the second
  # is the one that matters: the command fails, AND the item is left
  # RAW rather than stamped with a failed or partial result. A run that
  # wrote something anyway would be indistinguishable from success to every
  # downstream reader, whatever its exit status said.
  Scenario: When the distiller cannot run, the item is left raw rather than stamped
    Given the distillation backend cannot start
    When I run "ctxloom fragment distill lore#fragments/rules -f"
    Then the command fails
    And the fragment "rules" in bundle "lore" has no distilled rendering
    # The failure names the engine's exit either way the race lands: the turn
    # frame answers with the engine's own error ("the engine process exited 1"),
    # or the runner's exit wins and the run's terminal names it ("exit status 1").
    And the output contains "failed: agent run:"
    And the output contains "exit"
