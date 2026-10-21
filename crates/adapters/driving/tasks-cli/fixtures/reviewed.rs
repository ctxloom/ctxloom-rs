//! What Rust taskloom's loadout is: Go's (`go/loadout.yaml`, which
//! `scripts/go-companion-fixtures.sh` regenerates from the Go source), with
//! the factual changes the human reviewed and approved applied.
//!
//! Each change is an exact `(old, new)` pair on the fixture's raw bytes, and
//! each `old` must occur exactly once: if Go rewords a sentence a pair
//! changes, the pair no longer matches and the parity tests fail until the
//! pair is reviewed again; if Go changes any other text, the expected text
//! changes and the byte comparison fails, as it did when the text was Go's
//! alone. The numbers are the review's (the port plan's Decided: "The prose
//! Rust taskloom serves"). The install placeholders await the human.

/// Go's loadout document, reviewed.
pub fn loadout() -> String {
    applied(include_str!("go/loadout.yaml"), LOADOUT)
}

pub const LOADOUT: &[(&str, &str)] = &[
    // 5. run.description: built from the Rust crate; the SQLite store.
    (
        "(built from github.com/ctxloom/ctxloom, cmd/taskloom). Tasks live in a per-project append-only log\n    \
         at ~/.ctxloom/tasks/<project-id>.jsonl keyed by harp IDs",
        "(built from github.com/ctxloom/ctxloom, crates/root/bin/taskloom). Tasks live in a per-project append-only log\n    \
         at ~/.ctxloom/projects/<project-id>/state.db keyed by harp IDs",
    ),
    // 6. run.installation: Rust has no install channel yet.
    (
        "    The ctxloom install script and `brew install ctxloom/tap/taskloom` ship the\n    \
         taskloom binary (fallback: go install github.com/ctxloom/ctxloom/cmd/taskloom@latest).\n",
        "    \u{ab}HUMAN: how the Rust taskloom binary is installed; it has no install channel yet.\u{bb}\n",
    ),
    // 7. mcp.taskloom.notes: built from the Rust crate.
    (
        "(built from github.com/ctxloom/ctxloom,\n        cmd/taskloom). The",
        "(built from github.com/ctxloom/ctxloom,\n        crates/root/bin/taskloom). The",
    ),
    // 8. mcp.taskloom.installation: no install command yet.
    (
        "      installation: brew install ctxloom/tap/taskloom\n",
        "      installation: \"\u{ab}HUMAN: the Rust taskloom's install command; none exists yet\u{bb}\"\n",
    ),
    // 9. The fragment: the SQLite store.
    (
        "(~/.ctxloom/tasks/<project-id>.jsonl) and are keyed by harp IDs",
        "(~/.ctxloom/projects/<project-id>/state.db) and are keyed by harp IDs",
    ),
    // 10. The fragment: the formats Rust's --format accepts.
    (
        "`--format` also accepts yaml, toml, text, markdown).",
        "`--format` also accepts text).",
    ),
];

/// `go` with each pair applied, each `old` found exactly once.
pub fn applied(go: &str, pairs: &[(&str, &str)]) -> String {
    let mut text = go.to_string();
    for (old, new) in pairs {
        let found = text.matches(old).count();
        assert!(
            found == 1,
            "a reviewed change's old text occurs {found} times, not once, in Go's text: \
             Go has changed it, so review the change again: {old:?}"
        );
        text = text.replacen(old, new, 1);
    }
    text
}
