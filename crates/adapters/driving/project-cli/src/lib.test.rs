//! The group parsed as a binary mounts it, run over a fake port: what each
//! command asks for, and how its answer is written.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use annals_diagnostic::Report;
use clap::Parser;
use presentation::{Catalog, Format};
use project_application::{
    Change, Checked, IdSource, ProjectAdmin, Projects, Registered, StoreCheck, Survey,
};
use serde_json::{json, Value};

use super::{run, Group, OrphansAction, ProjectCommand, Settings};

/// A binary mounting the group beside nothing else.
#[derive(Parser, Debug)]
struct Bin {
    #[command(subcommand)]
    group: Group,
}

fn parse(args: &[&str]) -> Group {
    Bin::try_parse_from(std::iter::once("bin").chain(args.iter().copied()))
        .unwrap()
        .group
}

struct Fake {
    survey: Survey,
    pinned: Vec<Option<String>>,
    applied: Vec<Vec<Change>>,
}

impl ProjectAdmin for Fake {
    fn survey(&mut self, pinned: Option<&str>) -> Result<Survey, Report> {
        self.pinned.push(pinned.map(str::to_string));
        Ok(self.survey.clone())
    }

    fn apply(&mut self, _: Option<&str>, _: &Survey, changes: &[Change]) -> Result<(), Report> {
        self.applied.push(changes.to_vec());
        Ok(())
    }
}

/// A database check answering `found`.
struct FakeCheck {
    found: Checked,
}

impl StoreCheck for FakeCheck {
    fn check(&mut self, _: &Path) -> Result<Checked, Report> {
        Ok(self.found.clone())
    }
}

fn entry(id: &str, path: &str, exists: bool) -> Registered {
    Registered {
        id: id.to_string(),
        path: path.to_string(),
        canonical: path.to_string(),
        exists,
    }
}

/// /w marked `m` but registered as `a`; a stray database.
fn projects() -> Projects<Fake, FakeCheck> {
    checked(Checked::Intact)
}

/// [`projects`], its database's check finding `found`.
fn checked(found: Checked) -> Projects<Fake, FakeCheck> {
    let admin = Fake {
        survey: Survey {
            work_root: "/w".to_string(),
            root: "/w".to_string(),
            resolved: Some(("m".to_string(), IdSource::Marker)),
            marker: Some("m".to_string()),
            legacy_marker: None,
            registry: vec![entry("a", "/w", true), entry("t", "/gone", false)],
            databases: vec!["stray".to_string(), "t".to_string()],
            projects: PathBuf::from("/h/.ctxloom/projects"),
        },
        pinned: Vec::new(),
        applied: Vec::new(),
    };
    Projects::new(admin, FakeCheck { found })
}

/// A store's damage, its catalog message differing from its English.
const STORE: (&str, &str) = (
    "fake-store",
    "fake-store-damaged = from the catalog: { $first } (+{ $more })\n",
);

fn catalog() -> Catalog {
    Catalog::en_us(&[project_application::EN_US, super::EN_US, STORE]).expect("the catalog loads")
}

fn damage() -> Report {
    Report::new("fake-store-damaged", "the English")
        .arg("first", "row 2 missing from index i")
        .arg("more", 1u64)
}

fn format(json: bool) -> Format {
    if json {
        Format::Json
    } else {
        Format::Text
    }
}

struct Ran {
    code: u8,
    out: String,
    err: String,
}

fn ran(args: &[&str], json: bool, projects: &mut Projects<Fake, FakeCheck>) -> Ran {
    let settings = Settings {
        format: format(json),
        project: None,
        prog: "taskloom",
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(
        parse(args),
        &settings,
        &catalog(),
        projects,
        &mut out,
        &mut err,
    );
    Ran {
        code,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

fn document(ran: &Ran) -> Value {
    assert_eq!(ran.code, 0, "{}", ran.err);
    serde_json::from_str(&ran.out).expect("one JSON document")
}

#[test]
fn test_each_command_parses() {
    assert_eq!(
        parse(&["project", "show"]),
        Group::Project(ProjectCommand::Show)
    );
    assert_eq!(
        parse(&["project", "heal", "--yes"]),
        Group::Project(ProjectCommand::Heal { yes: true })
    );
    assert_eq!(
        parse(&["project", "repoint", "t", "--share"]),
        Group::Project(ProjectCommand::Repoint {
            project_id: "t".to_string(),
            share: true,
            yes: false,
        })
    );
    assert_eq!(
        parse(&["project", "orphans"]),
        Group::Project(ProjectCommand::Orphans { action: None })
    );
    assert_eq!(
        parse(&["project", "orphans", "remove", "x", "y"]),
        Group::Project(ProjectCommand::Orphans {
            action: Some(OrphansAction::Remove {
                project_ids: vec!["x".to_string(), "y".to_string()],
                yes: false,
            })
        })
    );
    assert!(
        Bin::try_parse_from(["bin", "project", "orphans", "remove"]).is_err(),
        "a removal names what it removes"
    );
}

#[test]
fn test_show_as_json_is_one_document_with_every_part() {
    let ran = ran(&["project", "show"], true, &mut projects());
    assert_eq!(
        document(&ran),
        json!({
            "project_id": "m",
            "source": "marker",
            "work_root": "/w",
            "root": "/w",
            "marker": "m",
            "legacy_marker": null,
            "entries": [{"project_id": "a", "path": "/w", "exists": true}],
            "database": "/h/.ctxloom/projects/m/state.db",
            "shared_with": [],
            "problems": [
                {"kind": "two-ids", "root": "/w", "project_ids": ["a", "m"]},
                {"kind": "gone", "project_id": "t", "path": "/gone"},
                {"kind": "orphan", "project_id": "stray", "database": "/h/.ctxloom/projects/stray"},
                {"kind": "orphan", "project_id": "t", "database": "/h/.ctxloom/projects/t"},
            ],
        })
    );
}

#[test]
fn test_show_as_json_lists_the_other_trees_sharing_the_project() {
    let mut survey = projects().admin().survey.clone();
    survey.registry = vec![
        entry("m", "/w", true),
        entry("m", "/clone", true),
        entry("m", "/gone", false),
    ];
    let admin = Fake {
        survey,
        pinned: Vec::new(),
        applied: Vec::new(),
    };
    let mut projects = Projects::new(
        admin,
        FakeCheck {
            found: Checked::Intact,
        },
    );
    let shown = document(&ran(&["project", "show"], true, &mut projects));
    assert_eq!(shown["shared_with"], json!(["/clone"]));
    assert_eq!(
        shown["problems"],
        json!([
            {"kind": "gone", "project_id": "m", "path": "/gone"},
            {"kind": "orphan", "project_id": "stray", "database": "/h/.ctxloom/projects/stray"},
            {"kind": "orphan", "project_id": "t", "database": "/h/.ctxloom/projects/t"},
        ]),
        "sharing is no problem"
    );
}

#[test]
fn test_show_as_text() {
    let ran = ran(&["project", "show"], false, &mut projects());
    assert_eq!(ran.code, 0);
    assert_eq!(
        ran.out.lines().take(5).collect::<Vec<_>>(),
        [
            "Project:  m (from the marker)",
            "Root:     /w",
            "Marker:   m",
            "Registry: a\t/w",
            "Database: /h/.ctxloom/projects/m/state.db",
        ]
    );
    assert!(ran
        .out
        .contains("Problems:\n  - /w is bound to more than one project: a, m"));
}

#[test]
fn test_heal_reports_and_changes_only_with_yes() {
    let mut projects = projects();
    let would = ran(&["project", "heal"], true, &mut projects);
    assert_eq!(
        document(&would),
        json!({
            "applied": false,
            "changes": [
                {"kind": "register", "project_id": "m", "path": "/w"},
                {"kind": "unregister", "project_id": "a", "path": "/w"},
            ],
            "unresolved": [],
            "previous": null,
        })
    );
    let made = ran(&["project", "heal", "--yes"], false, &mut projects);
    assert_eq!(made.code, 0);
    assert!(
        made.out.starts_with("Changed:\n  - register m for /w\n"),
        "{}",
        made.out
    );
}

#[test]
fn test_heal_reports_damage_from_the_catalog_in_one_document_or_a_line() {
    let mut projects = checked(Checked::Damaged(damage()));
    let found = ran(&["project", "heal"], true, &mut projects);
    assert_eq!(
        document(&found)["unresolved"],
        json!([{
            "kind": "damaged",
            "project_id": "m",
            "database": "/h/.ctxloom/projects/m/state.db",
            "damage": {
                "code": "fake-store-damaged",
                "args": {"first": "row 2 missing from index i", "more": 1},
                "message": "from the catalog: row 2 missing from index i (+1)",
            },
        }])
    );
    let found = ran(&["project", "heal"], false, &mut projects);
    assert_eq!(found.code, 0);
    assert!(
        found.out.ends_with(
            "Not resolved:\n  - /h/.ctxloom/projects/m/state.db, the database of m: \
             from the catalog: row 2 missing from index i (+1); \
             heal reports damage and does not repair it\n"
        ),
        "{}",
        found.out
    );
}

#[test]
fn test_repoint_reports_what_it_was() {
    let ran = ran(&["project", "repoint", "t"], true, &mut projects());
    let doc = document(&ran);
    assert_eq!(doc["previous"], "m");
    assert_eq!(doc["changes"][0]["kind"], "write-marker");
}

#[test]
fn test_a_refused_command_exits_1_naming_the_program() {
    let ran = ran(&["project", "repoint", "tpyo"], false, &mut projects());
    assert_eq!(ran.code, 1);
    assert_eq!(ran.out, "");
    assert_eq!(
        ran.err,
        "taskloom: no project tpyo is registered or has a database: nothing to repoint to\n"
    );
}

#[test]
fn test_in_json_a_refused_command_writes_its_envelope_on_stderr() {
    let ran = ran(&["project", "repoint", "tpyo"], true, &mut projects());
    assert_eq!(ran.code, 1);
    assert_eq!(ran.out, "");
    let envelope: Value = serde_json::from_str(&ran.err).expect("one JSON document");
    assert_eq!(
        envelope,
        json!({
            "code": "project-application-unknown-project",
            "args": {"id": "tpyo"},
            "message": "no project tpyo is registered or has a database: nothing to repoint to",
        })
    );
}

#[test]
fn test_orphans_are_listed_and_removed_by_name() {
    let mut projects = projects();
    let listed = ran(&["project", "orphans"], true, &mut projects);
    assert_eq!(
        document(&listed),
        json!([
            {"project_id": "stray", "database": "/h/.ctxloom/projects/stray"},
            {"project_id": "t", "database": "/h/.ctxloom/projects/t"},
        ])
    );
    let text = ran(&["project", "orphans"], false, &mut projects);
    assert_eq!(
        text.out,
        "stray\t/h/.ctxloom/projects/stray\nt\t/h/.ctxloom/projects/t\n"
    );
    let removed = ran(
        &["project", "orphans", "remove", "stray", "--yes"],
        true,
        &mut projects,
    );
    assert_eq!(document(&removed)["applied"], true);
    assert_eq!(
        projects_applied(&projects),
        [vec![Change::RemoveDatabase {
            id: "stray".to_string()
        }]]
    );
}

fn projects_applied(projects: &Projects<Fake, FakeCheck>) -> Vec<Vec<Change>> {
    projects.admin().applied.clone()
}

#[test]
fn test_show_reports_the_pinned_project() {
    let mut projects = projects();
    let settings = Settings {
        format: Format::Json,
        project: Some("pinned-id".to_string()),
        prog: "p",
    };
    let code = run(
        parse(&["project", "show"]),
        &settings,
        &catalog(),
        &mut projects,
        &mut Vec::new(),
        &mut Vec::new(),
    );
    assert_eq!(code, 0);
    assert_eq!(projects.admin().pinned, [Some("pinned-id".to_string())]);
}

/// Output that cannot be written.
struct Broken;

impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_output_that_cannot_be_written_exits_1() {
    for json in [true, false] {
        for args in [
            &["project", "show"][..],
            &["project", "heal"],
            &["project", "orphans"],
        ] {
            let mut err = Vec::new();
            let settings = Settings {
                format: format(json),
                project: None,
                prog: "p",
            };
            let code = run(
                parse(args),
                &settings,
                &catalog(),
                &mut projects(),
                &mut Broken,
                &mut err,
            );
            assert_eq!(code, 1);
            let err = String::from_utf8(err).unwrap();
            if json {
                let envelope: Value = serde_json::from_str(&err).expect("an envelope");
                assert_eq!(envelope["code"], "project-cli-output");
                assert_eq!(envelope["message"], "cannot write the output: broken pipe");
            } else {
                assert_eq!(err, "p: cannot write the output: broken pipe\n");
            }
        }
    }
}
