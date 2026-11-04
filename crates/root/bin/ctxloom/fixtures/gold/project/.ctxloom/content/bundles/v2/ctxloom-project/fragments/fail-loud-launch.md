---
description: You are writing a check that can find something wrong; writing code that accepts a configuration value, a file, or any input from outside the program; deciding what an unknown, malformed, or missing value should resolve to; or choosing between refusing and carrying on with less. For anyone deciding how loudly a failure should surface, and where the set of acceptable values is declared. This one also governs ordinary resolution code, not only launch-time findings.
tags:
  - ctxloom
  - workflow
  - trust
---
# Refuse when something is amiss; `--degraded` is the way through

**The default posture is REFUSE, not proceed.** When ctxloom finds something
wrong at startup — broken config, an unresolvable profile or bundle, a failed
hook apply, an invalid document — it ABORTS the launch and says what is wrong
and how to fix it. It does not quietly launch something lesser.

This is a deliberate pivot away from always-launch. Always-launch produced
CONFIDENT WRONG WORK: a bad or missing agent name silently degraded to
`host`+`none`, discarding the runtime and permissions that were asked for, with
only a stderr warning nobody read. The agent then ran unisolated while everyone
believed otherwise. A launch that succeeds without doing the thing is worse
than one that refuses, because nothing downstream can tell the difference.

## First, declare what you accept — in code, in one place

A refusal is only meaningful against a DECLARATION. "Strict" that means
"whatever this function happened to check" is not strict, it is arbitrary — and
arbitrary drifts, because the next call site checks something slightly
different and nothing notices.

So before either mode below can exist, the acceptable shape of a value is
WRITTEN IN CODE, in one place, as something the program can read: a schema, a
typed parse, an enumerated set of names. Not a comment, not a doc page, not a
validation function per call site. Both modes then consume that one
declaration — the refusal names the declared set, the degraded path names the
declared value it fell back to — and neither invents its own idea of valid.

THE TEST, and it is one you can actually apply: point at the ONE thing a reader
would change to add a new accepted value. If the answer is "the validator, and
the error message, and the docs, and the schema", the declaration does not
exist. Those are four copies, and copies drift — the error message goes on
listing the old set with full authority while the validator has already
accepted a new one.

THIS IS NOT ONLY A LAUNCH CONCERN, and that is the trap. Everything below about
`strictness` governs findings raised at STARTUP. An ordinary resolution
function is not a launch check, raises no finding, and reaches none of that
machinery — which is exactly where silent substitution survives. The `--llm`
label is the worked example: `config.(*Config).ResolveLLM` falls back to the
built-in default on a label it does not know, because it has no engine
registry and cannot tell a typo from a bare engine name. So the finding lives
one layer up, in `operations.ResolveBackend`, the one place that holds BOTH
declarations (the configured labels and the registered engines) and can say
the label names neither. Put the refusal where the declaration is visible, not
where the lookup happens to miss.

## `--degraded` always reaches a working LLM

`--degraded` (and `CTXLOOM_DEGRADED=1`; the flag wins) lowers every finding
from an error to a WARNING and continues. That is a promise, not a
best-effort: **degraded mode always gets the user into a working LLM.** If a
degraded run can still refuse to launch for an ordinary finding, degraded mode
is broken.

So the two modes divide cleanly:

    default      refuse, and explain the remedy
    --degraded   warn, and launch anyway

## The one exception: launching is itself the danger

Security findings where starting the LLM is the harm do NOT yield to
`--degraded`. A breached trust or isolation boundary — credentials reachable
that should not be, an ownership mismatch on the runtime axis, unverifiable
content admitted to an execution surface — stays fatal in both modes.

The test is not "is this serious?" but **"does launching cause the harm?"** A
profile that fails to parse is serious and still degradable: the user gets a
working LLM with less context. A container runtime that cannot provide the
isolation it claimed is not degradable, because the launch IS the exposure.
`--degraded` falls back to the HOST, never to a different ownership mode.

## How to write one of these, mechanically

Report the finding; let `strictness` decide its fatality:

    strictness.FailOnce(report.KindConfig, "<the remedy>", ...)

Core packages may not import `strictness`; they raise the same finding through
the `report.Reporter` they are handed (`rep.FailOncef(report.KindConfig, ...)`)
— `internal/core/profiles` does exactly this for the empty-profile cases.
Fatality is decided in ONE place: the run's `strictness.Mode`, built once at
the CLI root from `--degraded` / `CTXLOOM_DEGRADED`, is what the startup gate
consults. That is what makes the guarantee above hold everywhere at once.

Therefore, at the site of a check:

- **Do NOT branch on degraded state.** No reading `Mode.Degraded` at a check,
  no `strict bool` parameter, no per-call fatality argument. A check that decides
  its own fatality is a second policy that will disagree with the first.
- **Do state the REMEDY, not just the complaint.** The refusal is the entire
  user interface for the failure. "invalid profile" is a dead end; "give the
  profile something to select (parents, bundles, fragments, select_tags) or
  delete it" is error correction. Write the sentence that ends the problem.
- **Do make sure the remedy is followable.** A message advising an action the
  caller cannot take — telling them to drop a flag they never passed — is
  proof the check is firing outside its own design premise. Fix the premise,
  not the wording.
- **Report every problem, not the first.** `FailOnce` records and the startup
  gate decides; a user fixing four things one launch at a time is paying for
  our convenience.

## Say what you are correcting TO — from the code that does the correcting

A degraded message that names only the FAULT is half a message. It says what
broke; it does not say what will now happen, and that is the only part that
lets someone decide whether to let the run continue. Name the value being
proceeded WITH:

    engine_home "sesion": unknown value (known: host, session)
      -> degraded: proceeding as "host"

That second line must be produced BY THE CORRECTION ITSELF, never by a
hand-written string standing beside it. The correction RETURNS the value it
chose; the announcement FORMATS that returned value. One source, one truth.

The moment the fallback's name is typed into a format string there are two
statements of one decision, and the compiler sees only one of them. Change the
fallback and nothing updates the sentence: it keeps its authority while lying,
to the person already dealing with a fault.

Measured, not hypothetical. A run here warned "this run uses the runtime's own
config home instead" and then ABORTED on a fatal finding, having taken no such
fallback. The prose and the behaviour were written separately and drifted
apart, and the message was believed. The same rule governs the refusal arm: the
valid set quoted in an error comes from the same declaration the validator
checks against, never from a second list maintained by hand.

## Outputs stay strict in both modes

None of the above touches the output side. What ctxloom EMITS — files, wire
messages, exit codes, generated config — is conservative always. Degraded mode
tolerates a bad INPUT; it never emits a malformed output, a partial file, or a
zero exit code over work that did not happen.

## Testing it

Pin BOTH arms, and mutate each. The same bad input must REFUSE by default
(nonzero exit, nothing launched) and WARN-then-launch under `--degraded`. A
test covering only the refusal lets degraded mode silently start refusing too,
which breaks the guarantee for every existing user; a test covering only the
degraded arm lets the refusal rot into a no-op. Assert the EFFECT — exit status
and whether an engine actually started — never the message text alone.
