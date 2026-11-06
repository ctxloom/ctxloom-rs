# world: surfaces
# Ported from journeys/j001900_diagnosis.feature
# NOT ported: every scenario but the last ("His assistant has the guidance and
#   hers does not, and something compares the two"): the other boundaries
#   (search, listing holds, profile show, dry-run context, wiring staleness,
#   the engine-ingestion statement) are not part of this slice.
# NOT ported: the step "Alice asks ctxloom to compare her delivered context
#   with Bob's" probes three spellings in Go, any of which may answer. Only
#   `ctxloom materialize default --diff FILE` exists; the other two,
#   `profile show default --compare FILE` and `doctor --compare FILE`, do not
#   and are not built (owner ruling 2026-11-05: port `--diff FILE` as is).
@doc
Feature: The day the assistant goes blind

  Monday morning. "The assistant knew our deploy process on Friday. It doesn't
  today." And the worst part of the report is the second half: it is still
  reaching HER assistant and not his. Nothing errored. Nothing is red. Content
  simply stopped arriving, and the person who has to find out why has a row of
  hops to search and no idea which one dropped it.

  This journey is a binary search over the delivery pipeline, and the product's
  bar is stated as a rule: EVERY stage boundary either names its inspector, or
  it is a defect. Content travels authored -> packaged -> distributed ->
  composed -> delivered -> ingested. One scenario per
  boundary. Each plants the cause at exactly that hop and then asks the
  inspector that owns it to say so out loud. A boundary whose inspector cannot
  name the cause is not a missing test — it is the reason a Monday like this one
  takes a day instead of a minute.

  NOTHING in the product can tell a user whether the engine ever read the file
  it was handed. That fact has not changed and never will — whether a vendor engine reads a
  file happens inside a process ctxloom does not own. What changed 2026-08-05
  is that ctxloom now SAYS so, plainly, instead of leaving that question
  unanswered in a way a reader could mistake for "checked and confirmed"; the
  scenario at the end of this journey asserts the stated limit, not the
  impossible capability. It is still the one this whole journey ends on,
  because it is the question every real diagnosis session ends on too.

  # NOTE ON ASSERTIONS. No INSPECTOR's exit code is asserted here. An inspector
  # that exits 0 while naming nothing is precisely the failure under test, so
  # "the command succeeds" would assert the bug. Every Then reads a payload:
  # the bytes of the assembled context, or the inspector's own words naming a
  # specific bundle, fragment, or file.
  #
  # NOTE ON TAGS. A scenario still @wip carries its own untag condition. Every
  # scenario in this file STARTED @wip, including the ones believed to pass, because this file is a
  # to-do list to be walked one at a time and a scenario that arrived green
  # would be indistinguishable from one nobody had looked at. Untagging is
  # therefore the record that somebody looked — see the UNTAGGED notes below,
  # which name the mutation each one was confirmed to bite on.

  Background:
    Given Alice's team ships its deploy process as ctxloom content

  # ---- M5: the two-machine symptom --------------------------------------
  # Where the journey OPENED: "it is reaching her assistant and not his." That
  # is a comparison, and every diagnostic ctxloom has is single-machine. The
  # user has the answer sitting in front of her in two files and no way to ask
  # the tool which one differs and where. Recorded as miss M5 in
  # FLOWS-UNIFIED §4 finding class (b).
  #
  # The fixture makes the divergence genuinely two-sided: Bob's delivered
  # context is materialized and captured OUTSIDE the project before Alice's
  # profile is changed, so this is two real deliveries being compared, not one
  # file read twice.
  #
  # UNTAGGED 2026-08-16: `ctxloom materialize <profile>... --diff
  # <file>` is the surface. Of the three spellings this scenario probes
  # (`profile show --compare`, `doctor --compare`, `ctxloom materialize
  # --diff`), materialize already owns assembling a profile's delivered
  # context (the same operations.AssembleContext call Materialize makes
  # for the context surface) — comparing that assembled payload against an
  # already-delivered file elsewhere is a read-only extension of exactly that,
  # not a new responsibility bolted onto `show` (declared config, not
  # delivered payload) or `doctor` (system-health checks with no profile
  # argument). --diff makes --target optional and prints/returns a unified
  # diff (difflib) between the two
  # delivered contexts, naming every line present on one side and not the
  # other.
  Scenario: His assistant has the guidance and hers does not, and something compares the two
    Given Bob's checkout of the same project delivers the deploy guidance and Alice's does not
    When Alice asks ctxloom to compare her delivered context with Bob's
    Then ctxloom reports the deploy guidance as present for Bob and absent for her
