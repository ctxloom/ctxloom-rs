# Feature-file analysis: UX, best practices, missing extensions and idiosyncrasies

Scope: all 72 Go `.feature` files (rev f36e8dfb2) were read in full, with their `.doc.md` and the
Rust ports on `main` beside them. j002500 and j002600 are listed only (ruled out).
Citations are Go paths unless marked `rs:`. Claims about clig.dev, git, gh, kubectl and
cargo come from general knowledge; nothing was looked up.

## Rulings (2026-11-06)
**The plan is `docs/plans/ctxloom-features.md`; it records every ruling and supersedes the table
below, which covers only the first rounds.** The owner has ruled on many of the recommendations
below; each is marked **Ruled** where it stands. The names live in `2026-11-06-vocabulary.md`, the testing design in
`2026-11-06-output-and-mechanism-testing.md`, and the audit of what was skipped as "covered
elsewhere" in `2026-11-06-considered-complete-audit.md`. **Where this analysis and the vocabulary
doc disagree on a name, the vocabulary doc wins**; the findings below keep Go's spellings
because they describe Go's specs.

| Topic | Ruling | Detail |
|---|---|---|
| Vocabulary | One name per concept. **engine** is only the program (claude-code, opencode, mock); an engine has many agents. Model, effort and permissions are per-engine config defaults, overridden per agent. The `llm` noun and its labels are retired. The item kind is `mcp-server`. | vocabulary §1-4 |
| Confirmation | Every destructive command confirms. Off a TTY without `--yes` it previews, changes nothing and exits **4**. | vocabulary §6 |
| Exit codes | Exercised in the features, over D28's table (1 failed, 2 refused, 3 fatal findings) plus 4. | vocabulary §7; testing §3 |
| Output | JSON when stdout is not a terminal. Every output is exercised in JSON and English as a matter of course. | testing §1 |
| Mechanisms | The same behaviour runs as one scenario through each mechanism (host, container, live), switched like the format. | testing §2 |
| `run --dry-run` | Writes no files. | §1.6 |
| Added | `profile show --resolved`; the engine-swap preview; `config get`/`set`; `deps upgrade <ref>`; `deps why`; `doctor --strict`; a scenario that removing a remote stops its hooks and MCP servers. | §3 |
| Declined | Authoring through flags or stdin. `create` opens the user's `$EDITOR` on template content and stores the result. | §3 item 3 |
| Write scope | Every write defaults to the project; `--home` writes home. | vocabulary §4 |
| Sessions | `session purge <harp>` or `--older-than <age>`: the session is gone, from disk and the listing, all or nothing. `session remove`, `sweep`, `transcript purge` and `worktrees purge` are retired. The features are rewritten. | vocabulary §8 |
| `session adopt` | Dropped. | §4 |
| Doctor check ids | Stable, descriptive names (`credentials`, `stale-hooks`), lint-style; not harps (doctor stores nothing). | vocabulary §5 |
| Implementation history | Never in features; it belongs in git history. | §4 |
| Probes | Those testing Claude Code itself are dropped (P12, P13, P15-P18, and P14's direct cell). | §4 Probes |
| `--force` | Replaces `--even-if-live`: `--force` overrides a named safety refusal. | vocabulary §4 |
| stamp-plan | Taskloom's; it already is (`taskloom manage hook stamp-plan`). | audit |
| Considered complete | Audited; the gaps are listed in the audit doc. | audit |

**Still open:**
- The llm rework (§7): whether a `ctxloom engine` noun lists the engines, and the config block's name.
- The extensions and idiosyncrasies not listed above.

## 1. Cross-cutting findings (decide these before more porting)

These come up in every area. Each is cheaper to settle in the Rust CLI now than after
more scenarios freeze it.

### 1.1 One concept, many names
- **The engine** goes by `--llm` (agent create, run, profile modify), `--engine`
  (manage install, hook), `--backend` (materialize, container check) and `--type` (llm create).
  agent.feature:68, deps.feature:221, manage.feature:393, materialize.feature:21,
  llm.feature:171, j002000:79 vs :95 (the comment and the step disagree).
- **The edit verbs**: `edit` patches by flag on llm, remote, session and agent, but opens
  `$EDITOR` on profile and mcp server. On those two nouns the flag patch has its own verb:
  `profile modify`, `mcp server set` (profile.feature:458/474, mcp.feature:259/305).
- **Attaching a bundle** is `--include` on `profile create` and `--add-bundle` on modify.
  `--include-fragment` means "lift a veto" (profile.feature:135,140; deps.feature:72,612).
- **Command vs prompt vs skill**: the noun is `command`, but the flag is `--add-prompt`,
  the directory is `prompts/` and the selector is `#prompts/` (bundle.feature:336,
  command.feature:67, j001400:96). j000700's doc says "skill" where its feature says "command".
- **"Starts a session"** means `materialize` in j000200 and j000800, and `run` in j000100.
- **Superseded by the vocabulary doc**, which is authoritative for every name. The original
  recommendation here (`--llm` labels plus a `--type`) is withdrawn. `--engine` names the
  program, the `llm` noun and its labels are retired, and model, effort and permissions are
  engine defaults overridden per agent.

### 1.2 Destructive operations: the preview-first convention, applied unevenly
- **Done well (keep as the house rule):** a bare destroyer previews, exits 0, and reports
  `applied:false` plus an `apply` field holding the exact command to run (agent.feature:476-494,
  bundle.feature:547-571, llm.feature:273-284, profile.feature:631-643, remote.feature:107-117,
  clean.feature:63-73). llm.feature:264 calls it "the posture every destroyer in this CLI shares".
- **Breaks:**
  - `manage uninstall` and `manage statusline uninstall`: no preview, no `--yes`
    (manage.feature:935, 796).
  - `bundle move` deletes the source with no preview (bundle.feature:491-499).
  - `doctor --fix` rewrites settings.json with no preview (doctor.feature:262).
  - `deps pull` prunes bundles removed upstream with no confirmation (deps.feature:306-328).
  - `materialize` with no target *fails* without `--yes`, where every other command previews
    (materialize.feature:80).
  - The override flag is `--force` on import, `--even-if-live` for liveness, and `-f` on
    import but "force re-distill" on distill and "fragment" on run.
- **`--yes` is overloaded:** it confirms on remove, means "apply" on `deps upgrade`, and on
  `deps pull` means "also write engine files". A pull without it still writes the lock and
  prunes (deps.feature:226-253).
- **Recommendation:** every writer previews unless given `--yes`, and `--yes` only ever means
  "do what the preview showed". Give pull's engine-file refresh its own flag. Make `-f`
  mean `--force` and nothing else.
- **Ruled:** every destructive command confirms. Without `--yes` off a TTY it previews,
  changes nothing and exits 4.

### 1.3 Exit codes have no table
- A missing credential exits 3 from `doctor` but 1 from `init` (doctor.feature:45,
  init.feature:116).
- Usage errors are not told apart from runtime failures (run.feature:357, search.feature:72).
- An edit whose content saved but whose distillation failed exits non-zero
  (editing.feature:28), so a script cannot tell "saved, degraded" from "nothing saved".
- `deps hold` on a local bundle exits 0 with "nothing to hold" (bundle.feature:530). Compare
  `command list --bundle nosuchbundle`, which correctly fails (command.feature:235).
- fault_tolerance.feature:2-4 promises "a warning, never a crash", while its own scenarios
  assert exit 3 and failure (:30, :47).
- **Recommendation:** publish one table and pin it once: 0 ok, 1 failure, 2 usage, 3 fatal
  startup finding, plus a code for "succeeded, a side effect degraded".
- **Ruled:** exit codes are exercised in the features, over D28's 1/2/3 plus 4 for "not
  applied: confirmation required". Steps name outcomes; one feature pins the numbers.

### 1.4 JSON off a TTY
- JSON is the default whenever stdout is not a terminal (`@json-default` everywhere). That
  helps agents, but it departs from git, gh and kubectl, which change only decoration when
  piped: `ctxloom bundle list | grep` gets JSON.
- **The format must then hold everywhere, and it doesn't:**
  - `agent default` fails outright when asked for JSON, so it fails in any pipe
    (agent.feature:429-436).
  - `profile modify` and `profile edit` reject `--format json` (profile.feature:351, 450).
  - `profile list` and `session list` return bare arrays, so no count or empty envelope is
    possible (profile.feature:168, 645).
  - The `deps list` empty-state hint exists only in text (deps.feature:55).
  - Errors are prose in every format (search.feature:38), though the Rust CLAUDE.md already
    specifies a `{code,args,message}` envelope.
- **Recommendation for Rust:** every leaf answers in both formats, every list is an envelope,
  and hints and next steps go in the payload. Decide explicitly whether the piped default stays
  JSON. If it does, document it as the agent-first choice.
- **Ruled:** JSON when stdout is not a terminal. Every output is exercised in both JSON and
  English, as two runs of every scenario with errors checked against the catalog
  (testing doc §1).

### 1.5 Refusals that know the fix but don't say it
- **Done well:**
  - `llm create` points at `ctxloom llm edit big` (llm.feature:196).
  - `config create`, `remote create … file://`, `--even-if-live` and `use --force` are
    named in their refusals.
  - Doctor names `manage gitignore install` (j001300:63-73).
- **Gaps:**
  - `bundle show demo#fragments/x` does not suggest `bundle view`, which the header calls
    "the most common wrong turn" (bundle.feature:24, 184).
  - Typos never get a "did you mean" (config.feature:337, run.feature:360, session.feature:448).
  - Several refusals assert only that the command failed (profile.feature:345, remote.feature:224,
    mcp.feature:339, skill.feature:226). The retention refusal asserts only the word "retention"
    and names no escape (session.feature:710-718).
- **Recall is the opposite of engine switch.** A mistyped harp *warns and runs anyway*
  (j001200:110-112). A mistyped engine is refused with the valid set (j002000:92-97). The recall
  behaviour was a crash fix, not a design (j001200.doc.md:89-103).
  **Recommendation:** a named argument that does not resolve is refused.

### 1.6 `--dry-run` writes
`run --dry-run` still scaffolds `.ctxloom/` and `.gitignore`. The spec works around this with a
"WARM-UP" run before taking its snapshot (startup_boundaries.feature:20-28). A dry run must change
nothing. Fix this in Rust before porting that file. **Ruled:** `run --dry-run` writes no files. Separately, `materialize`, which writes into
the user's own files, has no dry run at all.

### 1.7 "Nothing to do" is answered three ways
- `profile modify dev` with no flags exits 0 with "No changes made." (profile.feature:356).
- `remote edit origin` with no flags fails (remote.feature:221).
- `session edit` with no flags fails and names `--name` (session.feature:457). This is the
  right one; adopt it for the other two.

## 2. Best practices

### 2.1 CLI UX against established practice

| Practice | Reference | Followed | Broken |
|---|---|---|---|
| Confirm before destroying | clig.dev; `git clean` requires `-f` | the preview-first destroyers (§1.2) | uninstall, move, `--fix`, pull pruning |
| Prompt on a TTY, flag in scripts | clig.dev; gh `--yes` | init asks only on a pty and otherwise names the steps (init.feature:31, 109) | no destroyer prompts on a TTY; setup's restart offer has no non-TTY spec (j000200:43) |
| Dry run changes nothing | `git clean -n`, `kubectl --dry-run`, `cargo publish --dry-run` | `run --dry-run` delivers nothing (startup_boundaries:63) | it scaffolds (§1.6); materialize has none |
| Idempotent apply; create refuses existing | `kubectl apply`; `git remote add` | re-materialize doesn't duplicate (manage:657); gitignore install is safe twice (manage:889); create refuses and names edit (llm:190) | no upsert or `--force` on `agent create`, which the spec itself proposes (agent:168) |
| `config get`/`set` | git, npm, gh, kubectl | `config show [section]` (config:243) | no way to change one key except `$EDITOR` (config:382) |
| stdout data, stderr messages | clig.dev, POSIX | hook stdout is JSON only (session_hooks:19-24); errors leave stdout empty (agent:175); `transcript show` is NDJSON only (session:552) | "the output contains" reads both streams together, so most warnings have no specified stream |
| Distinct, documented exit codes | gh's documented codes; sysexits | doctor's 3 vs 1 (doctor:16-25); prune's 3 vs 1 (container:376) | §1.3 |
| NO_COLOR, no ANSI off a TTY | no-color.org | TTY detection (mcp:388) | colour is never specified, the statusline included |
| Suggest corrections | git's "did you mean" | valid sets are listed for engines and types (manage:535, search:75) | no nearest-match suggestions anywhere |
| Never echo secrets | gh, kubectl redaction | auth status (auth:26-34); fingerprints in upgrade diffs (deps:581) | none |
| Undo for every action | `git restore`/reflog, `kubectl rollout undo`, `cargo yank --undo` | `manage uninstall`; `session keep --undo` | no undo for an engine switch, a source removal or a hold |
| Short onboarding plus a status verb | `gh auth login` / `gh auth status` | "Cloning IS the onboarding" (j000800:6); `manage check` and `doctor` | adoption takes two commands with test-only flags (j000100:36-40, see §4.2) |

### 2.2 Gherkin/BDD

- **Declarative over imperative.**
  - Early journeys do this well: "Bob clones the project / starts a session / his assistant
    receives…" (j000800:31-33).
  - Later journeys put literal command lines and MCP tool-call tables in journey steps:
    j001200:42, j001300:153, and j002100:49 (`the agent calls tool "agent_run" with: | role |`).
  - For per-command `cli/` specs, literal commands are fine, since the command *is* the
    behaviour. Push the assertions toward the domain ("the listing marks big as configured")
    and keep JSON paths in step code, not in step text such as
    `events[event=pre_tool].hooks[command=…].source_kind` (manage:703).
- **One behaviour per scenario.** Long When/Then chains:
  - j001300:293-313: 11 Givens and 8 Thens.
  - companion.feature:52-67: four register/remove cycles.
  - profile.feature:515: export, remove and import in one scenario.
  - startup_boundaries.feature:34-86: five phases.
  - Chains that pair a negative with its positive control are deliberate and good. Split the
    rest.
- **Outlines.**
  - The format triple (no flag / json / text) on almost every scenario tests one derivation
    hundreds of times. The no-flag and json rows are identical by construction, and the text
    row pins prose.
  - Several outlines have one row left over from engines that were cut (mcp:361, manage:614,
    skill:299, command.feature:46). command.feature:21 still promises "the full three-engine
    matrix".
  - Good outlines: the destroyer table (session:721) and the `--state` table (session:406).
- **Background.** deps.feature repeats the same three setup commands about 20 times. The Rust
  port already lifts shared Givens into a Background (rs:session_reclaim:8-10). Do the same for deps.
- **Personas.**
  - Alice authors and "Carol materializes" in j000600:35, and rs:j000600:39 copies it.
  - Priya is introduced in j002200:4, but the steps say Alice.
  - Third-person personas mix with "I run" and a faceless "the agent".
- **Journeys vs per-command specs.**
  - j002200 is half claude-code adapter contract (`CLAUDE_CONFIG_DIR`, `.claude.json`'s
    `oauthAccount`, `ARGV contains "--print"`, :461-602).
  - j001400 tests the storage layout (sidecar vs front matter, the sidecar `Order` field, :172-259).
  - j001900 carries format rows (:95-100).
- **Live apart from hermetic.**
  - `probes/` and `@live` gating are good, and rows skip loudly
    (`CTXLOOM_LIVE_REQUIRE`, j002300:211).
  - But `@live` scenarios still sit inside journeys (j000200:55, j000300:51/81, j000400:133),
    j001000 is `@live` only, and a probe step sits inside j002300:304.
- **Feature files as documentation.**
  - Comment blocks often outweigh the steps (j001400:21-110).
  - The `.doc.md` files go stale: j001900.doc.md still lists B3, B6 and M5 as red, all of which
    pass, and j002200:27 still says `--degraded` rescues a container run, which was inverted
    2026-09-15.

### 2.3 Spec conventions done well (hold these in Rust)
- **Prove the precondition before a negative assertion:** j000100 asserts the wired state
  before removal (:72-77), and the j001300 guard fails any no-op assertion when the command
  never ran.
- **Assert the effect, not the echo** (agent:72, bundle:49, config:363).
- **Unknown is never zero or healthy:** context_status "UNKNOWN … not the same as low"
  (:124); unreachable is not deleted (deps:330-431); doctor reports `info`, not `ok`, where it
  cannot know.
- **Distractor fixtures** (j001200:30-33) and **paired controls**: the plan sentinel's
  `@var-control`, and j002400's host-then-container differential run.
- **Config decoded strictly**, naming the file, key and expected type (config:281), with
  `--degraded` as the escape hatch.
- **"Cannot determine" never grants permission** to destroy (session:618).
  rs:session_worktrees names the owner by the lock state (alive/dead/indeterminate) instead of a
  pid, which improves on Go.

## 3. Missing logical extensions, ranked by value

1. **Ruled, added.** **An assembled-view inspector: `profile show <p> --resolved`.** Each item gets its origin and
   its masked or withheld status. Today every composition and mask scenario has to materialize
   into a scratch directory just to see the result (profile.feature:277-439). The source
   attribution in `manage hooks list` is the template.
2. **Ruled, added** (the preview; switching back is not ruled). **Preview what an engine switch loses, and switch back.** j002000 asks only after the switch.
   The Rust port dropped that scenario ("owner ruling pending"), so the journey has lost its
   payoff. Rust's materialize report already names lost surfaces and can feed the preview.
3. **Ruled, declined:** `create` opens the user's `$EDITOR` on template content and stores the result instead. **Scriptable authoring.** `fragment create`, `command create` and `bundle edit --add-*`
   write placeholder bodies ("Add content here.") and need `$EDITOR` for real content. Add
   `--content`, `--from <file>` and stdin, for agents and CI.
4. **Ruled, added.** **`config get`/`set <key>`.** Today config can only be read from the CLI.
5. **Ruled, added:** `deps upgrade <ref>` and `deps why` (the rest not ruled). **Day-two dependency flow:**
   - `deps upgrade <ref>` moves a single pin.
   - `deps why <ref>` shows what pulls it in.
   - `deps check --exit-code` fails when an update is available.
   - Releasing a hold, then showing what changed: `materialize --diff` already exists.
6. **Ruled, added:** `doctor --strict` (the rest not ruled). **CI gates:** `doctor --strict` (warnings fail; today a warn exits 0 by design),
   `doctor --check <id>`, and a preview for `doctor --fix`.
7. **Ruled, added.** **Revoking a source:** removing a repository provably stops its hooks and MCP servers. Adding
   a source is called "the trust act" (j000200:9), but nothing tests that it can be undone.
8. **Tamper detection:** a skill's drift is "withheld loudly" (j000600.doc.md:30-39), and a pinned
   SHA can be force-pushed away (j001400). Both are promised in prose, with no scenario in Go or Rust.
9. **`mcp server remove <bundle>#mcp/<n>`**, with a preview. `set` can create a server, but
   nothing removes one.
10. **`session approvals answer <id> allow|deny`.** Approvals can be listed from another terminal
    but not answered, and a spec pins that the command refuses an argument (session:787).
11. **Smaller gaps:**
    - `agent default --unset`; today `default ghost` is the de facto unset.
    - `companion add --project`, and an origin column in `companion list`.
    - `remote check` to re-verify access.
    - `search --tag` on every kind.
    - Deleting a command, then propagating the deletion through a pull (j000700 covers edits only).
    - Recall searching conversation bodies, or stating that it searches titles only.
    - j001300 "Step 3", turning lessons into authored content: at least a `@wip` scenario for
      the settled security requirement (doc.md:139-157).
    - A version-provenance line in doctor: "guarantees last verified against claude X",
      from the probe registry.

## 4. Idiosyncrasies that may not earn their keep

| Item | Where | Recommendation |
|---|---|---|
| Format triple on every scenario | `@json-default` everywhere | **Simplify**: one generic derived-default test over every leaf, and JSON assertions elsewhere. Rust already dropped most of them **Ruled:** two runs of every scenario (JSON, English) replace the triple; one scenario pins the default. |
| English prose and Go/cobra wording pinned | "unknown flag: --auth" (agent:318); "Applied 1 pin(s)."; `{mode=plan}` (Go's `%v` map, agent:258); git's own text (fault_tolerance:77) | **Simplify**: assert diagnostic codes and args; keep at most one prose check per catalog. rs:run.feature already had to map clap's message onto cobra's |
| "Gone, not hidden" checks for verbs Rust never had | `session rename`, `session watch`, `worktrees --reap`, `mcp server create/register`, `hook turn-changed`, `run --print` | **Drop** in Rust; Rust kept `--reap` (rs:session_worktrees:36) and `--print` (rs:run:52). Keep the credential-path absences (auth:49, llm `--env-file`), which are security posture |
| `session adopt` | a fix-up for transcripts orphaned before a claude `/clear` lineage fix (session:470) | **Drop**, or make it a doctor fix-up **Ruled:** dropped. |
| Opaque doctor markers | `DOCTOR-CHECK-DEPS-a1`, `-CREDENTIALS-m5`, `HOOKS-TRUST-d4` (its trust half has been dead since ADR 0037, rs:doctor:4) | **Rename** to stable semantic ids **Ruled:** stable, descriptive names (vocabulary §5). |
| `bundle distill <path/to/bundle.yaml>` | content_distill:59; every other bundle verb takes a name | **Drop** the path form |
| Coordination contract advertises 11 enum values; senders may use 4 | coordination_contract:233-268 | **Expose only the sendable values**; assert the schema, not the sentences |
| `evaluate_triggers` and Deferred tasks in ctxloom's MCP tools | mcp_tools:523-558 | **Move to taskloom**, as stamp-plan was (D17) |
| Bare `mcp` refusing off a TTY | mcp:403-417 (a migration aid; it breaks `ctxloom mcp \| less`) | **Time-box**, then the bare noun lists like every other |
| `manage check` shows `mcp=false` on every project at rest | manage:586-602 | Show `session-only`, or drop the row |
| `statusline install` only flips config; it takes effect on the next materialize | manage:796-807 | Apply now, or say "takes effect on next materialize" |
| Four session destroyers | `transcript purge`, `worktrees purge`, `purge` and `remove` (`purge` and `remove` differ only by the index entry) | Consider `remove [--keep-record]` **Ruled:** `session purge <harp>` or `--older-than`, all or nothing, and the session is gone; the other three are retired. |
| Three remove spellings, on profile only | `remove`/`rm`/`del` (profile:667) | One rule for every noun's aliases |
| Implementation history in comments | codenames, dated rulings, Go symbols (`operations.SetAgent`, `formatDebtAllowlist`); copied into rs:j000400:57-68, rs:j002000:58-99, rs:j001300_sweep:26, rs:j001800:26 | **Strip** on port: keep the claim and the why, move the history to commits and doc.md. rs:j000400:37/43 point at a table and a scenario the port doesn't have; rs:j001900's header says "one scenario per boundary" with one boundary ported **Ruled:** never in features. |
| j002200 session-home trio | :356, :386 and :405 assert one outcome | **Collapse** to one, plus the `host` opt-out; claude config-home cells go to adapter tests |
| Container launch proven three times | j002400, j002200 `@container`, j001400 `@container` | **Consolidate** **Ruled:** one scenario run through each mechanism (testing doc §2). |
| Other overlaps | "Bob without companions" (j000800:73 and j001800:175); "report what was not carried" (j000400:101 and j000800:120); three live codeword echoes; editing.feature vs command.feature edit | **Merge** |
| Rows that can only skip | `container-rootful` "WIRED, UNVERIFIED" (engine_isolation_matrix:167, isolation_probe:72) | **Drop** until a rootful runner exists; j002300:401 itself warns that such rows "look like coverage" |
| Placeholder strings pinned three ways | "Add content here.", "Add prompt content here.", "Example prompt content…" | Unify into one placeholder |
| Test-only flags in the first-hour journey | `manage install --no-pull`; `materialize --yes --degraded` (j000100:36-40, kept in rs:j000100:41-46) | Keep the network out with a fixture, so the story doesn't teach `--degraded` |
| Fixed-bug narratives pinned as specs | the gitignore comment-count "exactly 1 times" (manage:889); util's hew write-up (util:34-66) | Restate as "a second install is byte-identical"; one line of rationale per comment |
| A Rust inconsistency | rs:j000800 refuses the companion "not installed" row as vacuous; rs:j001800 ports exactly that case as its only scenario | Pick one rule and apply it to both |

**Probes.** Seven probes (P12-P18) run the vendor binary directly, "not through ctxloom". They
are **vendor-conformance canaries**: run them on an engine version bump, beside the claude
adapter, and port only those whose assumption the Rust adapter relies on. The probes that go
through ctxloom (the engine matrix, the isolation probe, P1-P4, remote_content_live and
distill_live) are a proper live lane.
**Ruled:** the probes that test Claude Code itself are dropped: P12, P13, P15-P18, and P14's
direct cell. P1 (`capability_context_approaches`) and the engine isolation matrix are kept and
now pending (one description word in each Go file was lowercased so gherkin 0.16 parses it).

## 5. Contradictions to fix before porting
- deps.feature:495 says "`deps pull` is offline by design", but pull reaches the network and
  prunes (:306-357).
- fault_tolerance's header contradicts its exit-3 scenarios.
- j002200:27 says `--degraded` rescues a container run; the 2026-09-15 inversion says it doesn't.
- j002000:145's title names the old bug, not the asserted behaviour.
- j001300 lists `session transcript purge` among its surfaces (:25) but never uses it.
- rs:cli/fragment.feature's header lists "show" and "editor/distill" scenarios that Go's file
  doesn't have. rs:cli/container.feature:77 refers to json rows that weren't ported.

## 6. Port status and suggested order

Ported in full: auth, clean, completion, doctor, post_tool_hooks, read_extras, util, j000600.
Mostly ported: container, manage, materialize, j000100, j000400, j000800, j002000.
Partly ported, with the rest now in `*.pending.feature`: agent (1/22), command, fragment,
init, llm (1/16), mcp, profile (1/20), run (1/14), session (12/37), skill, j000200, j000700,
j001300, j001800, j001900 (1/7).
Not ported, now pending: bundle, companion, config, content_distill, context_status,
coordination_contract, deps (36 scenarios), editing, fragment_draft_premise, mcp_resources,
mcp_tools, remote, search, startup_boundaries, version, fault_tolerance, j000300, j001000,
j001200, j001400, j002100-j002400, and 9 of the 15 probes (6 dropped).
`llm` is retired: its pending scenarios await rework around engine defaults.

Suggested next, by value per effort:
1. **version** (trivial).
2. **fault_tolerance**: the core error experience; assert diagnostic codes.
3. **run's agent-vs-profile conflict refusal** (run.feature:366): needs no engine.
4. **The agent / profile CRUD, in the ruled vocabulary** (`llm` retired; model, effort and
   permissions on engine defaults and agents).
5. **deps**: the largest gap. Fix the "offline" claim first; `--yes` only applies (ruled). Add
   `deps upgrade <ref>` and `deps why`.
6. **j001200 recall**, hermetic rows: refuse an unknown harp from the start.
7. **j002000's capability loss, plus j001900 B3 and B6.**
8. **j001400, slimmed**; **j002200, hermetic subset**.
9. **Companion loadouts:** j000300, plus the companion rows of j000800 and j001800, once a
   loadout probe exists.
10. **The coordinator journeys (j002100, j002300):** rewrite around the operator's view.
11. **The live and conformance lane:** j002400, j001000 and the probes.

## 7. Planned feature rework (from the rulings)

| Rework | State |
|---|---|
| Sessions: `session purge` is the one destroyer, all or nothing, and the session is gone | Done in `cli/session_purge.feature`, `cli/session_worktrees.feature`, `journeys/j001300_closeout.feature` and their pending files. Red until the product follows. |
| `--force` replaces `--even-if-live` | Done in `cli/session_purge.feature`. |
| Probes testing Claude Code itself dropped; P1 and the engine matrix pending | Done. |
| The llm layer retired | **Planned**, below. |

**The llm rework.** Every place the retired llm layer appears, about 80 references in 13 files:
1. **`cli/llm.feature`** (ported, runs). The rule that an `env:` block under a label is refused,
   naming its key, survives. It moves to the engine's config defaults: no credentials in
   committed config.
2. **`cli/llm.pending.feature`** (15 scenarios: list, create, edit, remove, default).
   - Labels, create, edit and remove are dropped: engines are fixed, and settings go through
     `config set engines.<engine>.<setting>`.
   - default becomes `agent default`.
   - "No way to put a credential into an entry" is kept for engine defaults.
   - The listing depends on the first question below.
3. **`--llm` on agents** (`cli/agent.pending.feature`, `cli/agent.feature`,
   `journeys/j002000_engine_switch.feature`): `--llm X` becomes `--engine X`. New scenarios cover
   `--model`, `--effort` and `--permissions` on `agent create`/`edit` overriding the defaults.
   j002000's switch is where the engine-swap preview lands.
4. **The `llm:` config block** in fixtures (`cli/manage.feature`, `cli/post_tool_hooks.feature`,
   and the config, startup_boundaries, fragment_draft_premise and profile pending files) becomes
   the engine-defaults block.
5. **`--llm` on run and profile** (profile and startup_boundaries pending files) becomes `--agent`
   or `--engine`.

Cost: the five files that run go red until the product follows. The config-block change breaks
config loading, so failures spread beyond the scenarios being changed.

**Open:**
- Does a `ctxloom engine` noun list the built-in engines and their effective defaults, or are
  they visible only through `config get engines.<engine>`?
- Is the config block named `engines:`?
