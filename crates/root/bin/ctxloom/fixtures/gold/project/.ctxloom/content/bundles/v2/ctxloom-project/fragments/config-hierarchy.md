---
description: You are adding or changing a flag, an environment variable, or a config key; working out which layer won when a setting resolved unexpectedly; or reading configuration to decide how something is declared. For anyone touching how this family of binaries is configured, however the moment is worded.
tags:
  - ctxloom
  - config
no_distill: true
---
# Configuration precedence

Every binary in the ctxloom family (ctxloom, taskloom, ltk) resolves
configuration through one shared chain, owned by
`internal/shared/confload`:

    home config file  <  project config file  <  ENV VARS  <  --config-set FLAGS

Later layers win. `confload` reads and deep-merges the file layers and
resolves both override channels against the merged result, so there is
exactly one place this ordering is implemented — do not re-derive it in
a command.

## The two override channels

**Environment** — a dedicated namespace under the product's env prefix
(`CTXLOOM_CONFIG_`), e.g. `CTXLOOM_CONFIG_LLM_DEFAULTS_PRIMARY=big`.

**`--config-set`** — a repeatable root flag taking `<dotted.path>=<value>`,
e.g. `--config-set agents.MyCoder.runtime=container`.

## A command's own flags are NOT a config source

`--config-set` is the ONLY flag channel. `confload` deliberately does not
treat a changed flag's NAME as a config path, and this is load-bearing
rather than stylistic — an earlier revision did exactly that, and it
silently coupled every current and future flag name to the config schema.
Two failures were confirmed in production:

- `ctxloom agent set coder --runtime container` clobbered the project's
  top-level `runtime` key, because `--runtime` resolved as a config path.
- `--format json` on a structured-output command printed a warning line
  into what a script expected to be pure JSON, because `--format`
  resolved as "unrecognized config key, setting it anyway".

So when you add a flag, it configures THAT invocation. If the value
belongs in config, give it a schema path and let the chain resolve it;
never bridge a flag name to a config key by convention.

## Case handling diverges between the two channels, deliberately

A shell destroys an env var name's case before Go sees it
(`CTXLOOM_CONFIG_AGENTS_MYCODER_RUNTIME` cannot say whether the author
wrote `MyCoder`, `mycoder`, or `MYCODER`). A `--config-set` value never
passes through those rules, so it preserves exactly what was typed.

Both resolve identically when the target already exists or is a fixed,
canonically-cased schema field: case-insensitive match, adopting the
existing casing. They diverge only at a dynamic level that nothing
existing covers — an agent label, an LLM config label. There, env falls
back to whatever the shell handed over, while `--config-set` preserves
the typed case. That is why `--config-set` can mint a brand-new
case-sensitive key (`agents.MyCoder.runtime=container`)
and env fundamentally cannot.