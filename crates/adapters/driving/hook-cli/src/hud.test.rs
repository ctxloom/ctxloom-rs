//! The statusline. Go: `hook_hud_test.go` (release/0.7) and
//! session_hooks.feature ("The statusline reports the engine's session and
//! ctxloom's own").

use super::{
    bar_color, context_bar, hud, render, ContextWindow, Cost, StatusPayload, Worktree, CYAN, DIM,
    GREEN, RED, RESET, YELLOW,
};

fn model(raw: &str) -> StatusPayload {
    StatusPayload {
        model: serde_json::from_str(raw).unwrap_or(serde_json::Value::Null),
        ..StatusPayload::default()
    }
}

// Go: TestAgentSession_ModelName: an agent may send `model` as an object
// (Claude's {display_name|name|id}) or as a bare string, preferring
// display_name, then name, then id.
#[test]
fn the_model_name_is_resolved_from_an_object_or_a_bare_string() {
    for (raw, want) in [
        (
            r#"{"display_name":"Claude Sonnet","id":"claude-x"}"#,
            "Claude Sonnet",
        ),
        (r#"{"name":"gemini-3-flash"}"#, "gemini-3-flash"),
        (r#"{"id":"o3-mini"}"#, "o3-mini"),
        (r#""gpt-5-codex""#, "gpt-5-codex"),
        ("", ""),
    ] {
        assert_eq!(model(raw).model_name(), want, "{raw}");
    }
}

// Go: TestFormatHud_HarpDisplay: the harp is in the statusline when set, with
// its glyph; none when unset or empty.
#[test]
fn the_harp_is_shown_with_its_glyph_when_there_is_one() {
    let out = render(&StatusPayload::default(), "swift-amber-falcon", 0);
    assert!(out.contains("swift-amber-falcon"), "{out}");
    assert!(
        out.contains('\u{2301}'),
        "the harp section is prefixed with the glyph: {out}"
    );
    let out = render(&StatusPayload::default(), "", 0);
    assert!(!out.contains('\u{2301}'), "no harp, no section: {out}");
}

#[test]
fn the_sections_compose() {
    let payload = StatusPayload {
        model: serde_json::json!({"display_name": "Claude Sonnet"}),
        context_window: ContextWindow {
            used_percentage: Some(42.0),
        },
        cost: Cost {
            total_cost_usd: 1.23,
        },
        worktree: Worktree::default(),
    };
    let out = render(&payload, "bold-crimson-thunder", 2);
    for want in [
        "Claude Sonnet",
        "42%",
        "$1.23",
        "\u{2709} 2",
        "bold-crimson-thunder",
    ] {
        assert!(out.contains(want), "{want:?} in {out:?}");
    }
}

// Go's exact line: ANSI colors, sections joined with " │ " (U+2502).
#[test]
fn the_line_is_gos_byte_for_byte() {
    let payload = StatusPayload {
        model: serde_json::json!({"display_name": "Opus 5"}),
        context_window: ContextWindow {
            used_percentage: Some(42.0),
        },
        cost: Cost {
            total_cost_usd: 1.5,
        },
        worktree: Worktree {
            name: "wt".to_string(),
            branch: "feature".to_string(),
        },
    };
    let want = [
        format!("{CYAN}Opus 5{RESET}"),
        format!(
            "{GREEN}\u{2588}\u{2588}\u{2588}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591} 42%{RESET}"
        ),
        format!("{DIM}$1.50{RESET}"),
        format!("{DIM}\u{2301} brisk-copper-moth{RESET}"),
        format!("{YELLOW}\u{2709} 3{RESET}"),
        format!("{DIM}\u{2387} feature{RESET}"),
    ]
    .join(" \u{2502} ");
    assert_eq!(render(&payload, "brisk-copper-moth", 3), want);
}

// Go: a worktree with no branch shows its name.
#[test]
fn a_worktree_without_a_branch_shows_its_name() {
    let payload = StatusPayload {
        worktree: Worktree {
            name: "wt".to_string(),
            branch: String::new(),
        },
        ..StatusPayload::default()
    };
    assert_eq!(render(&payload, "", 0), format!("{DIM}\u{2387} wt{RESET}"));
}

// Go: TestFormatHud_NeverEmptyOnSuccess: "a sparse session JSON plus no
// resolvable ctxloom config previously made the statusline vanish, which looks
// identical to a broken install".
#[test]
fn a_sparse_payload_still_says_ctxloom() {
    assert_eq!(render(&StatusPayload::default(), "", 0), "ctxloom");
}

// Go: a null used_percentage (a session with no usage yet) renders nothing,
// "exactly as a zero did before it became distinguishable from one".
#[test]
fn a_null_or_zero_usage_renders_no_bar() {
    assert_eq!(
        hud(r#"{"context_window":{"used_percentage":null}}"#, "", 0),
        "ctxloom"
    );
    assert_eq!(
        hud(r#"{"context_window":{"used_percentage":0}}"#, "", 0),
        "ctxloom"
    );
}

// Go: runHookHud: "Fault tolerant: output minimal HUD on read error / parse
// error."
#[test]
fn a_payload_that_is_not_json_is_the_sentinel() {
    assert_eq!(hud("not json", "amber-swift-owl", 0), "ctxloom");
    assert_eq!(hud("", "amber-swift-owl", 0), "ctxloom");
}

// session_hooks.feature: model and context from the engine, the harp from
// ctxloom's own.
#[test]
fn the_engine_s_half_and_ctxloom_s_are_both_there() {
    let out = hud(
        r#"{"model":{"display_name":"Opus 5"},"context_window":{"used_percentage":42}}"#,
        "brisk-copper-moth",
        0,
    );
    for want in ["Opus 5", "42%", "brisk-copper-moth"] {
        assert!(out.contains(want), "{want:?} in {out:?}");
    }
}

// Go: TestContextBar.
#[test]
fn the_bar_is_eight_cells_filled_in_proportion_and_clamped() {
    for (pct, filled, unfilled) in [(0.0, 0, 8), (50.0, 4, 4), (100.0, 8, 0), (125.0, 8, 0)] {
        let bar = context_bar(pct);
        assert_eq!(
            bar.chars().filter(|c| *c == '\u{2588}').count(),
            filled,
            "{pct}"
        );
        assert_eq!(
            bar.chars().filter(|c| *c == '\u{2591}').count(),
            unfilled,
            "{pct}"
        );
    }
}

// Go: TestContextBarColor.
#[test]
fn the_color_follows_the_thresholds() {
    assert_eq!(bar_color(0.0), GREEN);
    assert_eq!(bar_color(59.0), GREEN);
    assert_eq!(bar_color(60.0), YELLOW);
    assert_eq!(bar_color(79.0), YELLOW);
    assert_eq!(bar_color(80.0), RED);
    assert_eq!(bar_color(99.0), RED);
}
