//! Fakes for the ports, and a store that fails on cue, for the use cases'
//! tests. Every future they return is ready at once, so [`ready`] runs a use
//! case to its end without a runtime.

use std::collections::VecDeque;
use std::future::Future;
use std::path::PathBuf;
use std::pin::pin;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use annals::next::v1::{Cover, EventPage, Snapshot, Uuid};
use annals::next::{
    AsyncEventStore, AsyncStore, AsyncUnitOfWork, InMemoryStore, InMemoryTx, Ready, Slice,
    StoreError, Timestamp,
};
use annals_diagnostic::Report;
use ports::{Clock, CommandIds, Delay, Jitter, Names, Project, ProjectIdentity, TaskStores};

/// Run a future whose every await is ready at once.
pub fn ready<F: Future>(future: F) -> F::Output {
    let mut cx = Context::from_waker(Waker::noop());
    match pin!(future).poll(&mut cx) {
        Poll::Ready(out) => out,
        Poll::Pending => panic!("the fakes' futures are ready at once"),
    }
}

pub fn project() -> Project {
    Project {
        id: "jolly-mute-magma".to_string(),
        root: "/work/tree".to_string(),
        store: PathBuf::from("/home/.ctxloom/projects/jolly-mute-magma/state.db"),
    }
}

/// A work root with a project, or none yet. A write binds one; a pin
/// resolves, as an explicit id always does; the pin a call gave is
/// recorded.
/// Every project is `every`; the working directory is `rootless` when
/// it lies in no work root.
#[derive(Default)]
pub struct FakeIdentity {
    pub bound: Option<Project>,
    pub pins: Vec<Option<String>>,
    pub fails: Option<Report>,
    pub every: Vec<Project>,
    pub rootless: Option<String>,
}

impl ProjectIdentity for FakeIdentity {
    fn for_write(&mut self, pinned: Option<&str>) -> Result<Project, Report> {
        self.pins.push(pinned.map(str::to_string));
        if let Some(report) = &self.fails {
            return Err(report.clone());
        }
        Ok(self.bound.get_or_insert_with(project).clone())
    }

    fn for_read(&mut self, pinned: Option<&str>) -> Result<Option<Project>, Report> {
        self.pins.push(pinned.map(str::to_string));
        if let Some(report) = &self.fails {
            return Err(report.clone());
        }
        Ok(self.bound.clone().or_else(|| pinned.map(|_| project())))
    }

    fn every_project(&mut self) -> Result<Vec<Project>, Report> {
        if let Some(report) = &self.fails {
            return Err(report.clone());
        }
        Ok(self.every.clone())
    }

    fn rootless_directory(&mut self) -> Result<Option<String>, Report> {
        if let Some(report) = &self.fails {
            return Err(report.clone());
        }
        Ok(self.rootless.clone())
    }
}

/// How one unit of work goes wrong.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fault {
    /// Its commit fails and nothing lands: the store could not say.
    Unavailable,
    /// Its commit lands, then reports failure: the answer was lost.
    LandsThenFails,
    /// Its commit is refused: someone appended first.
    Conflict,
    /// Its commit fails in a way no retry mends.
    Discontinuous,
}

/// An in-memory store whose units of work fail on cue, in order.
pub struct Flaky {
    pub inner: Ready<InMemoryStore>,
    pub faults: VecDeque<Fault>,
    /// Units begun.
    pub begun: u32,
    /// Fails the next open.
    pub open_fails: bool,
}

impl Default for Flaky {
    fn default() -> Self {
        Flaky {
            inner: Ready(InMemoryStore::new()),
            faults: VecDeque::new(),
            begun: 0,
            open_fails: false,
        }
    }
}

impl Flaky {
    pub fn failing(faults: &[Fault]) -> Flaky {
        Flaky {
            faults: faults.iter().copied().collect(),
            ..Flaky::default()
        }
    }
}

pub struct FlakyUnit<'s> {
    inner: Ready<InMemoryTx<'s>>,
    fault: Option<Fault>,
}

fn unavailable() -> StoreError {
    StoreError::Unavailable(Report::new("fake-busy", "the database is locked"))
}

fn some_cover() -> Cover {
    Cover {
        domain: "tasks".to_string(),
        root: None,
        edition: None,
    }
}

impl AsyncEventStore for Flaky {
    async fn append(
        &mut self,
        stream: &Cover,
        expected: u64,
        pages: &[EventPage],
    ) -> Result<u64, StoreError> {
        self.inner.append(stream, expected, pages).await
    }

    async fn read_from(&mut self, stream: &Cover, from: u64) -> Result<Option<Slice>, StoreError> {
        self.inner.read_from(stream, from).await
    }

    async fn find_command(
        &mut self,
        stream: &Cover,
        command_id: &Uuid,
    ) -> Result<Vec<EventPage>, StoreError> {
        self.inner.find_command(stream, command_id).await
    }

    async fn covers(&mut self, domain: &str) -> Result<Vec<Cover>, StoreError> {
        self.inner.covers(domain).await
    }
}

impl AsyncStore for Flaky {
    type Unit<'s> = FlakyUnit<'s>;

    async fn begin(&mut self) -> FlakyUnit<'_> {
        self.begun += 1;
        let fault = self.faults.pop_front();
        FlakyUnit {
            inner: self.inner.begin().await,
            fault,
        }
    }
}

impl AsyncEventStore for FlakyUnit<'_> {
    async fn append(
        &mut self,
        stream: &Cover,
        expected: u64,
        pages: &[EventPage],
    ) -> Result<u64, StoreError> {
        self.inner.append(stream, expected, pages).await
    }

    async fn read_from(&mut self, stream: &Cover, from: u64) -> Result<Option<Slice>, StoreError> {
        self.inner.read_from(stream, from).await
    }

    async fn find_command(
        &mut self,
        stream: &Cover,
        command_id: &Uuid,
    ) -> Result<Vec<EventPage>, StoreError> {
        self.inner.find_command(stream, command_id).await
    }

    async fn covers(&mut self, domain: &str) -> Result<Vec<Cover>, StoreError> {
        self.inner.covers(domain).await
    }

    async fn read_snapshot(&mut self, stream: &Cover) -> Result<Option<Snapshot>, StoreError> {
        self.inner.read_snapshot(stream).await
    }
}

impl AsyncUnitOfWork for FlakyUnit<'_> {
    async fn commit(self) -> Result<(), StoreError> {
        match self.fault {
            None => self.inner.commit().await,
            Some(Fault::Unavailable) => Err(unavailable()),
            Some(Fault::LandsThenFails) => {
                self.inner.commit().await?;
                Err(unavailable())
            }
            Some(Fault::Conflict) => Err(StoreError::Conflict {
                stream: some_cover(),
                expected: 0,
                actual: 1,
            }),
            Some(Fault::Discontinuous) => Err(StoreError::Discontinuous {
                stream: some_cover(),
                expected: 0,
                found: 3,
            }),
        }
    }
}

/// One store, whichever project is asked for: absent until a write opens
/// it, if `absent`. The projects opened to write (`opened`) and to read
/// (`looked`) are recorded.
#[derive(Default)]
pub struct FakeStores {
    pub store: Flaky,
    pub absent: bool,
    pub opened: Vec<String>,
    pub looked: Vec<String>,
}

impl TaskStores for FakeStores {
    type Store = Flaky;

    async fn open(&mut self, project: &Project) -> Result<&mut Flaky, StoreError> {
        self.opened.push(project.id.clone());
        if self.store.open_fails {
            return Err(unavailable());
        }
        self.absent = false;
        Ok(&mut self.store)
    }

    async fn open_existing(&mut self, project: &Project) -> Result<Option<&mut Flaky>, StoreError> {
        self.looked.push(project.id.clone());
        if self.store.open_fails {
            return Err(unavailable());
        }
        Ok((!self.absent).then_some(&mut self.store))
    }
}

/// Task ids in the order given; project ids are never asked for here.
#[derive(Default)]
pub struct FakeNames {
    pub task_ids: VecDeque<String>,
    /// Given forever once `task_ids` runs out, if set.
    pub then: Option<String>,
    pub minted: u32,
}

impl FakeNames {
    pub fn of(ids: &[&str]) -> FakeNames {
        FakeNames {
            task_ids: ids.iter().map(|s| s.to_string()).collect(),
            ..FakeNames::default()
        }
    }
}

impl Names for FakeNames {
    fn task_id(&mut self) -> String {
        self.minted += 1;
        self.task_ids
            .pop_front()
            .or_else(|| self.then.clone())
            .expect("a task id scripted")
    }

    fn project_id(&mut self) -> String {
        unreachable!("the application mints no project id")
    }
}

/// A clock one second on at each reading, and command ids numbered from 1.
#[derive(Default)]
pub struct FakeClock {
    pub seconds: i64,
    pub ids: Vec<Uuid>,
}

impl Clock for FakeClock {
    fn now(&mut self) -> Timestamp {
        self.seconds += 1;
        Timestamp {
            seconds: 1_700_000_000 + self.seconds,
            nanos: 0,
        }
    }
}

impl CommandIds for FakeClock {
    fn command_id(&mut self) -> Uuid {
        let n = u8::try_from(self.ids.len() + 1).expect("few ids");
        let id = Uuid::from_bytes([n; 16]);
        self.ids.push(id.clone());
        id
    }
}

/// Waits recorded, never waited; jitter is a fixed fraction (in
/// thousandths) of the ceiling, or a fixed duration when `over` is set.
pub struct FakeDelay {
    pub slept: Vec<Duration>,
    pub ceilings: Vec<Duration>,
    pub permille: u32,
    pub over: Option<Duration>,
}

impl Default for FakeDelay {
    fn default() -> Self {
        FakeDelay {
            slept: Vec::new(),
            ceilings: Vec::new(),
            permille: 1000,
            over: None,
        }
    }
}

impl Delay for FakeDelay {
    async fn sleep(&mut self, duration: Duration) {
        self.slept.push(duration);
    }
}

impl Jitter for FakeDelay {
    fn up_to(&mut self, ceiling: Duration) -> Duration {
        self.ceilings.push(ceiling);
        self.over.unwrap_or(ceiling * self.permille / 1000)
    }
}
