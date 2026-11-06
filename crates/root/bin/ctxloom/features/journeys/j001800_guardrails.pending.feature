# Ported from journeys/j001800_guardrails.feature (rev f36e8dfb2): the scenarios not yet ported, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   j001800_guardrails.feature with its steps (gherkin_lint.test.rs).
#   The ported scenarios are in the files that say `Ported from journeys/j001800_guardrails.feature`.

@doc
@pending
Feature: Guardrails — when the assistant does not listen

  Every other journey in this suite proves DELIVERY: the right context reaches
  the right assistant, current. None of that answers a
  different question — what happens when the assistant, mid-session, does not
  listen? An LLM agent just handed a fragment saying "use the task runner"
  will still, sometimes, reach for the raw tool out of habit. One just told to
  check for an existing helper before writing a new one will still, sometimes,
  write a fourth copy. That is not a hypothetical edge case — it is the single
  most characteristic failure mode of an LLM coding agent, and a fragment can
  only ever ASK for the opposite.

  This is why ctxloom ships companions: ltk and reprise, standalone binaries
  that do mechanically what a fragment can only request. ctxloom's own job
  stops at delivery — getting each companion's guidance into the assembled
  context, and its hook wired into the assistant's real configuration, as
  honestly as the companion itself describes what it does. What each
  companion then actually DOES once installed — ltk's redirect, reprise's
  pre-commit duplicate check — is each companion's own mechanism, proven by
  its own test suite. This journey proves ctxloom's half: that the loadout
  reaches the assistant whether or not either binary happens to be on the
  machine, honestly described either way.

  # NOTE ON SCOPE: this journey does not prove a companion's redirect or check
  # actually FIRES mid-session — that would require driving a live coding
  # agent through a real PreToolUse hook, or a real `git commit` through
  # lefthook, neither of which this suite's subprocess-over-MCP-stdio harness
  # can observe from outside ctxloom's own process. What IS observable, and
  # had zero acceptance coverage before this file: whether the companion's own
  # guidance and hook wiring actually reach the assembled context and the
  # generated engine settings, and whether what reaches it is honest about
  # what the companion does.

  # LOCKED — delivery, for TWO independent companions at once: the
  # registered-companion loadout probe (internal/adapters/companions'
  # Prober.ProbeCompanionLoadouts) reaching both the assembled
  # CLAUDE.md AND the generated .claude/settings.json — the hook wiring, not
  # just prose. ltk's fragment content here is its REAL committed loadout
  # (cmd/ltk/loadout.yaml), read at test time, never a hand-typed stand-in.
  Scenario: Alice's assistant receives the task-runner and reuse-before-you-write guidance
    Given Alice's project exists
    And ltk and reprise are installed on Alice's machine
    When Alice starts a session
    Then her assistant receives ltk's task-runner guidance
    And her assistant receives reprise's reuse-before-you-write guidance
    And her assistant's engine is configured to run ltk on every shell command and file edit

  # LOCKED — the overclaim guardrail. ltk's OWN delivered fragment must say,
  # in so many words, that it is a cooperative redirect and NOT a sandbox —
  # what it actually returns is a message and a suggestion, never a hard
  # block or a security boundary. ctxloom's delivery must not launder that
  # into something stronger than the companion itself claims; this is the
  # exact overclaim a prior website-truthfulness audit already caught this
  # project making elsewhere.
  Scenario: ltk's own guidance is honest about what it does — a redirect, not a block
    Given Alice's project exists
    And ltk and reprise are installed on Alice's machine
    When Alice starts a session
    Then her assistant is told ltk is a cooperative redirect, not a sandbox
