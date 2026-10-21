# Plan: finishing the taskloom port

Written 2026-10-20. The Go taskloom is `cmd/taskloom` and
`internal/shared/tasks` (with `internal/taskloom/{config,engine,workdir}`) in
the Go ctxloom repository; about 12.9k lines. This plan ports it onto annals
and angzarr here.

## Where it stands

- `proto/ctxloom/tasks/v1/tasks.proto`: the task aggregate, its commands and
  events. Its tags are tagma's `tagma.v1.Tag` (step 3, below). Events carry the values the task holds after them, never a change
  to recompute: apply only assigns (angzarr's convention, its
  `operations/error-recovery.mdx` and `FundsDeposited.new_balance`).
- `core/wire/tasks-proto`: the tasks domain's wire types (`ctxloom.tasks.v1`),
  generated from that proto.
- `core/domain/tasks`: decide and apply for add, set-status, edit, retag and
  remove, with localizable rejections.
- `core/read-model/task-lists`: the list projector, with a resumable cursor,
  caught up over the sync or the async store port.
- `core/ports/ports`: project identity and its survey (`ProjectAdmin`), the
  store opener, names, the clock, command ids, and waiting with jitter. Every port future, and every use
  case's, is `Send`, so a server can spawn them; project identity is
  synchronous and blocks on the registry's lock, so an async server calls it
  through `spawn_blocking`.
- `core/application/tasks-application`: add, tag, list and import, and the
  tag schema, write-time admission and tag queries (step 3, below). An add mints a
  harp and re-mints on `TaskExists` (100 tries, a fresh command id each); on
  annals' `Recovery::Retry` the same command id runs again in a new unit
  after capped exponential backoff with full jitter (10ms doubling to 1s,
  10 attempts). A list hides terminal and Deferred tasks unless `--all` or
  `--status`, in the order added.
- `core/application/project-application`: `show`, `heal`, `repoint` and the
  orphaned databases, planned from the port's survey; it depends on no
  tasks crate. The marker is authoritative and the registry heals toward
  it. A change is reported, and made only when asked, under the registry's
  lock and only while a fresh survey matches the one it was planned from.
- `adapters/driven/project-registry`: the work root, `.project-id` (adopting
  Go's `.ctxloom/project-id` on a write), the registry in Go's schema, and a
  mint under `index.yaml.lock` that re-checks first, so racing first writes
  bind one id. A read never mints. Resolution reads the marker first, then
  Go's older marker, then the registry. A linked worktree with no marker of
  its own is its primary checkout's project, found through git's common
  directory (the `.git` file's `gitdir`, then its `commondir`), and is bound
  and marked there, never in the worktree; one whose git directory is gone
  is an error, as in Go. The lock is taken again when what it was granted is
  no longer the file at its path (Go's `takeCurrent`, 16 tries), and waited
  for at most 10s, then refused naming the lock. It also implements
  `ProjectAdmin`: the survey, and the changes (markers, registry entries,
  and removing `~/.ctxloom/projects/<id>/`, never through a link).
- `adapters/driven/harp-names`: harp-core behind the names port.
- `adapters/driven/go-task-log`: Go's `~/.ctxloom/tasks/<project-id>.jsonl`
  behind the `GoTaskLogs` port, read only (not even its lock file). A line
  Go's fold would refuse (malformed, no op, an unknown op) refuses the
  whole log, as Go's fold does.
- `adapters/driven/task-store-sqlite`: annals-store-sql's SQLite store at
  the project's path, opened on first use and held, behind the store
  opener.
- `adapters/driven/system`: the clock, UUIDv7 command ids, tokio's timer and
  jitter.
- `adapters/driven/tasks-config`: taskloom's configuration behind the
  `Configuration` port, as far as the tag schema needs it (step 3, below).
- `adapters/driving/presentation`: the driving edge's presentation for any
  domain (see "Driving adapters").
- `adapters/driving/tasks-cli`: `add`, `list` (`--tag-query`), `tag` and
  `import`, with `--config-set`, text and json, its errors and notices
  rendered from the catalogs, and `run_mounted`, which flattens another
  crate's commands in beside them and hands one parsed to its caller with
  the global flags and the catalog.
- `adapters/driving/project-cli`: the `project` group (`show`, `heal
  [--yes]`, `repoint <project-id> [--share] [--yes]`, `orphans`, `orphans
  remove <id>... [--yes]`), text or one JSON document each, its errors and
  notices rendered from the catalogs, for any binary to mount; taskloom
  does, and ctxloom can, since it names no tasks crate.
- `root/bin/taskloom`: the binary, over annals-store-sql's SQLite store at
  `~/.ctxloom/projects/<project-id>/state.db` (CLAUDE.md, "Storage").
  It mounts the `project` group over the project registry.
  Its tests are Go's `crossprocess_test.go` ported, each process separate
  on one database, and they build the binary themselves: 40 concurrent adds
  at a project's first sight (racing the mint and the migration; one
  registry entry), 20 adds among 10 listings of a seeded project (every
  row a whole task of the final listing), the same at first sight, and 40
  concurrent tags of one task. Reads create nothing: `list --project tpyo`
  leaves no trace, and `tag` where there is no project binds none.
- The importer (`taskloom import`, Decided 2): each Go record becomes the
  command that did what it did, run at the instant Go recorded it (annals'
  `Sampled.now`), so `created_at`, `status_set_at` and `deferred_since`
  keep Go's times with no proto change. Records replay in Go's fold order
  (a byte-identical repeated line once, then a stable sort by time), and a
  port of Go's fold decides what counts: a displaced duplicate add is
  reported unless a `repair` add names it (`repair_of`, Go's
  `anomalyKey`), a record to a harp with no live task is ignored, a retag
  changing nothing records nothing. A record's command id is a UUIDv5 of
  its line (annals' fact namespace, prefixed), so re-running finds what an
  earlier run imported, resumes one that stopped, and picks up what Go
  appended since. It binds no directory: the project is the log's id.
  It exits 1 when it left anything behind, naming each record and why.
  Its acceptance test compares the store task by task with Go's own fold
  (`scripts/go-fold`, laid into Go's package with `go test -overlay`) over
  synthetic fixtures, one per kind of record (`fixtures/go-logs`);
  `scripts/compare-go-import.sh` runs the same comparison over copies of
  real logs. On 2026-10-21's logs (56 files, 23 non-empty, 6241 records)
  all 1162 tasks agreed on every field and a second run applied nothing;
  they held no duplicate add, repair, removal or repeated line, and two
  records out of time order, which the replay order puts right. Re-run
  with tags as tagma's (step 3): every task agrees, and all 5200 tags of
  live tasks parse.
- Healing and repointing (step 2): `taskloom project`, its acceptance tests
  the binary in a temporary home. A copy that forked its own id is
  repointed to the original's and finds its tasks, its own database then
  reported orphaned and removed by name; a lost marker is shown and healed
  from the registry; a linked worktree made by `git worktree add` shares
  its primary checkout's tasks and gets no marker.
- Tags on tagma (step 3). tagma's repository (github.com/benjaminabbitt/tagma)
  gains `proto/tagma/v1/tag.proto` (`optional string namespace`, `string
  key`, `optional string value`) and `crates/tagma-proto` (prost 0.14,
  generated at build time, conversions both ways with `tagma_core::Tag`,
  `PROTO_INCLUDE` for importers), and tagma-core a `Display for Tag`, the
  canonical spelling Go's port already had. ctxloom-rs takes tagma-core and
  tagma-proto as path dependencies into tagma's `tag-proto` worktree until
  that branch merges (Cargo.toml says so). Tags on `TaskState`, `AddTask`,
  `Retag`, `TaskAdded` and `Retagged` are `repeated tagma.v1.Tag`; the
  domain dedupes them by tag and sorts them by canonical spelling, as Go
  sorted its strings.
  - A write parses its tag strings (trimmed, empties dropped) and admits
    each it adds, as Go's `validateTag`, before resolving anything: the
    grammar, the operator words in any case, the reserved `tagma`
    namespace, a declared enum or range (non-finite refused) and
    `type=semver` (a strict SemVer 2.0.0 parser written here). Removed
    tags are never checked. A scalar target's new value displaces the
    task's old one in the application, before `Retag`: decide never reads
    the schema.
  - The tag schema (Go's `tagschema`): the seven facets, immutable, the
    default declaring nothing; Go's `DefaultTagSchema` is the baseline a
    project's declarations extend, the last of a facet and target
    winning. It comes through the `Configuration` port from
    `tasks-config`: `~/.taskloom/config.yaml`, then the work root's, then
    `TASKLOOM_CONFIG_*`, then `--config-set`, a layer's `tag_schema`
    replacing the list below it, as Go's does.
  - `list --tag-query`: tagma-core's postfix engine, then its infix one
    when postfix cannot read the query, over the listing's candidates
    with Go's presence tag, type declarations and SemVer comparator.
    Every operator agrees with Go's: `fixtures/tag-queries/cases.tsv` is
    answered by Go's own `filterTasks` (`scripts/compare-go-tag-queries.sh`,
    writing `go.tsv`) and tagma-core alike, and no tagma-core change was
    needed.
  - Display hiding: tagma's `HideConfig` from the schema's hide facts,
    `tagma:*` hidden by default, in text; JSON keeps every tag, as Go's.
  - The importer converts at its boundary: a Go tag that does not parse is
    reported against its record and left out, and a retag takes the task
    from Go's tags before to Go's tags after, as tags, so a tag Go still
    holds under another spelling (`sig:"a.b"` beside `sig:a.b`, in a real
    log) stays.

Where the skeleton differs from Go, or from this plan:

- **Resolution reads the marker first, deliberately** (decided
  2026-10-21). The committed `.project-id` is authoritative and the
  registry heals toward it: `--project`, `CTXLOOM_PROJECT_ID`, the marker,
  Go's older marker, then the registry entry for the root. Go looks the
  path up in the registry first and treats the marker as advisory, warning
  when they disagree.
- Go's `Resolve` heals on its own (a missing marker rewritten, an unknown
  marker adopted, a proven move repointed, a copy forked). A Rust write
  only adopts Go's older marker or mints; the rest is `project heal` and
  `project repoint`, which report what they would change and make it only
  with `--yes`. Go has neither command.
- A linked worktree opts out of its primary checkout's project with a
  marker of its own (`.project-id`, or Go's `.ctxloom/project-id`); in Go,
  with a `.ctxloom` directory of its own.
- The registry's lock is waited for at most 10s; Go waits without end.
- `project heal` does not yet run the store's integrity check (see "Open").
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
- A read opens its store read-only (annals-store-sql's `open_existing`)
  and creates nothing; `list` of a project with no store yet is empty, and
  `tag` resolves its project as a read does. A read-only open of a file a
  writer has created but not yet migrated or moved into WAL fails, so the
  opener then opens that existing file to write, which waits on the
  writer's lock.
- annals-store-sql's `SqliteStore::open` future cannot be proven `Send`
  (sqlx's migrator fails the higher-ranked `Acquire` check), so
  task-store-sqlite drives it with `block_on` on a blocking thread. annals
  should make it provably `Send`; the workaround then goes.
- Work roots and registry entries are compared canonical, as Go's
  `cleanPath` (symlinks resolved, else cleaned lexically), and the
  canonical path is registered. A status starting with `@` is refused in
  `list --status` as on writes.
- harp-core has one word list, Go's "long" group, so project ids are three
  words from it, not from Go's smaller default group.
- The importer brings over only what the tasks domain accepts. What Go's
  fold kept but the domain refuses (empty text, Deferred with no trigger,
  a status that is empty or starts with `@`, a blank harp) is reported,
  not imported, as is every later record to a task whose add was refused.
  A Go status is kept raw and a Rust one trimmed, so `" Done "` imports as
  `Done` unreported. An unresolved displaced add is reported and not
  imported: Go's `taskloom repair`, then a re-import, brings it (Decided
  5 drops `repair` here). Go decodes JSON keys case-insensitively; the
  importer reads them as Go writes them. It takes no lock on a Go log, so
  a log Go is mid-way through appending to can end in a torn line, which
  stops that project's import; run it again.
- `list` with no project here says so and lists nothing; Go falls back to
  `--global`. The `@open`/`@terminal` classes are not taken, and JSON has
  no `derived_priority` and escapes no HTML: they come with statuses and
  priority.
- Tags, against Go: a refused write names its first refused tag, where Go
  names every one, and there is no `--degraded` to write past a refusal.
  An enum or range declaration that cannot be read refuses the schema when
  it is parsed (Go refused only when a tag met it). A tag spelled two ways
  is one tag. An `and`/`or`/`not` key is refused however it is quoted. An
  environment or `--config-set` list does not split inside double quotes
  (Go split an enum declaration's members apart), and an override naming
  no key is refused (Go wrote it, and then its schema refused it).
  `--tag-query` takes infix as well as Go's postfix. `--write-upgrades` is
  not ported: there is no config version to upgrade from.
- angzarr-build maps only angzarr's own packages by absolute path
  (`extern_path`), so `tasks-proto` nests its generated module as its
  package is (`ctxloom::tasks::v1`) and re-exports tagma-proto's `v1` at
  `tagma`, where prost's relative path lands; the domain layer finds `Tag`
  re-exported in `tasks_proto::v1`. An `extern_path` argument in
  angzarr-build would retire both.

## Open, and the step each waits for

Found in the walking skeleton's review and since; each is settled at the step named,
not filed as a task.

- **`project heal` runs no integrity check of the store.** SQLite's
  `PRAGMA integrity_check` has to come through the store adapter, and
  annals-store-sql offers none: it needs a check on `SqliteStore` in annals
  (ctxloom writes no SQL, and task-store-sqlite links no sqlx of its own).
  Then a port for it, called by heal. Waits on that annals change.
- **A clone with the committed marker writes to the original's database
  unasked.** The marker is authoritative, so a second clone on the machine
  resolves to the same id, and a write goes to the same database with no
  refusal. `project show` reports it ("shared") and `project heal` will not
  register it, but nothing stops the write, and there is no command to give
  the clone an id of its own (repoint binds only to an existing id).
  Decided 4 means repoint's refusal to keep them apart; a write needs the
  same. Settle before MCP serves sessions (step 4).
- **Store reads are not one snapshot.** annals' `read_from` should run as
  one read transaction, and annals should gain a read-only unit, so that
  `covers` and the reads after it see one snapshot. Needed before purge
  and prune (step 5, "Purging a stream in annals"): otherwise a purge in
  the middle of a listing yields a blank task.
- **`TaskService` takes `&mut self`**, so an MCP server holding one service
  serializes every call. Revisit with MCP (step 4), before its tools
  multiply (step 5).
- **annals exports no catalogs.** Its crates' `.ftl` files are not
  reachable from a dependent, so an annals error (a store failure, a
  kernel error) prints its English `Display`, in text and in the envelope's
  `message`. annals should export each crate's en-US catalog (as ctxloom's
  crates export `EN_US`); the binary then embeds them.
- **JSON escapes no HTML** where Go's encoder escapes `<`, `>` and `&`:
  normalize it when comparing the Go and Rust binaries for parity ("Parity
  and cutover").

## Order of work

Per CLAUDE.md: the highest-risk pieces first, each slice the smallest
complete unit that retires a risk, and each reviewed by an architect before
the next. A slice cuts through every layer it needs; the sections after this
one are the reference for what each layer finally holds, not an order.

Step 1, the importer, step 2, healing and repointing a project's store,
and step 3, tags on tagma, are done (see "Where it stands"); step 2 leaves
`project merge` and the integrity check (see "Project identity: heal and
repoint" and "Open").

4. **MCP and the companion contract**: `taskloom mcp` with `task_list` and
   `task_add`, plus `loadout` and `version --format json`. Risk retired:
   ctxloom loading the Rust binary as its companion. After this the Rust
   taskloom can serve real sessions.
5. **The remaining commands and tools**: status, edit, show, summary,
   tags, statuses, prune and lint (which reads the tag schema), and the
   remaining MCP tools and resources.
6. **Priority**: choosing or writing the formula evaluator. It is isolated
   from everything else, so it can move earlier if the evaluator looks
   risky.
7. **`manage`, `watch` and `run`**, then cutover.

## Tags on tagma

Done (step 3, "Where it stands"). tagma is the one tag model for taskloom,
ctxloom and ltk alike: a later domain that tags (bundles, sessions, ltk
rules) imports `tagma.v1.Tag` as `tasks.proto` does. What remains is
outside this repository: merging tagma's `tag-proto` branch, then pointing
Cargo.toml's path at tagma's primary checkout or a pinned commit.

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
  `CTXLOOM_PROJECT_ID`, then the marker (`.project-id`, adopting Go's
  `.ctxloom/project-id`), then the registry
  (`~/.ctxloom/projects/index.yaml`). A write adopts or mints; a read only
  looks up; heal and repoint are commands. A linked worktree goes to its
  primary checkout unless it has its own marker. The work root is
  `CTXLOOM_ROOT`, then the git root, then cwd.
- Configuration (`tasks-config`, done as far as the tag schema needs it):
  `~/.taskloom/config.yaml`, then `<root>/.taskloom/config.yaml`, then
  `TASKLOOM_CONFIG_*`, then `--config-set`. It carries `schema_version`
  (`--write-upgrades`, not ported) and `tag_schema` extending the default
  schema, where the last declaration wins.
- The importer for Go's jsonl logs (Decided, 2): `go-task-log`.
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

The identity logic is shared (`project-application` over the
`ProjectAdmin` port, `project-cli` for the commands), and taskloom presents
it as the `project` group; ctxloom can mount the same. `show`, `heal`,
`repoint` and the orphaned databases are done (see "Where it stands").
What remains:

- `project heal` running the store's integrity check (see "Open").
- `project merge <project-id>`: replay another project's task streams into
  this one, for a fork that gathered tasks on both sides. A harp held on
  both sides is re-minted, and the result says so. Not started: it needs a
  tasks use case reading one project's store into another's, beyond the
  identity ports.

## Driving adapters

- CLI: the Go command set and flags (inventory below). Exit codes 0 and 1.
  Warnings and errors carry a fix line.
- **Two formats, decided**: JSON, for jq and other programs, and UTF-8
  text, for people; JSON when stdout is not a terminal. Go's yaml, toml and
  markdown (and any ASCII fallback) are dropped until something needs them;
  one more `Format` variant and its render arm would add one. JSON exists
  for jq: stdout carries exactly one document per invocation and nothing
  else (notices such as the project a write went to are on stderr); a
  stream (`watch`) is JSON Lines; field names are stable, in a fixed order,
  with absent values explicit (`[]`, `""`, `null`). An error in JSON is a
  `{code, args, message}` envelope on stderr, a nested cause an envelope in
  its turn, and the exit code is 1; in text it is the catalog message.
- **Presentation's shape, decided.** Formats are not core ports: the core
  returns typed values and `TaskService` is the driving seam (`watch`
  returns a stream of typed changes, not a push into a presenter).
  `adapters/driving/presentation` is domain-free, so ctxloom's binary and
  later domains reuse it (a test keeps every workspace crate but
  annals-diagnostic out of its dependencies). It owns format selection
  (`Format::choose`, from the flags and the terminal), the `Render` trait
  and the JSON writer, the Fluent `Catalog` (each crate exports its
  embedded en-US `.ftl` as `EN_US`; the composition root hands the driven
  adapters' to the CLI's), the diagnostic envelope, and display-width text
  helpers (`unicode-width`). Domain shapes and views (`TaskJson`, the task
  table, a written task's line) stay in `tasks-cli`, and the project
  group's (`ShowJson`, `PlanJson`, the orphans) in `project-cli`, each
  implementing `Render`; a mounted group gets the binary's catalog through
  `run_mounted`, so its errors render as the task commands' do. MCP (step 4) needs `TaskJson` too, so it moves then into a tasks-specific
  presentation crate both use. Output goes to an injected `&mut dyn Write`.
  annals' crates export no catalog yet, so their codes render their English
  `Display` until they do.
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
