# world: surfaces
# Ported from cli/agent.feature
# NOT ported: every other scenario of the Go file. Ported: show saying where
#   a headless run's asks go.
@doc
Feature: agent — the bindings that decide what runs, on what context, and where

  Rule: show says where a headless run's asks go, before anything is launched

    A delegated child or a one-shot has nobody at its engine, so a call its
    posture leaves open goes to its approver. The human — the default — is
    asked in the approvals modal of the interactive run at the root, and only
    there: a root that cannot present one denies the call at once. Finding that
    out from a BLOCKED report is too late; `agent show` says it up front.

    Scenario: A binding that declares no approver asks the human, and says what happens when nobody can answer
      Given an initialized ctxloom project
      And a profile "dev" exists
      And I run "ctxloom agent create developer --llm claude-code --profiles dev"
      When I run "ctxloom agent show developer --format text"
      Then the command succeeds
      And the output contains "Headless posture: acceptEdits, approver human — asks go to the root's approvals modal"
      And the output contains "denied at once when the root is unattended (one-shot, --plain-terminal, headless)"
      When I run "ctxloom agent show developer --format json"
      Then the command succeeds
      And the output reports "resolved.approver" as "human"
      And the output reports "resolved.effectivePermissions" as "acceptEdits"
