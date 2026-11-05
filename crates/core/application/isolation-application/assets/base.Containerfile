# Default agent-image BASE: the shared stage-1 of every locally-built agent
# image (tagged ctxloom-agent-base:latest). The per-engine Containerfiles under
# container/production/ layer the agent specifics — engine CLI install, the
# ctxloom binary, the build gates — onto this via ARG BASE_IMAGE, so the base is
# the ONE place the common environment lives (DRY), and the one thing a user
# replaces to bring their own: set isolation_base to devcontainer (`ctxloom
# container scaffold` seeds one from this file) or to an image ref, and the
# same agent layers build on top of it instead.
#
# node:22-slim is the default: Debian (apt — the engine layers' install contract)
# with a current node runtime, which the claude layer's npm install needs anyway.
# A user base must keep that contract: Debian-family with apt, plus node/npm for
# claude-backed images (the in-image build gates fail fast when it doesn't).
#
# The tool layer is the coding-agent baseline the engines expect at RUN time:
# git (repo operations, worktree-in-container), ripgrep (claude's Grep tool —
# the CLI no longer vendors its own), curl + ca-certificates (fetches, the
# opencode installer), unzip (installer archives), jq (ubiquitous in hook scripts),
# strace (the acceptance harness's read-observation probe wraps the engine exec
# in it; inert under the default seccomp profile, which denies ptrace).
FROM node:22-slim

# Default TERM so interactive agent runs get color/ANSI. The runtime starts
# processes with a bare environment, leaving TERM=dumb, which makes every CLI
# (git, ripgrep, the engine itself) strip color and cursor control. A sane
# 256-color default restores it; the isolation runtime can still override TERM
# per run when the host terminal reports something richer or poorer.
ENV TERM=xterm-256color \
    COLORTERM=truecolor

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        git ripgrep curl ca-certificates unzip jq strace \
    && rm -rf /var/lib/apt/lists/*
