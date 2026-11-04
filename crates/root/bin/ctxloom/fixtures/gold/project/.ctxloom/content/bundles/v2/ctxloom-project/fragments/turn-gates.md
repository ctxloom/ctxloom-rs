---
description: You are writing or changing a test during a turn, and need it to survive the close-out mutation run; choosing which build, test, lint, or mutation command to run; interpreting an exit code or a suite's output; or deciding whether the gate you ran actually covers the claim you are making. For anyone deciding which command proves a change is done. This is about WHICH gate and how to read it, not about whether a passing test means anything.
tags:
  - ctxloom
  - testing
  - workflow
---
# Which gate is real in this repo, and how to read one

Companion to green-is-not-passing, which says a test is not passing until a
mutation dies. This says which command actually proves that here, and how to
read what it tells you.

## The gates, and what each one is worth

    just build                    ~4s
    just lint                    ~19s
    just test-arch               ~20s   the architectural class gates, -tags arch
    just test-integration        ~30s
    just test-acceptance        ~510s   THE REAL GATE
    just test-acceptance-focus features/<f>.feature   ~10-17s, one feature

`just test` only COMPILES the integration suite. It is not the gate, and a green
one proves far less than it looks like it does.

Iterate on the narrow target; run `test-acceptance` before claiming anything is
done. When you need a single acceptance scenario — hand-mutating one at
close-out, say — `test-acceptance-focus` runs a whole feature in seconds.

## During the turn: write the tests the close-out mutation run will judge

Every behaviour you add or change ships with its test IN THE SAME TURN. You do
not mutate it now — close-out runs `test-mutation-diff` over the whole change
and scores every changed line — but write each test knowing that run is
coming, because a mutant that LIVES there is a test you have to come back and
fix. With gomutants' mutators (`.gomutants.yml`), on each line you change it
will:

- flip conditional boundaries and negations (`<`→`<=`, `==`→`!=`) — test BOTH
  sides of every condition you add, at the boundary value itself;
- swap `&&`/`||` and arithmetic, increments, sign, bitwise and assignment
  operators, and nudge integer and float literals — assert the exact computed
  value, not just non-zero or "changed";
- turn `break`/`continue` around, break out of loops early and drop
  self-assignments — assert what the loop produced or skipped, element by
  element;
- empty `if`/`else`/`case` bodies, remove statements, and force returns to
  `nil` error, zero value, `true` or `false` — assert every effect and every
  returned value, and errors by identity, not merely that one occurred.

And the shapes no mutator needs to find, because they pass with the code
deleted: green-is-not-passing lists them (exit 0 without the effect, the
echoed argument, the generic substring, absence-satisfies-absence). Assert
the EFFECT; assert errors by identity (`errors.Is`), not `err != nil`.

Report such a test as "green, not yet mutation-checked" until close-out has run.

## Read the EXIT CODE — and know where that is not enough

Gate on the exit status, never on grepping output for "PASS" or "ok". Then know
the three inversions, each of which reports success while telling you otherwise:

- `gofmt -l` LISTS unformatted files and exits 0. Check whether the OUTPUT is
  empty, not the status.
- `go test -run <pattern>` whose pattern matches NOTHING exits 0 having run no
  test. Confirm it actually ran.
- a pipeline reports the LAST command's status, so `cmd | tail; echo $?` gives
  tail's exit. Redirect to a file and check directly, or use PIPESTATUS.

The shape to watch for throughout: exit 0, a success message, and zero bytes
written.

## Which mutation method reaches which code

Mutation runs at CLOSE-OUT (the `closeout` skill, step 2), once over the whole
change — not after each test or each turn. Close-out runs everything relevant
to the DELTA and nothing wider; full-module coverage and mutation are CI's:

    just test-pkg ./<pkg>/ -cover      each package the change touched: unit + line %
    just test-mutation-diff <BASE>     changed lines: kill % (gate 80) + mutant coverage %

When mutation runs, the DEFAULT is the tool:

    just test-mutation-diff <BASE>     BASE = the commit before the work began

It mutates every changed line and scores each mutant killed / lived / not
covered / timed out — systematic, repeatable, and faster than editing by hand.
Run it, then read its LIVED and NOT COVERED lists. Do not swap in hand mutation
because the tool "covers the whole diff" (that is the point) or seems slow.

Hand mutation is for ONE gap only. gomutants is coverage-gated: it mutates
source and runs `go test`, and the acceptance suite and testenv integration
tests exec a PRE-BUILT binary, so a mutant reachable only through the CLI
reports NOT COVERED and is never attempted.

    unit-testable code               just test-mutation-diff <BASE>
    NOT COVERED, reached via the CLI hand mutation, after the run

HAND MUTATION MUST REBUILD. Those suites exec a previously built binary and
`just test-pkg` does not rebuild. Edit production code, run `just build`, THEN
the gate. Skip the build and the mutation survives against the old binary — you
will call a good test vacuous, or a vacuous one fine.

When a mutation SURVIVES, suspect you aimed at the wrong layer before concluding
the test is vacuous. The tell is that the mutated code is demonstrably live
elsewhere: if breaking it reddens some other package's tests, the code is
reachable, just not from the path under test. Find the reachable gate and kill
that instead.

## A fresh worktree cannot commit until it is built

`just build` first, and make sure `bin/archlint` exists there. Without a built
tree the pre-commit hook fails with an `archvocabulary: failed prerequisites`
cascade that looks like a broken analyzer and is not.

`test-pkg` takes a package PATTERN — `./internal/cli`, with the leading `./`.
Without it you get a misleading "is not in std" error.
