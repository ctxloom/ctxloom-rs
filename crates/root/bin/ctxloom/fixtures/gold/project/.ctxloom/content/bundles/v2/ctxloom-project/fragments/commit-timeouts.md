---
description: You are about to run git commit, git merge or git push through the shell tool — directly, inside a script, or as the last step of a change. For anyone whose next command fires this repository's git hooks, however the moment is worded.
tags:
  - ctxloom
  - git
  - workflow
---
# A commit can outlive the shell tool's timeout

This repository's git hooks run real gates, and on a busy machine they take
longer than a shell tool's default timeout.

Pass an explicit timeout equal to the configured `shell_timeout.max` on every
shell call that runs `git commit`, `git merge` or `git push`.

A command that outlives its timeout is moved to the background, not finished.
Before you report a commit, confirm it landed with `git log -1`. If it is not
there, the hooks are still running or have failed: read their output rather
than retrying blind.
