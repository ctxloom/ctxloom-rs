---
distilled_by: claude-opus-5-5
---
# Not all of this project's guidance is in your context

CONDITIONAL fragments each carry a PREMISE (the situation they apply to) and
are not loaded at launch. You have only the unconditional ones; the rest exist,
and nothing fires when one becomes relevant. This fragment has no premise: it
is the pointer to the ones you cannot see.

**Why pulled, not pushed.** Assembled context is delivered once, at launch,
and an in-process subagent inherits it wholesale. With no per-agent scoping, a
pushed index fits nobody and never reaches a child ctxloom did not mediate. A
catalog you ASK for has neither problem, and is answered at the only moment
the answer means anything: when you have something to match against. The cost:
an agent that does not know to ask never learns those fragments exist.

**Ask.** `ctxloom fragment premises` lists every premised fragment, its
condition, and the instruction for deciding between them. Over MCP the same
menu is the `ctxloom://fragments` resource (each entry: its premise and the
qualified ref to quote back); bodies come from `ctxloom://fragments/{name}`.
Judge the premise, not the name: read the guidance the listing ships rather
than guessing from a title.

**Load.** `ctxloom fragment show <qualified-ref>`, or over MCP call
`assemble_context` with the chosen names in its `bundles` argument.

**When.** When you are about to ACT and this corpus might hold a rule about
that action — not once at session start, and not against the conversation
behind you. Ask again later in a long session: what you are about to do has
changed, so the answer has too.
