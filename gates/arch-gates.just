# The architecture gate, copied from ctxloom-lib's templates/arch-gates.just
# (the copies are independent) and imported by the justfile:
#
#     import 'gates/arch-gates.just'
#
# The importing justfile defines, as gates/gates.just's parameters are:
#
#   TOP         the repository's top level (`git rev-parse --show-toplevel`);
#   arch_lint   the command that runs the arch-lint binary, e.g.
#               "cargo run -q -p arch-lint --" in this repository, or the path
#               of a binary installed from a pinned commit of this repository
#               (`cargo install --git <this repository> --rev <sha> arch-lint
#               --locked --root <dir>`) in a consumer;
#
# and a recipe `_arch-lint-ready`, which makes `arch_lint` runnable (a no-op
# where it is `cargo run`; in a consumer, the install of the pinned binary,
# if it is not there yet; here, the justfile's `_arch-lint-ready`); and has arch-rules.toml at its top level (templates/arch-rules.toml is the
# table to start from). Make `lint` depend on `lint-arch`. The gates it runs
# through are gates/gates.just's `_run`.

# The workspace's dependency edges against the rules table (arch-rules.toml):
# a crate's directory, crates/<ring>/<role>/<crate>, is its role, and the
# table says which roles may depend on which. A pass is one `ok` line (then
# one line per warning, which are dev-dependency findings); a failure is the
# count and the findings, capped by `_run`.
[group('check')]
[doc("The workspace's dependency edges against arch-rules.toml")]
[no-exit-message]
lint-arch: _arch-lint-ready
    #!/usr/bin/env bash
    set -euo pipefail
    just --justfile {{TOP}}/justfile _run lint-arch workspace arch-lint \
        {{arch_lint}} --manifest-path {{TOP}}/Cargo.toml --rules {{TOP}}/arch-rules.toml
    # A pass with warnings: the log's warning lines, so they are seen.
    log=${GATE_LOG_DIR:-{{TOP}}/target/logs}/lint-arch.log
    grep '^warning: ' "$log" | head -n 20 || true

# The extractor `_run` calls on a failure: the tool's own report, whose first
# line is the count (or, when the rules or the workspace could not be read,
# the reason).
[positional-arguments]
_extract-arch-lint log rc:
    #!/usr/bin/env bash
    set -euo pipefail
    cat "$1"
