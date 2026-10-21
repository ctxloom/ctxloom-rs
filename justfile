TOP := `git rev-parse --show-toplevel`

# Build every crate. Needs protoc on PATH and access to github.com (angzarr-proto).
build:
    cargo build --manifest-path {{TOP}}/Cargo.toml --workspace

test:
    cargo test --manifest-path {{TOP}}/Cargo.toml --workspace

# Clippy, warnings denied. Clippy reads only the nearest clippy.toml, so the
# core's repeats the root's nesting threshold; refuse to lint if they differ.
lint:
    #!/usr/bin/env bash
    set -euo pipefail
    root=$(grep '^excessive-nesting-threshold' {{TOP}}/clippy.toml)
    core=$(grep '^excessive-nesting-threshold' {{TOP}}/crates/core/clippy.toml)
    if [ "$root" != "$core" ]; then
        echo "clippy.toml ($root) and crates/core/clippy.toml ($core) disagree on nesting" >&2
        exit 1
    fi
    cargo clippy --manifest-path {{TOP}}/Cargo.toml --workspace --all-targets -- -D warnings

fmt:
    cargo fmt --manifest-path {{TOP}}/Cargo.toml --all

fmt-check:
    cargo fmt --manifest-path {{TOP}}/Cargo.toml --all -- --check

# Dependency policy: licenses, sources, bans and RustSec advisories (deny.toml).
# Fetches the advisory database, so it needs the network.
lint-deps:
    cargo deny --manifest-path {{TOP}}/Cargo.toml check

# Scan the STAGED changes for secrets: lefthook's pre-commit step. Rules and
# allowlists are .gitleaks.toml; output is redacted. A missing binary fails
# rather than skipping: a scan that passes by not running looked at nothing.
secrets-scan-staged:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v gitleaks >/dev/null 2>&1; then
        echo "gitleaks is not on PATH; the secret scan cannot pass by not running." >&2
        exit 1
    fi
    if ! gitleaks git --pre-commit --staged --redact --verbose --no-banner --config {{TOP}}/.gitleaks.toml {{TOP}}; then
        echo "gitleaks found a secret in the staged changes (redacted above)." >&2
        echo "A real credential: unstage it and rotate it. A test fixture: end its line with 'gitleaks:allow'." >&2
        exit 1
    fi

# Wire lefthook's git hooks into .git/hooks.
hooks-install:
    lefthook install
