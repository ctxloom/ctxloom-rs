# Companions

A companion is a program ctxloom runs to ask what it contributes to a session.
ctxloom runs only the companions you register, with the command you give it.
Nothing is searched for: there is no name-to-binary rule and no scan of the
`PATH`. (The operating system may find a bare program name when the command is
run, as it does for any command.)

## Registering one

    ctxloom companion add acme --command /opt/acme/bin/acme --as-team core

`--command` takes the program, then its arguments (everything after the
program is the program's; put `--format` before `--command`). ctxloom runs
`<command...> loadout --format yaml` once, and records the registration only
if the command exits successfully and prints a loadout.

    ctxloom companion list      # name, command, whether the program exists
    ctxloom companion remove acme --yes

`companion list` only checks that the program exists; it never runs a
companion. `companion remove` without `--yes` says what it would do.

The registration lives in your home config (`~/.ctxloom/config.yaml`); a
project's config may add names, and cannot remove one the home registers. The
command of a name registered in both is the project's.

## The config shape (schema 9)

    schema_version: 9
    companions:
      acme:
        command: [/opt/acme/bin/acme, --as-team, core]

Names are `^[A-Za-z0-9][A-Za-z0-9._-]*$`. `command` is the program, then its
arguments.

### Go compatibility

Go ctxloom reads schema 8 only, where `companions` is a list of bare names
(`companions: [ltk]`) that Go resolves to a binary by name. Schema 9 is not
readable by Go, and ctxloom does not write schema 8.

A schema 7 or 8 file that lists bare names is refused, naming the file and the
names, with exit 2: a name alone says nothing of what to run, so ctxloom does
not convert it. Register each companion again with `companion add --command`,
then remove the old list. A file with no companions (or an empty list) loads
as before and is read as schema 9 in memory; it is not rewritten.

## What a companion contributes

A companion's `loadout --format yaml` prints one document with a `run:`
section (a bundle, consumed every session) and an `init:` section (typed
fields consumed once at setup); either may be omitted, a document with
neither is refused, and a key the document does not model is refused.

This build delivers the `run:` bundle's fragments, in every assembly
(`materialize`, `context assemble`, `run`): after the profile-selected
fragments, companions by name and fragments by name, under the reference
`ctxloom+companion:<name>#fragments/<fragment>`. A fragment with a `premise`
is held back from the context and offered in the premise index, as any
fragment is. A companion fragment that duplicates one already assembled is
assembled once. The loadout is fetched fresh at each command, under a
3-second limit.

It also delivers the bundle's hooks, commands and MCP servers to the engine's
native files (`materialize`, and the hook listing), by the same path
a bundle's take. All are unconditional: not chosen by a profile, and a
profile's curation of commands does not remove a companion's.

- **Hooks** follow the profiles' own and precede the profiles' bundles' (then
  ctxloom's own callbacks), companions by name (Go's `ResolveBundleHooksFor`).
  A `pre_tool` hook may declare `tool: shell` or `tool: file_edit`; the engine
  maps the class to its own tool names (claude: `Bash|PowerShell` and
  `Edit|Write|MultiEdit|NotebookEdit`).
- **Commands** are named `<companion>/<command>`, companions by name.
- **MCP servers** claim their names before the profiles' bundles do; a
  profile bundle declaring a name a companion holds is refused as any contest
  for a name is, and `exclude_mcp:` removes a companion's server too.
- **Skills** are not delivered: a loadout is one document, and a document
  carries no skill files (only a tree-form bundle does), so there is nothing
  of a skill to deliver. Go delivers none either.

`--no-companions` delivers none of it. The `init:` section is parsed and not
yet used.

### When a companion fails

A companion never stops a command. The command still runs, and one warning
says why that companion contributed nothing:

| outcome | what happened | warning |
|---|---|---|
| program not found | the command's program does not exist | `companion-not-on-path` |
| exited with a status | it ran and offers no loadout (an older companion) | none |
| probe failed | killed by a signal, timed out, could not be started, or printed nothing | `loadout-probe-failed` |
| unparseable | it printed a loadout that does not parse | `loadout-unparseable` |

Warnings are warnings under `--strict`-style runs too.

### Turning companions off

`--no-companions`, or `CTXLOOM_NO_COMPANIONS` set to `1`, `t`, `true`, `y`,
`yes` or `on` (any case), runs no companion. This build treats the flag and
the variable as an OR; Go lets an explicitly given flag win in either
direction.
