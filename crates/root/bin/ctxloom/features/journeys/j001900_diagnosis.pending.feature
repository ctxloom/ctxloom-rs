# Ported from journeys/j001900_diagnosis.feature (rev f36e8dfb2): the scenarios not yet ported, as Go wrote them.
# NOT ported: any of it yet. The feature is @pending: the runner skips it and
#   no World has its steps. Porting a scenario moves it, unchanged, into
#   j001900_diagnosis.feature with its steps (gherkin_lint.test.rs).
#   The ported scenarios are in the files that say `Ported from journeys/j001900_diagnosis.feature`.

@doc
@pending
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

  # ---- B1: authored -> packaged -----------------------------------------
  # Silent-loss mode: the text exists, in no bundle. Inspector per the boundary
  # table: `search` / `bundle show`. Verdict OK — so this scenario is the
  # control that proves the walk's first hop really does answer.
  #
  # UNTAGGED 2026-08-05, confirmed to pass as written AND to bite. Mutation:
  # deleting `fmt.Fprintln(w, "No results found.")` from cli.runUnifiedSearch's
  # zero-result branch turns this red on the "search printed NOTHING at all"
  # assertion — which is the half that matters, since "no such item" and "the
  # search never ran" must not look the same. (A first attempt that suppressed
  # printUnifiedResults' "Results (N):" header did NOT bite: the search here
  # genuinely returns zero results and never reaches that branch.)
  Scenario: Nothing packaged the deploy process, and the search says so
    Given the deploy process exists only as a loose file in Alice's repo, in no bundle
    When I run "ctxloom search deploy"
    Then the search results name no packaged item carrying the deploy process
    And her assistant does not receive the deploy guidance

  # ---- B3: packaged -> distributed --------------------------------------
  # Silent-loss mode: published, never pulled — or, as here, deliberately frozen
  # and then forgotten about. A hold is a decision someone made on purpose; a
  # hold nobody can see is indistinguishable from a broken pull. Inspector:
  # `bundle list`. Verdict OK.
  #
  # WAS RED, and was a finding against B3's "OK" verdict: `ctxloom deps hold`
  # froze the bundle correctly — the payload half always passed — but
  # `ctxloom bundle list` rendered the runbook as an ordinary entry at "(v1.0.0)"
  # and said NOTHING about the hold. A deliberate freeze and a broken pull were
  # the same two lines of output, and the only way to tell them apart was
  # diffing lockfiles by hand.
  #
  # BundleInfo carries Held, stamped from the lockfile entry by
  # operations.stampLockState (the loader reads bundle CONTENT and knows nothing
  # about pins, so the join can only happen there), and the listing renders
  # "[held]" on the name line.
  #
  # The payload half is asserted separately, so a listing that says "held" while
  # actually delivering the new bytes could never pass.
  # Tabled by format: `bundle list` is wired to emit(), so off a terminal
  # (which this harness always is) the no-flag row now gets the JSON
  # BundleInfo array, not the "[held]" name-line suffix the old assertion
  # checked unconditionally. The JSON row selects the runbook's own entry
  # ([name=<bundle>]) and reads its held field directly.
  Scenario Outline: The runbook is frozen at an older version, and the listing names the hold
    Given Carol published the runbook, and Alice's assistant receives its deploy guidance
    And Carol publishes a newer runbook while Alice's copy is held
    And Alice syncs on Monday
    When I run "ctxloom <flags> bundle list"
    Then the installed-bundle listing names the runbook as held
    And her assistant still receives the older deploy guidance and not the newer

    @json-default
    Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
      | flags         |
      |               |
      | --format json |
      | --format text |

  # ---- B5: distributed -> composed --------------------------------------
  # Silent-loss mode: the item is installed and belongs to no
  # profile the agent composes. Inspectors: `profile show`, `agent show`.
  # Verdict OK.
  #
  # UNTAGGED 2026-08-05, confirmed to pass as written — but only after one of
  # its three Thens was STRENGTHENED, because it was tautological.
  #
  # "the agent listing names the profile it composes" asserted that "default"
  # appeared somewhere in `agent show default`'s output. The agent in this
  # fixture is itself NAMED "default", so the assertion was satisfied by the
  # echoed argument: deleting the Profiles bullet list from renderAgentShow
  # outright left it green. The step now reads the rendered "Profiles:" section
  # and the "- default" bullet under it, and that same deletion turns it red.
  #
  # The other two Thens bite as written. `profile show` rendering nothing at
  # all turns the profile-listing Then red on its explicit no-output guard; and
  # composing the runbook into the profile after all — the one product state
  # this scenario says is absent — turns it red on the runbook being named.
  # Same command, two fixture states, two different answers, which is what
  # makes the negative assertion mean something.
  # `agent show`'s Then is tabled by format (it is wired to emit(), so off a
  # terminal — this harness, always — the no-flag row now gets agentShowJSON
  # rather than renderAgentShow's bullet list). `profile show`'s Then stays a
  # plain absence check on whichever format that row asked for: it is a
  # negative claim ("the runbook is named nowhere"), which a substring check
  # states identically for prose or JSON without needing a path.
  Scenario Outline: The runbook is admitted but in no profile, and profile show says so
    Given the runbook is installed and admitted, but composed into no profile
    When I run "ctxloom <flags> profile show default"
    Then the profile listing does not name the runbook among its bundles
    When I run "ctxloom <flags> agent show default"
    Then the agent listing names the profile it composes
    And her assistant does not receive the deploy guidance

    @json-default
    Examples: no --format at all takes the derived default off a terminal; an explicit one wins in both directions
      | flags         |
      |               |
      | --format json |
      | --format text |

  # B5's THIRD nominated inspector, and a finding. The boundary table lists
  # `run --dry-run` as an inspector for "is my content actually composed?" —
  # the question a user reaches for it to answer. The flag exists (`-n`), but
  # its own help says "Show command that would be executed", which is a
  # different question: it inspects the INVOCATION, not the CONTEXT. If it does
  # not render the composed context, then B5 has two working inspectors rather
  # than three, and the boundary table's third entry is aspirational.
  #
  # MEASURED: this one PASSES. `ctxloom run -n -p default` does render the
  # composed context, deploy guidance and all — so B5 genuinely has three
  # working inspectors and FLOWS-UNIFIED's U5 arc is wrong where it lists
  # "`run --dry-run` (composed?) — absent". Correct that document; do not
  # correct this scenario. It is kept precisely because it is the hop that
  # works, and a diagnosis walk with no green hops teaches nothing about which
  # ones are broken.
  #
  # UNTAGGED 2026-08-05, confirmed to pass as written AND to bite. Mutation:
  # making runState.emitDryRun print "(no context)" unconditionally instead of
  # st.ctxResult.Context turns this red — which is precisely the world the
  # scenario's own comment describes, a --dry-run that inspects the INVOCATION
  # and not the CONTEXT.
  Scenario: The dry run shows her the context that would be sent, not just the command line
    Given the runbook is composed into Alice's profile
    When I run "ctxloom run -n -p default"
    Then the dry run shows her the deploy guidance that would be composed

  # ---- B6: composed -> delivered ----------------------------------------
  # Silent-loss mode: hooks not installed, or a materialized surface that has
  # gone stale under content that moved on. Inspectors: `manage check`,
  # `doctor`. Verdict OK in the boundary table — this scenario tests the
  # staleness half specifically, which is the half a user actually hits.
  #
  # WAS RED, and a finding against B6's "OK" verdict: `manage check` reported
  # the project path, whether MCP auto-registration and the statusline are on,
  # one "not configured" line per engine, and the companion binaries it found —
  # never a materialized surface, so it could not report one as stale, fresh,
  # or missing. B6's inspector reported on WIRING, not on DELIVERY.
  #
  # The fixture proves the divergence is real before the inspector ever runs:
  # it asserts the surface on disk holds last week's bytes and NOT this week's.
  # So the red here was a product answer, not a harness artifact.
  #
  # `manage check` judges every backend's native context surface that is
  # materialized under the project root (operations.surfaceCurrencies, printed
  # by the CLI's printSurfaceCurrencies) against the freshly composed context,
  # from the ownership record (operations.contextFileCurrency) — read-only, it
  # never re-writes the surface it inspects. A surface with nothing
  # materialized stays silent; one that no longer carries what is composed is
  # named "stale" in the report's "Materialized surfaces:" section.
  Scenario: The composed context moved on and the engine's file did not, and the wiring report says so
    Given the runbook is composed into Alice's profile
    And the engine's own surface on disk still holds last week's copy
    When I run "ctxloom manage check"
    Then the wiring report names the materialized surface as stale

  # ---- B7: delivered -> ingested ----------------------------------------
  # THE LIMIT, and it is a real one: boundary-table verdict NONE / DEFECT, and
  # both predecessor documents converged on it independently. Every inspector
  # above can be green and the assistant can still be blind, because whether
  # the vendor engine actually READ the file ctxloom handed it happens inside
  # a process ctxloom does not own — a changed surface format, a moved config
  # key, an engine silently ignoring a path, all look identical from here.
  # J000400's live table proves ingestion in CI; it is not a tool a user can run.
  # Nothing ctxloom does will ever cross that boundary.
  #
  # THIS ROW USED TO ASSERT THAT IMPOSSIBLE CAPABILITY, and stayed red on
  # purpose forever, which is exactly the problem: a row that can never pass
  # is dead weight. It inflates every count of remaining work and states an
  # aspiration ctxloom cannot have, rather than a fact about what the product
  # actually does.
  #
  # INVERTED 2026-08-05 (the human's call). Instead of asserting ctxloom can
  # answer a question it structurally cannot, the row now asserts the LIMIT
  # itself: that ctxloom says, plainly, that it does not know — rather than
  # staying silent in a way a reader could mistake for "checked, nothing to
  # report", and never implying the engine consumed what it was handed. That
  # is a testable, mutation-killable fact, and the regression it now guards
  # against is real: "we delivered it" quietly becoming "they read it" is
  # exactly the kind of overclaim this project's docs have been audited for
  # before.
  #
  # WHAT THE PRODUCT ACTUALLY SAID, measured against a fresh build before this
  # change: doctor, manage check, agent show, and session list all said
  # NOTHING about whether the engine read anything — not an overclaim, just a
  # silence a reader could misread as "nothing to report" instead of "this
  # cannot be known". `ctxloom doctor` now carries a new check,
  # DOCTOR-CHECK-INGESTION-q7 (cli.doctorCheckIngestionLimit,
  # internal/adapters/cli/doctor_cmd.go), an "info" line — like an info stated boundary
  # beside it, a stated boundary rather than a probe with a pass/fail outcome
  # — reading "ctxloom writes the assembled context onto <engine>'s own
  # on-disk agent surface; whether <engine> actually reads what was written
  # happens inside a process ctxloom does not own, and nothing in this
  # product can confirm it — verify by asking the engine itself."
  #
  # IT BITES, verified 2026-08-05: replacing that Detail string with the
  # fabrication this row exists to catch — "ctxloom confirms claude-code read
  # the delivered context" — turns the scenario red, because none of the
  # three phrases the Then requires (ctxloom's own "writes", the "does not
  # own" disclaimer, "nothing in this product can confirm it") survive the
  # rewrite. Reverted after confirming red.
  #
  # UNTAGGED 2026-08-05, once doctor stated the limit for real.
  @json-default
  Scenario: Nothing claims the engine read the file, and something says so
    Given every earlier inspector is green and the deploy guidance is materialized into the engine's own surface
    When Alice asks ctxloom what her engine actually ingested
    Then some inspector states plainly that it cannot confirm the engine read what ctxloom delivered
