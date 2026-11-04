---
description: Review a diff against the project's standards
tags:
  - review
installation: Run after staging your changes.
exports:
  claude-code:
    enabled: true
    description: Review the staged diff
    argument_hint: "[path]"
    allowed_tools:
      - Read
      - Grep
    model: sonnet
  disabled-engine:
    enabled: false
    argument_hint: "[path]"
  some-future-engine:
    description: carried verbatim, not dropped
---
Review the staged diff.
