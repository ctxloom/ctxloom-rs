# Plan: ctxloom's features, from Go's specs to the Rust port

Written 2026-11-06. Go ctxloom's 72 `.feature` files (rev f36e8dfb2) were read in full against
the Rust ports on `main`. They were reviewed for UX, best practice, missing extensions and
idiosyncrasies, and the owner ruled on the findings. This plan records those rulings and the
work they set.

Supporting documents, in `docs/reviews/`:
- `2026-11-06-feature-analysis.md`: the findings, with citations.
- `2026-11-06-vocabulary.md`: the glossary, one name per concept. It is authoritative for names.
- `2026-11-06-output-and-mechanism-testing.md`: the harness design and its sources.
- `2026-11-06-considered-complete-audit.md`: Go behaviour skipped as "covered elsewhere".

## Stance

ctxloom is a prototype. Breaking changes are fine. There are no aliases for old spellings, no
migration aids, and no scenarios asserting that a retired verb or flag is gone ("gone, not
hidden" is dropped as a concept). A spec states what ctxloom does, never what it used to do.
Implementation history (rulings, dates, codenames, Go symbols, bug narratives) never goes in a
feature file; it belongs in git history.

## Where it stands

- **Pending scenarios.** Every Go scenario not yet ported sits, verbatim, in the Rust file that
  ports its Go file (`crates/root/bin/ctxloom/features/**/<name>.feature`), tagged `@pending` on
  the scenario or on its Rule. A file none of whose scenarios is ported yet is `@pending` on its
  Feature and has no `# world:`. The runner leaves pending scenarios out, and the parity lint
  still compares each with Go's (convention: `gherkin_lint.test.rs`). The journeys still keep
  theirs in `journeys/<name>.pending.feature` until each is folded into its port.
- **Not copied:** j002500 and j002600 (taskloom's), and the probes that test Claude Code itself.
  The two probe files gherkin 0.16 could not parse (`capability_context_approaches`,
  `engine_isolation_matrix`) are pending. One description word in each Go file was lowercased
  so they parse.
- **Sessions rewritten** to the rulings below: `cli/session_purge.feature`,
  `cli/session_worktrees.feature` and `journeys/j001300_closeout.feature` (all @pending).
  Eleven scenarios fail against today's binary until the product follows (step 4).

## Decided

### Names (vocabulary doc §1-5)
- **engine** is only the program: claude-code, opencode, mock. It is a fixed set; an engine has
  many agents.
  - Model, effort and permissions are per-engine defaults in an `engines:` config block
    (`config set engines.claude-code.model opus`), overridden per agent (`--model`, `--effort`,
    `--permissions`).
  - A read-only `ctxloom engine` noun (`list`, `show <engine>`) shows each engine's effective
    defaults and the surfaces and hook events it carries.
  - The `llm` noun, its labels (`claude-fast`, `big`), `--llm`, `--backend` and `--type` (for
    an engine) are retired.
- **agent** is what runs: an engine with its settings, plus profiles, runtime, workspace and env.
  `agent default [--unset]` says which agent runs when none is named.
- **Item kinds** have one spelling throughout (noun, bundle dir, ref, flag): `fragment`,
  `command`, `skill`, `mcp-server`, `hook`. A ref is `<bundle>#<kind>s/<name>`, and every item
  verb takes one ref.
- **Verbs:**
  - `create` opens `$EDITOR` on a template when given no content, and stores the result.
  - `edit` patches by flag, or opens `$EDITOR` with no flags. `profile modify` and
    `mcp server set` are retired.
  - `show` takes any ref (`bundle view` is retired).
  - `--mask`/`--unmask` replace the include/exclude/veto flags.
  - Toggles undo with `--undo`.
  - `rm` is the one alias, on every noun.
- **Flags:**
  - `--yes` only applies what the preview showed.
  - `--force`/`-f` only overrides a named safety refusal, and replaces `--even-if-live`.
    Retention has no override.
  - `--dry-run` writes nothing.
  - `--to` is the destination; `--home` writes home (every write defaults to the project).
  - `--container-tool` names the container program. `--runtime` is the agent's.
- **Doctor checks** have stable, descriptive kebab-case names: `credentials`, `stale-hooks`,
  `git-identity` and so on. Doctor stores nothing; only `--fix` writes.
- **"Did you mean"**: unknown verbs, flags and names suggest the nearest match.

### Confirmation and exit codes (vocabulary §6-7)
- **Every destructive command confirms.**
  - On a TTY it previews and prompts.
  - Off a TTY without `--yes` it previews (`applied:false`, `apply:<exact command>`), changes
    nothing and exits **4**.
  - The engine-swap preview is `agent edit --engine`'s confirmation.
- **Exit codes:** 0 succeeded; 1 failed; 2 refused before any state was read (usage); 3 fatal
  findings (D28); 4 not applied, confirmation required; 5 succeeded but a side effect degraded;
  6 a check found something (`doctor --strict`, `deps check --strict`).
- **A name that does not resolve is refused.** Recall no longer warns and runs anyway.
- **Nothing to do is refused**, naming the fields.

### Output and testing (testing doc)
- **Format:** JSON when stdout is not a terminal, text on a terminal.
- **Every scenario runs twice, in JSON and in English.**
  - The harness adds `--format` and checks after every command that the output is the format
    asked for.
  - Errors are asserted by diagnostic code and args; the English run checks the exact message
    from the binary's own catalog. One `cli/format.feature` pins the default.
  - Text data renderers move their fixed phrases into the catalogs. Until they do, a
    transitional `in text it says` step is allowed and counted down to zero.
- **Mechanisms are lanes.**
  - `@every-runtime` scenarios run on host, container and live lanes; `@container-only` and
    `@live-only` are specific to one. Untagged scenarios are host-only.
  - Pre-commit runs the host lane. Container and live have their own gates, where a missing
    runtime fails rather than skips.
- **Outcomes are steps:** `it succeeds`, `it fails`, `it is refused`,
  `it reports fatal findings`. One `cli/exit_codes.feature` pins the numbers, and every
  command's outcome must be asserted.

### Commands
- **Added:**
  - `profile show <p> --resolved`;
  - the engine-swap preview;
  - `config get`/`set`;
  - `deps upgrade <ref>`, `deps why <ref>`, `deps check --strict`;
  - `doctor --strict`, `doctor --check <name>`;
  - `mcp-server remove <ref>`;
  - `agent default --unset`;
  - `remote check`;
  - an origin column in `companion list`;
  - a scenario showing that removing a remote stops its hooks and MCP servers.
- **Switching an engine back** is `agent edit --engine <old>`; there is no new verb.
- **Sessions:** `session purge <harp>` or `session purge --older-than <age>` is the one
  destroyer.
  - The session is gone, from disk and the listing, all or nothing. A session holding anything
    the human wrote is refused whole, naming each thing.
  - The output dir is never touched.
  - Retention and keep refuse with no override; unprovable liveness refuses unless `--force`.
  - `session remove`, `sweep`, `transcript purge` and `worktrees purge` are retired.
    `session worktrees` stays as a listing.
- **Dropped:**
  - `session adopt`;
  - the `bundle distill <path>` form;
  - the bare `mcp` refusal;
  - tamper detection: signing is gone, local changes are the user's, and remote changes go
    through their own review.
- **Fixed:**
  - `run --dry-run` writes no files;
  - `manage check` reports `mcp=session-only`, not `false`;
  - `statusline install` takes effect immediately;
  - only the four sendable coordination kinds are advertised.
- **Deferred:** `session approvals answer` (with the coordinator), search across every kind,
  and recall searching conversation bodies.

### Specs
- **Journeys are use cases; cli features are contracts.** A journey (`features/journeys/`)
  follows what a real user actually does, in the order they do it: Alice starts with
  `ctxloom init`, not `manage install`. A command's contract (its flags, its files, its edge
  cases) belongs in its `features/cli/` file, never in a journey. A journey step that exists
  only to exercise a command is moved to that command's cli feature. A journey's comments
  and description tell the user's story and nothing else: no history (rulings, dates, what
  ctxloom used to do) and no technical detail (files, keys, harness mechanics, why an
  assertion is shaped as it is). Rationale a test still needs lives in the cli feature or
  the step definitions.
- **Companions are added, never discovered.** No scan finds companions, so a journey that
  relies on one shows the user adding it to the project (`companion add`) before it matters
  (j000300's setup augmentation first).
- **Dropped:**
  - the probes that test Claude Code itself (P12, P13, P15-P18, and P14's direct cell);
  - the `container-rootful` rows that can only skip;
  - the test-only `--no-pull --degraded` in j000100 (a fixture keeps the network out).
- **Moved:** `evaluate_triggers` and Deferred tasks go to taskloom. j002200's claude
  config-home checks go to the claude adapter's tests.
- **Collapsed:**
  - j002200's three session-home scenarios become one, plus the `host` opt-out;
  - the overlapping journeys merge (companionless Bob, the not-carried report, the live
    codeword echoes);
  - container launch is one scenario run through the lanes, with j002400's differential kept as
    the one container-only proof;
  - one placeholder string.
- **Companion rows:** j000800's and j001800's are ported together, once companion loadouts
  exist.
- **Not ctxloom's:** stamp-plan is taskloom's, and already is (`taskloom manage hook stamp-plan`).

## What remains, in order

1. **The harness.**
   - Run every feature twice, in JSON and English, with the format checks after each command.
   - Steps for diagnostic codes and catalog messages; outcome steps with the
     asserted-outcome check; lanes and their tags and gates.
   - `cli/format.feature` and `cli/exit_codes.feature`.
   - Migrate the existing scenarios' format rows to the two runs.
2. **The lint and helpers.**
   - Compare substitution scenarios after normalizing the folded Background.
   - Assert clap's exact wording until codes replace English.
   - Make the bundle-fixture step refuse keys it does not translate.
3. **The vocabulary pass over the features.** Rename to the decided names in ported and pending
   scenarios, and strip implementation history from every comment. Pending scenarios then carry
   `# substitution:` lines where they differ from Go.
4. **The product follows the features.** Each item below is a feature change, then the code that
   turns it green:
   - **sessions** (already rewritten; the eleven red scenarios);
   - **the llm rework:**
     - `cli/llm.feature`'s credential refusal moves to `engines:`;
     - `cli/llm.feature`'s @pending scenarios are dropped, except the credential rule and what
       `engine list`/`show` covers;
     - `--llm` becomes `--engine`, plus `--model`, `--effort` and `--permissions` on agents;
     - `llm:` fixtures become `engines:`;
     - run and profile take `--agent` or `--engine`;
   - **confirmation and exit 4** on every destroyer;
   - **`run --dry-run` writes nothing.**
5. **The audit follow-ups.**
   - Port j000100's doctor-on-a-wired-project scenario as Go wrote it.
   - List Go's 15 doctor checks that Rust lacks in doctor.feature's header, and decide each when
     doctor is next touched.
6. **Porting, by value per effort.**
   1. version
   2. fault_tolerance (asserting codes)
   3. run's agent-vs-profile refusal
   4. agent and profile CRUD in the new vocabulary, with `profile show --resolved`
   5. deps, with `upgrade <ref>`, `why` and `check --strict`, after fixing the "offline" claim
   6. config, with `get`/`set`
   7. j001200 recall, refusing an unknown harp
   8. j002000's swap preview, and j001900 B3 and B6
   9. j001400 slimmed, and j002200's hermetic subset
   10. companion loadouts (j000300, j000800, j001800)
   11. the coordinator journeys (j002100, j002300), rewritten around the operator's view
   12. the live lane: j002400, j001000 and the remaining probes

## Decisions still open

None from the review. New ones go here as the work raises them.
