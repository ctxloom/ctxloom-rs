# Ported from cli/fragment_draft_premise.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   fragment_draft_premise.feature with its steps (gherkin_lint.test.rs).

@pending
Feature: fragment draft-premise — a drafted and critiqued premise is a proposal, never a write

  A premise decides whether a fragment is ever loaded, so authoring one is
  never autonomous: the model drafts, a second independent pass attacks the
  draft, and only a human at a terminal accepts, edits or rejects it. This
  harness is never a terminal, so every scenario here is the PROPOSE path —
  the one scripts and agents reach — and the claim each makes is that the
  proposal arrives whole and that nothing is written.

  The mock answers every turn with the same reply, so the reply carries both
  the drafter's premise and the critic's findings; each pass reads only its
  own keys.

  Background:
    Given an initialized ctxloom project
    And a bundle "demo" exists
    And a fragment "worktrees" in bundle "demo" exists
    And the mock LLM responds "{premise: You are about to delete a worktree, moments: [removing a worktree], findings: [{kind: too-broad, evidence: fires on every worktree command}]}"

  Scenario: Off a terminal the draft and its critique are shown and nothing is written
    When I run "ctxloom fragment draft-premise demo#fragments/worktrees --llm mock --format text"
    Then the command succeeds
    And the output contains "Draft premise: You are about to delete a worktree"
    And the output contains "[too-broad]"
    And the output contains "Nothing written"
    And the mock recorded input contains "FRAGMENT-BODY-worktrees"
    When I run "ctxloom fragment premises --format text"
    Then the output does not contain "demo#fragments/worktrees"

  Scenario: A structured caller gets the whole proposal, marked as only proposed
    When I run "ctxloom fragment draft-premise demo#fragments/worktrees --llm mock --format json"
    Then the command succeeds
    And the output is valid JSON
    And the output reports "decision" as "proposed"
    And the output reports "draft.premise" as "You are about to delete a worktree"
    And the output reports "critique.findings.0.kind" as "too-broad"
