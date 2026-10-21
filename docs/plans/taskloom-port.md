# Plan: finishing the taskloom port

Written 2026-10-20. The Go taskloom is `cmd/taskloom` and
`internal/shared/tasks` (with `internal/taskloom/{config,engine,workdir}`) in
the Go ctxloom repository; about 12.9k lines. This plan ports it onto annals
and angzarr here.

Steps 1 to 7 are done: the importer, healing and repointing, tags on
tagma, MCP and the companion contract, the remaining commands and tools,
priority, and `manage`, `watch` and `run`. What remains is parity and the
cutover ("What remains").

## Where it stands

- `proto/ctxloom/tasks/v1/tasks.proto`: the task aggregate, its commands and
  events. Its tags are tagma's `tagma.v1.Tag`. Events carry the values the
  task holds after them, never a change to recompute: apply only assigns
  (angzarr's convention, its `operations/error-recovery.mdx` and
  `FundsDeposited.new_balance`).
- `core/wire/tasks-proto`: the tasks domain's wire types (`ctxloom.tasks.v1`),
  generated from that proto.
- `core/domain/tasks`: decide and apply for add, set-status, edit, retag and
  remove, with localizable rejections.
- `core/read-model/task-lists`: the list projector, with a resumable cursor,
  caught up over the sync store port, or over the async one's read-only
  unit of work, one moment of the store. It keeps, per stream, its last
  projected sequence and its first event's fingerprint (the cause, a
  command id, and when it was recorded), so a task minted on a harp a
  prune freed, whose stream restarts at sequence 0, is seen as a new
  stream and listed, even when the prune, its purge and the mint all
  landed between two catch-ups; a stream read back with no events lists
  nothing. It names each task a prune removed whose stream is still held
  (`unpurged`), for a prune to finish. A catch-up drops each task whose
  stream the store no longer holds, and forgets the stream: a prune that
  landed whole since a long-lived list last read.
- `core/ports/ports`: project identity and its survey (`ProjectAdmin`), the
  store opener, `PurgingStore` (a store whose store and units of work
  implement annals' opt-in `AsyncPurge`, as SQLite's do), `WatchedStores`
  (a store's change token), names, the clock, command ids, waiting with
  jitter, configuration, Go's task logs (`GoTaskLogs`), the store's
  integrity check (`StoreCheck`) and agents' MCP configurations
  (`AgentConfigs`). Every port future, and every use case's, is `Send`, so
  a server can spawn them; project identity is synchronous and blocks on
  the registry's lock, so an async server calls it through
  `spawn_blocking`.
- `core/application/tasks-application`: add, status, edit, tag, list,
  show, summary, statuses, tags, lint, prune and import, and the tag schema
  (which `tag_schema` answers, resolving no project), write-time
  admission and tag queries. An add mints a harp and re-mints on
  `TaskExists` (100 tries, a fresh command id each); on annals'
  `Recovery::Retry` the same command id runs again in a new unit after
  capped exponential backoff with full jitter (10ms doubling to 1s, 10
  attempts). A status change, an edit and a tag resolve their project as a
  read does (no project, or no store, is no task). Every read goes through
  one scan of the project's live tasks, or with `--global` every project's
  with a store (by project id, then harp id, as Go's); with no project,
  outside any work root (no git repository, no CTXLOOM_ROOT), it reads
  every project and says why, as Go's does, and inside one it reads
  nothing. A list takes statuses and the classes `@open` and `@terminal`,
  a term, a tag query, `--all`, `--limit` and `--compact`, and hides
  terminal and Deferred tasks unless `--all` or a status is given, saying
  on stderr what a search's default view hid and what a limit left out.
  `show` needs every id found (and, across projects, held once); `summary`
  counts every task whatever a listing would filter; `statuses` is Go's
  five then every other in use; `tags` counts active and total over what
  its filters select, the schema's hidden tags left out; `lint` (Go's
  `internal/shared/tasks/lint`) sweeps every task and the schema's
  formulas, each finding a diagnostic, and exits 1 on a violation or when
  it checked no target.
  - Prune (`PruneService`, opt-in on a `PurgingStore`) is Go's: it selects
    live tasks by status (at least one; Done by default), by their status
    last changing strictly before a cut (Go's `agebound`: an offset in
    Go's duration grammar plus `d` and `w`, or a `YYYY-MM-DD` date at
    midnight UTC; 7 days by default), and by an optional tag query, in the
    order added. Without `apply` it reports; with it, each task is removed
    (`TaskRemoved{PRUNED}`) in a unit of work that first reads the task
    and declines if it no longer qualifies (moved, retagged, removed or
    purged since the selection), then its stream is purged in a unit that
    first checks the stream still holds the removed task, so a task minted
    on the freed harp meanwhile is never purged; both retry as a command
    does. It reports the events purged (the removal included, as the store
    counts them; a report predicts the same), the tasks left, and every
    dangling reference a survivor holds: a tag carrying the harp as a word
    (`relates:<harp>`), or the harp as a word of its text or trigger.
  - `WatchService` (its own trait beside `TaskService`): a watch resolves
    its project as a read does and holds the task-list projector over
    every task; each poll asks the store's change token first
    (`WatchedStores`: annals' `ChangeToken` and the handle that answered
    it), and only when it moved catches the projector up and answers the
    changes past its cursor, the projector's own sequence. A project with
    no store yet answers nothing until one appears (a look at the
    filesystem each poll). A prune landing whole between two polls is seen
    from the stream's absence (a removal); a poll between its removal and
    its purge answers the removal and later polls nothing new; a prune and
    a mint on its harp between two polls is the new task. The binary's
    in-process watch tests force each, tick by tick.
- `core/application/project-application`: `show`, `heal`, `repoint` and the
  orphaned databases, planned from the port's survey; it depends on no
  tasks crate. The marker is authoritative and the registry heals toward
  it. A second live tree holding the same id (a clone with the committed
  marker) is the same project (Decided 4): `show` lists it under
  `shared_with`, information and not a problem, and `heal` registers the
  tree beside it. `repoint` onto an id another live tree holds needs
  `--share`, unless the tree already resolves to that id. A change is reported, and made only when asked, under the registry's
  lock and only while a fresh survey matches the one it was planned from.
  `heal` first checks the resolved project's database through the
  `StoreCheck` port and reports damage as an unresolved finding (the
  store's diagnostic), never repaired; no database is no damage, and a
  check that cannot run fails the heal before anything changes.
- `core/application/registration-application`, over the `AgentConfigs`
  port: `manage install|uninstall|check` (Go's `manage`), choosing the
  agent `--engine` names or every one in use at the scope.
- `adapters/driven/project-registry`: the work root, `.project-id` (adopting
  Go's `.ctxloom/project-id` on a write), the registry in Go's schema, and a
  mint under `index.yaml.lock` that re-checks first, so racing first writes
  bind one id. A read never mints. Resolution reads the marker first, then
  Go's older marker, then the registry (Decided 8). A linked worktree with
  no marker of its own is its primary checkout's project, found through
  git's common directory (the `.git` file's `gitdir`, then its
  `commondir`), and is bound and marked there, never in the worktree; one
  whose git directory is gone is an error, as in Go. The lock is
  file-lock's (below). It also implements `ProjectAdmin`: the survey, and the
  changes (markers, registry entries, and removing
  `~/.ctxloom/projects/<id>/`, never through a link).
- `adapters/driven/file-lock`: ctxloom's advisory locks, taken as Go's
  `safefs` takes them, for any adapter to use (it is no port's: the
  registry and agent-configs each take it inside their own operations).
  `flock(2)` on a lock file, shared or exclusive, refused when it is not a
  regular file; taken again when what it was granted is no longer the file
  at its path (Go's `takeCurrent`, 16 tries: lock protocol 2); waited for
  at most 10s, then refused naming the lock. And Go's
  `sessions.WithFileLock` for a file ctxloom does not own: the sidecar
  `paths.HomePathFor` names (`~/.ctxloom/locks/<FlatName(abs path)>.lock`,
  or a session's own `<harp>/locks/` for a file inside a session's
  directory), recording the path it guards when it creates it, taken with
  the reap interlock (`~/.ctxloom/locks/reap.interlock`) held shared when
  it is in the home lock directory, a reap that holds it past 30s refusing
  the lock. Its paths are held to Go's own `HomePathFor`'s output.
- `adapters/driven/harp-names`: harp-core behind the names port.
- `adapters/driven/go-task-log`: Go's `~/.ctxloom/tasks/<project-id>.jsonl`
  behind the `GoTaskLogs` port, read only (not even its lock file). A line
  Go's fold would refuse (malformed, no op, an unknown op) refuses the
  whole log, as Go's fold does.
- `adapters/driven/task-store-sqlite`: annals-store-sql's SQLite store at
  the project's path, opened on first use and held, behind the store
  opener; it purges a stream through annals-store-sql's purge (a
  `PurgingStore`), and answers a watch annals-store-sql's change token
  (`PRAGMA data_version` with `total_changes()`), paired with the opening
  of the store that answered it. And `StoreCheck`, annals-store-sql's
  `integrity_check` (SQLite's `PRAGMA integrity_check`) on a fresh
  read-only open. The port is synchronous, as the project commands are, so
  the check runs on a runtime of its own on a thread of its own (the
  binary's caller is already on a runtime).
- `adapters/driven/system`: the clock, UUIDv7 command ids, tokio's timer and
  jitter.
- `adapters/driven/tasks-config`: taskloom's configuration behind the
  `Configuration` port, as far as the tag schema needs it.
- `adapters/driven/agent-configs`: Claude Code's `~/.claude.json` and
  `<dir>/.mcp.json` behind `AgentConfigs`. One member is written or taken
  out by splicing bytes as hew (Go's patcher) does, so every other byte
  stays; an entry already there is taken out first only when it runs
  taskloom (Go's `exectoken`), and one running anything else is refused.
  A file that is not JSON is copied to `<path>.corrupt-<unix>` and left
  alone. The whole read-modify-write, a dry run's included, runs under
  file-lock's `lock_file` on the file, the lock Go's confpatch takes, so
  Rust and Go writers of one file take turns. `scripts/go-manage-fixtures.sh` runs Go's binary over 29 files in
  temporary homes (`fixtures/go-manage`); the parity tests hold the adapter
  to Go's bytes, refusals and copies, for install, recordless uninstall,
  uninstall after Go's install, and check.
- `adapters/driving/presentation`: the driving edge's presentation for any
  domain (Decided 9 and 10, "Driving adapters").
- `adapters/driving/tasks-presentation`: the tasks domain's shapes on the
  driving edge (`TaskJson`, each tag in its canonical spelling, hidden ones
  kept, and RFC 3339 instants; a compact row, a task with its project, a
  summary and a tag count, each Go's), shared by the CLI and the MCP
  server, so one jq filter reads both.
- `adapters/driving/tasks-cli`: `add`, `status`, `edit`, `list`, `show`,
  `summary`, `statuses`, `tag`, `tags`, `lint`, `prune` (Go's flags and
  defaults, `--yes` to act; Go's report in text, one JSON document in Go's
  `pruneReport` shape, and on stderr, after a report that would take
  something, the exact command that applies it) and `import`, with
  `--config-set`, text and json in Go's shapes, its errors and notices
  rendered from the catalogs, and `run_mounted`, which flattens another
  crate's commands in beside them and hands one parsed to its caller with
  the global flags (the format and `--project`; not `--config-set`) and
  the catalog. Text hides the tags the schema hides; JSON keeps them, as
  Go does. Also the companion contract: `version` (`--format json` is
  Go's `{name, version, lock_protocol}`, the lock protocol file-lock's
  `LOCK_PROTOCOL`, 2, true of every lock file taskloom takes: the
  registry's and `manage`'s, each re-checked as Go's `takeCurrent` does) and
  `loadout`, Go's `cmd/taskloom/loadout.yaml` with the reviewed factual
  changes (Decided 11), printed in yaml alone (`--format yaml` is accepted by
  `loadout` only).
- `adapters/driving/tasks-mcp`: `taskloom mcp`, over rmcp (the official
  Rust MCP SDK, pinned at 3.5.1: the newest release at least two weeks old
  on 2026-10-21; server and stdio transport only, no macros). It serves
  Go's five tools (`task_add`, `task_edit`, `task_list`,
  `task_set_status`, `task_tag`) and two resources
  (`taskloom://tag-schema`, `taskloom://tag-vocabulary`), with Go's
  instructions, capabilities (logging, and `listChanged` on tools and
  resources; a logging level is accepted and nothing is logged), tool
  descriptions and input and output schemas and resource descriptions
  copied from Go (the instructions and tool descriptions with the reviewed
  factual changes, Decided 11; the rest verbatim), and Go's result shapes
  on tasks-presentation's.
  `task_list` serves every parameter Go's does; outside a work root with
  no project it reads every project, with Go's `global`, `project_count`
  and `notice` (the command line's stderr notice, word for word). The
  resources are read fresh each time, through `tag_schema` and `tags`,
  and their documents are Go's byte for byte (indented as Go's encoder
  indents, its float form and HTML escaping included). Writes' tags are
  parsed and admitted under the configured tag schema as the CLI's are.
  An error is a tool error whose text is the diagnostic's envelope; an
  unknown tool is a protocol error carrying it, an unknown resource a
  resource-not-found one, and a resource that cannot be read an internal
  one. Each call runs on a service of its own, made by the composition
  root, on a blocking thread (Decided 7).
- `adapters/driving/tasks-watch`: `taskloom watch` (hidden, as Go's), Go's
  line `{"event":"changed","kind":"tasks","project":<id>}` as JSON Lines
  (presentation's `write_json_line`), once at the start and once per burst
  of commits: the use case is polled each tick (100ms, Go's
  `watchDebounce`) and a burst gets its line at its first quiet tick, or
  ten ticks in when it does not stop, as Go's debounce and ceiling. It
  ends at SIGINT or SIGTERM (tokio's `signal` feature), as Go's, never
  at a closed stdin (the VS Code extension starts it with stdin from
  /dev/null), and with why when a line cannot be written. Its tests drive
  every tick and script every poll; the binary's run it with stdin from
  /dev/null and see a second line from a live process.
- `adapters/driving/tasks-run`: `taskloom run [harp] [--no-start]`: the
  task named (any status), or Go's line-buffered picker on a terminal,
  handed to `ctxloom run [--session <origin>] --seed-task <harp>
  [--seed-status "To Do"] --prompt "Work on this task (`<harp>`): <text>"`,
  Go's argv, through a `Launch`; the binary's runs the `ctxloom` on PATH
  with the process's stdio. Tests run only stub scripts alone on a PATH.
- `adapters/driving/registration-cli`: the `manage` group. The binary
  mounts it with its words unparsed, and it reads them with its own
  parser: Go's `--project` is a switch there and a project id elsewhere.
  Notices on stderr as Go's (text and JSON); in JSON one document on
  stdout; `check` Go's table in text.
- `adapters/driving/project-cli`: the `project` group (`show`, `heal
  [--yes]`, `repoint <project-id> [--share] [--yes]`, `orphans`, `orphans
  remove <id>... [--yes]`), text or one JSON document each, its errors and
  notices rendered from the catalogs, for any binary to mount; taskloom
  does, and ctxloom can, since it names no tasks crate.
- `root/bin/taskloom`: the binary, over annals-store-sql's SQLite store at
  `~/.ctxloom/projects/<project-id>/state.db` (CLAUDE.md, "Storage"). It
  mounts the `project` group over the project registry, `mcp` (served
  once the command line is read, each call's service the CLI's own wiring
  made fresh), and `watch`, `run` and `manage`. Its tests are Go's
  `crossprocess_test.go` ported, each process separate on one database,
  and they build the binary themselves: 40 concurrent adds at a project's
  first sight (racing the mint and the migration; one registry entry), 20
  adds among 10 listings of a seeded project (every row a whole task of
  the final listing), the same at first sight, and 40 concurrent tags of
  one task. Reads create nothing: `list --project tpyo` leaves no trace,
  and `tag` where there is no project binds none. `taskloom mcp` is run as
  a client runs it, in a temporary home: initialize, tools/list and
  resources/list against Go's, every tool called, both resources read (the
  tag schema against Go's document), the fallback outside a work root with
  the CLI's notice, and the CLI reading the same store; `version --format
  json` is fed to a port of Go ctxloom's boot-probe parse and its lock
  reaper's check, and `loadout --format yaml` is compared with Go's byte
  for byte. `prune` reports, then with `--yes` removes and purges, naming
  what dangles, each answer one JSON document.
- The companion text: the MCP instructions, the capabilities, the tools
  and resources as Go's tools/list and resources/list answer them, both
  resources' documents (the tag schema with nothing configured, the
  vocabulary of a seeded project), and the loadout document are fixtures
  `scripts/go-companion-fixtures.sh` extracts from the Go source (a Go
  test laid into `cmd/taskloom` with `go test -overlay`); the parity tests
  hold Rust's copies to them byte for byte (the instructions, tool
  descriptions and loadout with the reviewed changes applied, Decided
  11), so a change in Go fails a test until the copy is taken up
  deliberately.
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
  with tags as tagma's: every task agrees, and all 5200 tags of live
  tasks parse.
- Healing and repointing: `taskloom project`, its acceptance tests the
  binary in a temporary home. A copy that forked its own id is repointed
  to the original's and finds its tasks, its own database then reported
  orphaned and removed by name; a lost marker is shown and healed from the
  registry; a linked worktree made by `git worktree add` shares its
  primary checkout's tasks and gets no marker; a second clone given the
  first's marker reads and writes the same tasks at once, `show` names the
  other tree as sharing, and `heal` registers it. `heal` over a database
  whose index was damaged in a copy (annals-store-sql's approach) reports
  the damage, code and args, in text and in the one JSON document, and
  leaves the file as it was; a sound database and a missing one are no
  damage.
- Tags on tagma. tagma's repository (github.com/benjaminabbitt/tagma)
  gains `proto/tagma/v1/tag.proto` (`optional string namespace`, `string
  key`, `optional string value`) and `crates/tagma-proto` (prost 0.14,
  generated at build time, conversions both ways with `tagma_core::Tag`,
  `PROTO_INCLUDE` for importers), and tagma-core a `Display for Tag`, the
  canonical spelling Go's port already had.
  tagma is the one tag model for taskloom, ctxloom and ltk alike: a later
  domain that tags (bundles, sessions, ltk rules) imports `tagma.v1.Tag`
  as `tasks.proto` does. Tags on `TaskState`, `AddTask`, `Retag`,
  `TaskAdded` and `Retagged` are `repeated tagma.v1.Tag`; the domain
  dedupes them by tag and sorts them by canonical spelling, as Go sorted
  its strings.
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

Where the port differs from Go:

- **Resolution reads the marker first** (Decided 8): `--project`,
  `CTXLOOM_PROJECT_ID`, the marker, Go's older marker, then the registry
  entry for the root. Go looks the path up in the registry first and
  treats the marker as advisory, warning when they disagree.
- Go's `Resolve` heals on its own (a missing marker rewritten, an unknown
  marker adopted, a proven move repointed, a copy forked). A Rust write
  only adopts Go's older marker or mints; the rest is `project heal` and
  `project repoint`, which report what they would change and make it only
  with `--yes`. Go has neither command.
- A linked worktree opts out of its primary checkout's project with a
  marker of its own (`.project-id`, or Go's `.ctxloom/project-id`); in Go,
  with a `.ctxloom` directory of its own.
- Every lock (the registry's, `manage`'s) is waited for at most 10s; Go waits without end.
- Concurrent writers are serialized by SQLite itself: `BEGIN IMMEDIATE` per
  unit of work and sqlx's default 5s busy timeout. With retries switched off
  the acceptance tests still pass, so the backoff is a guard, not what makes
  them pass.
- A new database opened by several processes at once failed its migration
  ("table events already exists"): sqlx takes no migration lock on SQLite.
  annals-store-sql now migrates under `BEGIN IMMEDIATE`.
- Lists, prune's selection and the read-back after a write read through
  annals' read-only unit of work (`AsyncReadStore::begin_read`, one SQLite
  read transaction), which every
  `TaskStores::Store` provides: a domain's `covers` and every stream read
  after it are one moment, so a write or a purge by another process in
  between is not seen (tested by forcing one there).
- A read opens its store read-only (annals-store-sql's `open_existing`)
  and never creates, migrates or writes; `list` of a project with no store
  yet is empty, and `tag` resolves its project as a read does. A file a
  writer has created but not yet migrated or moved into WAL is waited for
  by annals-store-sql (up to its 5s busy timeout); one that stays empty, or holds a database with
  migrations pending, is refused as unmigrated until a write migrates it.
- Work roots and registry entries are compared canonical, as Go's
  `cleanPath` (symlinks resolved, else cleaned lexically), and the
  canonical path is registered.
- harp-core has one word list, Go's "long" group, so project ids are three
  words from it, not from Go's smaller default group.
- A read with no project falls back to every project only outside a
  work root, as Go's; inside one (a git repository's first call) Go mints
  the project and lists it empty, and here nothing is minted and the read
  says there is no project yet.
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
- Priority (`core/read-model/task-priority`, on expr-lang; Decided 12),
  against Go's: an unknown name in a formula is refused when a task is
  scored, not when the formula compiles, and one in a branch no task
  takes is never refused. `--limit` with `--sort priority` keeps the
  highest priorities; Go's single-project listing cuts the page before it
  sorts, so it keeps the first added, re-ordered. Across projects, a tie
  within a project keeps harp-id order (Go's keeps log order). A warning
  is a diagnostic (`tasks-application-priority-*`), a line on stderr or
  one of `priority_warning`'s lines. Every task's JSON, the CLI's too,
  carries `derived_priority`, null unless sorted by it, a whole number
  written without a fraction as Go writes it.
- `taskloom mcp`, against Go's: a result's `path` is the project's store
  (`state.db`), not Go's jsonl log; there is no `refused` (no
  `--degraded`) and no project-resolution `warning` but "no project
  here". An explicit `global` carries no `notice`: Go's always names
  repo-homed projects as out of its reach, and there are none here
  (Decided 3), as the CLI's `--global` says nothing either. The
  vocabulary resource reads as `tags` does, so outside a work root with
  no project it counts every project and says why in its `warning`;
  Go's counts nothing there. Neither the tools nor the resources take
  the launch's `--config-set` (Go's handlers see the launch flags): each
  call's configuration is the files' and the environment's. A tool
  result's text is serde_json's, escaping no HTML (see "Open"); the
  resources' documents escape as Go's do. Go's MCP results happen to
  have their keys sorted; Rust's do too (serde_json's map), though
  nothing depends on it.
- The loadout is Go's document with the reviewed factual changes
  (Decided 11), its `command: taskloom` included. Its two install texts
  are placeholders until the human writes them ("Then parity and the
  cutover"). Every tool and resource it and the MCP instructions name is
  served.
- Prune, against Go: `--status` refuses `@open`/`@terminal`, which `list`
  takes, until prune takes the classes too. A report counts each task's
  events plus the removal a prune records before it purges, which is what
  the store reports when it acts; Go counted log lines. Go rewrote its log
  under its write lock; here each task is removed and purged in units of
  its own, and one that no longer qualifies when its unit reads it is
  left. A purge that fails for good leaves its task removed, not purged:
  no list shows it, but its stream and harp stay until the next prune,
  which reports it and with `--yes` purges it, whatever that prune
  selects by (the projector names each stream whose last event is a
  prune's removal). Go has no `remove` command (its `Store.Remove` is
  called by none), so none is ported; the domain's removal by name is
  there for one.
- `version` reports the crate's version (`0.1.0`), not Go's versionator
  stamp; its yaml form is not ported.
- `@open` is every status not terminal, a status outside the default set
  included; Go's expands to the default set's non-terminal names only.
  `statuses` lists the default five, then every other status in use where
  the read reaches, by name; Go's lists its five alone. Both read the
  project, so `statuses` takes `--global`, and so does `lint`, which Go
  runs over the one project.
- Across projects a row's `project_dir` is the path the registry holds
  for its project (Go leaves it empty), and each table of a `--global`
  listing is headed by that label rather than the id alone.
- `lint`'s violations carry `project_id` and their finding's envelope
  (`finding`) beside Go's `harp_id` and `reason`, and its reasons are
  catalog messages, not Go's wording. Go's check for a stored tag that
  does not parse has nothing to find: every stored tag here is parsed.
- `show`'s ids not found are named in one message that agrees in number
  (Fluent's plural); the notices for what a search hid always give both
  counts.
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
- `watch` sources changes from the store, not the filesystem: each tick
  reads the store's change token (annals-store-sql's `PRAGMA
  data_version` with `total_changes()`, on the one connection the watch
  holds), and only when it moved re-runs the projector's catch-up, which
  re-reads every task stream (the projector skips what it has seen). A
  token that moves with nothing new answers nothing, so no line. A
  burst's line comes 100-200ms after its last commit (Go's: 100ms after
  its last file event).
- `run` hands the task to Go ctxloom's `run --seed-task`, which marks the
  task In Progress in Go's jsonl log, not the Rust store (Decided 1
  accepts the loss). A child that fails is exit 1 with an error naming
  its status, not the child's own exit code (Go's does not propagate it
  either).
- `manage` writes no application records (hew's §9.7 records, Go's
  memory of what it wrote): Rust has no hew. It reads Go's newest record
  for one fact, whether Go made the `mcpServers` object itself, and then
  an uninstall that empties it takes it out, as Go's would. Without that,
  Go's next install refuses the file as drifted (shown against Go's
  binary). With no Go record, an install and an uninstall leave
  `"mcpServers": {}` where the file had none, as Go's does without its
  record. It takes Go's lock on the file (file-lock's `lock_file`), waited
  for at most 10s where Go waits without end. An entry under the name that runs another
  program is refused (Go replaces it in place, then usually refuses on a
  byte mismatch). A parse error's detail is this parser's words, not
  Go's `encoding/json`'s, and names the file's own name where Go's says
  `.mcp.json` for every file.
- angzarr-build maps only angzarr's own packages by absolute path
  (`extern_path`), so `tasks-proto` nests its generated module as its
  package is (`ctxloom::tasks::v1`) and re-exports tagma-proto's `v1` at
  `tagma`, where prost's relative path lands; the domain layer finds `Tag`
  re-exported in `tasks_proto::v1`. An `extern_path` argument in
  angzarr-build would retire both.

## Open

Each waits on a human decision, or on the step named; none is filed as a
task.

- **JSON escapes no HTML** where Go's encoder escapes `<`, `>` and `&`:
  normalize it when comparing the Go and Rust binaries ("Parity and
  cutover").

## What remains

Per CLAUDE.md: the highest-risk pieces first, each slice the smallest
complete unit that retires a risk, and each reviewed by an architect before
the next. A slice cuts through every layer it needs; the sections after
this one are the reference for what each layer finally holds, not an
order.

Parity and the cutover:

- Port the behavior Go's tests pin: concurrent processes (no lost write, no
  duplicate harp), CLI/MCP notice parity, every project-scoped read taking
  `--global`, degraded mode, and launch flags reaching MCP handlers.
- Run both binaries on the same inputs and compare.
- Run the importer over every existing log, then switch the companion
  registration to the Rust binary (the human's). Risk retired: ctxloom loading the Rust binary
  as its companion.
- Fill the install placeholders; un-ignore the placeholder test
  (`test_no_served_text_carries_a_placeholder`, Decided 11).

Loose ends outside the steps:

- `project merge <project-id>`: replay another project's task streams into
  this one, for a fork that gathered tasks on both sides. A harp held on
  both sides is re-minted, and the result says so. Not started: it needs a
  tasks use case reading one project's store into another's, beyond the
  identity ports.
- Outside this repository: pinning annals and tagma by commit as git
  dependencies once pushed, in place of the local path dependencies.

## Purging a stream in annals

Done, in the annals repository, for Decided 6: a
purge on the store port, sync and async, deletes a stream's events, its
snapshots and the command ids recorded with them in one unit of work.
Stores that can purge opt in (`Purge`, `AsyncPurge`), so a store with no
purge stays append-only by type; the store contract's purge section,
`InMemoryStore` and annals-store-sql have it. annals'
`docs/design/audit.md` records the cost: a command redelivered after its
stream is purged is decided again, so an `AddTask` redelivered after a
prune re-creates the task. Prune selects only tasks whose status is older
than its cut (7 days by default), so only a redelivery older than that can
do it, and that is accepted. How the task-list projector tells a stream
restarted on a freed harp is under "Where it stands".

## Driven adapters

- Store: annals-store-sql at `~/.ctxloom/projects/<project-id>/state.db`.
- Project identity, from Go `projectid`. Resolution order: `--project`, then
  `CTXLOOM_PROJECT_ID`, then the marker (`.project-id`, adopting Go's
  `.ctxloom/project-id`), then the registry
  (`~/.ctxloom/projects/index.yaml`). A write adopts or mints; a read only
  looks up; heal and repoint are commands. A linked worktree goes to its
  primary checkout unless it has its own marker. The work root is
  `CTXLOOM_ROOT`, then the git root, then cwd. A project's database is
  keyed by its id, so binding a directory to a different database is a
  change to the marker and the registry, and the database itself is never
  rewritten.
- Configuration (`tasks-config`, as far as the tag schema needs it):
  `~/.taskloom/config.yaml`, then `<root>/.taskloom/config.yaml`, then
  `TASKLOOM_CONFIG_*`, then `--config-set`. It carries `schema_version`
  (`--write-upgrades`, not ported) and `tag_schema` extending the default
  schema, where the last declaration wins. Priority's `priority_fn` and
  `decay_fn` come through it.
- The importer for Go's jsonl logs (Decided, 2): `go-task-log`.
- Harp minting: the same word lists as Go's `harp` (adjective-noun for tasks).
- Clock.

## Driving adapters

- CLI: the Go command set and flags (inventory below). Exit codes 0 and 1.
  Warnings and errors carry a fix line.
- Formats (Decided 9): JSON, for jq and other programs, and UTF-8 text,
  for people; JSON when stdout is not a terminal. Go's yaml, toml and
  markdown (and any ASCII fallback) are dropped until something needs them;
  one more `Format` variant and its render arm would add one. JSON exists
  for jq: stdout carries exactly one document per invocation and nothing
  else (notices such as the project a write went to are on stderr); a
  stream (`watch`) is JSON Lines; field names are stable, in a fixed order,
  with absent values explicit (`[]`, `""`, `null`). An error in JSON is a
  `{code, args, message}` envelope on stderr, a nested cause an envelope in
  its turn, and the exit code is 1; in text it is the catalog message.
- Presentation (Decided 10). Formats are not core ports: the core returns
  typed values and `TaskService` is the driving seam (`WatchService` is
  polled for typed changes, not pushed into a presenter).
  `adapters/driving/presentation` is domain-free, so ctxloom's binary and
  later domains reuse it (a test keeps every workspace crate but
  annals-diagnostic out of its dependencies). It owns format selection
  (`Format::choose`, from the flags and the terminal), the `Render` trait
  and the JSON writer, the Fluent `Catalog` (each crate exports its
  embedded en-US `.ftl` as `EN_US`; the composition root hands the driven
  adapters' to the CLI's), the diagnostic envelope, and display-width text
  helpers (`unicode-width`). Domain shapes and views stay out of it: a
  task's JSON (`TaskJson`) is `tasks-presentation`'s, which the CLI and the
  MCP server share; the task table and a written task's line are
  `tasks-cli`'s, and the project group's (`ShowJson`, `PlanJson`, the
  orphans) `project-cli`'s, each implementing `Render`; a mounted group
  gets the binary's catalog through `run_mounted`, so its errors render as
  the task commands' do. Output goes to an injected `&mut dyn Write`.
  annals' crates export their catalogs as ctxloom's do (`annals::EN_US`,
  `annals_store_sql::EN_US`), and the binary's catalog embeds those it
  links, so an annals error renders from its catalog too.
- MCP over stdio (`tasks-mcp`): the tools `task_list`, `task_add`,
  `task_set_status`, `task_edit` and `task_tag`, the resources
  `taskloom://tag-schema` and `taskloom://tag-vocabulary`, and the server
  instructions, which are normative (add only once the user agrees, and
  so on). A service per call (Decided 7).
- Companion: `loadout --format yaml` (hooks, the MCP server, the `taskloom`
  fragment), and `version --format json` with `lock_protocol`, which
  ctxloom's boot probe parses.
- `manage install|uninstall|check`, `watch` (a JSONL change stream,
  debounced) and `run` (`ctxloom run --seed-task`).

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
   one machine then hold the same id, and they are the same project: they
   share its store by design (decided 2026-10-21, "accept sharing as the
   default"). `project show` names the other tree as sharing, and `project
   heal` registers each. The accepted cost: an agent working in a scratch
   clone writes into the shared task list. Revisit if that bites; the
   remedy then is a way to give a clone an id of its own. `project
   repoint`'s refusal still guards a tree with another id (or none) moving
   onto an id a live tree holds, which merges two projects' work, so it
   asks for `--share`; a tree that already resolves to the id is not
   refused. The id is resolved through
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

7. **`TaskService` keeps `&mut self`; a server makes a service per call**
   (decided 2026-10-21, step 4). The MCP server holds a factory, not a
   service: each tool call gets a `Service` of its own (the registry, a
   fresh SQLite opener, names, the system), run to its end on a blocking
   thread (`spawn_blocking`, its future driven there by the runtime's
   handle), because project resolution is synchronous and may wait on the
   registry's lock. Nothing is shared between calls in the process, so no
   call waits on another's mutex while an add retries, and two calls in
   one process contend exactly as two processes do, in SQLite, which the
   cross-process tests already cover. Rejected: `&self` with interior
   mutability, which would put locks around the ports (the store opener
   hands out `&mut` stores) and restructure the service for no gain the
   per-call service lacks; one service behind a mutex, which would stall
   every call during one add's backoff. The cost is a store opened per
   call. `tasks-mcp`'s tests hold one add inside its retry (its first
   commit conflicts, its wait is held open) and show a listing answers
   meanwhile, and that resolution runs off the server's thread.
8. **Resolution reads the marker first** (decided 2026-10-21). The
   committed `.project-id` is authoritative and the registry heals toward
   it: `--project`, `CTXLOOM_PROJECT_ID`, the marker, Go's older marker,
   then the registry entry for the root. Go reads the registry first and
   treats the marker as advisory.
9. **Two formats: JSON and UTF-8 text.** JSON for jq
   and other programs, one document per invocation on stdout (a stream is
   JSON Lines), and the default when stdout is not a terminal; text for
   people. Go's yaml, toml and markdown, and any ASCII fallback, are
   dropped until something needs them (`loadout`'s yaml is its own
   document, not a format of answers).
10. **Presentation is domain-free.** `adapters/driving/presentation`
    knows no domain (no workspace crate but annals-diagnostic, held by its
    tests), so ctxloom's binary and later domains reuse it; a domain's
    shapes live with its driving adapters (a shared `<domain>-presentation`
    crate where the CLI and MCP server both write them). Formats are not
    core ports: the core returns typed values.
11. **The prose Rust taskloom serves is Go's text plus a reviewed factual
    diff** (decided 2026-10-21). The MCP instructions, the tool
    descriptions and the loadout are Go's, with the changes the human
    reviewed and approved where Go's text is false of Rust: the SQLite
    store path in place of the jsonl log, repo homing gone, `--format`
    taking text alone, and where the binary is built from and installed. The repository stays
    `github.com/ctxloom/ctxloom`, because the Rust code replaces the Go
    code there, with the crate path `crates/root/bin/taskloom`. The
    install text is pending: two placeholders stand in the loadout until
    the human writes it. The parity tests keep Go's fixtures
    (`scripts/go-companion-fixtures.sh`) and hold each change as an exact
    `(old, new)` pair beside them (`fixtures/reviewed.rs` in tasks-mcp and
    tasks-cli), applied to the fixture's raw bytes, each `old` found
    exactly once: a Go rewording of a changed sentence fails the pair, and
    any other Go change fails the byte comparison. Other wording is not
    changed: prompt text is the human's voice.
12. **`manage` keeps no hew records; no hew port for now** (decided
    2026-10-21). `manage` edits with its own byte-preserving JSON patcher
    and reads Go's newest record for one fact (whether Go created
    `mcpServers`). Accepted cost: after a Rust-only install then uninstall,
    an empty `"mcpServers": {}` stays behind, as Go leaves it when its
    record is lost. Revive when Rust ctxloom's own config writers need
    reversible edits (the sized port plan is in the session's plan
    directory, `hew-rust-port.plan.md`).
13. **`@open` means every non-terminal status, custom ones included**
    (decided 2026-10-21). Go's expands to its three default open statuses
    only; the parity comparison must normalize it.
14. **annals keeps WAL** (decided 2026-10-21). Snapshot reads (annals'
    read-only unit) need it: in rollback mode an open read blocks every
    writer. Accepted cost: a WAL store needs a writable directory even to
    read (SQLite code 1544), so an isolation mount must give taskloom a
    writable projects directory; that is the tooling's job.

12. **Priority's formulas run on expr-lang** (decided 2026-10-21), the
    Rust port of the `expr` Go evaluates them with
    (`github.com/jdx/expr.rs`, pinned `=2.1.0`, default features off).
    Its values matched Go's on the shipped formulas and the operators
    they use; it checks names only as a formula runs, and that is
    accepted (above, "Where it stands"). `task-priority` is the one read
    model linking beyond the core's crates (CLAUDE.md).

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
