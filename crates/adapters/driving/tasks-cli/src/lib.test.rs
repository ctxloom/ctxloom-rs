//! The command line over a fake service: what each command asks the
//! application for, and how its answer is written.

use std::ffi::OsString;
use std::future::Future;
use std::path::PathBuf;
use std::pin::pin;
use std::task::{Context as TaskContext, Poll, Waker};

use annals_diagnostic::Report;
use tasks_application::{
    AppError, ListQuery, Listing, NewTask, Project, TagChange, Task, TaskService, Written,
};

use presentation::Catalog;

use super::{run, Context};

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

fn written(task: Task) -> Option<Result<Written, AppError>> {
    Some(Ok(Written {
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
    };
    let args: Vec<OsString> = std::iter::once("taskloom")
        .chain(args.iter().copied())
        .map(OsString::from)
        .collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = ready(run(args, &context, &catalog(), fake, &mut out, &mut err));
    Ran {
        code,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

fn tagged_task() -> Task {
    let mut task = shapes::task("swift-amber", "To Do");
    task.tags = vec!["a".to_string(), "b".to_string()];
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
