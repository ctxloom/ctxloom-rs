# ctxloom vocabulary: one name per concept

Status: **ruled** (2026-11-06); the plan is `docs/plans/ctxloom-features.md`.
Each row gives the name to use, the names it replaces (from Go's features), and the rule.

ctxloom is a prototype, so renames are breaking and that is fine. No old spelling survives as an
alias, and no spec asserts that a retired name is gone.

## 1. Things

| Concept | Name | Replaces | Rule |
|---|---|---|---|
| The assistant program ctxloom drives (Claude Code, opencode, mock) | **engine** | backend, backend type, `--type`, llm, "mock agent" | A fixed set built into ctxloom; nobody creates one. `--engine` only ever names a program. An engine has many agents (owner ruling). |
| An engine's settings: model, effort, permissions | **engine defaults** in the `engines:` config block, **overridden per agent** | the `llm` noun and its labels (`claude-fast`, `big`), "[configured]/[built-in]" rows | `config set engines.claude-code.model opus`; `agent create reviewer --engine claude-code --model haiku --effort low`. The `llm` noun and its create/edit/remove/default are retired. A read-only noun `ctxloom engine` (`list`, `show <engine>`) shows each engine's effective defaults and the surfaces and hook events it carries. |
| What an agent may do unattended | **permissions** | "posture", "headless posture", `permissions.<e>.mode` shown raw; set on both llm entries and agents in Go | One of the engine settings: an engine default, overridden per agent. `--permissions <mode>` on input; reported as `permissions: <mode>`. |
| What runs: an engine with its model, effort and permissions, plus profiles, runtime, workspace and env | **agent** | (unchanged) | `agent default` says what runs when none is named. One agent run is a "run of agent X". The MCP tool `agent_run` keeps the word. |
| Where an agent's engine executes | **runtime**: `host`, `container-rootless`, … | (agent `--runtime`) | **Collision:** `container prune --runtime rkt` means the container tool. Rename that one to **`--container-tool`** (podman, docker). |
| The container program | **container tool** | `--runtime` on container commands, "backend" in container check | |
| A git repository ctxloom pulls content from | **remote** | source, repository, "the trust act" source | Noun `remote` (as in git). "Source" stays in prose only as a synonym the glossary defines. |
| The forge hosting a remote | **forge** | (unchanged) | |
| A package of content | **bundle** | | |
| Content inside a bundle | **item**, of a **kind**: `fragment`, `command`, `skill`, `mcp-server`, `hook` | prompt (for command), `mcp` / `mcp server` | One kind name throughout: noun, directory, selector and flag (§3). `mcp-server` (owner ruling). |
| A reference to one item | **ref**: `<bundle>#<kind>s/<name>` | positional `bundle name` pairs; `#prompts/`; `#mcp/` | Every verb on an item takes one ref: `command create demo#commands/review`. |
| A named composition of bundles, parents and masks | **profile** | | |
| A profile's own content withheld from what it assembles | **mask** | exclude, veto, `exclude_mcp`, `--include-fragment` (lift a veto) | `--mask <ref>` / `--unmask <ref>`, working on any kind. |
| An engine-native place content is delivered to (context file, commands dir, skills dir, hook settings, MCP config) | **surface** | | |
| Writing surfaces into the project at rest | **materialize** | "starts a session" (j000200, j000800) | Materialize is never called a session. |
| One `ctxloom run` lifetime | **session** | | |
| A session's (or any ctxloom-minted thing's) human id: adjective-adjective-noun | **harp** | | Defined in the glossary and help text before any spec uses it. Positional `<harp>`. Doctor checks are not harps (§5). |
| A session's recorded conversation | **transcript** | | |
| A session's scratch checkout | **worktree** | | |
| A registered external tool (ltk, reprise) | **companion** | | |
| A bundle pinned from a remote | **dependency** (noun `deps`) | | |
| The recorded commit of a dependency | **pin** (in the **lock**) | | |
| A dependency frozen against upgrade | **hold** | | |
| A fatal startup finding ctxloom may run past on request | **degraded** run | | `--degraded` only overrides findings; it never weakens isolation (j002200's inversion). |

## 2. Verbs

| Verb | Means | Replaces / rule |
|---|---|---|
| `list` (bare noun = `list`) | enumerate | Every bare noun lists, never prints help. |
| `show <x\|ref>` | one thing in full; on a bundle ref it shows the item | Replaces `bundle view`. `show` takes any ref. |
| `create <x> [--field …]` | make a new thing; refuses an existing name and names `edit` | With no content given, opens `$EDITOR` on a template and stores the result (owner ruling). An empty result is refused. |
| `edit <x> [--field …]` | change a thing: flags patch; no flags opens `$EDITOR` on its document | Replaces `profile modify`, `mcp server set`. Where there is no document, no flags is refused, naming the fields (`session edit`'s behaviour). |
| `remove <x>` (alias `rm`) | destroy what was authored | One alias rule for every noun (drop `del`). |
| `purge <harp>` / `purge --older-than <age>` | sessions only: the session is gone, from disk and the listing | Replaces `session remove` and `session sweep` (§8). |
| `clean` | reclaim regenerable cache | |
| `install` / `uninstall` | wire / unwire ctxloom into a project (`manage`) | |
| `pull` / `upgrade [<ref>]` / `check` / `why <ref>` / `hold <ref>` | dependency lifecycle | `upgrade <ref>` moves one pin; `why` shows what pulls a ref in. |
| `keep <harp>` / `hold <ref>` with `--undo` | toggles | Every toggle undoes with `--undo`. Replaces deps `--release`. |
| `get <key>` / `set <key> <value>` | config | New (owner ruling). |
| `default [<x>]` | set or show the default; `--unset` clears | Replaces "bind a nonexistent name" as the de facto unset. |
| `export` / `import` | move a thing between projects | One shape: `export <x> --to <path>`, `import <path>`. |

## 3. Kinds: one spelling everywhere

| Kind | Noun | Bundle dir | Selector | Profile/bundle flags |
|---|---|---|---|---|
| fragment | `fragment` | `fragments/` | `#fragments/` | `--add-fragment` / `--remove-fragment` |
| command | `command` | `commands/` (was `prompts/`) | `#commands/` (was `#prompts/`) | `--add-command` (was `--add-prompt`) |
| skill | `skill` | `skills/` | `#skills/` | `--add-skill` |
| mcp-server | `mcp-server` (was `mcp server`) | `mcp-servers/` | `#mcp-servers/` (was `#mcp/`) | `--add-mcp-server` (was `--add-mcp`) |
| hook | `hook` | `hooks/` | `#hooks/` | `--add-hook` |

Profiles take `--add-bundle` / `--remove-bundle` and `--add-parent` / `--remove-parent` on both
`create` and `edit`. `--include` goes away.

## 4. Flags: one meaning each

| Flag | Meaning, everywhere | Retired spellings / collisions |
|---|---|---|
| `--yes`, `-y` | Apply what the preview showed. Never means anything else. | `deps pull --yes` "also write engine files": give that its own `--materialize` |
| `--force`, `-f` | Override a named safety refusal (overwrite, unprovable liveness). The refusal names the guard it overrides. Retention has no override: `keep --undo` and time release it. Replaces `--even-if-live` (owner ruling). | `--even-if-live`; `-f` as "fragment" on run and "re-distill" on distill (`distill --again`) |
| `--dry-run` | Do everything except the effect; **writes nothing**, scaffolding included | |
| `--format json\|text` | Output encoding; `text` is the en-US catalog's rendering | Default: JSON when stdout is not a terminal, text on a terminal (owner ruling) |
| `--engine <engine>` | Name a program: claude-code, opencode, mock | `--llm`, `--backend`, `--type` |
| `--model <model>`, `--effort <level>` | Engine settings, on `agent create`/`edit` (`config set engines.<engine>.<setting>` for the defaults) | `llm create --model` |
| `--kind <kind>` | Item kind, on `search` only | `--type` |
| `--profile <p>` (repeatable) | Select a profile | `--profiles` (agent create), `-p` kept |
| `--to <path>` | Destination | `--target` (materialize), positional export dirs, `-o` (skill export) |
| `--home` | Write to home config instead of the project | Every write defaults to the project (owner ruling), `companion add` included; `--home` is the one way to write home |
| `--all` | Every project | `--all-projects` |
| `--strict` | Warnings fail (doctor; `deps check` with updates available) | New |
| `--resolved` | Show what a profile assembles, with origins and masks | New: `profile show --resolved` |

## 5. Doctor checks have stable, descriptive names (owner ruling)
Each check is named for what it checks, in kebab-case, the way clippy, ESLint and rustc name their
lints and the way ctxloom names its diagnostic codes. A name is never reused for a different
check.

- **Where the name appears:** the JSON row's `check` field, `doctor --check <name>`, the
  `ctxloom-doctor` skill and feature assertions. The row's message comes from the catalog, so
  rewording or translating it never moves the key.
- **Retired:** the `DOCTOR-CHECK-<AREA>-<suffix>` markers and their meaningless suffixes.
- **Not harps:** a harp names a minted instance, and a doctor run has no instance. Doctor
  stores nothing; only `--fix` writes, and that writes the engine's settings files.

| Today | Name |
|---|---|
| `SETUP-MARKER-e5` | `project-config` |
| `DEPS-a1` | `git-identity` |
| `AGENTS-b2` | `agents-resolve` |
| `CREDENTIALS-m5` | `credentials` |
| `VERSION-c3` | `version` |
| `HOOKS-TRUST-d4` | `hooks-registered` |
| `GITIGNORE-f6` | `gitignore` |
| `MCP-INVOCATION-g7` | `mcp-invocation` |
| `STALE-HOOKS-n5` | `stale-hooks` |
| `INGESTION-q7` | `engine-ingestion` |

## 6. Confirmation (owner ruling: every destructive command confirms)
- **Applies to:** every remove, purge, clean, prune, uninstall and import-over, `bundle move`,
  `doctor --fix`, `deps pull`'s pruning, materialize overwrites, and `agent edit --engine` (whose
  preview is the engine-swap loss report).
- **Without `--yes` on a TTY:** show the preview, then prompt.
- **Without `--yes` off a TTY:** show the preview (JSON: `applied:false`, `apply:<exact command>`),
  change nothing, and exit **4** (owner ruling), so a script that forgot `--yes` does not pass
  silently.

## 7. Exit codes (to be exercised in features)
Codes 1-3 are D28's, already published in `ctxloom-cli/src/error.rs`. Codes 4-6 are proposed
extensions.

| Code | Meaning |
|---|---|
| 0 | succeeded |
| 1 | failed: the operation was attempted and did not succeed (D28) |
| 2 | refused before any state was read: unknown verb/flag, bad value, missing argument (clap's default) (D28) |
| 3 | fatal findings: refused at start (`--degraded` overrides where allowed); doctor's fail row (D28) |
| 4 | not applied: confirmation required (a preview off a TTY, no `--yes`) (owner ruling) |
| 5 | succeeded, a side effect degraded (saved but not distilled) |
| 6 | check found something: `doctor --strict` warnings, `deps check --strict` updates |

Features name the outcome, not the number: `Then it is refused as a usage error`. The step maps
outcome to code from this one table, so the table is pinned once.

## 8. Session destroyers (owner ruling 2026-11-06)
**`session purge <harp>`** or **`session purge --older-than <age>`**: the session is gone. Its
index entry, its transcript and its scratch worktrees go, from disk and from `session list`.

- **Replaces** `session remove` (retired) and `session sweep` (now `--older-than`).
  `--older-than` replaces `--purge-older-than`; the configured `session_purge_age` stands where
  no flag is given, and with neither, nothing is purged.
- **Kept from today:**
  - previews without `--yes` (exit 4 off a TTY);
  - refuses a transcript within retention (30 days) or marked keep, with no override;
  - refuses a session whose lock cannot prove it ended, unless `--force` (replaces
    `--even-if-live`, owner ruling).
- **All or nothing:** a session holding anything the human wrote (uncommitted work in a
  worktree, unfiled notes) is refused whole, naming each thing. The output dir (plans, summaries)
  is never touched.
- **Single-population purges retired** (owner ruling): `session transcript purge` and
  `session worktrees purge` are gone. `session worktrees [list]` stays as a read-only listing.
- **Elsewhere,** `remove` keeps its meaning (delete an authored thing now). `purge` means "gone
  for good" and is used only for sessions.
