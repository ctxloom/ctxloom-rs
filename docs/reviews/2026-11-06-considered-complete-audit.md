# Audit: Go behaviour not ported but considered complete

Status: findings (2026-11-06). Method: every Go scenario was matched by name to the Rust files
claiming `# Ported from` its file, ported or `.pending`. Examples rows and steps were compared.
Each "covered elsewhere" claim was traced to the test that supposedly carries it.

## Verdicts
| Item | Go | Rust / claimed coverage | Verdict | Action |
|---|---|---|---|---|
| Hand-authored context file survives byte-for-byte | cli/fragment.feature:150 | j000400_multi_engine.feature:124 (same rows, step as strict as Go's) | covered | none |
| session-bind records harp in transcript and index | cli/session_hooks.feature:58 | `src/session.test.rs:628` (header wrongly says hook.test.rs) | covered | fix the header citation |
| No active harp: silent stdout, warning | :73 | `src/hook.test.rs:143` (stricter: stdout empty) | covered | none |
| Statusline / hud | :125 | `src/hook.test.rs:75-139` | covered, stronger | none |
| Retired turn-changed | :141 | `src/hook.test.rs:200` (exit 2) | covered | none |
| **stamp-plan**: plan file stamped, ordinary file untouched | :87, :108 | "taskloom's (D17)" | covered in taskloom (`taskloom manage hook stamp-plan`) | none; the ctxloom-rs walking-skeleton copy is stale |
| **j000100: doctor on a wired project** | j000100:60 | cli/doctor.feature:60 runs on an *unwired* project | **partial** | port as Go wrote it (wired step exists, `gherkin_surfaces.test.rs:233`) |
| **probes P1 context approaches, engine isolation matrix** | probes/ (4 and 12 Examples) | none: not ported, pending, or ruled out | **not recorded** | see Gap 2 |
| 15 Go doctor checks with no Rust counterpart (GITIDENT-l2, SETUP-DEPS-h8, SETUP-COMPANIONS-i9, SECRETS-STORAGE-k1, LOCAL-STATE-p6, ORPHAN-CONTAINERS-z2, WITHHELD-ITEMS-w2, STARTUP-FINDINGS-x4, TRANSCRIPT-READER-v2, SPOOL-COUNTERS-w3, SPOOL-BACKLOG-t0, TTY-INJECTION-m3, SUPERSEDED-IMAGES-x4, MCP-COMMANDS-r3, PROJECT-OWNER-v4) | Go doctor code; no Go feature asserts them | doctor.feature says "NOT ported: nothing" | not covered, invisible | list them in doctor's header; decide which to port |
| HOOKS-TRUST trust-store half | doctor.go | ADR 0037 | correctly dropped | none |
| j000400 "codex and opencode rows" | j000400 | header line 5 | stale note: opencode is only in the `@live` outline, now pending | fix the header |
| 12 `# substitution:` scenarios (manage ×2, session_reclaim ×9, j001300_sweep Background) | | | wording only, none weaker | let the lint compare them (normalize placeholders and the folded Background) so they can't drift |
| "the session index records harp …" is a no-op step | post_tool_hooks | `gherkin_surfaces.test.rs:155` | correctly unneeded (plan 8.4), but silent | mark the scenario `# substitution:` |
| clap wording map ("unknown flag" → "unexpected argument"; "Available Commands:" → "Commands:") | run, container | `gherkin_common.test.rs:99` | partial: looser | assert `unexpected argument '--print'`; superseded once refusals assert diagnostic codes |
| Bundle translator ignores keys it doesn't know | | `gherkin_surfaces.test.rs:162` | fine today, fragile | panic on keys it does not translate |

Every Examples row of every ported Outline is present in Rust. None was silently dropped.

## Gaps, ranked
1. ~~stamp-plan is wired but exists nowhere.~~ **Withdrawn.** The finding came from the stale
   walking-skeleton copy of tasks-cli in ctxloom-rs. The taskloom repository
   (`~/workspace/ctxloom/taskloom`) has the hook as `taskloom manage hook stamp-plan`
   (`crates/root/bin/taskloom/src/main.rs:291`, `plans-cli/src/hook.rs`), and its loadout
   calls `taskloom` (`tasks-cli/src/loadout.yaml:71-72`). Owner ruling: stamp-plan is
   taskloom's.
2. **Two live probe files are untracked.**
   - **Why.** `capability_context_approaches` and `engine_isolation_matrix` were left out because
     gherkin 0.16 rejects their descriptions: lines starting with "Examples". The lint parses the
     Go source too, so a Rust-side reflow cannot pass.
   - **They are kept.** Both go through ctxloom, not Claude Code alone, so the owner's ruling
     keeps them.
   - **Fix options:** reflow those description lines in Go's repo, or have the lint compare
     such a file as Rust-reflowed text.
3. **Nothing runs doctor against hooks ctxloom itself wired.** Port j000100's scenario.
4. **The lint is blind to substitution scenarios.**
5. **Tighten the clap mapping and the bundle translator.**
