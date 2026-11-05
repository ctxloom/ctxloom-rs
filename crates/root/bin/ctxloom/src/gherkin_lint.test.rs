//! The parity lint: a Rust `.feature` file says what it ported from Go and
//! what it left out, and every scenario it ports reads exactly as Go's does.
//!
//! The convention (every file under `features/`), as comment lines at the
//! head of the file, before `Feature:`:
//!
//! ```text
//! # world: base                      (base | surfaces | isolation: the cucumber World that runs it)
//! # Ported from cli/run.feature      (a path under the Go repo's tests/acceptance/features; one line per Go file)
//! #   free prose may continue on lines that do not start a `key:`
//! # NOT ported: <what is left out, and why>   (or "nothing")
//! ```
//!
//! Each scenario of the file is then either Go's, word for word, or marked.
//! Inside the scenario (between its `Scenario:` line and the next scenario),
//! a comment line says so:
//!
//! - `# substitution: <what differs from Go, and why>`: the scenario is Go's
//!   with a deliberate difference (a mock engine this port registers, a
//!   clap wording). The lint does not compare it, the comment is the record.
//! - `# new: <why>`: the scenario is not in Go at all.
//!
//! The same markers go in the `Background:` block. The lint compares each
//! unmarked scenario's steps (keyword, text, docstring, table) and its
//! `Examples:` tables with the same-named scenario in the listed Go files;
//! tags and descriptions are not compared. Without the Go checkout
//! (`CTXLOOM_GO_FEATURES`, default `GO_FEATURES` below) the comparison is
//! skipped with a message; the header checks still run.
//!
//! Porting the next scenario: copy it from Go unchanged, add the file's
//! `# Ported from` and `# NOT ported` lines, give it the `# world:` of the
//! World whose steps it needs (`gherkin*.test.rs`; `gherkin_common` has the
//! steps all share), and run `just test`: the discovery test runs it, this
//! lint compares it. A step the Rust port must say differently is a
//! `# substitution:`, never a silent edit.

use std::path::{Path, PathBuf};

use cucumber::gherkin::{Examples, Feature, GherkinEnv, Scenario, Step};

use super::gherkin_common::{feature_files, features_dir, header_values, world_of};
use super::host_var;
use crate::tests::files;

/// The Go checkout's feature files.
const GO_FEATURES: &str = "/home/babbitt/workspace/ctxloom/ctxloom/main/tests/acceptance/features";

fn go_dir() -> PathBuf {
    host_var("CTXLOOM_GO_FEATURES")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(GO_FEATURES))
}

/// A scenario or a background, as the lint compares it.
struct Unit {
    /// `Background` or the scenario's name.
    name: String,
    /// The line it starts on (1-based).
    line: usize,
    /// Its steps and examples, one canonical text.
    body: String,
}

fn step_text(step: &Step) -> String {
    let mut text = format!("{} {}", step.keyword.trim(), step.value.trim());
    if let Some(doc) = &step.docstring {
        text.push_str("\n  \"\"\"");
        for line in doc.lines() {
            text.push_str(&format!("\n  {}", line.trim_end()));
        }
        text.push_str("\n  \"\"\"");
    }
    if let Some(table) = &step.table {
        for row in &table.rows {
            text.push_str(&format!("\n  | {} |", row.join(" | ")));
        }
    }
    text
}

fn examples_text(examples: &Examples) -> String {
    let mut text = format!("\nExamples: {}", examples.name.as_deref().unwrap_or(""));
    if let Some(table) = &examples.table {
        for row in &table.rows {
            text.push_str(&format!("\n  | {} |", row.join(" | ")));
        }
    }
    text
}

fn scenario_unit(scenario: &Scenario) -> Unit {
    let mut body = scenario
        .steps
        .iter()
        .map(step_text)
        .collect::<Vec<_>>()
        .join("\n");
    for examples in &scenario.examples {
        body.push_str(&examples_text(examples));
    }
    Unit {
        name: scenario.name.clone(),
        line: scenario.position.line,
        body,
    }
}

/// The feature's backgrounds and scenarios (a rule's included), in file order.
fn units(feature: &Feature) -> Vec<Unit> {
    let mut found = Vec::new();
    let background = |b: &cucumber::gherkin::Background| Unit {
        name: "Background".to_string(),
        line: b.position.line,
        body: b.steps.iter().map(step_text).collect::<Vec<_>>().join("\n"),
    };
    if let Some(b) = &feature.background {
        found.push(background(b));
    }
    found.extend(feature.scenarios.iter().map(scenario_unit));
    for rule in &feature.rules {
        if let Some(b) = &rule.background {
            found.push(background(b));
        }
        found.extend(rule.scenarios.iter().map(scenario_unit));
    }
    found.sort_by_key(|u| u.line);
    found
}

/// Every line a unit owns: from its keyword line up to the next unit's (or a
/// Rule's, or the end of the file).
fn region<'a>(text: &'a str, feature: &Feature, unit: &Unit) -> Vec<&'a str> {
    let mut starts: Vec<usize> = units(feature).iter().map(|u| u.line).collect();
    starts.extend(feature.rules.iter().map(|r| r.position.line));
    let end = starts
        .into_iter()
        .filter(|&l| l > unit.line)
        .min()
        .unwrap_or(usize::MAX);
    text.lines()
        .enumerate()
        .filter(|(i, _)| i + 1 >= unit.line && i + 1 < end)
        .map(|(_, l)| l.trim())
        .collect()
}

fn marker<'a>(region: &[&'a str], key: &str) -> Option<&'a str> {
    let prefix = format!("# {key}:");
    region
        .iter()
        .find_map(|l| l.strip_prefix(prefix.as_str()))
        .map(str::trim)
}

fn parse(text: &str) -> Result<Feature, String> {
    Feature::parse(text, GherkinEnv::default()).map_err(|e| e.to_string())
}

/// The header's findings: a world, the Go files ported from, what is left out.
fn header_findings(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    if let Err(e) = world_of(text) {
        found.push(e);
    }
    let left_out = header_values(text, "# NOT ported:");
    if left_out.iter().all(|v| v.is_empty()) {
        found.push(
            "no `# NOT ported: <what is left out and why>` header line (say `nothing` if so)"
                .to_string(),
        );
    }
    found
}

/// The Go files a feature says it ported from.
fn ported_from(text: &str) -> Vec<String> {
    header_values(text, "# Ported from")
        .into_iter()
        .filter_map(|v| v.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

/// A scenario's findings against the Go files' units (each Go file's units).
fn compare(rust_text: &str, go: &[(String, Vec<Unit>)]) -> Vec<String> {
    let mut found = Vec::new();
    let feature = match parse(rust_text) {
        Ok(f) => f,
        Err(e) => return vec![format!("does not parse: {e}")],
    };
    for unit in units(&feature) {
        let region = region(rust_text, &feature, &unit);
        if marker(&region, "new").is_some() {
            continue;
        }
        let kind = if unit.name == "Background" {
            "Background".to_string()
        } else {
            format!("scenario {:?}", unit.name)
        };
        let same_name: Vec<&(String, Vec<Unit>)> = go
            .iter()
            .filter(|(_, units)| units.iter().any(|g| g.name == unit.name))
            .collect();
        let substituted = marker(&region, "substitution");
        if substituted.is_some_and(str::is_empty) {
            found.push(format!(
                "{kind} (line {}): `# substitution:` says nothing; say what differs and why",
                unit.line
            ));
            continue;
        }
        if substituted.is_some() {
            if same_name.is_empty() {
                found.push(format!(
                    "{kind} (line {}): `# substitution:` but no Go scenario of that name in {:?}",
                    unit.line,
                    go.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>()
                ));
            }
            continue;
        }
        if same_name.is_empty() {
            found.push(format!(
                "{kind} (line {}): not in {:?}; mark it `# new: <why>` (or rename it to Go's)",
                unit.line,
                go.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>()
            ));
            continue;
        }
        let equal = same_name
            .iter()
            .flat_map(|(_, units)| units.iter().filter(|g| g.name == unit.name))
            .any(|g| g.body == unit.body);
        if !equal {
            let go_body = same_name
                .iter()
                .flat_map(|(_, units)| units.iter().filter(|g| g.name == unit.name))
                .map(|g| g.body.as_str())
                .next()
                .unwrap_or_default();
            found.push(format!(
                "{kind} (line {}) differs from Go's; mark it `# substitution: <what and why>`:\n{}",
                unit.line,
                first_difference(go_body, &unit.body)
            ));
        }
    }
    found
}

/// The first line where Go's text and Rust's part, for a message.
fn first_difference(go: &str, rust: &str) -> String {
    let (go_lines, rust_lines): (Vec<_>, Vec<_>) = (go.lines().collect(), rust.lines().collect());
    for i in 0..go_lines.len().max(rust_lines.len()) {
        let (g, r) = (go_lines.get(i), rust_lines.get(i));
        if g != r {
            return format!(
                "      Go:   {}\n      Rust: {}",
                g.copied().unwrap_or("(nothing)"),
                r.copied().unwrap_or("(nothing)")
            );
        }
    }
    "      (no line differs)".to_string()
}

fn go_units(dir: &Path, rel: &str) -> Result<Vec<Unit>, String> {
    let path = dir.join(rel);
    let text = files::read(&path)
        .map_err(|_| format!("`# Ported from {rel}`: no such file in the Go checkout"))?;
    Ok(units(&parse(&text).map_err(|e| format!("{rel}: {e}"))?))
}

/// All the findings of one Rust feature file; `go` is the Go checkout, when
/// there is one.
fn findings(rel: &str, text: &str, go: Option<&Path>) -> Vec<String> {
    let mut found = header_findings(text);
    let sources = ported_from(text);
    let feature = parse(text);
    let all_new = feature.as_ref().is_ok_and(|f| {
        units(f)
            .iter()
            .all(|u| marker(&region(text, f, u), "new").is_some())
    });
    if sources.is_empty() && !all_new {
        found.push("no `# Ported from <path under the Go features>` header line (and not every scenario is `# new:`)".to_string());
    }
    if let Some(dir) = go {
        let mut units = Vec::new();
        for source in &sources {
            match go_units(dir, source) {
                Ok(u) => units.push((source.clone(), u)),
                Err(e) => found.push(e),
            }
        }
        found.extend(compare(text, &units));
    }
    found.into_iter().map(|f| format!("{rel}: {f}")).collect()
}

#[test]
fn every_feature_declares_what_it_ported_and_says_what_differs() {
    let dir = go_dir();
    let go = files::is_dir(&dir).then_some(dir.as_path());
    if go.is_none() {
        eprintln!(
            "SKIPPED the Go comparison: no Go checkout at {} (set CTXLOOM_GO_FEATURES); the header checks still run",
            dir.display()
        );
    }
    let mut found = Vec::new();
    for rel in feature_files() {
        let text = files::read(features_dir().join(&rel)).expect("a feature file");
        found.extend(findings(&rel, &text, go));
    }
    assert!(
        found.is_empty(),
        "{} parity finding(s) (convention: gherkin_lint.test.rs):\n{}",
        found.len(),
        found.join("\n")
    );
}

#[cfg(test)]
mod lint {
    use super::*;

    const GO: &str = "Feature: f\n  Background:\n    Given a thing\n\n  Scenario: one\n    When I run \"ctxloom a\"\n    Then the command succeeds\n\n  Scenario Outline: two\n    When I run \"ctxloom <x>\"\n    Examples:\n      | x |\n      | b |\n";

    fn go() -> Vec<(String, Vec<Unit>)> {
        vec![("f.feature".to_string(), units(&parse(GO).unwrap()))]
    }

    #[test]
    fn identical_text_is_clean_whatever_its_layout() {
        let rust = GO.replace("    ", "      ").replace("\n\n", "\n# note\n\n");
        assert_eq!(compare(&rust, &go()), Vec::<String>::new());
    }

    #[test]
    fn a_changed_step_is_a_finding_that_names_both_lines() {
        let rust = GO.replace("ctxloom a", "ctxloom z");
        let found = compare(&rust, &go());
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("scenario \"one\""), "{found:?}");
        assert!(found[0].contains("ctxloom a") && found[0].contains("ctxloom z"));
    }

    #[test]
    fn a_changed_example_row_and_a_changed_background_are_findings() {
        let rust = GO.replace("| b |", "| c |").replace("a thing", "another");
        let found = compare(&rust, &go());
        assert_eq!(found.len(), 2, "{found:?}");
    }

    #[test]
    fn a_substitution_comment_excuses_its_scenario_only() {
        let rust = GO
            .replace("ctxloom a", "ctxloom z")
            .replace("| b |", "| c |")
            .replace(
                "Scenario: one\n",
                "Scenario: one\n    # substitution: z is a here\n",
            );
        let found = compare(&rust, &go());
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("\"two\""));
    }

    #[test]
    fn an_empty_substitution_and_an_unknown_scenario_are_findings() {
        let rust = GO.replace("Scenario: one\n", "Scenario: one\n    # substitution:\n")
            + "\n  Scenario: three\n    Then x\n";
        let found = compare(&rust, &go());
        assert_eq!(found.len(), 2, "{found:?}");
        let marked = rust.replace(
            "Scenario: three\n",
            "Scenario: three\n    # new: Rust's own\n",
        );
        assert_eq!(compare(&marked, &go()).len(), 1);
    }

    #[test]
    fn the_header_needs_a_world_and_a_not_ported_line() {
        assert_eq!(header_findings("Feature: f\n").len(), 2);
        let ok = "# world: base\n# NOT ported: nothing\nFeature: f\n";
        assert!(header_findings(ok).is_empty());
        assert!(header_findings("# world: nope\n# NOT ported:\n").len() == 2);
    }

    #[test]
    fn ported_from_takes_the_path_and_ignores_the_prose() {
        let head = "# Ported from cli/a.feature (rev x)\n# Ported from b.feature\n";
        assert_eq!(ported_from(head), vec!["cli/a.feature", "b.feature"]);
    }
}
