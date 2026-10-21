//! `taskloom watch`: a project's task changes as a JSON Lines stream, one
//! line per change, in Go taskloom's shape:
//! `{"event":"changed","kind":"tasks","project":"<project-id>"}`. A
//! subscriber (a GUI) re-queries the task list on each line. One line is
//! written at once, before any change, so a subscriber renders the
//! current state with no separate first query to race.
//!
//! Changes come from the store, not the filesystem: the watch use case
//! polled on every tick of a [`Ticks`] (every [`DEBOUNCE`] in the binary).
//! A burst of commits is one line, written once a tick finds nothing new,
//! or after [`CEILING_TICKS`] ticks of a burst that does not stop, as Go's
//! debounce and its ceiling do. The stream runs until its stop future
//! resolves (the binary's: SIGINT or SIGTERM, as Go's; never its stdin
//! closing, since a subscriber such as the VS Code extension starts it
//! with stdin from /dev/null), or a line cannot be written.
//!
//! Both formats are the stream: text and JSON alike write JSON Lines, as
//! Go's watch does. An error is reported as every command's is.

mod error;

use std::future::{poll_fn, Future};
use std::io::{self, Write};
use std::pin::pin;
use std::task::Poll;
use std::time::Duration;

use clap::Subcommand;
use presentation::{Catalog, Format};
use serde::Serialize;
use tasks_application::{WatchRequest, WatchService};

pub use error::{diagnostic_samples, WatchError, EN_US};

/// The quiet a burst of commits waits for before its line: Go's
/// `watchDebounce`, and the binary's tick.
pub const DEBOUNCE: Duration = Duration::from_millis(100);

/// The ticks a burst that does not stop may withhold its line for: Go's
/// `debounceCeiling`, ten debounces.
pub const CEILING_TICKS: u32 = 10;

/// The command, for the binary to mount.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum Group {
    /// Stream task-store change events as JSONL (one line per change)
    #[command(
        hide = true,
        long_about = "Watch the project's task store and emit a JSONL event whenever it changes.\n\n\
Long-lived: runs until interrupted (Ctrl-C / SIGTERM); a closed stdin does not\n\
stop it. One line at once, then one per burst of changes, e.g.\n\
{\"event\":\"changed\",\"kind\":\"tasks\",\"project\":\"swift-amber-falcon\"}. A GUI\n\
subscribes to this and re-queries the task list on each event, so it stays live\n\
without polling or watching the store files itself (the watch plumbing lives in\n\
the backend, not the client)."
    )]
    Watch,
}

/// What the stream is run with.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Settings {
    /// How an error is reported; the stream itself is JSON Lines in both.
    pub format: Format,
    /// `--project`: the project to watch, overriding resolution.
    pub project: Option<String>,
    /// The program's name: every diagnostic's prefix.
    pub prog: &'static str,
}

/// When to poll: the stream polls once each tick.
pub trait Ticks {
    fn tick(&mut self) -> impl Future<Output = ()>;
}

/// Ticks every `period`, the first a period after it starts; a tick late
/// (a slow poll) delays the ones after it rather than bunching them.
pub struct Interval(tokio::time::Interval);

impl Interval {
    /// Ticks every `period`. Needs a tokio runtime with its timer.
    pub fn every(period: Duration) -> Interval {
        let start = tokio::time::Instant::now() + period;
        let mut interval = tokio::time::interval_at(start, period);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        Interval(interval)
    }
}

impl Ticks for Interval {
    async fn tick(&mut self) {
        self.0.tick().await;
    }
}

/// When a burst of changes, seen tick by tick, gets its line.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Debounce {
    /// Ticks since the burst under way began; none between bursts.
    burst: Option<u32>,
}

impl Debounce {
    /// One tick, which found changes or did not: whether to write a line
    /// now. A burst's line is written at its first quiet tick, or once it
    /// has gone on for [`CEILING_TICKS`], when the next change begins a new
    /// one.
    pub fn tick(&mut self, changed: bool) -> bool {
        match (self.burst, changed) {
            (None, false) => false,
            (None, true) => {
                self.burst = Some(0);
                false
            }
            (Some(_), false) => {
                self.burst = None;
                true
            }
            (Some(ticks), true) => {
                let ticks = ticks + 1;
                if ticks >= CEILING_TICKS {
                    self.burst = None;
                    return true;
                }
                self.burst = Some(ticks);
                false
            }
        }
    }
}

/// One line of the stream: Go's `watchEvent`.
#[derive(Serialize, Clone, PartialEq, Eq, Debug)]
pub struct ChangeLine<'a> {
    /// Always "changed".
    pub event: &'static str,
    /// Always "tasks".
    pub kind: &'static str,
    /// The project the change belongs to.
    pub project: &'a str,
}

impl<'a> ChangeLine<'a> {
    pub fn of(project: &'a str) -> ChangeLine<'a> {
        ChangeLine {
            event: "changed",
            kind: "tasks",
            project,
        }
    }
}

/// Stream `service`'s changes to `out`, polling on each of `ticks`, until
/// `stop` resolves (exit 0) or the watch fails (exit 1, the error on
/// `err`, rendered from `catalog`).
pub async fn run<W: WatchService>(
    service: &mut W,
    settings: &Settings,
    ticks: &mut impl Ticks,
    stop: impl Future<Output = ()>,
    catalog: &Catalog,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    match stream(service, settings, ticks, stop, out).await {
        Ok(()) => 0,
        Err(error) => {
            let _ = report(&error, settings, catalog, err);
            1
        }
    }
}

/// What woke the stream.
enum Woken {
    Stop,
    Tick,
}

async fn stream<W: WatchService>(
    service: &mut W,
    settings: &Settings,
    ticks: &mut impl Ticks,
    stop: impl Future<Output = ()>,
    out: &mut dyn Write,
) -> Result<(), WatchError> {
    let request = WatchRequest {
        project: settings.project.clone(),
    };
    let mut watch = service
        .watch(request)
        .await
        .map_err(WatchError::App)?
        .ok_or(WatchError::NoProject)?;
    let project = watch.project.id.clone();
    let emit = |out: &mut dyn Write| {
        presentation::write_json_line(out, &ChangeLine::of(&project)).map_err(|e| {
            WatchError::Output {
                detail: e.to_string(),
            }
        })
    };
    emit(out)?;
    let mut stop = pin!(stop);
    let mut debounce = Debounce::default();
    loop {
        let woken = {
            let mut tick = pin!(ticks.tick());
            poll_fn(|cx| {
                if stop.as_mut().poll(cx).is_ready() {
                    return Poll::Ready(Woken::Stop);
                }
                tick.as_mut().poll(cx).map(|()| Woken::Tick)
            })
            .await
        };
        if let Woken::Stop = woken {
            return Ok(());
        }
        let changes = service.poll(&mut watch).await.map_err(WatchError::App)?;
        if debounce.tick(!changes.is_empty()) {
            emit(out)?;
        }
    }
}

/// `error` on stderr: its envelope in JSON; in text its catalog message
/// after the program's name.
fn report(
    error: &WatchError,
    settings: &Settings,
    catalog: &Catalog,
    err: &mut dyn Write,
) -> io::Result<()> {
    match settings.format {
        Format::Json => presentation::write_json(err, &catalog.envelope(error)),
        Format::Text => writeln!(err, "{}: {}", settings.prog, catalog.message(error)),
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
