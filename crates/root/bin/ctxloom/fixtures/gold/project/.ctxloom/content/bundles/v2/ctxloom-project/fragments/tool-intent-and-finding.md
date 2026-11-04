---
tags:
  - ctxloom
  - workflow
content_hash: sha256:6134dfc69cac3e33a6da2af0b98f3cde01b2af78161a34789ab1eac6e2c01391
---
# Say what you are looking for, and what you found

Two short statements per tool call. They are not narration — they are the only
part of a tool interaction that survives compaction.

- BEFORE a tool call: what you are trying to learn or change.
- AFTER the result: what you actually learned — including "nothing" and "not
  what I expected", which are results.

## Why this is a rule and not a style preference

A session transcript is compacted into an essence that a later session resumes
from, and tool RESULTS do not survive that. They are reduced to their shape —
`[2,431 bytes, 47 lines]` — because a truncated fragment of a grep is neither
the information nor a summary of it, and the content is re-derivable by asking
again.

What is NOT re-derivable is what you were trying to find out, and what the
answer changed. Omit it and the essence records that a command ran and how many
lines it printed, with the reasoning gone.

Measured on this project's own transcripts: tool traffic is 50-69% of a
session. Bash already enforces the first half — `description` is a required
field, at 100% coverage across 1,286 calls. The second half is stated after
12% of tool results. That gap is the entire cost.

## What this looks like

Weak: "Let me check the config."
Strong: "Checking whether the byte cap applies per-argument or to the whole
object — it decides whether eliding one key buys anything."

Weak: "Done." (the result's shape already said that)
Strong: "It caps the whole object, so eliding the payload should free budget
for the path — but 210 of 210 paths were already visible, so it does not."

A finding that overturns what you expected is the most valuable thing you can
write. Say it plainly and carry on.