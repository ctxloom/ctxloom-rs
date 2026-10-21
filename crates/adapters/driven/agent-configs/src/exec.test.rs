//! Go's exectoken cases.

use super::{program, runs};

#[test]
fn test_the_program_is_the_first_tokens_base_name() {
    for (command, want) in [
        ("ctxloom hook hud", "ctxloom"),
        ("\"/usr/bin/ctxloom\" hook hud", "ctxloom"),
        ("/home/me/go/bin/ctxloom session bind", "ctxloom"),
        ("\"C:\\Tools\\ctxloom.exe\" hook stamp-plan", "ctxloom"),
        ("'/Apps/My Tools/ctxloom' mcp", "ctxloom"),
        ("ltk evaluate --config x", "ltk"),
        ("/usr/local/bin/ctxloomctl whatever", "ctxloomctl"),
        (" \ttaskloom\tmcp", "taskloom"),
        ("\"/unterminated taskloom", "unterminated taskloom"),
        ("", ""),
    ] {
        assert_eq!(program(command), want, "{command:?}");
    }
}

#[test]
fn test_an_entry_is_the_owners_when_it_runs_the_owners_program() {
    for command in [
        "taskloom",
        "taskloom mcp",
        "/usr/local/bin/taskloom mcp",
        "\"/opt/my tools/taskloom\" mcp",
    ] {
        assert!(runs(command, "taskloom"), "{command:?}");
    }
    for command in ["mywrapper mcp", "/usr/bin/taskloomctl", "npx taskloom", ""] {
        assert!(!runs(command, "taskloom"), "{command:?}");
    }
    assert!(!runs("taskloom", ""), "an empty owner owns nothing");
}
