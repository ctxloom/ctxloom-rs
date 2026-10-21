//! What Rust taskloom's MCP text is: Go's (`go/`, which
//! `scripts/go-companion-fixtures.sh` regenerates from the Go source),
//! with the factual changes the human reviewed and approved applied.
//!
//! Each change is an exact `(old, new)` pair on the fixture's raw bytes,
//! before any JSON parse, so a pair in `tools.json` matches its escapes
//! (`\u003c` for `<`). Each `old` must occur exactly once: if Go rewords a
//! sentence a pair changes, the pair no longer matches and the parity tests
//! fail until the pair is reviewed again; if Go changes any other text, the
//! expected text changes and the byte comparison fails, as it did when the
//! text was Go's alone. The numbers are the review's (the port plan's
//! Decided: "The prose Rust taskloom serves").

/// Go's MCP server instructions, reviewed.
pub fn instructions() -> String {
    applied(include_str!("go/instructions.txt"), INSTRUCTIONS)
}

/// Go's tools, as tools/list answers them, reviewed.
pub fn tools() -> String {
    applied(include_str!("go/tools.json"), TOOLS)
}

pub const INSTRUCTIONS: &[(&str, &str)] = &[
    // 1. Rust's store is SQLite; it writes no jsonl.
    (
        "Never read the raw ~/.ctxloom/tasks/<project>.jsonl directly",
        "Never read the raw ~/.ctxloom/projects/<project>/state.db directly",
    ),
];

pub const TOOLS: &[(&str, &str)] = &[
    // 2. task_list: repo homing is dropped, and no notice says so.
    (
        r#"pass global=true to aggregate every PRIVATELY-homed project instead (a repo-homed project -- homing: repo -- is registered nowhere global and is never included, even if it is the current project; the result's notice field always says so when global is true)."#,
        r#"pass global=true to aggregate every project instead."#,
    ),
    (
        r#"falls back to global too, with the same notice field explaining why."#,
        r#"falls back to global too, with the notice field explaining why."#,
    ),
    // 3. task_list's global parameter: the same.
    (
        r#"aggregate tasks across every PRIVATELY-homed project instead of just the current one."#,
        r#"aggregate tasks across every project instead of just the current one."#,
    ),
    (
        r#" A repo-homed project (homing: repo, its log checked into \u003crepo\u003e/.taskloom/tasks.jsonl) is registered nowhere global and is NEVER included, even if it is the current project -- every global listing's notice field says so.""#,
        r#"""#,
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
