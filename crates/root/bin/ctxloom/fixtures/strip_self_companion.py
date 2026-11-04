#!/usr/bin/env python3
"""Normalize Go ctxloom's output into a golden (see scripts/go-library-goldens.sh).

usage: strip_self_companion.py PROJECT HOME ID < stdout

Two things, and only these:

1. The project's and the home's paths become {PROJECT} and {HOME}.
2. Go's own companion loadout (the `ctxloom` companion: the fragment
   ctxloom+companion:ctxloom#fragments/isolation-axes and the `ctxloom` MCP
   server) is removed from the output. Its texts are the owner's (D24) and
   name MCP tools whose cut list is open, so this port does not ship them
   yet. Everything it contributes is removed where it appears, and the
   counts and the token estimate that include it are recomputed:
   the estimate is the byte length over four, rounded up.
"""
import json
import re
import sys

proj, home, test_id = sys.argv[1], sys.argv[2], sys.argv[3]
raw = sys.stdin.read()
raw = raw.replace(proj, "{PROJECT}").replace(home, "{HOME}")

SELF_FRAGMENT = "ctxloom+companion:ctxloom#fragments/isolation-axes"
SELF_BUNDLE = "ctxloom:companion@ctxloom"
SEP = "\n\n---\n\n"
ISOLATION_HEAD = "## Isolation: specify both axes"


def tokens(text):
    n = len(text.encode("utf-8"))
    return (n + 3) // 4


def cut_context(context):
    """The assembled context without the companion's section (always last)."""
    i = context.find(ISOLATION_HEAD)
    if i < 0:
        return context
    head = context[:i]
    if head.endswith(SEP):
        head = head[: -len(SEP)]
    return head


def strip_package(d):
    """A dry-run or agent-resolved document: fragments, context, tokens."""
    if "fragments" in d:
        d["fragments"] = [f for f in d["fragments"] if f != SELF_FRAGMENT]
    if "context" in d:
        d["context"] = cut_context(d["context"])
        if "tokens" in d:
            d["tokens"] = tokens(d["context"])


def strip_json(v):
    if isinstance(v, list):
        out = []
        for e in v:
            if isinstance(e, dict) and (e.get("bundle") == SELF_BUNDLE or e.get("source") == SELF_BUNDLE):
                continue
            out.append(strip_json(e))
        return out
    if isinstance(v, dict):
        if "servers" in v:
            v["servers"] = [s for s in v["servers"] if s.get("name") != "ctxloom"]
        if "local" in v and "count" in v:
            v["local"] = [e for e in strip_json(v["local"]) if not (e.get("type") == "mcp_server" and e.get("name") == "ctxloom" and e.get("source") == "session-endpoint")]
            v["count"] = len(v["local"]) + len(v.get("remote") or [])
        if "context" in v and "fragments" in v and "profiles" in v:
            strip_package(v)
        if "resolved" in v and isinstance(v["resolved"], dict) and "context" in v["resolved"]:
            strip_package(v["resolved"])
        return {k: (strip_json(x) if k not in ("context",) else x) for k, x in v.items()}
    return v


def strip_search_text(t):
    """`search` prose: the companion's fragment and its `ctxloom` MCP server
    are removed, empty headings with them, and the count recomputed."""
    lines = t.split("\n")
    out = []
    removed = 0
    for line in lines:
        if line.startswith("  - ") and ("(ctxloom:companion@ctxloom)" in line or line.startswith("  - ctxloom (session-endpoint)")):
            removed += 1
            continue
        out.append(line)
    # a heading left with no entries goes
    cleaned = []
    for i, line in enumerate(out):
        is_heading = line.endswith(":") and not line.startswith(" ") and not line.startswith("Results")
        if is_heading and (i + 1 >= len(out) or not out[i + 1].startswith("  - ")):
            continue
        cleaned.append(line)
    t = "\n".join(cleaned)
    m = re.search(r"Results \((\d+)\):", t)
    if m:
        n = int(m.group(1)) - removed
        t = t.replace(m.group(0), "Results (%d):" % n)
        if n == 0:
            t = "No results found."
    return t


def strip_text(t):
    t = t.rstrip("\n") + "\n"
    if test_id.endswith("-search-text") or "search" in test_id and test_id.endswith("text"):
        return strip_search_text(t.rstrip("\n")).rstrip("\n")
    # fragment list: the companion's group and the count in the heading.
    if "fragment-list" in test_id and "Fragments (" in t:
        t = re.sub(r"  ctxloom:companion@ctxloom:\n(    - .*\n)+\n?", "", t)
        m = re.search(r"Fragments \((\d+)\):", t)
        if m:
            t = t.replace(m.group(0), "Fragments (%d):" % (int(m.group(1)) - 1))
    t = re.sub(r"Composed fragments: (\d+)", lambda m: "Composed fragments: %d" % (int(m.group(1)) - 1), t)
    # mcp server list.
    t = re.sub(r"  ctxloom\n    Served by: .*\n    Bundle: ctxloom\+companion:ctxloom\n", "", t)
    # dry-run text.
    if "=== Assembled Context" in t:
        t = t.replace("  " + SELF_FRAGMENT + "\n", "")
        m = re.search(r"=== Assembled Context \(~\d+ tokens\) ===\n(.*?)\n\n=== Prompt ===", t, re.S)
        if m:
            ctx = cut_context(m.group(1))
            body = ctx if ctx else "(no context)"
            t = t[: m.start()] + "=== Assembled Context (~%d tokens) ===\n%s\n\n=== Prompt ===" % (tokens(ctx), body) + t[m.end():]
    return t.rstrip("\n")


try:
    doc = json.loads(raw)
except ValueError:
    sys.stdout.write(strip_text(raw))
else:
    sys.stdout.write(json.dumps(strip_json(doc), indent=2, ensure_ascii=False))
