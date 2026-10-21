//! `run` over a fake service and a recording launcher: Go's argv, the
//! picker as Go draws and reads it, and each way a run launches nothing.
//! The real launcher runs only stub scripts in a temporary directory, the
//! one directory on the PATH it is given: never a real ctxloom.

use std::future::Future;
use std::io::{BufRead, Cursor};
use std::path::Path;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use annals_diagnostic::Report;
use presentation::{Catalog, Format};
use serde_json::Value;
use tasks_application::{
    AppError, Details, Hidden, LintReport, ListQuery, Listing, NewTask, Project, Reach, Row, Scope,
    ShowQuery, Shown, StatusChange, Statuses, Summary, TagChange, TagSchema, Task, TaskService,
    TextEdit, Timestamp, Vocabulary, Written,
};

use super::{
    launch_args, pick, run, Ctxloom, Group, Launch, RunError, Settings, IN_PROGRESS, TO_DO,
};

fn ready<F: Future>(future: F) -> F::Output {
    let mut cx = Context::from_waker(Waker::noop());
    match pin!(future).poll(&mut cx) {
        Poll::Ready(out) => out,
        Poll::Pending => panic!("the fake is ready at once"),
    }
}

fn task(harp: &str, text: &str, status: &str, origin: &str) -> Task {
    Task {
        harp_id: harp.to_string(),
        text: text.to_string(),
        status: status.to_string(),
        checked: false,
        text_hash: "0123456789ab".to_string(),
        trigger: String::new(),
        origin_session: origin.to_string(),
        tags: Vec::new(),
        created_at: Timestamp::default(),
    }
}

/// A project's tasks, listed; each query recorded. Only `list` is used.
#[derive(Default)]
struct Fake {
    tasks: Vec<Task>,
    fails: Option<AppError>,
    listed: Vec<ListQuery>,
}

impl Fake {
    fn of(tasks: Vec<Task>) -> Fake {
        Fake {
            tasks,
            ..Fake::default()
        }
    }
}

impl TaskService for Fake {
    async fn add(&mut self, _: NewTask) -> Result<Written, AppError> {
        unreachable!("run adds nothing")
    }

    async fn tag(&mut self, _: TagChange) -> Result<Written, AppError> {
        unreachable!("run tags nothing")
    }

    async fn list(&mut self, query: ListQuery) -> Result<Listing, AppError> {
        self.listed.push(query);
        if let Some(error) = self.fails.clone() {
            return Err(error);
        }
        let project = Project {
            id: "jolly-mute-magma".to_string(),
            root: "/work/tree".to_string(),
            store: std::path::PathBuf::new(),
        };
        Ok(Listing {
            reach: Reach::Project(project.clone()),
            rows: self
                .tasks
                .iter()
                .map(|task| Row {
                    project: project.clone(),
                    task: task.clone(),
                })
                .collect(),
            shown: Shown::default(),
            hidden: Hidden::default(),
            searched: false,
            omitted: 0,
            priorities: None,
            priority_notes: Vec::new(),
        })
    }

    async fn set_status(&mut self, _: StatusChange) -> Result<Written, AppError> {
        unreachable!("run moves nothing itself")
    }

    async fn edit(&mut self, _: TextEdit) -> Result<Written, AppError> {
        unreachable!("run edits nothing")
    }

    async fn show(&mut self, _: ShowQuery) -> Result<Details, AppError> {
        unreachable!("run shows nothing")
    }

    async fn summary(&mut self, _: Scope) -> Result<Summary, AppError> {
        unreachable!("run summarizes nothing")
    }

    async fn statuses(&mut self, _: Scope) -> Result<Statuses, AppError> {
        unreachable!("run lists no statuses")
    }

    async fn tags(&mut self, _: ListQuery) -> Result<Vocabulary, AppError> {
        unreachable!("run counts no tags")
    }

    async fn lint(&mut self, _: Scope) -> Result<LintReport, AppError> {
        unreachable!("run lints nothing")
    }

    async fn tag_schema(&mut self, _: Vec<String>) -> Result<TagSchema, AppError> {
        unreachable!("run reads no tag schema")
    }
}

/// Records each launch; answers `answer`.
#[derive(Default)]
struct Recorder {
    launched: Vec<Vec<String>>,
    answer: Option<RunError>,
}

impl Launch for Recorder {
    fn launch(&mut self, args: &[String]) -> Result<(), RunError> {
        self.launched.push(args.to_vec());
        self.answer.clone().map_or(Ok(()), Err)
    }
}

fn settings(interactive: bool) -> Settings {
    Settings {
        format: Format::Text,
        project: None,
        prog: "taskloom",
        interactive,
    }
}

fn catalog() -> Catalog {
    let mut sources = vec![crate::EN_US];
    sources.extend(tasks_application::CATALOGS);
    Catalog::en_us(&sources).expect("the catalog loads")
}

fn named(harp: &str) -> Group {
    Group::Run {
        harp_id: Some(harp.to_string()),
        no_start: false,
    }
}

const PICK: Group = Group::Run {
    harp_id: None,
    no_start: false,
};

struct Ran {
    code: u8,
    err: String,
}

fn run_with(
    fake: &mut Fake,
    command: Group,
    settings: &Settings,
    launcher: &mut Recorder,
    input: &mut dyn BufRead,
) -> Ran {
    let mut err = Vec::new();
    let code = ready(run(
        fake,
        command,
        settings,
        launcher,
        input,
        &catalog(),
        &mut err,
    ));
    Ran {
        code,
        err: String::from_utf8(err).unwrap(),
    }
}

fn three() -> Vec<Task> {
    vec![
        task("a", "open todo", TO_DO, "h1"),
        task("b", "done thing", "Done", "h1"),
        task("c", "in progress", IN_PROGRESS, "h2"),
    ]
}

// ------------------------------------------------------------ argv

#[test]
fn test_a_task_with_an_origin_continues_its_session() {
    let args = launch_args(
        &task("swift-amber", "wire up X", TO_DO, "origin-harp"),
        false,
    );
    assert_eq!(
        args,
        [
            "run",
            "--session",
            "origin-harp",
            "--seed-task",
            "swift-amber",
            "--prompt",
            "Work on this task (`swift-amber`): wire up X"
        ]
    );
}

#[test]
fn test_a_task_with_no_origin_has_no_session_and_no_start_seeds_to_do() {
    let args = launch_args(&task("quiet-heron", "fix Y", TO_DO, ""), true);
    assert_eq!(
        args,
        [
            "run",
            "--seed-task",
            "quiet-heron",
            "--seed-status",
            "To Do",
            "--prompt",
            "Work on this task (`quiet-heron`): fix Y"
        ]
    );
}

// ------------------------------------------------------------ run

#[test]
fn test_a_named_task_of_any_status_is_launched_without_a_picker() {
    let mut fake = Fake::of(three());
    let mut launcher = Recorder::default();
    let ran = run_with(
        &mut fake,
        named("b"),
        &settings(false),
        &mut launcher,
        &mut Cursor::new(""),
    );
    assert_eq!(ran.code, 0, "{}", ran.err);
    assert_eq!(ran.err, "");
    assert_eq!(launcher.launched, [launch_args(&three()[1], false)]);
    assert!(fake.listed[0].all, "every status, so a Done task launches");
}

#[test]
fn test_no_start_reaches_the_launch() {
    let mut fake = Fake::of(three());
    let mut launcher = Recorder::default();
    let command = Group::Run {
        harp_id: Some("a".to_string()),
        no_start: true,
    };
    run_with(
        &mut fake,
        command,
        &settings(false),
        &mut launcher,
        &mut Cursor::new(""),
    );
    assert_eq!(launcher.launched, [launch_args(&three()[0], true)]);
}

#[test]
fn test_a_task_not_found_launches_nothing_and_says_so() {
    let mut fake = Fake::of(three());
    let mut launcher = Recorder::default();
    let ran = run_with(
        &mut fake,
        named("z"),
        &settings(true),
        &mut launcher,
        &mut Cursor::new("1\n"),
    );
    assert_eq!(ran.code, 1);
    assert_eq!(ran.err, "taskloom: task not found: z\n");
    assert!(launcher.launched.is_empty());
}

#[test]
fn test_off_a_terminal_a_harp_id_is_required() {
    let mut fake = Fake::of(three());
    let mut launcher = Recorder::default();
    let json = Settings {
        format: Format::Json,
        ..settings(false)
    };
    let ran = run_with(
        &mut fake,
        PICK,
        &json,
        &mut launcher,
        &mut Cursor::new("1\n"),
    );
    assert_eq!(ran.code, 1);
    let envelope: Value = serde_json::from_str(&ran.err).expect("an envelope");
    assert_eq!(envelope["code"], "tasks-run-not-interactive");
    assert!(launcher.launched.is_empty());
}

#[test]
fn test_a_task_picked_on_a_terminal_is_launched() {
    let mut fake = Fake::of(three());
    let mut launcher = Recorder::default();
    let ran = run_with(
        &mut fake,
        PICK,
        &settings(true),
        &mut launcher,
        &mut Cursor::new("2\n"),
    );
    assert_eq!(ran.code, 0);
    assert_eq!(
        launcher.launched,
        [launch_args(&three()[2], false)],
        "row 2 of the open view is the In Progress task"
    );
    assert!(
        ran.err.starts_with(" [1] To Do"),
        "the picker on stderr: {}",
        ran.err
    );
}

#[test]
fn test_quitting_the_picker_launches_nothing_and_is_no_error() {
    for input in ["q\n", "", "\n", "quit\n", "exit\n"] {
        let mut fake = Fake::of(three());
        let mut launcher = Recorder::default();
        let ran = run_with(
            &mut fake,
            PICK,
            &settings(true),
            &mut launcher,
            &mut Cursor::new(input),
        );
        assert_eq!(ran.code, 0, "{input:?}");
        assert!(
            ran.err.ends_with("> taskloom: cancelled\n"),
            "{input:?}: {}",
            ran.err
        );
        assert!(launcher.launched.is_empty());
    }
}

#[test]
fn test_a_launch_that_fails_is_the_runs_error() {
    let mut fake = Fake::of(three());
    let mut launcher = Recorder {
        answer: Some(RunError::Exited { status: 3 }),
        ..Recorder::default()
    };
    let ran = run_with(
        &mut fake,
        named("a"),
        &settings(false),
        &mut launcher,
        &mut Cursor::new(""),
    );
    assert_eq!(ran.code, 1);
    assert_eq!(ran.err, "taskloom: ctxloom run exited with status 3\n");
}

#[test]
fn test_a_listing_that_fails_launches_nothing_and_the_project_is_asked_for() {
    let mut fake = Fake {
        fails: Some(AppError::Failed(Report::of(&AppError::NoProject))),
        ..Fake::of(three())
    };
    let mut launcher = Recorder::default();
    let pinned = Settings {
        project: Some("jolly-mute-magma".to_string()),
        ..settings(false)
    };
    let ran = run_with(
        &mut fake,
        named("a"),
        &pinned,
        &mut launcher,
        &mut Cursor::new(""),
    );
    assert_eq!(ran.code, 1);
    assert!(launcher.launched.is_empty());
    assert_eq!(fake.listed[0].project.as_deref(), Some("jolly-mute-magma"));
}

/// A stdin that fails every read.
struct Broken;

impl std::io::Read for Broken {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("stdin exploded"))
    }
}

#[test]
fn test_a_read_that_fails_is_an_error_not_a_quit() {
    let mut fake = Fake::of(three());
    let mut launcher = Recorder::default();
    let ran = run_with(
        &mut fake,
        PICK,
        &settings(true),
        &mut launcher,
        &mut std::io::BufReader::new(Broken),
    );
    assert_eq!(ran.code, 1);
    assert!(
        ran.err
            .ends_with("taskloom: read task selection: stdin exploded\n"),
        "{}",
        ran.err
    );
}

// ------------------------------------------------------------ picker

fn picked(input: &str, all: &[Task]) -> (Option<String>, String) {
    let mut out = Vec::new();
    let chosen = pick(&mut out, &mut Cursor::new(input), all).unwrap();
    (chosen.map(|t| t.harp_id), String::from_utf8(out).unwrap())
}

#[test]
fn test_the_picker_draws_gos_rows_and_prompt() {
    let (chosen, drawn) = picked("1\n", &three()[..1]);
    assert_eq!(chosen.as_deref(), Some("a"));
    let text = format!("{:<50}", "open todo");
    assert_eq!(
        drawn,
        format!(
            " [1] To Do        {text} \u{b7} h1\nChoose [1-1] launch \u{b7} a toggle all (showing open) \u{b7} q quit\n> "
        )
    );
}

#[test]
fn test_the_picker_toggles_every_status_and_names_a_task_with_no_origin() {
    let mut all = three();
    all[1].origin_session.clear();
    let (chosen, drawn) = picked("a\n2\n", &all);
    assert_eq!(chosen.as_deref(), Some("b"));
    assert!(drawn.contains("(showing all)"), "{drawn}");
    assert!(drawn.contains(" \u{b7} (none)\n"), "{drawn}");
    let (_, drawn) = picked("a\na\nq\n", &all);
    assert_eq!(drawn.matches("(showing open)").count(), 2, "and back");
}

#[test]
fn test_the_picker_asks_again_for_a_row_it_cannot_read() {
    for wrong in ["0", "4", "x", "-1"] {
        let (chosen, drawn) = picked(&format!("{wrong}\n1\n"), &three());
        assert_eq!(chosen.as_deref(), Some("a"), "{wrong}");
        assert!(
            drawn.contains("  (enter a row number, a to toggle all statuses, q to quit)\n"),
            "{wrong}: {drawn}"
        );
    }
    let (chosen, _) = picked("2\n", &three());
    assert_eq!(chosen.as_deref(), Some("c"), "the open view's last row");
}

#[test]
fn test_the_picker_with_nothing_open_or_nothing_at_all() {
    let (chosen, drawn) = picked("q\n", &[]);
    assert_eq!((chosen, drawn.as_str()), (None, "(no tasks)\n"));
    let done = vec![task("b", "done", "Done", "")];
    let (chosen, drawn) = picked("q\n", &done);
    assert_eq!(chosen, None);
    assert!(drawn.starts_with("(no open tasks; press a to show all statuses, q to quit)\n"));
}

#[test]
fn test_a_long_text_is_not_cut() {
    let long = "x".repeat(60);
    let (_, drawn) = picked("q\n", &[task("a", &long, TO_DO, "h")]);
    assert!(drawn.contains(&format!("{long} \u{b7} h")), "{drawn}");
}

// ------------------------------------------------------------ the real launcher

/// A stub `ctxloom` in `dir`, running `body` under /bin/sh.
fn stub(dir: &Path, body: &str) {
    let path = dir.join("ctxloom");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn on(dir: &Path) -> Ctxloom {
    Ctxloom {
        path: Some(dir.as_os_str().to_owned()),
    }
}

#[test]
fn test_the_launcher_runs_the_ctxloom_on_its_path_with_the_args() {
    let dir = tempfile::tempdir().unwrap();
    let record = dir.path().join("args");
    stub(
        dir.path(),
        &format!("printf '%s\\n' \"$@\" > '{}'", record.display()),
    );
    let args = launch_args(&task("a", "two words", TO_DO, "h1"), false);
    on(dir.path()).launch(&args).expect("the stub succeeds");
    let recorded = std::fs::read_to_string(&record).unwrap();
    assert_eq!(recorded.lines().collect::<Vec<_>>(), args);
}

#[test]
fn test_the_launcher_reports_the_childs_status_or_its_absence() {
    let dir = tempfile::tempdir().unwrap();
    stub(dir.path(), "exit 3");
    assert_eq!(
        on(dir.path()).launch(&[]),
        Err(RunError::Exited { status: 3 })
    );
    stub(dir.path(), "kill -9 $$");
    assert!(matches!(
        on(dir.path()).launch(&[]),
        Err(RunError::Launch { .. })
    ));

    let empty = tempfile::tempdir().unwrap();
    assert_eq!(on(empty.path()).launch(&[]), Err(RunError::NoCtxloom));
    assert_eq!(Ctxloom { path: None }.launch(&[]), Err(RunError::NoCtxloom));
}

#[test]
fn test_the_launcher_skips_a_ctxloom_it_cannot_run() {
    let first = tempfile::tempdir().unwrap();
    std::fs::write(first.path().join("ctxloom"), "not executable").unwrap();
    std::fs::create_dir(first.path().join("sub")).unwrap();
    let second = tempfile::tempdir().unwrap();
    stub(second.path(), "exit 0");
    let path = std::env::join_paths([first.path(), second.path()]).unwrap();
    let found = Ctxloom { path: Some(path) }.find();
    assert_eq!(found, Some(second.path().join("ctxloom")));
}
