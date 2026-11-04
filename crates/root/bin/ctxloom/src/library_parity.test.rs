//! Go parity (S1): the same commands on the same fixtures give what Go
//! ctxloom gave.
//!
//! `fixtures/parity.tsv` lists the commands; `scripts/go-library-goldens.sh`
//! ran each against Go ctxloom (release/0.7, 860336c4e, which is ca14d6046 or
//! later) in a copy of its fixture, and kept the output as
//! `fixtures/golden/<id>.stdout` and `<id>.exit`. These tests run the same
//! lines against this binary and compare. What consumers read (the VS Code
//! extension, agents, companions) is what is held: the JSON of every list and
//! show, the premise index, the words of the text forms.
//!
//! WHAT IS NOT COMPARED, and why (each a later slice's or an owner's):
//!
//! - Go's own companion loadout (the fragment `isolation-axes`, the `ctxloom`
//!   MCP server) is taken out of the goldens (the script says how): its texts
//!   are the owner's (D24) and name MCP tools whose cut list is open.
//! - A dry run's `resolved`, `environment`, `delivery` and `engine_home`
//!   (engine surfaces and isolation: S9 and S11), and the text sections that
//!   print them. Its `llm`, `backend`, `profiles`, `fragments`, `context`,
//!   `tokens`, `prompt` and `findings` (D16) are compared.
//! - Error texts. A failing command's stdout is empty in both and its exit
//!   code is compared; the words are asserted in `library_cli.test.rs` from
//!   the Gherkin. A usage error is exit 2 here (D28, ruled), where Go's
//!   cobra gave 1: `USAGE_REFUSALS`.

use std::collections::BTreeMap;

use serde_json::Value;

use super::library::{fixtures, Project};

/// Rows where Go's exit 1 is clap's usage refusal in this binary: D28 maps
/// those to 2.
const USAGE_REFUSALS: [&str; 2] = ["config-misspelled-subcommand", "run-dry-agent-and-profile"];

/// Rows where the owner overrides Go (D15, 2026-11-03): a config whose
/// `schema_version` is newer, older, unstamped or not a number is refused with
/// exit 2, where Go warned and read on (`config_version_refusal_test.go`).
const OWNER_REFUSALS: [&str; 8] = [
    "cfg-newer-show-json",
    "cfg-newer-bundle-list",
    "cfg-older-show-json",
    "cfg-older-bundle-list",
    "cfg-unstamped-show-json",
    "cfg-unstamped-bundle-list",
    "cfg-notint-show-json",
    "cfg-notint-bundle-list",
];

/// A dry run's keys that are compared (D16); the rest print later slices'
/// surfaces.
const DRY_RUN_KEYS: [&str; 9] = [
    "agent",
    "runtime",
    "llm",
    "backend",
    "profiles",
    "fragments",
    "context",
    "tokens",
    "prompt",
];

/// A dry run's text sections that print later slices' surfaces.
const DRY_RUN_SECTIONS: [&str; 3] = ["Environment", "Delivery", "Context File"];

struct Row {
    id: String,
    fixture: String,
    env: Vec<(String, String)>,
    args: Vec<String>,
}

fn rows() -> Vec<Row> {
    let text = std::fs::read_to_string(fixtures().join("parity.tsv")).expect("parity.tsv");
    text.lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|line| {
            let mut fields = line.splitn(3, '\t');
            let id = fields.next().expect("an id").to_string();
            let fixture = fields.next().expect("a fixture").to_string();
            let mut env = Vec::new();
            let mut args = Vec::new();
            for word in fields.next().expect("arguments").split_whitespace() {
                match word.split_once('=') {
                    Some((name, value))
                        if args.is_empty()
                            && name.chars().all(|c| c.is_ascii_uppercase() || c == '_') =>
                    {
                        env.push((name.to_string(), value.to_string()));
                    }
                    _ => args.push(word.to_string()),
                }
            }
            Row {
                id,
                fixture,
                env,
                args,
            }
        })
        .collect()
}

fn golden(id: &str, extension: &str) -> String {
    let path = fixtures().join("golden").join(format!("{id}.{extension}"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn is_dry_run(id: &str) -> bool {
    id.contains("run-dry") || id.starts_with("prof-run")
}

/// The text without the named `=== Section ===` blocks.
fn without_sections(text: &str, names: &[&str]) -> String {
    let mut out = Vec::new();
    let mut skipping = false;
    for line in text.lines() {
        if let Some(name) = line
            .strip_prefix("=== ")
            .and_then(|l| l.strip_suffix(" ==="))
        {
            let name = name.split(" (").next().unwrap_or(name);
            skipping = names.contains(&name);
        }
        if !skipping {
            out.push(line);
        }
    }
    out.join("\n")
}

/// Why `row`'s run differs from Go's, if it does.
fn mismatch(row: &Row, project: &Project) -> Option<String> {
    let args: Vec<&str> = row.args.iter().map(String::as_str).collect();
    let env: Vec<(&str, &str)> = row
        .env
        .iter()
        .map(|(n, v)| (n.as_str(), v.as_str()))
        .collect();
    let ran = project.run_with(&env, &args);

    let go_exit: i32 = golden(&row.id, "exit")
        .trim()
        .parse()
        .expect("an exit code");
    if OWNER_REFUSALS.contains(&row.id.as_str()) {
        // owner override of Go, D15, 2026-11-03
        return (ran.code != 2 || !ran.stdout.trim().is_empty())
            .then(|| format!("want a refusal (exit 2, no answer): {}", ran.said()));
    }
    let want_exit = if USAGE_REFUSALS.contains(&row.id.as_str()) {
        2
    } else {
        go_exit
    };
    if ran.code != want_exit {
        return Some(format!(
            "exit {} (Go {go_exit}, want {want_exit}): {}",
            ran.code,
            ran.said()
        ));
    }
    let want = golden(&row.id, "stdout");
    let got = project.normalized(&ran.stdout);
    if want.trim().is_empty() {
        return (!got.trim().is_empty()).then(|| format!("stdout should be empty: {got:?}"));
    }
    match serde_json::from_str::<Value>(&want) {
        Ok(want) => {
            let Ok(got) = serde_json::from_str::<Value>(&got) else {
                return Some(format!("stdout is not JSON: {got:?}"));
            };
            json_mismatch(&row.id, &want, &got)
        }
        Err(_) => {
            let (want, got) = if is_dry_run(&row.id) {
                (
                    without_sections(&want, &DRY_RUN_SECTIONS),
                    without_sections(&got, &DRY_RUN_SECTIONS),
                )
            } else {
                (want, got)
            };
            (want.trim_end() != got.trim_end())
                .then(|| format!("text differs\n--- Go\n{want}\n--- here\n{got}"))
        }
    }
}

fn json_mismatch(id: &str, want: &Value, got: &Value) -> Option<String> {
    if !is_dry_run(id) {
        return (want != got).then(|| format!("JSON differs\n--- Go\n{want:#}\n--- here\n{got:#}"));
    }
    let mut wrong = Vec::new();
    for key in DRY_RUN_KEYS {
        if want.get(key) != got.get(key) {
            wrong.push(format!(
                "{key}: Go {:?}, here {:?}",
                want.get(key),
                got.get(key)
            ));
        }
    }
    // Findings: how many, and which are fatal (their words are the
    // catalog's).
    let flags = |doc: &Value| -> Vec<(Value, Value)> {
        doc["findings"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|f| (f["kind"].clone(), f["fatal"].clone()))
                    .collect()
            })
            .unwrap_or_default()
    };
    if flags(want) != flags(got) {
        wrong.push(format!(
            "findings: Go {:?}, here {:?}",
            flags(want),
            flags(got)
        ));
    }
    (!wrong.is_empty()).then(|| wrong.join("\n"))
}

/// Every row whose fixture starts with `prefix`, against this binary.
fn check(prefix: &str) {
    let mut projects: BTreeMap<String, Project> = BTreeMap::new();
    let mut wrong = Vec::new();
    let mut checked = 0;
    for row in rows().iter().filter(|r| r.fixture.starts_with(prefix)) {
        let project = projects
            .entry(row.fixture.clone())
            .or_insert_with(|| Project::from_fixture(&row.fixture));
        checked += 1;
        if let Some(why) = mismatch(row, project) {
            wrong.push(format!("{}: {why}", row.id));
        }
    }
    assert!(checked > 0, "no row for {prefix}");
    assert!(
        wrong.is_empty(),
        "{} of {checked} differ from Go:\n{}",
        wrong.len(),
        wrong.join("\n\n")
    );
}

// Go's acceptance features for each noun, on the fixture `lib`.
#[test]
fn test_lib_fixture_matches_go() {
    check("lib");
}

// internal/core/composite/assemble_golden_test.go's corpus.
#[test]
fn test_gold_corpus_matches_go() {
    check("gold");
}

// internal/core/bundles/upgrade.go envelopeSteps: generations 0 to 4.
#[test]
fn test_envelope_generations_match_go() {
    check("gens");
}

// fault_tolerance.feature: a malformed bundle is skipped and named.
#[test]
fn test_malformed_bundles_match_go() {
    check("bad");
}

// internal/core/profiles/profiles.go ResolveProfile, composite/select.go.
#[test]
fn test_profile_resolution_matches_go() {
    check("prof");
}

// config.feature, fault_tolerance.feature: schema 8 and older, layering.
#[test]
fn test_configs_match_go() {
    check("cfg/");
}

// empty_listing_hint, cli/agent.feature: nothing there.
#[test]
fn test_empty_projects_match_go() {
    check("empty");
}
