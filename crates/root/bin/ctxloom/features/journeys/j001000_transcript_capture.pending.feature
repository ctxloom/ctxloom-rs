# Ported from journeys/j001000_transcript_capture.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   j001000_transcript_capture.feature with its steps (gherkin_lint.test.rs).

@doc
@pending
Feature: Cross-engine transcript capture — every engine's native log becomes one transcript you own

  You drive whichever engine you like,
  through its own native TUI, and you still keep a durable memory of what
  happened. Each engine writes its conversation into its OWN private,
  version-unstable session store (claude's project JSONL, for one). This
  journey
  is the promise on top of that mess: whatever engine ran, its native log
  becomes ONE canonical transcript ctxloom owns — the same on-disk schema
  (transcript.jsonl, one JSONL Record per line) a live structured/ACP session
  already tees out — so a single downstream reader (resume, search, the VSCode
  companion) never special-cases three vendor formats.

  AND NOBODY RUNS AN IMPORT. The conversion fires when an interactive run
  exits, when `session adopt --yes` records an older session's lineage, and
  before ctxloom shows or resumes a session (`run --session`, `session show`,
  the title of a `session list`/`search` row that has none yet). A session
  whose run did not exit through ctxloom (killed, crashed, in a container,
  its engine run outside ctxloom) is read as its engine's log has it now, not
  as its last capture left it. There is no import chore to remember.

  COMMAND SURFACES THIS FILE COVERS: under @live, an interactive
  `ctxloom run`.

  WHY IT MATTERS: without this seam, an engine driven through its own pty is a
  black hole — the structured tee (transcript.Tee/TeeAndClose) never reaches a
  pty, so ctxloom has zero memory of that work. WHAT BREAKS WITHOUT IT: every
  interactive session is unresumable — exactly the interactive-pty gap ADR
  0035 accepted and docs/transcript-schema.md's "Capture regimes" now closes.

  # NO HERMETIC DOOR. No hermetic step binds a session to an engine's own
  # log, so the conversion is pinned by unit tests (internal/adapters/operations
  # vendor-reader and refresh-on-read tests, the cli exit seam's and its
  # session show/list/search and run --session readers'), and end to end only
  # under @live.

  Background:
    Given an initialized ctxloom project

  # @live — the ON-EXIT trigger is only real with a real engine driven through
  # its own pty. The conversion fires solely for an interactive execution mode
  # with a registered backend; the mock never reaches it. This row self-skips
  # when no credentials for <agent> are present (the same gate every @live
  # scenario opens with), so it is a documented, non-faked deferral; the
  # conversion itself is pinned hermetically in internal/adapters/operations
  # (the vendor-reader tests) and internal/adapters/cli (the exit seam).
  @live
  Scenario Outline: An interactive engine run's own log is captured when it exits
    Given a real <agent> agent is available
    When I run the <agent> agent interactively to completion with prompt "leave a mark"
    Then on exit the canonical transcript for that session holds its real conversation turns
    And every line of that canonical transcript is stamped engine "<engine>"

    Examples:
      | agent       | engine      |
      | Claude      | claude-code |
