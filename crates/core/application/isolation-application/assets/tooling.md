You are folding bundle-declared tooling into this project's **agent container
image**. The declarations below were collected from the companions this machine
registered (`ctxloom companion add`), but a Containerfile edit is still a
code-execution surface: **every change needs the user's explicit approval —
show the exact diff before writing anything.**

## Do this

1. **Locate the editable base.** It is the project devcontainer's Dockerfile
   (`.devcontainer/devcontainer.json`'s `build.dockerfile`). If the project has
   no devcontainer, run `ctxloom container scaffold` — it writes one seeded
   from the embedded default base (the same one the default auto-build was
   already using), so nothing changes until you edit. Then read the file. If
   config sets `isolation_base` to `ctxloom` or an image ref, agent images do
   not build on the devcontainer: the user either unsets it, or builds their
   own Containerfile into an image and sets `isolation_base: <image ref>` —
   config does not take a Containerfile path.
2. **Propose the edits.** For each bundle declaration below, translate its
   required tools into concrete Containerfile additions (install layers,
   version pins where the declaration asks). Keep the file's existing
   structure — the engine's agent stage layers on top of this base, so only
   the base is yours to touch. Attribute each addition with a short comment
   naming the source bundle.
3. **Show the user the full diff and ask before writing.** Per-change
   approval, not a blanket yes. If the user declines a bundle's tooling, skip
   it — its content still works, just without in-image tools.
4. **Apply approved edits, then rebuild:** `ctxloom container build`. The
   default auto-build also picks the file up on the next containerized run
   when `isolation_base` is unset or `devcontainer`.

## Never

- Never apply tooling automatically on pull/sync or without showing the diff.
- Never install from a declaration the user hasn't seen.
- Never edit anything other than the base's Dockerfile for this purpose.

## Bundle declarations
