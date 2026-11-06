# Testing every output in JSON and English, and one behaviour through several mechanisms

Status: **recommendation** (2026-11-06), from research into established BDD practice (sources at
the end) and the current harness (`gherkin_common.test.rs`, `gherkin_features.test.rs`).

## The principle
Scenarios state domain facts. A driver layer decides how each fact is exercised and checked, and
the *run* picks the drivers. This is Dave Farley's four layers: test case, DSL, protocol driver,
system under test, with "a protocol driver for each channel". It is also Nat Pryce's
"acceptance and end-to-end are orthogonal", and cucumber's screenplay.js, whose world parameters
choose the session. Tags say which scenarios may vary; scenarios never list the variants.

Go's `@json-default` triple did the opposite: it listed variants as Examples rows. That is why
English was pinned in cells, why the no-flag and json rows were identical, and why any leaf
outside an outline was never tried in the other format.

## 1. Output format: two runs of every scenario
- **Run settings.** `run_feature` takes a `Run { leg: Json|Text, lane }` and puts it in the World
  through `Cucumber::before`. `every_feature_runs` runs each feature once per leg. A failure names
  its run: `cli/agent.feature [text, host]`.
- **Adding the flag.** `Common::run_lines` adds `--format <leg>` unless the line already has one.
  A scenario that writes `--format` itself must be tagged `@format-specific` (lint-checked) and
  runs in one leg only.
- **Checks with no step needed**, after every command. These catch a leaf that refuses JSON, or
  ignores `--format text`, everywhere:
  - JSON leg, success: stdout parses as JSON.
  - JSON leg, failure: stderr is the `{code,args,message}` envelope, and `message` equals the
    catalog's rendering.
  - Text leg, success: stdout is not JSON.
  - Text leg, failure: stderr starts `ctxloom: `.
  - Exempt: `@raw-output` (completion, hook protocol, `--version`).
- **Steps that state facts:**

  | Step | JSON leg | Text leg |
  |---|---|---|
  | `the output reports "<path>" as "<value>"` | selector equals value | value appears as a whole word |
  | `it is refused with "<code>":` + args table | envelope code and args equal | stderr equals the message rendered from the binary's own `catalog()` with those args |
  | `the listing is empty` | `[]` / empty envelope | the catalog's "none" message |

  The feature names a code and args, never English. The text leg still checks the exact en-US
  text, taken from the same catalog the binary uses.
- **The derived default.** One scenario, `cli/format.feature` (`@format-specific`), checks the
  default off a TTY and that an explicit flag wins either way. It replaces Go's hundreds of
  no-flag rows.
- **The weak spot.** Text *data* renderers still hard-code English (for example
  `library-cli/src/text.rs`'s "No agents defined.", "Headless posture: …"). Until those phrases
  move into the `.ftl` catalogs, the text leg can only check that the values appear. Meanwhile a
  transitional `in text it says "<prose>"` step is allowed, with a lint counting its uses so the
  English it pins shrinks to zero. Moving those phrases into the catalogs is the work that takes
  English out of step text entirely.

## 2. Mechanism: lanes, opted into by tags
- **Choosing a lane.** `CTXLOOM_LANE=host|container|live:<engine>` chooses the lane driver on the
  one World, which is the Screenplay idea of abilities. Steps never name the runtime. "When the
  pulled surfaces are delivered to Alice's agent" goes to the lane driver: the mock engine on the
  host, the mock image in a rootless container, or a real engine. Each driver supplies its own
  evidence (the mock's record file, or a marker a real engine must bring back).
- **Tags:**
  - `@every-runtime`: runtime-independent, runs in every lane its oracle allows.
  - `@container-only` / `@live-only`: specific to one mechanism.
  - Untagged: host only.
- **Gates:**
  - `just test` and pre-commit run the host lane, both legs, and are hermetic.
  - `just test-container` runs the container lane, JSON leg only. Format is presentation and
    doesn't change with the lane, so the format runs and the lanes are not multiplied together.
    A missing daemon fails the gate rather than skipping.
  - `just test-live <engine>` keeps the `CTXLOOM_LIVE_REQUIRE` floor.
  - `@live` never enters a hermetic gate.
- **The three duplicated container proofs:**
  - j002400 keeps its host-versus-container differential, `@container-only`, as the one proof
    that container launch is different.
  - j001400's host and container outlines merge into one `@every-runtime` scenario. First check
    why the container outline never asserted `mode`.
  - j002200's two `@container` scenarios become `@every-runtime`.

## 3. Exit codes: named outcomes over D28's table
| Step | Code |
|---|---|
| `it succeeds` | 0 |
| `it fails` | 1 (tightened: today's `the command fails` means any non-zero) |
| `it is refused` / `it is refused with "<code>":` | 2 |
| `it reports fatal findings` | 3 |

- **The numbers, once.** `cli/exit_codes.feature` has one representative command per outcome with
  its number. It is the only place the numbers appear.
- **"As a matter of course".** After each `When` command, an outcome step must run before the
  next `When` or the end of the scenario. `Common.observed` is checked in an `after` hook.
- **To verify:** that cucumber-rs counts a panicking `after` hook as a failure. If it doesn't,
  check at the next `When` and at the last step.
- **Parity lint.** Reshaped Go scenarios are `# substitution: format matrix: the triple's rows are
  the harness's legs`. A `--format` Examples column is refused unless the scenario is
  `@format-specific`.

## Rejected
- **Examples rows per format** (Go's triple): pins prose, duplicates rows, and covers only the
  leaves someone remembered to write an outline for.
- **Snapshots as the specification** (trycmd, snapbox, insta): a failure is a diff, not a broken
  rule, and harps, times and paths need redaction. They can be an optional extra on `@doc` text
  legs.
- **Mechanism as Examples rows:** tags attach to Examples blocks, not rows. That is how Go ended
  up with duplicated tagged outlines.
- **A step set per mechanism** (one World per lane): cucumber-rs gathers steps per World type, so
  every shared step would be duplicated.
- **A lane with no tags:** the container lane would rerun everything, and nothing would show
  which scenarios are meant to be runtime-independent.
- **Three format runs:** the no-flag run equals the JSON run off a TTY.

## Cost
- The hermetic scenario count doubles. It is subprocess-only and already parallel.
- Lanes add no format multiplier.

## Sources
- Farley, the protocol-driver layer:
  [ATDD how-to](https://www.scribd.com/document/539310652/ATDD-How-to-Guide),
  [QCon 2014](https://qconlondon.com/london-2014/dl/qcon-london-2014/slides/DaveFarley_TheProcessTechnologyPracticeOfContinuousDelivery.pdf),
  [BCS deck](https://edinburgh.bcs.org/events/2015/acceptance_testing_for_continuous_delivery.pdf).
- Pryce: [GOOS thread](https://groups.google.com/g/growing-object-oriented-software/c/alcUyNkDMAo),
  ["Having Our Cake and Eating It"](https://speakerdeck.com/npryce/having-our-cake-and-eating-it-1).
- [cucumber/screenplay.js](https://github.com/cucumber/screenplay.js);
  [cucumber-js profiles](https://github.com/cucumber/cucumber-js/blob/main/docs/profiles.md).
- [Writing better Gherkin](https://cucumber.io/docs/bdd/better-gherkin/);
  [BRIEF (Seb Rose)](https://cucumber.io/blog/bdd/keep-your-scenarios-brief/).
- [cucumber-rs 0.21 `Cucumber`](https://docs.rs/cucumber/0.21.1/cucumber/struct.Cucumber.html)
  (`before`, `after`, `filter_run`).
- [trycmd](https://docs.rs/trycmd), [snapbox](https://docs.rs/snapbox/latest/snapbox/),
  [clig.dev](https://clig.dev/),
  [Aruba exit statuses](https://github.com/cucumber/aruba/blob/main/features/03_testing_frameworks/cucumber/steps/command/check_for_exit_statuses.feature),
  [bats-assert](https://github.com/bats-core/bats-assert).
