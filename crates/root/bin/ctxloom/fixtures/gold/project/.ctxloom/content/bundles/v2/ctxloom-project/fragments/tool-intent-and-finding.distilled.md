---
distilled_by: claude-opus-5-5
---
# Say what you are looking for, and what you found

Two short statements per tool call — not narration, but the only part of a
tool interaction that survives compaction:

- BEFORE: what you are trying to learn or change.
- AFTER: what you actually learned, including "nothing" and "not what I
  expected", which are results.

**Why it is a rule.** Compaction reduces tool RESULTS to their shape
(`[2,431 bytes, 47 lines]`): a truncated grep is neither the information nor
a summary of it, and the content can be re-derived by asking again. What
cannot be re-derived is what you were trying to find out and what the answer
changed; omit it and the essence records only that a command ran. Measured on
this project's transcripts: tool traffic is 50-69% of a session; Bash's
required `description` covers the BEFORE half (100% of 1,286 calls), but the
AFTER half follows only 12% of results. That gap is the entire cost.

Weak: "Let me check the config." Strong: "Checking whether the byte cap
applies per-argument or to the whole object — it decides whether eliding one
key buys anything."

Weak: "Done." (the result's shape already said that). Strong: "It caps the
whole object, so eliding the payload should free budget for the path — but
210 of 210 paths were already visible, so it does not."

A finding that overturns what you expected is the most valuable thing you can
write. Say it plainly and carry on.
