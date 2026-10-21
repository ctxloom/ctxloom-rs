# Plan: finishing the taskloom port

Written 2026-10-20. The Go taskloom is `cmd/taskloom` and
`internal/shared/tasks` (with `internal/taskloom/{config,engine,workdir}`) in
the Go ctxloom repository; about 12.9k lines. This plan ports it onto annals
and angzarr here.

## Where it stands

- `proto/ctxloom/tasks/v1/tasks.proto`: the task aggregate, its commands and
  events. Events carry the values the task holds after them, never a change
  to recompute: apply only assigns (angzarr's convention, its
  `operations/error-recovery.mdx` and `FundsDeposited.new_balance`).
- `core/wire/tasks-proto`: the tasks domain's wire types (`ctxloom.tasks.v1`),
  generated from that proto.
- `core/domain/tasks`: decide and apply for add, set-status, edit, retag and
  remove, with localizable rejections.
- `core/read-model/task-lists`: the list projector, with a resumable cursor,
  caught up over the sync or the async store port.
- `core/ports/ports`: project identity, the store opener, names, the clock,
  command ids, and waiting with jitter.
- `core/application/tasks-application`: add, tag and list. An add mints a
  harp and re-mints on `TaskExists` (100 tries, a fresh command id each); on
  annals' `Recovery::Retry` the same command id runs again in a new unit
  after capped exponential backoff with full jitter (10ms doubling to 1s,
  10 attempts). A list hides terminal and Deferred tasks unless `--all` or
  `--status`, in the order added.
- `adapters/driven/project-registry`: the work root, `.project-id` (adopting
  Go's `.ctxloom/project-id` on a write), the registry in Go's schema, and a
  mint under `index.yaml.lock` that re-checks first, so racing first writes
  bind one id. A read never mints.
- `adapters/driven/harp-names`: harp-core behind the names port.
- `adapters/driving/tasks-cli`: `add`, `list` and `tag`, text and json.
- `root/bin/taskloom`: the binary, over annals-store-sql's SQLite store at
  `~/.ctxloom/projects/<project-id>/state.db` (CLAUDE.md, "Storage").
  Its tests are Go's `crossprocess_test.go` ported: 40 concurrent adds, 20
  adds among 10 concurrent listings, 40 concurrent tags of one task, each a
  separate process on one database; they build the binary themselves.

Where the skeleton differs from Go, or from this plan:

- Concurrent writers are serialized by SQLite itself: `BEGIN IMMEDIATE` per
  unit of work and sqlx's default 5s busy timeout. With retries switched off
  the acceptance tests still pass, so the backoff is a guard, not what makes
  them pass.
- A new database opened by several processes at once failed its migration
  ("table events already exists"): sqlx takes no migration lock on SQLite.
  annals-store-sql now migrates under `BEGIN IMMEDIATE`, on annals' branch
  `concurrent-writers` (not merged), which this workspace's path dependency
  needs checked out.
- A read outside a unit of work (`SqliteStore::read_from`) is three
  statements, not one transaction, so a stream appended to mid-read can come
  back inconsistent. Lists and read-backs go through it; not fixed yet.
- harp-core has one word list, Go's "long" group, so project ids are three
  words from it, not from Go's smaller default group.
- `list` with no project here says so and lists nothing; Go falls back to
  `--global`. Hidden `tagma:*` tags are shown, the `@open`/`@terminal`
  classes are not taken, and JSON has no `derived_priority` and escapes no
  HTML: they come with tags, statuses and priority.

## Order of work

Per CLAUDE.md: the highest-risk pieces first, each slice the smallest
complete unit that retires a risk, and each reviewed by an architect before
the next. A slice cuts through every layer it needs; the sections after this
one are the reference for what each layer finally holds, not an order.

1. **The importer**, over the real `~/.ctxloom/tasks/*.jsonl`. Risk retired:
   data fidelity. For every task, Go's fold and the Rust state must agree.
   It also gives every later slice real data.
2. **Healing and repointing a project's store** (see "Project identity:
   heal and repoint"). Risk retired: tasks that seem lost because a
   directory resolved to the wrong id.
3. **Tags on tagma.** The `Tag` proto in the tagma repository, imported
   here, then validation and queries. Risk retired: the cross-repository
   proto dependency, and query parity between `tagma-core` and Go's
   `tagquery.go`.
4. **MCP and the companion contract**: `taskloom mcp` with `task_list` and
   `task_add`, plus `loadout` and `version --format json`. Risk retired:
   ctxloom loading the Rust binary as its companion. After this the Rust
   taskloom can serve real sessions.
5. **The remaining commands and tools**: status, edit, show, summary,
   tags, statuses, prune and lint, `tag`'s schema handling, and the remaining
   MCP tools and resources.
6. **Priority**: choosing or writing the formula evaluator. It is isolated
   from everything else, so it can move earlier if the evaluator looks
   risky.
7. **`manage`, `watch` and `run`**, then cutover.

## Tags on tagma

Tags are tagma's `(namespace:)key(=value)` grammar. The Rust engine is
`tagma-core` (github.com/benjaminabbitt/tagma, `crates/tagma-core`); the Go
module ctxloom uses is its port. It has no dependencies, so the core may link
it.

tagma is the one tag model for taskloom, ctxloom and ltk alike, so the tag
message and its conversions belong to no single domain.

1. A `Tag` message in the tagma repository (e.g. `proto/tagma/v1/tag.proto`,
   with its prost crate beside `tagma-core`): `optional string namespace`,
   `string key`, `optional string value`, matching `tagma_core::Tag`, with
   the conversions both ways. ctxloom-rs imports it as it imports
   angzarr-proto (a git dependency pinned by commit; a path dependency while
   the two change together), and `tasks.proto` imports the message. Tags on `TaskState`, `AddTask`,
   `Retag`, `TaskAdded` and `Retagged` become `repeated Tag`; normalization
   (dedupe, sort) is over tags, not strings. Any later domain that tags
   (bundles, sessions, ltk rules) uses the same message.
2. Validation at write time, from Go `operations.validateTag`: unparseable
   tags, the words `and`/`or`/`not`, the reserved `tagma` namespace, values
   outside a declared enum or range (non-finite included), invalid semver for
   `type=semver`.
3. The tag schema (Go `internal/shared/tasks/tagschema`):
   `tagma.<facet>:"<target>"=<value>` declarations from config. The facets are
   arity, enum, range, hide, type, priority_fn and decay_fn. A schema is
   immutable, and an absent schema means nothing is declared. Scalar collapse
   (a new `priority=high` displacing `priority=low`) is resolved by the
   application before `Retag` is sent: decide never reads the schema.
4. Tag queries: `tagma-core`'s postfix and infix engines replace Go's
   `tagquery.go`. Check its operators (`= != > >= < <= ~`, `*:key`, `+:key`,
   `ns:*`) against Go's before relying on it.
5. Display hiding: `tagma_core::HideConfig`, with `tagma:*` hidden by default.

## Ports and application

Ports (`core/ports`): the task store (annals' store port), the clock, harp
minting, project resolution, and configuration.

Command handlers (`core/application`):

- add: mint a harp, and retry on `TaskExists` up to 100 times, as Go's
  `harp.UniqueFrom` does.
- set-status, edit, tag: tag applies its adds, then its removes, after scalar
  collapse.
- remove and prune: prune selects by status, `status_set_at` older than a cut,
  and a tag query. It reports without `--yes`, and reports dangling
  `relates:`, text and trigger references.

Queries:

- list: statuses and the `@open`/`@terminal` classes, a term, a tag query,
  `--all`, global, sort, compact, limit. By default it hides terminal and
  Deferred tasks, and an explicit status turns that hiding off.
- show (every id must resolve), summary (counts ignore filters and limit),
  the tags vocabulary (active and total counts), statuses, and lint
  (exits non-zero on a violation, and when it checked no target).

Priority, computed when read and never stored: the `priority_fn` and
`decay_fn` formulas with `age_days` and `age_factor`, rank-normalized to
`[0, 5]` over non-terminal tasks, and `triage:exploited-in-wild` forced to 5.
Go compiles them with `expr`, so this needs a Rust evaluator: choose a crate,
or write a small one for the arithmetic the formulas use. Carry over the
warnings: no `priority_fn`, every task tied, coverage under 50%. Fix the
defects Go's tasks-schema.md records: NaN passes every guard, a zero
`created_at`, division by zero at −90 days.

## Driven adapters

- Store: annals-store-sql at `~/.ctxloom/projects/<project-id>/state.db`.
- Project identity, from Go `projectid`. Resolution order: `--project`, then
  `CTXLOOM_PROJECT_ID`, then the registry (`~/.ctxloom/projects/index.yaml`)
  and the marker (`.project-id`, adopting Go's `.ctxloom/project-id`). A
  write resolves (heal, adopt, move or fork, else mint); a read only looks
  up. A linked worktree goes to its
  primary checkout unless it has its own marker. The work root is
  `CTXLOOM_ROOT`, then the git root, then cwd.
- Configuration: `~/.taskloom/config.yaml`, then `<root>/.taskloom/config.yaml`,
  then `TASKLOOM_CONFIG_*`, then `--config-set`. It carries `schema_version`
  (`--write-upgrades`) and `tag_schema` extending the default schema, where
  the last declaration wins.
- The importer for Go's jsonl logs (Decided, 2).
- Harp minting: the same word lists as Go's `harp` (adjective-noun for tasks).
- Clock.

## Purging a stream in annals

Work in the annals repository, before prune is ported:

- A purge on the store port, sync and async: it deletes a stream's events,
  its snapshots, and the command ids recorded with them, in one unit of
  work. Stores that can purge opt in, so a store with no purge stays
  append-only by type. It is added to the store contract
  (`store_contract!`, `async_store_contract!`), to `InMemoryStore`, and to
  annals-store-sql.
- `docs/design/audit.md` revised: it says deduplication is forever because
  events are permanent, so no purge exists. With purge, a command
  redelivered after its stream is purged is decided again. For `AddTask`
  that would re-create the pruned task. Prune selects only tasks whose
  status is older than its cut (7 days by default), so it can only fire for
  a redelivery older than that, and it is an accepted cost. Write it down
  beside the others.

In ctxloom-rs, prune records `TaskRemoved{PRUNED}` first, so every list drops
the task, and then purges the stream. The task-list projector forgets a
pruned stream's last sequence when it projects that event. Without that, a
new task minted on the freed harp, whose stream starts again at sequence 1,
would be skipped as already seen.

## Project identity: heal and repoint

A project's database is keyed by its id, so binding a directory to a
different database is a change to the marker and the registry, and the
database itself is never rewritten. Go's `Resolve` heals on its own (a
missing marker rewritten, an unknown marker adopted, a proven move
repointed, a copy forked), but offers no command for someone to correct it
when it chose wrong. The worst case is a fresh clone while the old tree
still exists: it forks a new id, and every task appears lost.

The identity logic is a shared adapter, and both ctxloom and taskloom
present it as a `project` command group:

- `project show`: the id, the root, the marker, every registry entry for
  the path, the database path, and every inconsistency: two ids for one
  directory, a missing marker, a registered path that no longer exists, a
  database that no registered path points at.
- `project heal`: repair registry and marker skew in either direction, and
  run the store's integrity check. It reports what it would change, and
  applies only with `--yes`.
- `project repoint <project-id>`: bind this directory to an existing id and
  its database (a wrong fork, a fresh clone, a restored backup). It refuses
  while the id's registered tree still exists, unless told that the two
  trees are meant to share it. The id this directory had before keeps its
  database, which `project show` then reports as orphaned.
- `project merge <project-id>`: replay another project's task streams into
  this one, for a fork that gathered tasks on both sides. A harp held on
  both sides is re-minted, and the result says so. It may come later than
  the other three.
- Orphaned databases (no live registered path) are listed, and are removed
  only by name and with `--yes`.

## Driving adapters

- CLI: the Go command set and flags (inventory below). Formats text, json,
  yaml, toml and markdown; json when stdout is not a terminal. Warnings and
  errors carry a fix line, and structured formats get envelopes. Exit codes 0
  and 1.
- MCP over stdio: the tools `task_list`, `task_add`, `task_set_status`,
  `task_edit` and `task_tag`, the resources `taskloom://tag-schema` and
  `taskloom://tag-vocabulary`, and the server instructions, which are
  normative (add only once the user agrees, and so on).
- Companion: `loadout --format yaml` (hooks, the MCP server, the `taskloom`
  fragment), and `version --format json` with `lock_protocol`, which
  ctxloom's boot probe parses.
- `manage install|uninstall|check`, `watch` (a JSONL change stream,
  debounced), `run` (execs `ctxloom run --seed-task`).

## Parity and cutover

- Port the behavior Go's tests pin: concurrent processes (no lost write, no
  duplicate harp), CLI/MCP notice parity, every project-scoped read taking
  `--global`, degraded mode, and launch flags reaching MCP handlers.
- Run both binaries on the same inputs and compare.
- Run the importer over every existing log, then switch the companion
  registration to the Rust binary.

## Decided

1. **Go ctxloom and taskloom lose capabilities as the rewrite lands, and
   that is accepted.** The cutover does not wait for Go ctxloom to stop
   reading the jsonl log (`run --seed-task`, trigger evaluation, coord,
   isolation mounts, doctor); Rust writes only its own store.
2. **An importer brings existing logs over.** It folds each
   `~/.ctxloom/tasks/*.jsonl` into commands against the new store, keeping
   harps, timestamps, sessions and tags. It must handle duplicate-add
   anomalies (two tasks sharing a harp) and `repair` records, and say what it
   could not bring.
3. **Tasks live in the home directory only; repo homing is dropped.** A
   committed log was growing too large for the repository. `--homing` and
   the `homing` config key go.
4. **A project's store is keyed by its project id**, held in `.project-id`
   at the project root and shared by taskloom and ctxloom. It replaces Go's
   `.ctxloom/project-id`, and unlike Go's marker (gitignored private state)
   it is committed, so a fresh clone keeps the project's id. Two clones on
   one machine then claim the same id; `project repoint`'s refusal while the
   other tree exists is what keeps them from sharing a store unasked. The id is resolved through
   `~/.ctxloom/projects/index.yaml`, and the store is
   `~/.ctxloom/projects/<project-id>/state.db`. It is keyed by the id, not by
   a hash of the root path: the marker survives a move or a fresh clone, and
   the registry already handles moves, forks and worktrees. Where only
   `.ctxloom/project-id` exists, resolution adopts its id into `.project-id`,
   so a project keeps its id and its imported tasks.
5. **`repair` goes.** One stream per harp makes a duplicate add a refused
   command, so the anomaly cannot arise; only the importer meets it.
6. **Prune deletes the task's stream, reclaiming its space and freeing its
   harp**, as Go's prune does; a removed task's stream and harp stay. annals'
   store only appends today, so annals gains a purge (see "Purging a stream
   in annals").

## Decisions still open

1. **Deferred: `plan list` and `plan show`.** They read ctxloom's session
   index, so they wait for that. `gendocs` is dropped.

## Not taskloom's

Trigger evaluation (deciding whether a deferred task's revive condition has
fired) is ctxloom's, run by its triage agent; taskloom only exposes
`deferred_since`. It ports with ctxloom, not here.

## Inventory of the Go surface

CLI: `list`, `add`, `status`, `edit`, `tag`, `tags`, `summary`, `statuses`,
`show`, `prune`, `repair`, `lint`, `run`, `plan list|show`, `watch`, `mcp`,
`manage install|uninstall|check`, `loadout`, `version`. Persistent flags:
`--format`, `--json`, `--degraded`, `--project`, `--homing` (dropped),
`--config-set`, `--write-upgrades`. `repair` is dropped too.

Default statuses, in order: In Progress, To Do, Deferred (needs a trigger),
Done and Archived (terminal). The set is open: any name not starting with
`@`.
