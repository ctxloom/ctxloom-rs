---
description: You are about to give an agent docker access, ask for docker-in-docker for a cell, or work out why an ordinary build, test or lint recipe fails inside an agent container. For anyone reaching for a container runtime from inside a cell, however the moment is worded. This is about whether the cell needs docker AT ALL; turn-gates is about which gate proves a change.
tags:
  - ctxloom
  - container
  - isolation
---
# An agent cell is already the dev environment

## The layering

ctxloom agents run inside ctxloom's own containers, and this project's dev
container sits ON TOP of that. A cell is therefore not a bare box waiting for
tooling to be delivered to it — it already carries the toolchain the gates
expect.

That is the whole reason most agents need no docker access at all. Building or
entering a container from in there constructs a second copy of the environment
the agent is already running in.

## DinD is not forbidden; it is rarely the answer

Nothing here prohibits docker-in-docker, and some work genuinely needs a
daemon — work whose SUBJECT is container behaviour: the isolation runtimes,
image builds, the docker-gated tests. Ask for it there, deliberately, and say
why.

What is almost always wrong is reaching for it to make an ordinary build or
test command work. That impulse is a symptom, not a fix, and the next section
is what it is usually a symptom of.

## The trap that manufactures the wrong fix

The root justfile's recipes name dev-image as a PREREQUISITE, and dev-image
shells straight out to a container build with no guard of its own. A just
prerequisite runs BEFORE the recipe body — so inside a cell the dependency
fails on the missing socket before `_run` is ever reached, and `_run` is the
part that already dispatches to justfile.container whenever DEVCONTAINER, CI
or GITHUB_ACTIONS is set.

So the failure presents as "this cell needs docker" when the truth is that the
dispatch is correct and merely unreachable. Granting the cell a daemon answers
a question nobody asked: it spends a real isolation boundary to build an image
the cell already is.

## If you do drive the container justfile by hand, SAY WHICH ONE YOU RAN

justfile.container is a SEPARATE recipe set, and it has silently diverged from
the root justfile's package lists before — the recipes share a name, which is
exactly what made the divergence invisible. An exit code from one is therefore
not the same claim as an exit code from the other.

An orchestrator deciding whether to merge is relying on that distinction, so
name the recipe path you used when you report. "test-integration exit 0" with
no path is an ambiguous claim, and the ambiguity always resolves in the
optimistic direction.
