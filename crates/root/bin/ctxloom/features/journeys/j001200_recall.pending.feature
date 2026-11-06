# Ported from journeys/j001200_recall.feature (rev f36e8dfb2): every scenario, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   j001200_recall.feature with its steps (gherkin_lint.test.rs).

@doc
@pending
Feature: The archaeologist — what did we decide in March?

  Somebody asks why the worktree layout is the way it is. Three people remember
  the conversation. Nobody remembers the reason, and the one person who might
  have written it down has left. The decision was made in a session, in March,
  and the session is still on disk — which is only useful if anyone can find it
  and put it back in front of a model.

  ctxloom captures a great deal. It tees every structured run into a canonical
  transcript it owns, and converts four different vendors' private stores into
  that same format when an interactive run the tee never covered exits, so
  history survives an engine switch and outlives the tool that produced it. That is the expensive half, and it is built.

  This journey is the other half: the payoff. Search finds the session by the
  question its user asked, show prints what it was about, and `run --session`
  folds it back into a live run's context. History that can be captured but
  not recalled is an archive, not a memory — the disk fills up and nothing
  gets easier.

  # NOTE ON SCOPE. J001000 (j001000_transcript_capture.feature) owns CAPTURE.
  # Nothing here re-drives that conversion. This journey owns RECALL.
  #
  # NOTE ON THE TWO MARKERS. A session's title is its first user prompt, read
  # from its transcript; the rest of the conversation is only in the
  # transcript. Each carries its OWN marker, so "found by its title" and
  # "resumed with its conversation" are distinguishable outcomes rather than
  # one assertion either would satisfy.
  #
  # NOTE ON THE FIXTURE'S SECOND SESSION. Every search assertion has a wrong
  # answer available to give: an unrelated session sits beside the March one.
  # A search returning one hit proves nothing if there was nothing else it
  # could have returned.

  Background:
    Given a design question everyone remembers deciding and nobody remembers why

  # A session is titled by the first thing its user asked — the question,
  # which is what an archivist half-remembers. Mutation: dropping the title
  # from cli.sessionMetadataHaystack turns this red.
  Scenario: A phrase from the question she asked finds the session that answered it
    When I run "ctxloom session search J001200-TITLE-WORKTREE-QUESTION"
    Then the search names the March session by a phrase from its first prompt

  # The empty result, which is the one an archivist hits most and the one most
  # likely to be silent. "Nothing matched" and "the search did not run" must
  # not look the same — and a command that answers with zero bytes and exit 0
  # is this codebase's characteristic bug, not a stylistic quibble.
  #
  # UNTAGGED 2026-08-05, confirmed to pass as written AND to bite. Mutation:
  # deleting `ew.Println("(no sessions)")` from cli.renderSessionRows' empty
  # branch turns this red. That is the codebase's characteristic bug staged
  # deliberately — exit 0, zero bytes — and the assertion catches it, which is
  # the whole reason this row asserts on the BYTES and not the exit code.
  Scenario: A query that matches nothing says so rather than printing nothing
    When I run "ctxloom session search J001200-MATCHES-ABSOLUTELY-NOTHING"
    Then ctxloom says plainly that nothing matched

  # Mutation: making renderSessionShow print the placeholder instead of the
  # title turns this red while the command still exits 0.
  Scenario: Show prints what the session was about
    When I run "ctxloom session show amber-quiet-heron --format text"
    Then ctxloom prints the question the session was opened with

  # THE PAYOFF. Everything upstream — the tee, the readers, the canonical
  # schema, four vendors' conversion — exists to make this one line work.
  #
  # UNTAGGED 2026-08-05, confirmed to go red on the described defect and green
  # on the fix. It WAS red exactly as predicted: operations.RecordedSessionEntries
  # resumed by asking the BACKEND's own session reader for entry.SessionID
  # rather than reading the canonical transcript ctxloom itself captured — so
  # ctxloom maintained two archives (its own canonical copy, and the vendor's
  # native store) and resumed from the wrong one. Fixed by making
  # RecordedSessionEntries prefer entry.CanonicalTranscriptPath (populated
  # only when transcript.jsonl exists on disk) via transcript.ParseTranscriptFile,
  # which is now its only source. A dry run reads it through
  # CapturedSessionEntries, which shares that read (capturedEntries) without
  # the refresh. Mutation: disabling the canonical-transcript read turns this
  # red again.
  Scenario: Resuming the March session puts that conversation back in front of a model
    When I run "ctxloom run --session amber-quiet-heron --dry-run -p default"
    Then the assembled context carries the conversation she had in March

  # The payoff row above proves the transcript reaches the ASSEMBLED CONTEXT.
  # This one proves it reaches the MODEL, which is a different claim: a context
  # that assembles correctly and is then dropped on the way to the engine looks
  # identical from the preview. The assertion reads what the engine actually
  # received, not what the CLI reported having sent.
  Scenario: Resuming puts the conversation in front of the model
    Given the mock LLM responds "MOCK-REPLY"
    When I run "ctxloom run --one-shot --session amber-quiet-heron -p default Remind me what we concluded."
    Then the command succeeds
    And the mock recorded input contains "J001200-TRANSCRIPT-ONLY-MARKER"

  # A FILED DEFECT (task diffusive-dazzler), reproduced here at the surface a
  # user actually touches. A harp is three random words; mistyping one is the
  # single most ordinary error available on this command. An unknown harp used
  # to take the process down with a nil dereference — operations.GetSession
  # returns (nil, nil) for an absent harp and RecordedSessionEntries
  # dereferenced entry.SessionID without checking — while the UNBOUND case
  # (a harp that exists with no transcript) degraded correctly. The contract
  # resumeFullContext documents for itself covers both halves; only one was
  # honoured.
  #
  # UNTAGGED: RecordedSessionEntries now returns an error naming the harp and
  # pointing at `session list`, and resumeFullContext's existing warn-and-carry-
  # on path does the rest. Fixed at the shared primitive, so every resume path
  # that had the same hole — the coordinator's ended-child resume included — is
  # covered by the same change.
  Scenario: A mistyped harp warns instead of taking the process down
    When I run "ctxloom run --session no-such-harp-anywhere --dry-run -p default"
    Then ctxloom warns and runs anyway, naming the harp it could not find

  # Recall is a READ. This is the scenario that would catch a resume
  # implementation that consumes, truncates or re-writes the record it
  # read — a memory that erases itself on use, which is worse than no memory
  # because it fails on the SECOND person to ask the question.
  #
  # UNTAGGED 2026-08-05, confirmed to pass as written AND to bite — with a
  # measured blind spot recorded here rather than left to be rediscovered.
  #
  # Mutation that kills it: deleting the canonical transcript at the TOP of
  # operations.capturedEntries (the read a dry run's CapturedSessionEntries
  # and RecordedSessionEntries share), before anything reads it. So the subject
  # genuinely runs and the assertion genuinely reads the bytes on disk; this is
  # not a row satisfied by nothing happening.
  #
  # THE BLIND SPOT, RE-MEASURED 2026-08-05 AND NOW CLOSED: it used to be true
  # that a deletion placed AFTER the (then-only) backend session read
  # succeeded would NOT turn this red, because in this fixture that read FAILS
  # (the mock backend has no `seeded-amber-quiet-heron`) and the resume warned
  # and gave up before reaching any code downstream of it. That was inherited
  # from "resuming the March session" two rows up, which has since been fixed:
  # RecordedSessionEntries now reads entry.CanonicalTranscriptPath (the path
  # that actually runs in this fixture) and nothing else. Re-measured with an
  # os.Remove(entry.CanonicalTranscriptPath) placed immediately after a
  # successful transcript.ParseTranscriptFile — the position a consuming
  # resume would actually occupy now — and it DOES turn this row red. The gap
  # is closed, not merely narrowed: this guard now watches the path every
  # normal resume of a canonical-backed session actually takes.
  Scenario: Recalling a session does not consume it
    When I run "ctxloom run --session amber-quiet-heron --dry-run -p default"
    Then the canonical transcript she recalled from is still on disk, untouched
