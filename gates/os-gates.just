# The OS gates, as a justfile fragment consumers copy (to gates/os-gates.just,
# or beside their justfile) and import:
#
#     import 'gates/os-gates.just'
#
# The importing justfile defines, as gates/gates.just's parameters are:
#
#   TOP               the repository's top level (`git rev-parse --show-toplevel`);
#   windows_packages  the cargo arguments naming what `check-windows` compiles,
#                     e.g. "-p adapter-a -p adapter-b" (or "--workspace");
#   clippy_template   where the consumer's checkout of
#                     templates/clippy-disallowed.toml is, e.g.
#                     "../ctxloom-lib/templates/clippy-disallowed.toml".
#
# and has scripts/os-cfg-check.sh, scripts/os-cfg-allow.txt and
# scripts/clippy-sync-check.sh from this repository's scripts/ (copied; the
# copies are independent). The self-tests (scripts/*.selftest.sh) stay here.

# Nothing in this workspace names an operating system: what differs between
# them is angzarr-platform's os crate's alone. No cfg naming one, `cfg!(`,
# `std::os::`, `split_paths` or target-specific dependency, tests included,
# but the allowlist's (scripts/os-cfg-allow.txt).
[no-exit-message]
lint-os:
    @{{TOP}}/scripts/os-cfg-check.sh

# This workspace's clippy.toml still holds every entry of the canonical OS and
# filesystem list (templates/clippy-disallowed.toml in ctxloom-lib): run it
# after a change to either. It may disallow more, never less.
[no-exit-message]
lint-clippy-sync:
    @{{TOP}}/scripts/clippy-sync-check.sh {{TOP}}/clippy.toml {{clippy_template}}

# The crates that use the platform seams, with the seams' Windows
# implementors, compiled for a Windows target but not run: the quick check a
# Linux host can make before CI's Windows runner builds and tests the whole
# workspace natively. Needs the x86_64-pc-windows-gnu target (`rustup target
# add`); no linker, since nothing is linked. Crates with C dependencies need a
# mingw C compiler, so name only the ones without in `windows_packages`.
[no-exit-message]
check-windows:
    #!/usr/bin/env bash
    set -euo pipefail
    terse=(-q --message-format=short)
    [ "${VERBOSE:-}" = 1 ] && terse=()
    exec just --justfile {{TOP}}/justfile _run check-windows platform cargo \
        cargo check "${terse[@]}" --manifest-path {{TOP}}/Cargo.toml --target x86_64-pc-windows-gnu \
        {{windows_packages}} --all-targets

# The self-tests of the gate-kit's scripts against fixture trees (this
# repository's; a consumer copies the scripts, not these).
[no-exit-message]
selftest-gate-kit:
    @{{TOP}}/scripts/os-cfg-check.selftest.sh
    @{{TOP}}/scripts/clippy-sync-check.selftest.sh
    @{{TOP}}/scripts/clippy-disallowed.selftest.sh
