//! The command line over a fake service: what each command asks the
//! application for, and how its answer is written.

use std::ffi::OsString;
use std::future::Future;
use std::path::PathBuf;
use std::pin::pin;
use std::task::{Context as TaskContext, Poll, Waker};

use annals_diagnostic::Report;
use tasks_application::{
    AppError, ImportReport, ImportRequest, ImportService, ListQuery, Listing, NewTask, Project,
    ProjectImport, PruneRequest, PruneService, Pruning, TagChange, Task, TaskService, Written,
};

use presentation::{Catalog, Format};

use super::{run, run_mounted, Context, Mounted};

mod shapes {
    use tasks_application::{Task, Timestamp};

    pub fn task(harp: &str, status: &str) -> Task {
        Task {
            harp_id: harp.to_string(),
            text: "Ship it".to_string(),
            status: status.to_string(),
            checked: false,
            text_hash: "0123456789ab".to_string(),
            trigger: String::new(),
            origin_session: String::new(),
            tags: Vec::new(),
            created_at: Timestamp::default(),
        }
    }
}

fn ready<F: Future>(future: F) -> F::Output {
    let mut cx = TaskContext::from_waker(Waker::noop());
    match pin!(future).poll(&mut cx) {
        Poll::Ready(out) => out,
        Poll::Pending => panic!("the fake is ready at once"),
    }
}

fn catalog() -> Catalog {
    super::catalog(&[]).expect("the catalog loads")
}

fn project() -> Project {
    Project {
        id: "jolly-mute-magma".to_string(),
        root: "/work/tree".to_string(),
        store: PathBuf::new(),
    }
}

/// Answers scripted per use case; requests recorded.
#[derive(Default)]
struct Fake {
    added: Vec<NewTask>,
    tagged: Vec<TagChange>,
    listed: Vec<ListQuery>,
    written: Option<Result<Written, AppError>>,
    listing: Option<Result<Listing, AppError>>,
    imports: Vec<ImportRequest>,
    report: Option<Result<ImportReport, AppError>>,
    prunes: Vec<PruneRequest>,
    pruning: Option<Result<Pruning, AppError>>,
}

impl TaskService for Fake {
    async fn add(&mut self, task: NewTask) -> Result<Written, AppError> {
        self.added.push(task);
        self.written.clone().expect("an add answer scripted")
    }

    async fn tag(&mut self, change: TagChange) -> Result<Written, AppError> {
        self.tagged.push(change);
        self.written.clone().expect("a tag answer scripted")
    }

    async fn list(&mut self, query: ListQuery) -> Result<Listing, AppError> {
        self.listed.push(query);
        self.listing.clone().expect("a list answer scripted")
    }
}

impl ImportService for Fake {
    async fn import(&mut self, request: ImportRequest) -> Result<ImportReport, AppError> {
        self.imports.push(request);
        self.report.clone().expect("an import answer scripted")
    }
}

impl PruneService for Fake {
    async fn prune(&mut self, request: PruneRequest) -> Result<Pruning, AppError> {
        self.prunes.push(request);
        self.pruning.clone().expect("a prune answer scripted")
    }
}

fn written(task: Task) -> Option<Result<Written, AppError>> {
    Some(Ok(Written {
        shown: tasks_application::Shown::default(),
        project: project(),
        task,
    }))
}

struct Ran {
    code: u8,
    out: String,
    err: String,
}

fn run_on(fake: &mut Fake, terminal: bool, args: &[&str]) -> Ran {
    let context = Context {
        session: "cross-proc-harp".to_string(),
        stdout_is_terminal: terminal,
        version: "1.2.3".to_string(),
        lock_protocol: 2,
    };
    run_in(&context, fake, args)
}

fn run_in(context: &Context, fake: &mut Fake, args: &[&str]) -> Ran {
    let args: Vec<OsString> = std::iter::once("taskloom")
        .chain(args.iter().copied())
        .map(OsString::from)
        .collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = ready(run(args, context, &catalog(), fake, &mut out, &mut err));
    Ran {
        code,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

fn tagged_task() -> Task {
    let mut task = shapes::task("swift-amber", "To Do");
    task.tags = vec![tag("a"), tag("b")];
    task
}

// ------------------------------------------------------------ add

#[test]
fn test_add_joins_its_words_and_passes_its_flags_and_the_session() {
    let mut fake = Fake {
        written: written(shapes::task("swift-amber", "Deferred")),
        ..Fake::default()
    };
    let ran = run_on(
        &mut fake,
        true,
        &[
            "--project",
            "pinned",
            "add",
            "Ship",
            "it",
            "--status",
            "Deferred",
            "--trigger",
            "later",
            "--tag",
            "a",
            "--tag",
            "b",
            "--config-set",
            "tag_schema=x",
            "--config-set",
            "homing=home",
        ],
    );
    assert_eq!(ran.code, 0, "{}", ran.err);
    assert_eq!(
        fake.added,
        [NewTask {
            project: Some("pinned".to_string()),
            text: "Ship it".to_string(),
            status: "Deferred".to_string(),
            trigger: "later".to_string(),
            tags: vec!["a".to_string(), "b".to_string()],
            origin_session: "cross-proc-harp".to_string(),
            config_set: vec!["tag_schema=x".to_string(), "homing=home".to_string()],
        }]
    );
    assert_eq!(ran.err, "taskloom: project /work/tree (jolly-mute-magma)\n");
    assert_eq!(ran.out, "swift-amber\tDeferred\tShip it\n");
}

#[test]
fn test_add_with_no_flags_leaves_status_trigger_and_tags_empty() {
    let mut fake = Fake {
        written: written(shapes::task("swift-amber", "To Do")),
        ..Fake::default()
    };
    run_on(&mut fake, true, &["add", "Ship it"]);
    let added = &fake.added[0];
    assert_eq!(added.project, None);
    assert_eq!(added.status, "");
    assert_eq!(added.trigger, "");
    assert!(added.tags.is_empty());
}

#[test]
fn test_off_a_terminal_add_answers_in_json() {
    let mut fake = Fake {
        written: written(shapes::task("swift-amber", "To Do")),
        ..Fake::default()
    };
    let ran = run_on(&mut fake, false, &["add", "Ship it"]);
    let value: serde_json::Value = serde_json::from_str(&ran.out).unwrap();
    assert_eq!(value["harp_id"], "swift-amber");
}

#[test]
fn test_an_explicit_format_wins_over_the_terminal() {
    let answer = || written(shapes::task("swift-amber", "To Do"));
    for (terminal, args) in [
        (true, &["--format", "json", "add", "x"][..]),
        (true, &["add", "x", "--json"][..]),
        (true, &["--json", "--format", "text", "add", "x"][..]),
    ] {
        let mut fake = Fake {
            written: answer(),
            ..Fake::default()
        };
        let ran = run_on(&mut fake, terminal, args);
        assert!(ran.out.starts_with('{'), "{args:?}: {}", ran.out);
    }
    let mut fake = Fake {
        written: answer(),
        ..Fake::default()
    };
    let ran = run_on(&mut fake, false, &["--format", "text", "add", "x"]);
    assert_eq!(ran.out, "swift-amber\tTo Do\tShip it\n");
}

#[test]
fn test_add_needs_text() {
    let mut fake = Fake::default();
    let ran = run_on(&mut fake, true, &["add"]);
    assert_eq!(ran.code, 1);
    assert!(ran.err.contains("required"), "{}", ran.err);
    assert!(fake.added.is_empty());
}

#[test]
fn test_a_failed_add_says_why_on_stderr_and_exits_1() {
    let mut fake = Fake {
        written: Some(Err(AppError::HarpsExhausted { attempts: 100 })),
        ..Fake::default()
    };
    let ran = run_on(&mut fake, true, &["add", "x"]);
    assert_eq!(ran.code, 1);
    assert_eq!(ran.out, "");
    assert_eq!(ran.err, "taskloom: no free task id after 100 tries\n");
}

// ------------------------------------------------------------ tag

#[test]
fn test_tag_passes_its_adds_and_removes_and_answers_with_the_tags() {
    let mut fake = Fake {
        written: written(tagged_task()),
        ..Fake::default()
    };
    let ran = run_on(
        &mut fake,
        true,
        &[
            "tag",
            "swift-amber",
            "--add",
            "a",
            "--add",
            "b",
            "--remove",
            "c",
        ],
    );
    assert_eq!(ran.code, 0);
    assert_eq!(
        fake.tagged,
        [TagChange {
            project: None,
            harp_id: "swift-amber".to_string(),
            add: vec!["a".to_string(), "b".to_string()],
            remove: vec!["c".to_string()],
            config_set: Vec::new(),
        }]
    );
    assert_eq!(ran.err, "taskloom: project /work/tree (jolly-mute-magma)\n");
    assert_eq!(ran.out, "swift-amber\tTo Do\ta,b\n");
}

#[test]
fn test_tag_in_json_is_the_task() {
    let mut fake = Fake {
        written: written(tagged_task()),
        ..Fake::default()
    };
    let ran = run_on(
        &mut fake,
        false,
        &["--project", "p", "tag", "swift-amber", "--add", "a"],
    );
    assert_eq!(fake.tagged[0].project.as_deref(), Some("p"));
    let value: serde_json::Value = serde_json::from_str(&ran.out).unwrap();
    assert_eq!(value["tags"], serde_json::json!(["a", "b"]));
}

#[test]
fn test_a_failed_tag_exits_1() {
    let mut fake = Fake {
        written: Some(Err(AppError::Vanished {
            harp_id: "x".to_string(),
        })),
        ..Fake::default()
    };
    let ran = run_on(&mut fake, true, &["tag", "x", "--add", "a"]);
    assert_eq!(ran.code, 1);
    assert!(ran.err.starts_with("taskloom: task x"), "{}", ran.err);
}

// ------------------------------------------------------------ list

fn listing(tasks: Vec<Task>) -> Option<Result<Listing, AppError>> {
    Some(Ok(Listing {
        shown: tasks_application::Shown::default(),
        project: Some(project()),
        tasks,
    }))
}

#[test]
fn test_list_passes_its_filters() {
    let mut fake = Fake {
        listing: listing(Vec::new()),
        ..Fake::default()
    };
    run_on(
        &mut fake,
        true,
        &[
            "--project",
            "p",
            "list",
            "--status",
            "Done,To Do",
            "--status",
            "Deferred",
            "--all",
            "--tag-query",
            "urgent/not",
        ],
    );
    assert_eq!(
        fake.listed,
        [ListQuery {
            project: Some("p".to_string()),
            statuses: vec![
                "Done".to_string(),
                "To Do".to_string(),
                "Deferred".to_string()
            ],
            all: true,
            tag_query: "urgent/not".to_string(),
            config_set: Vec::new(),
        }]
    );
    let mut fake = Fake {
        listing: listing(Vec::new()),
        ..Fake::default()
    };
    run_on(&mut fake, true, &["list"]);
    assert_eq!(fake.listed, [ListQuery::default()]);
}

#[test]
fn test_list_as_text_names_the_project_then_the_table() {
    let mut fake = Fake {
        listing: listing(vec![shapes::task("swift-amber", "To Do")]),
        ..Fake::default()
    };
    let ran = run_on(&mut fake, true, &["list"]);
    assert_eq!(ran.code, 0);
    assert_eq!(
        ran.out,
        "Project: /work/tree (jolly-mute-magma)\n\n[ ] swift-amber  To Do        Ship it\n"
    );
    assert_eq!(ran.err, "");
}

#[test]
fn test_list_as_json_is_an_array_of_tasks() {
    let mut fake = Fake {
        listing: listing(vec![
            shapes::task("swift-amber", "To Do"),
            shapes::task("calm-otter", "To Do"),
        ]),
        ..Fake::default()
    };
    let ran = run_on(&mut fake, false, &["list"]);
    let value: Vec<serde_json::Value> = serde_json::from_str(&ran.out).unwrap();
    assert_eq!(value.len(), 2);
    assert_eq!(value[1]["harp_id"], "calm-otter");
}

#[test]
fn test_list_where_there_is_no_project_says_so_and_lists_nothing() {
    for (terminal, shown) in [(true, "(no tasks)\n"), (false, "[]\n")] {
        let mut fake = Fake {
            listing: Some(Ok(Listing {
                shown: tasks_application::Shown::default(),
                project: None,
                tasks: Vec::new(),
            })),
            ..Fake::default()
        };
        let ran = run_on(&mut fake, terminal, &["list"]);
        assert_eq!(ran.code, 0);
        assert_eq!(ran.out, shown);
        assert_eq!(ran.err, "taskloom: no project here yet: nothing to list\n");
    }
}

#[test]
fn test_a_failed_list_exits_1() {
    let mut fake = Fake {
        listing: Some(Err(AppError::HarpsExhausted { attempts: 1 })),
        ..Fake::default()
    };
    let ran = run_on(&mut fake, true, &["list"]);
    assert_eq!(ran.code, 1);
    assert!(ran.err.starts_with("taskloom: "), "{}", ran.err);
}

// ------------------------------------------------------------ usage

#[test]
fn test_help_is_written_to_stdout_and_exits_0() {
    let ran = run_on(&mut Fake::default(), true, &["--help"]);
    assert_eq!(ran.code, 0);
    assert!(ran.out.contains("Usage: taskloom"), "{}", ran.out);
    assert_eq!(ran.err, "");
}

#[test]
fn test_an_unknown_command_is_a_usage_error_exiting_1() {
    let ran = run_on(&mut Fake::default(), true, &["frobnicate"]);
    assert_eq!(ran.code, 1);
    assert_eq!(ran.out, "");
    assert!(ran.err.contains("frobnicate"), "{}", ran.err);
}

// ------------------------------------------------------------ output

/// A writer that refuses everything.
struct Broken;

impl std::io::Write for Broken {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "closed",
        ))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_output_that_cannot_be_written_fails_with_why() {
    let context = Context {
        stdout_is_terminal: true,
        ..Context::default()
    };
    for args in [&["taskloom", "add", "x"][..], &["taskloom", "list"][..]] {
        let mut fake = Fake {
            written: written(shapes::task("swift-amber", "To Do")),
            listing: listing(Vec::new()),
            ..Fake::default()
        };
        let mut err = Vec::new();
        let argv = args.iter().map(OsString::from);
        let code = ready(run(
            argv,
            &context,
            &catalog(),
            &mut fake,
            &mut Broken,
            &mut err,
        ));
        assert_eq!(code, 1);
        assert_eq!(
            String::from_utf8(err).unwrap().lines().last(),
            Some("taskloom: cannot write the output: closed"),
            "{args:?}"
        );
    }
    let mut fake = Fake {
        listing: listing(Vec::new()),
        ..Fake::default()
    };
    let mut err = Vec::new();
    let argv = ["taskloom", "--json", "list"].map(OsString::from);
    let code = ready(run(
        argv,
        &context,
        &catalog(),
        &mut fake,
        &mut Broken,
        &mut err,
    ));
    assert_eq!(code, 1);
    let envelope: serde_json::Value = serde_json::from_slice(&err).unwrap();
    assert_eq!(envelope["code"], "tasks-cli-output");
    assert_eq!(envelope["message"], "cannot write the output: closed");
}

// ------------------------------------------------------------ errors

#[test]
fn test_a_text_error_is_its_catalog_message_not_its_english() {
    let mut fake = Fake {
        written: Some(Err(AppError::Rejected(Report::new(
            "tasks-text-required",
            "the English fallback",
        )))),
        ..Fake::default()
    };
    let ran = run_on(&mut fake, true, &["add", "x"]);
    assert_eq!(ran.code, 1);
    assert_eq!(ran.err, "taskloom: a task needs text\n");
}

#[test]
fn test_a_json_error_is_an_envelope_on_stderr_with_its_cause_nested() {
    let mut fake = Fake {
        written: Some(Err(AppError::GaveUp {
            attempts: 10,
            error: Report::new(
                "annals-store-sql-unavailable",
                "the database failed: locked",
            )
            .arg("detail", "locked"),
        })),
        ..Fake::default()
    };
    let ran = run_on(&mut fake, false, &["add", "x"]);
    assert_eq!(ran.code, 1);
    assert_eq!(ran.out, "", "stdout holds answers only");
    let envelope: serde_json::Value = serde_json::from_str(&ran.err).unwrap();
    assert_eq!(
        envelope,
        serde_json::json!({
            "code": "tasks-application-gave-up",
            "args": {
                "attempts": 10,
                "error": {
                    "code": "annals-store-sql-unavailable",
                    "args": {"detail": "locked"},
                    "message": "the database failed: locked",
                },
            },
            "message": "gave up after 10 attempts: the database failed: locked",
        })
    );
}

#[test]
fn test_a_usage_error_in_json_is_an_envelope() {
    for (terminal, args) in [
        (false, &["frobnicate"][..]),
        (true, &["--json", "frobnicate"][..]),
        (true, &["--format", "json", "frobnicate"][..]),
        (true, &["--format=json", "frobnicate"][..]),
        (true, &["--format", "json", "--format", "yaml", "list"][..]),
    ] {
        let ran = run_on(&mut Fake::default(), terminal, args);
        assert_eq!(ran.code, 1, "{args:?}");
        assert_eq!(ran.out, "", "{args:?}");
        let envelope: serde_json::Value =
            serde_json::from_str(&ran.err).unwrap_or_else(|e| panic!("{args:?}: {e}: {}", ran.err));
        assert_eq!(envelope["code"], "tasks-cli-usage", "{args:?}");
        assert_eq!(envelope["message"], envelope["args"]["detail"], "{args:?}");
    }
}

#[test]
fn test_a_usage_error_in_text_is_clap_s_own_words() {
    for args in [
        &["frobnicate"][..],
        &["--format", "text", "--json-ish"][..],
        &["--format=text", "frobnicate"][..],
        &["frobnicate", "--", "--json"][..],
        &["frobnicate", "--format"][..],
    ] {
        let ran = run_on(&mut Fake::default(), true, args);
        assert_eq!(ran.code, 1, "{args:?}");
        assert!(ran.err.starts_with("error: "), "{args:?}: {}", ran.err);
    }
}

// ------------------------------------------------------------ import

fn imported(lost: usize) -> ProjectImport {
    ProjectImport {
        project_id: "jolly-mute-magma".to_string(),
        records: 3,
        tasks: 1,
        applied: 3,
        lost: (0..lost)
            .map(|line| tasks_application::Lost {
                line: u32::try_from(line).unwrap(),
                op: tasks_application::GoOp::Status,
                harp_id: "swift-amber".to_string(),
                text: String::new(),
                why: tasks_application::Why::TaskNotImported,
            })
            .collect(),
        ..ProjectImport::default()
    }
}

fn report(projects: Vec<ProjectImport>) -> Option<Result<ImportReport, AppError>> {
    Some(Ok(ImportReport { projects }))
}

#[test]
fn test_import_imports_every_project_and_says_what_it_did() {
    let mut fake = Fake {
        report: report(vec![imported(0)]),
        ..Fake::default()
    };
    let ran = run_on(&mut fake, true, &["import"]);
    assert_eq!(ran.code, 0, "{}", ran.err);
    assert_eq!(fake.imports, [ImportRequest { project: None }]);
    assert_eq!(
        ran.out,
        "jolly-mute-magma: 3 records, 1 tasks added, 3 records imported\n"
    );
    assert_eq!(ran.err, "");
}

#[test]
fn test_import_of_one_project_names_it() {
    let mut fake = Fake {
        report: report(vec![imported(0)]),
        ..Fake::default()
    };
    run_on(
        &mut fake,
        true,
        &["import", "--project", "jolly-mute-magma"],
    );
    assert_eq!(
        fake.imports,
        [ImportRequest {
            project: Some("jolly-mute-magma".to_string())
        }]
    );
}

#[test]
fn test_import_answers_json_off_a_terminal() {
    let mut fake = Fake {
        report: report(vec![imported(0)]),
        ..Fake::default()
    };
    let ran = run_on(&mut fake, false, &["import"]);
    let json: serde_json::Value = serde_json::from_str(&ran.out).unwrap();
    assert_eq!(json["complete"], true);
    assert_eq!(json["projects"][0]["project_id"], "jolly-mute-magma");
}

#[test]
fn test_import_that_left_records_behind_fails_after_saying_which() {
    let mut fake = Fake {
        report: report(vec![imported(2)]),
        ..Fake::default()
    };
    let ran = run_on(&mut fake, true, &["import"]);
    assert_eq!(ran.code, 1);
    assert!(ran.out.contains("not imported: line 1"), "{}", ran.out);
    assert_eq!(
        ran.err,
        "taskloom: not everything was imported: 0 projects stopped, 2 records not imported\n"
    );
}

#[test]
fn test_import_that_stopped_in_a_project_fails() {
    let mut stopped = imported(0);
    stopped.failed = Some(annals_diagnostic::Report::new("x-bad", "bad log"));
    let mut fake = Fake {
        report: report(vec![stopped, imported(0)]),
        ..Fake::default()
    };
    let ran = run_on(&mut fake, true, &["import"]);
    assert_eq!(ran.code, 1);
    assert_eq!(
        ran.err,
        "taskloom: not everything was imported: 1 projects stopped, 0 records not imported\n"
    );
}

#[test]
fn test_import_that_cannot_start_fails_with_why() {
    let mut fake = Fake {
        report: Some(Err(AppError::NoProject)),
        ..Fake::default()
    };
    let ran = run_on(&mut fake, true, &["import"]);
    assert_eq!(ran.code, 1);
    assert_eq!(ran.out, "");
    assert!(
        ran.err.starts_with("taskloom: there is no project"),
        "{}",
        ran.err
    );
}

// ------------------------------------------------------------ companion

/// Go taskloom's loadout document, as `scripts/go-companion-fixtures.sh`
/// copied it from the Go source (`cmd/taskloom/loadout.yaml`); run it again
/// to regenerate it.
const GO_LOADOUT: &str = include_str!("../fixtures/go/loadout.yaml");

#[test]
fn test_version_in_json_is_gos_shape_with_the_lock_protocol() {
    let ran = run_on(&mut Fake::default(), true, &["version", "--format", "json"]);
    assert_eq!(ran.code, 0, "{}", ran.err);
    assert_eq!(
        ran.out,
        "{\n  \"name\": \"taskloom\",\n  \"version\": \"1.2.3\",\n  \"lock_protocol\": 2\n}\n"
    );
    assert_eq!(ran.err, "");
}

#[test]
fn test_version_in_text_is_the_bare_version() {
    let ran = run_on(&mut Fake::default(), true, &["version"]);
    assert_eq!((ran.code, ran.out.as_str()), (0, "1.2.3\n"));
}

#[test]
fn test_a_version_with_no_lock_protocol_leaves_it_out_as_go_does() {
    let context = Context {
        version: "1.2.3".to_string(),
        ..Context::default()
    };
    let ran = run_in(&context, &mut Fake::default(), &["version", "--json"]);
    let value: serde_json::Value = serde_json::from_str(&ran.out).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"name": "taskloom", "version": "1.2.3"})
    );
}

#[test]
fn test_the_loadout_is_gos_byte_for_byte() {
    for args in [&["loadout"][..], &["loadout", "--format", "yaml"]] {
        let ran = run_on(&mut Fake::default(), false, args);
        assert_eq!(ran.code, 0, "{args:?}: {}", ran.err);
        assert!(
            ran.out == GO_LOADOUT,
            "{args:?}: the loadout differs from Go's"
        );
        assert_eq!(ran.err, "");
    }
}

#[test]
fn test_the_loadout_in_another_format_is_refused_and_prints_nothing() {
    for (args, asked) in [
        (&["loadout", "--json"][..], "json"),
        (&["loadout", "--format", "json"], "json"),
        (&["--format", "text", "loadout"], "text"),
    ] {
        let ran = run_on(&mut Fake::default(), true, args);
        assert_eq!(ran.code, 1, "{args:?}");
        assert_eq!(ran.out, "", "{args:?}");
        assert!(
            ran.err.contains(&format!("not {asked}")),
            "{args:?}: {}",
            ran.err
        );
    }
}

#[test]
fn test_yaml_is_only_the_loadout_s() {
    for args in [
        &["list", "--format", "yaml"][..],
        &["version", "--format", "yaml"],
    ] {
        let ran = run_on(&mut Fake::default(), true, args);
        assert_eq!(ran.code, 1, "{args:?}");
        assert_eq!(ran.out, "", "{args:?}");
        assert_eq!(
            ran.err, "taskloom: the format yaml is not one of text or json\n",
            "{args:?}"
        );
    }
}

// ------------------------------------------------------------ mounted

/// Another crate's commands, as a binary mounts them.
#[derive(clap::Subcommand, Debug, PartialEq)]
enum Extra {
    /// Say hello.
    Hello { name: String },
}

fn run_extra(terminal: bool, args: &[&str]) -> (Ran, Vec<(Extra, Mounted)>) {
    let context = Context {
        stdout_is_terminal: terminal,
        ..Context::default()
    };
    let args: Vec<OsString> = std::iter::once("taskloom")
        .chain(args.iter().copied())
        .map(OsString::from)
        .collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut handed = Vec::new();
    let catalog = catalog();
    let mounted = |command: Extra,
                   flags: Mounted,
                   given: &Catalog,
                   out: &mut dyn std::io::Write,
                   err: &mut dyn std::io::Write| {
        assert!(std::ptr::eq(given, &catalog), "the run's own catalog");
        writeln!(out, "out").unwrap();
        writeln!(err, "err").unwrap();
        handed.push((command, flags));
        7
    };
    let code = ready(run_mounted(
        args,
        &context,
        &catalog,
        &mut Fake::default(),
        mounted,
        &mut out,
        &mut err,
    ));
    let ran = Ran {
        code,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    };
    (ran, handed)
}

#[test]
fn test_a_mounted_command_gets_the_global_flags_and_gives_the_exit_code() {
    let (ran, handed) = run_extra(true, &["--project", "p", "hello", "world", "--json"]);
    assert_eq!(
        (ran.code, ran.out.as_str(), ran.err.as_str()),
        (7, "out\n", "err\n")
    );
    assert_eq!(
        handed,
        [(
            Extra::Hello {
                name: "world".to_string()
            },
            Mounted {
                format: Format::Json,
                project: Some("p".to_string()),
            }
        )]
    );
    let (_, handed) = run_extra(true, &["hello", "x"]);
    assert_eq!(
        handed[0].1,
        Mounted {
            format: Format::Text,
            project: None,
        },
        "text on a terminal"
    );
}

#[test]
fn test_mounted_commands_are_listed_beside_the_task_commands() {
    let (ran, handed) = run_extra(true, &["--help"]);
    assert_eq!(ran.code, 0);
    assert!(handed.is_empty());
    assert!(ran.out.contains("hello"), "{}", ran.out);
    assert!(ran.out.contains("import"), "{}", ran.out);
}

fn tag(key: &str) -> tasks_application::Tag {
    tasks_application::Tag {
        namespace: None,
        key: key.to_string(),
        value: None,
    }
}

// ------------------------------------------------------------ prune

fn pruning(applied: bool, taken: &[&str]) -> Pruning {
    Pruning {
        project: Some(project()),
        statuses: vec!["Done".to_string()],
        before: tasks_application::Timestamp {
            seconds: 1_767_225_600,
            nanos: 0,
        },
        tag_query: String::new(),
        applied,
        pruned: taken
            .iter()
            .map(|harp| tasks_application::Pruned {
                task: shapes::task(harp, "Done"),
                status_set_at: tasks_application::Timestamp::default(),
            })
            .collect(),
        events_removed: 2 * taken.len() as u64,
        tasks_remaining: 1,
        dangling: vec![tasks_application::Dangling {
            harp_id: "keeper".to_string(),
            pruned: "swift-amber".to_string(),
            via: tasks_application::Via::Tag("relates:swift-amber".to_string()),
        }],
        shown: tasks_application::Shown::default(),
    }
}

#[test]
fn test_prune_by_default_reports_done_tasks_older_than_seven_days() {
    let mut fake = Fake {
        pruning: Some(Ok(pruning(false, &[]))),
        ..Fake::default()
    };
    let ran = run_on(&mut fake, true, &["prune"]);
    assert_eq!(ran.code, 0, "{}", ran.err);
    assert_eq!(
        fake.prunes,
        [PruneRequest {
            project: None,
            statuses: vec!["Done".to_string()],
            older_than: "7d".to_string(),
            tag_query: String::new(),
            apply: false,
            config_set: Vec::new(),
        }]
    );
    assert_eq!(ran.err, "", "nothing would be taken: nothing to say");
}

#[test]
fn test_prune_passes_its_flags() {
    let mut fake = Fake {
        pruning: Some(Ok(pruning(true, &["swift-amber"]))),
        ..Fake::default()
    };
    let ran = run_on(
        &mut fake,
        true,
        &[
            "--project",
            "pinned",
            "prune",
            "--older-than",
            "30d",
            "--status",
            "Done,Archived",
            "--status",
            "To Do",
            "--tag-query",
            "release",
            "--yes",
            "--config-set",
            "tag_schema=x",
        ],
    );
    assert_eq!(ran.code, 0, "{}", ran.err);
    assert_eq!(
        fake.prunes,
        [PruneRequest {
            project: Some("pinned".to_string()),
            statuses: vec![
                "Done".to_string(),
                "Archived".to_string(),
                "To Do".to_string()
            ],
            older_than: "30d".to_string(),
            tag_query: "release".to_string(),
            apply: true,
            config_set: vec!["tag_schema=x".to_string()],
        }]
    );
    assert_eq!(ran.err, "", "applied: no report-only notice");
    assert!(
        ran.out
            .starts_with("Project: /work/tree (jolly-mute-magma)\n"),
        "{}",
        ran.out
    );
}

#[test]
fn test_a_prune_report_that_would_take_something_says_how_to_apply_it() {
    let mut fake = Fake {
        pruning: Some(Ok(pruning(false, &["swift-amber"]))),
        ..Fake::default()
    };
    let ran = run_on(
        &mut fake,
        true,
        &["prune", "--status", "To Do", "--tag-query", "level<=2"],
    );
    assert_eq!(ran.code, 0);
    assert_eq!(
        ran.err,
        "taskloom: removed nothing: this was a report, not a removal; to apply exactly this plan, \
         run it again with --yes: taskloom prune --older-than 7d --status 'To Do' --tag-query 'level<=2' --yes\n"
    );
}

#[test]
fn test_off_a_terminal_a_prune_is_one_json_document_and_its_notices_are_on_stderr() {
    let mut fake = Fake {
        pruning: Some(Ok(pruning(false, &["swift-amber"]))),
        ..Fake::default()
    };
    let ran = run_on(&mut fake, false, &["prune"]);
    let value: serde_json::Value = serde_json::from_str(&ran.out).unwrap();
    assert_eq!(value["pruned"][0]["harp_id"], "swift-amber");
    assert_eq!(
        value["dangling_references"][0]["via"],
        "tag relates:swift-amber"
    );
    assert!(ran.err.contains("removed nothing"), "{}", ran.err);
}

#[test]
fn test_a_prune_where_there_is_no_project_says_so_on_stderr() {
    let mut answer = pruning(false, &[]);
    answer.project = None;
    answer.dangling.clear();
    let mut fake = Fake {
        pruning: Some(Ok(answer)),
        ..Fake::default()
    };
    let ran = run_on(&mut fake, true, &["prune"]);
    assert_eq!(ran.code, 0);
    assert_eq!(ran.err, "taskloom: no project here yet: nothing to prune\n");
    assert_eq!(
        ran.out,
        "Nothing to prune (status Done, last status change before 2026-01-01 00:00 UTC).\n"
    );
}

#[test]
fn test_a_refused_prune_says_why_and_exits_1() {
    let refusal = annals_diagnostic::Report::new(
        "tasks-application-prune-age-unreadable",
        "the age soon is unreadable",
    )
    .arg("age", "soon");
    let mut fake = Fake {
        pruning: Some(Err(AppError::Prune(refusal))),
        ..Fake::default()
    };
    let ran = run_on(&mut fake, true, &["prune", "--older-than", "soon"]);
    assert_eq!(ran.code, 1);
    assert_eq!(ran.out, "");
    assert_eq!(
        ran.err,
        "taskloom: the age soon is neither an offset (30d, 12w, 720h) nor a date (2026-01-01)\n",
        "the catalog's message"
    );
}
