//! The stream over a scripted watch: each tick is driven by the test and
//! each poll answers what the script says was committed before it, so the
//! interleaving of commits and ticks is the test's, never the clock's.

use std::cell::Cell;
use std::collections::VecDeque;
use std::future::{poll_fn, Future};
use std::path::PathBuf;
use std::pin::pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use annals_diagnostic::Report;
use presentation::{Catalog, Format};
use serde_json::Value;
use tasks_application::{AppError, Changed, Project, Watch, WatchRequest, WatchService};

use super::{run, ChangeLine, Debounce, Interval, Settings, Ticks, CEILING_TICKS, DEBOUNCE};

/// Run a future whose every await is ready at once.
fn ready<F: Future>(future: F) -> F::Output {
    let mut cx = Context::from_waker(Waker::noop());
    match pin!(future).poll(&mut cx) {
        Poll::Ready(out) => out,
        Poll::Pending => panic!("the fakes' futures are ready at once"),
    }
}

fn project() -> Project {
    Project {
        id: "jolly-mute-magma".to_string(),
        root: "/work/tree".to_string(),
        store: PathBuf::from("/home/.ctxloom/projects/jolly-mute-magma/state.db"),
    }
}

fn removed() -> Changed {
    Changed::Removed {
        harp_id: "swift-amber".to_string(),
    }
}

/// A watch whose polls answer, in turn, what the script holds; then
/// nothing.
#[derive(Default)]
struct Scripted {
    project: Option<Project>,
    fails: Option<AppError>,
    polls: VecDeque<Result<Vec<Changed>, AppError>>,
    requests: Vec<WatchRequest>,
    polled: u32,
}

impl Scripted {
    fn of(polls: Vec<Vec<Changed>>) -> Scripted {
        Scripted {
            project: Some(project()),
            polls: polls.into_iter().map(Ok).collect(),
            ..Scripted::default()
        }
    }
}

impl WatchService for Scripted {
    async fn watch(&mut self, request: WatchRequest) -> Result<Option<Watch>, AppError> {
        self.requests.push(request);
        if let Some(error) = self.fails.clone() {
            return Err(error);
        }
        Ok(self.project.clone().map(Watch::of))
    }

    async fn poll(&mut self, _: &mut Watch) -> Result<Vec<Changed>, AppError> {
        self.polled += 1;
        self.polls.pop_front().unwrap_or(Ok(Vec::new()))
    }
}

/// `n` ticks, each ready at once; after the last, the stop is asked for.
struct Steps {
    left: u32,
    stop: Rc<Cell<bool>>,
}

impl Ticks for Steps {
    async fn tick(&mut self) {
        assert!(self.left > 0, "the stream ticks only until stopped");
        self.left -= 1;
        if self.left == 0 {
            self.stop.set(true);
        }
    }
}

/// `n` ticks and the stop after them.
fn steps(n: u32) -> (Steps, impl Future<Output = ()>) {
    let stop = Rc::new(Cell::new(n == 0));
    let asked = Rc::clone(&stop);
    let future = poll_fn(move |_| {
        if asked.get() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    });
    (Steps { left: n, stop }, future)
}

fn settings(format: Format) -> Settings {
    Settings {
        format,
        project: None,
        prog: "taskloom",
    }
}

fn catalog() -> Catalog {
    let mut sources = vec![crate::EN_US];
    sources.extend(tasks_application::CATALOGS);
    Catalog::en_us(&sources).expect("the catalog loads")
}

/// What a run wrote: its exit code, stdout's lines, stderr.
struct Streamed {
    code: u8,
    lines: Vec<String>,
    err: String,
}

fn stream(service: &mut Scripted, settings: &Settings, ticks: u32) -> Streamed {
    let (mut ticks, stop) = steps(ticks);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = ready(run(
        service,
        settings,
        &mut ticks,
        stop,
        &catalog(),
        &mut out,
        &mut err,
    ));
    Streamed {
        code,
        lines: String::from_utf8(out)
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect(),
        err: String::from_utf8(err).unwrap(),
    }
}

const LINE: &str = r#"{"event":"changed","kind":"tasks","project":"jolly-mute-magma"}"#;

#[test]
fn test_a_line_is_written_at_once_before_any_change() {
    let mut service = Scripted::of(vec![]);
    let streamed = stream(&mut service, &settings(Format::Text), 0);
    assert_eq!(streamed.code, 0, "{}", streamed.err);
    assert_eq!(streamed.lines, [LINE]);
    assert_eq!(service.polled, 0, "stopped before any tick");
}

#[test]
fn test_each_line_is_gos_event_one_json_document_per_line() {
    let mut service = Scripted::of(vec![vec![removed()]]);
    let streamed = stream(&mut service, &settings(Format::Json), 2);
    assert_eq!(streamed.lines.len(), 2);
    for line in &streamed.lines {
        let event: Value = serde_json::from_str(line).expect("one JSON document a line");
        assert_eq!(
            event,
            serde_json::json!({"event": "changed", "kind": "tasks", "project": "jolly-mute-magma"})
        );
    }
    assert_eq!(
        serde_json::to_string(&ChangeLine::of("p")).unwrap(),
        r#"{"event":"changed","kind":"tasks","project":"p"}"#,
        "Go's field order"
    );
}

#[test]
fn test_ticks_that_find_nothing_write_nothing() {
    let mut service = Scripted::of(vec![]);
    let streamed = stream(&mut service, &settings(Format::Text), 5);
    assert_eq!(streamed.code, 0);
    assert_eq!(streamed.lines, [LINE]);
    assert_eq!(service.polled, 5, "a poll each tick");
}

#[test]
fn test_a_burst_of_commits_is_one_line_once_a_tick_finds_nothing_new() {
    let mut service = Scripted::of(vec![vec![removed()], vec![removed(), removed()], vec![]]);
    // Two ticks into the burst nothing has been written for it.
    let streamed = stream(&mut service, &settings(Format::Text), 2);
    assert_eq!(streamed.lines, [LINE]);

    let mut service = Scripted::of(vec![vec![removed()], vec![removed(), removed()], vec![]]);
    let streamed = stream(&mut service, &settings(Format::Text), 3);
    assert_eq!(streamed.lines, [LINE, LINE]);
}

#[test]
fn test_a_burst_that_does_not_stop_still_gets_a_line_at_the_ceiling() {
    let busy = vec![vec![removed()]; 30];
    let mut service = Scripted::of(busy.clone());
    let streamed = stream(&mut service, &settings(Format::Text), CEILING_TICKS);
    assert_eq!(streamed.lines.len(), 1, "not yet");
    let mut service = Scripted::of(busy);
    let streamed = stream(&mut service, &settings(Format::Text), CEILING_TICKS + 1);
    assert_eq!(streamed.lines.len(), 2, "the ceiling, ten debounces in");
}

#[test]
fn test_the_debounce_writes_at_the_first_quiet_tick_or_the_ceiling() {
    let mut debounce = Debounce::default();
    assert!(!debounce.tick(false), "no burst, no line");
    assert!(!debounce.tick(true), "a burst begins");
    assert!(!debounce.tick(true));
    assert!(debounce.tick(false), "quiet: the burst's line");
    assert!(!debounce.tick(false), "one line a burst");

    let fired: Vec<bool> = (0..=2 * CEILING_TICKS + 1)
        .map(|_| debounce.tick(true))
        .collect();
    let at: Vec<usize> = fired
        .iter()
        .enumerate()
        .filter_map(|(i, &f)| f.then_some(i))
        .collect();
    let ceiling = usize::try_from(CEILING_TICKS).unwrap();
    assert_eq!(at, [ceiling, 2 * ceiling + 1], "each burst starts afresh");
    assert_eq!(CEILING_TICKS, 10);
    assert_eq!(DEBOUNCE, Duration::from_millis(100), "Go's debounce");
}

#[test]
fn test_a_watch_where_there_is_no_project_says_so_and_writes_no_line() {
    let mut service = Scripted::default();
    let streamed = stream(&mut service, &settings(Format::Text), 3);
    assert_eq!(streamed.code, 1);
    assert!(streamed.lines.is_empty());
    assert_eq!(
        streamed.err,
        "taskloom: no project here yet; add a task first (taskloom add \"<task text>\")\n"
    );
    assert_eq!(service.polled, 0);

    let streamed = stream(&mut Scripted::default(), &settings(Format::Json), 3);
    let envelope: Value = serde_json::from_str(&streamed.err).expect("an envelope");
    assert_eq!(envelope["code"], "tasks-watch-no-project");
}

#[test]
fn test_a_pinned_project_is_the_one_watched() {
    let mut service = Scripted::of(vec![]);
    let pinned = Settings {
        project: Some("jolly-mute-magma".to_string()),
        ..settings(Format::Text)
    };
    stream(&mut service, &pinned, 0);
    assert_eq!(
        service.requests,
        [WatchRequest {
            project: Some("jolly-mute-magma".to_string())
        }]
    );
}

#[test]
fn test_a_watch_or_a_poll_that_fails_ends_the_stream_with_why() {
    let why = AppError::Failed(Report::of(&AppError::NoProject));
    let mut service = Scripted {
        fails: Some(why.clone()),
        ..Scripted::of(vec![])
    };
    let streamed = stream(&mut service, &settings(Format::Json), 3);
    assert_eq!(streamed.code, 1);
    assert!(streamed.lines.is_empty());
    let envelope: Value = serde_json::from_str(&streamed.err).unwrap();
    assert_eq!(envelope["code"], "tasks-application-no-project");

    let mut service = Scripted::of(vec![vec![removed()]]);
    service.polls.push_back(Err(why));
    let streamed = stream(&mut service, &settings(Format::Text), 3);
    assert_eq!(streamed.code, 1);
    assert_eq!(streamed.lines, [LINE], "what was written stays written");
    assert!(streamed.err.starts_with("taskloom: "), "{}", streamed.err);
}

/// A reader that went away.
struct Gone;

impl std::io::Write for Gone {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_a_stream_no_one_reads_ends_with_why() {
    let mut service = Scripted::of(vec![]);
    let (mut ticks, stop) = steps(3);
    let mut err = Vec::new();
    let code = ready(run(
        &mut service,
        &settings(Format::Json),
        &mut ticks,
        stop,
        &catalog(),
        &mut Gone,
        &mut err,
    ));
    assert_eq!(code, 1);
    let envelope: Value = serde_json::from_str(&String::from_utf8(err).unwrap()).unwrap();
    assert_eq!(envelope["code"], "tasks-watch-output");
    assert_eq!(service.polled, 0);
}

#[test]
fn test_an_interval_ticks_no_sooner_than_its_period() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let period = Duration::from_millis(20);
    let started = Instant::now();
    runtime.block_on(async {
        let mut interval = Interval::every(period);
        interval.tick().await;
        interval.tick().await;
    });
    assert!(started.elapsed() >= 2 * period, "{:?}", started.elapsed());
}
