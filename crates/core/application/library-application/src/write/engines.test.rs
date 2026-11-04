use super::*;

// engines/claude/permission_model.go
#[test]
fn test_claude_takes_its_four_modes_without_regard_to_case() {
    for (said, want) in [
        ("default", "default"),
        ("acceptEdits", "acceptEdits"),
        ("ACCEPTEDITS", "acceptEdits"),
        (" plan ", "plan"),
        ("bypassPermissions", "bypassPermissions"),
        ("BypassPermissions", "bypassPermissions"),
    ] {
        assert_eq!(mode("claude-code", said).as_deref(), Ok(want), "{said:?}");
    }
}

#[test]
fn test_claude_refuses_a_mode_that_is_not_its_own_in_its_words() {
    assert_eq!(
        mode("claude-code", "weird").unwrap_err(),
        "claude permissions: mode \"weird\" is not a claude mode (known: default|acceptEdits|plan|bypassPermissions)"
    );
    // A retired spelling says what it became.
    assert_eq!(
        mode("claude-code", "bypass").unwrap_err(),
        "claude permissions: mode \"bypass\" is declared as mode: bypassPermissions"
    );
    assert_eq!(
        mode("claude-code", "dontAsk").unwrap_err(),
        "claude permissions: mode \"dontAsk\" is declared as approver: none"
    );
    assert_eq!(
        mode("claude-code", "AUTO").unwrap_err(),
        "claude permissions: mode \"AUTO\" is declared as approver: reviewer"
    );
}

// engines/mock/permission_model.go, engines/opencode/permission_model.go
#[test]
fn test_mock_and_opencode_take_their_modes_exactly() {
    for engine in ["mock", "mock-launch", "mock-lossy", "mock-noskills"] {
        assert_eq!(mode(engine, "bypass").as_deref(), Ok("bypass"));
        assert_eq!(
            mode(engine, "acceptEdits").unwrap_err(),
            "mock permissions: mode acceptEdits (known: default|plan|bypass)"
        );
        assert!(mode(engine, "Plan").is_err());
    }
    assert_eq!(mode("opencode", "auto").as_deref(), Ok("auto"));
    assert_eq!(
        mode("opencode", "plan2").unwrap_err(),
        "opencode permissions: mode plan2 (known: build|plan|auto)"
    );
}

#[test]
fn test_an_engine_nobody_knows_takes_no_mode() {
    assert!(mode("nosuch", "plan").unwrap_err().contains("nosuch"));
    assert!(!known("nosuch") && known("claude-code") && known("mock-noskills"));
}

#[test]
fn test_the_postures_are_each_engines_modes() {
    assert_eq!(
        postures("claude-code"),
        ["default", "acceptEdits", "plan", "bypassPermissions"]
    );
    assert_eq!(postures("opencode"), ["build", "plan", "auto"]);
    assert_eq!(postures("mock"), ["default", "plan", "bypass"]);
    assert!(postures("nosuch").is_empty());
}

// engines/claude/surfaces.go and definition.go
#[test]
fn test_claude_offers_one_approach_per_surface() {
    for (kind, approach) in [
        ("context", "system-prompt"),
        ("mcp", "mcp-config"),
        ("settings", "settings"),
        ("hooks", "settings-hooks"),
        ("commands", "commands-dir"),
        ("skills", "skills-dir"),
    ] {
        assert_eq!(
            approaches("claude-code", kind),
            Some([approach].as_slice()),
            "{kind}"
        );
    }
    assert_eq!(approaches("claude-code", "nosuch"), None);
    // An engine whose declaration is not carried yet answers none: the caller
    // checks the shape alone.
    assert_eq!(approaches("opencode", "context"), None);
}

#[test]
fn test_the_vocabularies_are_the_ones_goes() {
    assert_eq!(ROOTS, ["session-home", "project-root", "work-dir"]);
    assert_eq!(SURFACE_KINDS.len(), 6);
}
