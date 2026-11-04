---
tags:
  - ctxloom
  - workflow
content_hash: sha256:93cd9d453e1d3c679a46351a6ea3e1a829ead8bb493117afac413b29ca4cfb3d
---
# Not all of this project's guidance is in your context

Some fragments here are CONDITIONAL: each carries a PREMISE — the situation it
applies to — and is deliberately not loaded at launch. You were given the
unconditional ones. The rest exist, you do not have them, and nothing fires
when one becomes relevant.

This fragment carries no premise itself, which is why you can read it: it is
the pointer to the ones you cannot see.

## Why they are pulled rather than pushed

Assembled context is delivered ONCE, at launch, into the session prompt, and an
in-process subagent inherits it wholesale. There is no per-agent scoping, so a
pushed index could be tailored to nobody and could never reach a child that
ctxloom did not mediate. A catalog you ASK for has neither problem — and the
moment you have something to match against is the only moment the answer means
anything.

The cost of that design is this: an agent who does not know to ask never learns
those fragments exist.

## Asking

    ctxloom fragment premises

lists every premised fragment, the condition it applies under, and the
instruction for deciding between them. Over MCP the same menu is the
`ctxloom://fragments` resource: each entry carries its premise and the qualified
ref to quote back. Descriptions live there; bodies come from
`ctxloom://fragments/{name}`, for the ones you chose.

Do not select from the names. The premise is the thing to judge, and the
listing ships the guidance for judging it — read what comes back rather than
guessing from a title.

## Loading what you chose

    ctxloom fragment show <qualified-ref>

or, over MCP, call `assemble_context` with the chosen names in its `bundles`
argument.

## When to ask

Ask when you are about to ACT and this corpus might hold a rule about that
action — not once at the start of a session, and not against the conversation
behind you. Ask again later in a long session: what you are about to do has
changed, so the answer has too.
