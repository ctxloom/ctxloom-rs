---
description: You have just written or changed a test; you are reading a gate result and deciding what it proves; or you are about to describe something as verified, working, or done. For anyone about to treat a green result as evidence, however that moment arises. More than one fragment may apply here; this one is about whether the evidence is real.
tags:
  - ctxloom
  - testing
no_distill: true
---
# A test does not pass until a mutation dies

A test is not passing because it is green. It is passing when it is
green AND a mutation to the production code it names makes it FAIL.
Green alone means the test ran. It does not mean the test looked.

Apply it in both directions:

- **Writing a test**: green is a claim, not proof. The proof is a
  mutant dying against the production code the test names, and it is
  collected ONCE, at close-out, by the diff mutation run over the whole
  change (the `closeout` skill, step 2; `turn-gates` says which
  command) — not by mutating after every test. A mutant that LIVES
  there in code your test names means you wrote a tautology, which is
  worse than no test because it reports coverage you do not have.
- **Trusting a test**: a suite's green tells you nothing about a
  specific claim until a mutant has died against it. "It passes" is
  not evidence. "It failed when X was broken" is. Until close-out has
  run, say "green, not yet mutation-checked" — never "verified".

## Why this is a rule here and not a preference

An audit of this project's acceptance suite on 2026-08-04 read 380
scenarios, ran 41 mutations, and found 25 assertions that passed
while proving nothing. Not edge cases — one asserted that the output
matched the regex `.` (any one character). Others asserted an
argument the command had echoed back, or a MIME type that is a
static field on the envelope, or the ABSENCE of something the
fixture never created. Several were satisfied by the subject never
running at all: a scenario titled "the same engines proceed" passed
when the engine never launched, and one titled "a per-item
acceptance and a rejection ARE RECORDED" passed with the record
store neutered to write nothing.

Every one of those had been green for as long as it had existed.

## The shapes that produce a false green

- asserting exit 0 without asserting the EFFECT (this project's
  characteristic silent no-op: exit 0, a success message, zero bytes)
- asserting a name, a header, or an argument the command echoes
- asserting a file exists without asserting its content or mode
- asserting the tool's REPORT of what it did instead of the state it
  changed
- absence-satisfies-absence: asserting something is missing in a
  fixture where it was never present
- a substring so generic it survives gutting the thing under test

## Where the bar is already met, and worth copying

`tests/acceptance/steps_skill.go` compares bytes with an explicit
zero-length guard — "comparing two empty reads is trivially
identical" — which is the line that stops a byte comparison from
being vacuous. `j002600` asserts an exact file COUNT rather than
existence. `run.feature`'s recorded-input assertion reads what the
engine actually received rather than what the CLI said it sent.

## Test against a FRESH BUILD, never the installed binary

Any manual or agent-driven verification runs against a binary built
from the tree under test — `just build`, then `./ctxloom` — never
`ctxloom` from `$PATH`.

The installed binary is a different program. On 2026-08-05 a check
of whether a newly added fragment was delivered came back "missing";
the installed `ctxloom` was `v0.7.0-cefeb77-20260728T201548-dirty`,
eight days and ~150 commits behind the tree, and dirty on top. Built
from source, the same check passed. The measurement was wrong, not
the code — and a wrong measurement that agrees with your fear is the
expensive kind.

`just test-acceptance` already does this correctly: it depends on
`build` and drives the binary it just produced. The exposure is
hand-run commands and agent verification, which reach for `$PATH`
by default.

The `$PATH` binary's currency may NOT be assumed in either direction. It
may be current, stale, or dirty, and you cannot tell by looking -- which is
exactly why verification drives the binary `just build` just produced rather
than reasoning about which program answered. A stale binary plus an
exit-code check agrees with anything.
