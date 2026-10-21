//! Tag queries: tagma-core held to Go taskloom's answers, and Go's own
//! filterTasks cases.
//!
//! `fixtures/tag-queries/cases.tsv` holds the cases; `go.tsv` is Go's
//! filterTasks (tagma's Go port) over them, written by
//! `scripts/compare-go-tag-queries.sh`. Every query answers as Go answers,
//! but the infix ones, which Go refuses (it reads postfix only): each
//! answers as the postfix it means does in Go.

use std::collections::{BTreeMap, BTreeSet};

use annals_diagnostic::Diagnostic;

use super::{check, select, QueryError, QUERY_GRAMMAR};
use crate::schema::TagSchema;
use crate::tags::parse;
use crate::Tag;

const CASES: &str = include_str!("../fixtures/tag-queries/cases.tsv");
const GO: &str = include_str!("../fixtures/tag-queries/go.tsv");

/// Each infix case, and the postfix case it means.
const INFIX: [(&str, &str, &str); 8] = [
    ("flat", "urgent and release", "urgent/release/and"),
    ("flat", "urgent or release", "urgent/release/or"),
    ("flat", "not urgent", "urgent/not"),
    ("flat", "urgent release", "urgent/release"),
    (
        "flat",
        "(urgent or release) and not level=3",
        "urgent/release/or/level=3/not/and",
    ),
    ("flat", "urgent AND NOT release", "urgent/release/not/and"),
    ("flat", "not (urgent or release)", "urgent/release/or/not"),
    ("status-scoped", "not urgent", "urgent/not"),
];

#[derive(Default)]
struct Scenario {
    schema: Vec<String>,
    statuses: Vec<String>,
    /// `(harp, status, tags)`.
    tasks: Vec<(String, String, Vec<Tag>)>,
    queries: Vec<String>,
}

fn scenarios() -> BTreeMap<String, Scenario> {
    let mut all: BTreeMap<String, Scenario> = BTreeMap::new();
    let mut current = String::new();
    for line in CASES
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields[0] == "scenario" {
            current = fields[1].to_string();
            all.insert(current.clone(), Scenario::default());
            continue;
        }
        let scenario = all.get_mut(&current).expect("a scenario first");
        match fields[0] {
            "schema" => scenario.schema.push(fields[1].to_string()),
            "status" => scenario.statuses.push(fields[1].to_string()),
            "task" => scenario.tasks.push((
                fields[1].to_string(),
                fields[2].to_string(),
                fields[3..]
                    .iter()
                    .map(|t| parse(t).expect("a tag").expect("parses"))
                    .collect(),
            )),
            "query" => scenario
                .queries
                .push(fields.get(1).copied().unwrap_or_default().to_string()),
            other => panic!("unknown line {other}"),
        }
    }
    all
}

/// Go's answer to each `(scenario, query)`: the ids, or `None` for an
/// error.
fn go() -> BTreeMap<(String, String), Option<BTreeSet<String>>> {
    GO.lines()
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            let answer = match fields[2] {
                "ok" => Some(
                    fields[3]
                        .split(',')
                        .filter(|id| !id.is_empty())
                        .map(str::to_string)
                        .collect(),
                ),
                _ => None,
            };
            ((fields[0].to_string(), fields[1].to_string()), answer)
        })
        .collect()
}

fn run(scenario: &Scenario, query: &str) -> Result<BTreeSet<String>, QueryError> {
    let schema = TagSchema::parse(&scenario.schema).expect("the schema parses");
    let candidates = scenario
        .tasks
        .iter()
        .filter(|(_, status, _)| scenario.statuses.is_empty() || scenario.statuses.contains(status))
        .map(|(harp, _, tags)| (harp.as_str(), tags.as_slice()));
    select(query, candidates, &schema)
}

#[test]
fn test_every_query_answers_as_go_taskloom_answers() {
    let go = go();
    let infix: BTreeMap<(&str, &str), &str> =
        INFIX.iter().map(|(s, i, p)| ((*s, *i), *p)).collect();
    let mut compared = 0;
    let mut differences = Vec::new();
    for (name, scenario) in scenarios() {
        for query in &scenario.queries {
            let key = (name.clone(), query.clone());
            let ours = run(&scenario, query).ok();
            let theirs = match infix.get(&(name.as_str(), query.as_str())) {
                Some(postfix) => {
                    assert_eq!(go[&key], None, "Go reads no infix: {query}");
                    go[&(name.clone(), postfix.to_string())].clone()
                }
                None => go[&key].clone(),
            };
            compared += 1;
            if ours != theirs {
                differences.push(format!("{name} {query:?}: ours {ours:?}, Go's {theirs:?}"));
            }
        }
    }
    assert_eq!(differences, Vec::<String>::new());
    assert_eq!(compared, go.len(), "every case Go answered");
}

// Go's TestFilterTasksMalformedTagQueryErrors and its remedy: a malformed
// query fails loud, saying how to write one.
#[test]
fn test_a_malformed_query_is_refused_with_the_grammar() {
    let error = check("and").unwrap_err();
    assert_eq!(error.code(), "tasks-application-tag-query");
    assert!(error.to_string().contains(QUERY_GRAMMAR), "{error}");
    assert!(error.to_string().contains("and"), "{error}");
    let names: Vec<&str> = error.args().into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, ["query", "postfix", "infix", "grammar"]);
    assert!(!error.postfix.is_empty() && !error.infix.is_empty());
    assert_eq!(check(""), Ok(()));
    assert_eq!(check("urgent and release"), Ok(()));
    assert_eq!(check("urgent/release/and"), Ok(()));
}

#[test]
fn test_an_empty_query_selects_every_candidate() {
    let tags = [parse("x").unwrap().unwrap()];
    let selected = select(
        " ",
        [("a", &tags[..]), ("b", &[][..])],
        &TagSchema::default(),
    );
    assert_eq!(selected.unwrap().len(), 2);
}

// The schema's type declarations are indexed, but never answer a query:
// the item holding them is no candidate.
#[test]
fn test_the_type_declarations_are_never_selected() {
    let schema = TagSchema::parse([r#"tagma.type:"triage:x"=semver"#]).unwrap();
    let selected = select("*:*", [("a", &[][..])], &schema).unwrap();
    assert_eq!(selected, BTreeSet::from(["a".to_string()]));
    let selected = select("tagma.type:*", [("a", &[][..])], &schema).unwrap();
    assert!(selected.is_empty());
}
