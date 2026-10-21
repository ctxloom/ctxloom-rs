//! The real use cases over fake ports, for the server's tests: a store
//! whose first commits conflict, a wait between retries that can be held
//! open, and an identity that records the thread it was asked on.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use std::time::Duration;

use annals::next::v1::{Cover, EventPage, Snapshot, Uuid};
use annals::next::{
    AsyncEventStore, AsyncStore, AsyncUnitOfWork, InMemoryStore, InMemoryTx, Ready, Slice,
    StoreError, Timestamp,
};
use annals_diagnostic::Report;
use ports::{Clock, CommandIds, Delay, Jitter, Names, Project, ProjectIdentity, TaskStores};
use tasks_application::Service;
use tokio::sync::oneshot;

pub fn project() -> Project {
    Project {
        id: "jolly-mute-magma".to_string(),
        root: "/work/tree".to_string(),
        store: PathBuf::from("/home/.ctxloom/projects/jolly-mute-magma/state.db"),
    }
}

/// A work root with a project, or none; every thread it was asked on is
/// recorded.
pub struct FakeIdentity {
    pub bound: bool,
    pub asked_on: Arc<Mutex<Vec<ThreadId>>>,
}

impl FakeIdentity {
    fn asked(&self) {
        self.asked_on
            .lock()
            .unwrap()
            .push(std::thread::current().id());
    }
}

impl ProjectIdentity for FakeIdentity {
    fn for_write(&mut self, _pinned: Option<&str>) -> Result<Project, Report> {
        self.asked();
        Ok(project())
    }

    fn for_read(&mut self, _pinned: Option<&str>) -> Result<Option<Project>, Report> {
        self.asked();
        Ok(self.bound.then(project))
    }

    fn every_project(&mut self) -> Result<Vec<Project>, Report> {
        self.asked();
        Ok(Vec::new())
    }

    /// Always in a work root: the server's reads never fall back to every
    /// project here.
    fn rootless_directory(&mut self) -> Result<Option<String>, Report> {
        self.asked();
        Ok(None)
    }
}

/// An in-memory store whose first `conflicts` commits are refused, as when
/// another writer appended first.
pub struct Contended {
    inner: Ready<InMemoryStore>,
    conflicts: u32,
}

pub struct ContendedUnit<'s> {
    inner: Ready<InMemoryTx<'s>>,
    conflict: bool,
}

impl AsyncEventStore for Contended {
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

impl AsyncStore for Contended {
    type Unit<'s> = ContendedUnit<'s>;

    async fn begin(&mut self) -> ContendedUnit<'_> {
        let conflict = self.conflicts > 0;
        self.conflicts = self.conflicts.saturating_sub(1);
        ContendedUnit {
            inner: self.inner.begin().await,
            conflict,
        }
    }
}

impl AsyncEventStore for ContendedUnit<'_> {
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

impl AsyncUnitOfWork for ContendedUnit<'_> {
    async fn commit(self) -> Result<(), StoreError> {
        if self.conflict {
            return Err(StoreError::Conflict {
                stream: Cover {
                    domain: "tasks".to_string(),
                    root: None,
                    edition: None,
                },
                expected: 0,
                actual: 1,
            });
        }
        self.inner.commit().await
    }
}

/// One store, made on a write's open; none for a read before one.
pub struct FakeStores {
    store: Option<Contended>,
    conflicts: u32,
}

impl TaskStores for FakeStores {
    type Store = Contended;

    async fn open(&mut self, _project: &Project) -> Result<&mut Contended, StoreError> {
        let conflicts = self.conflicts;
        Ok(self.store.get_or_insert_with(|| Contended {
            inner: Ready(InMemoryStore::new()),
            conflicts,
        }))
    }

    async fn open_existing(
        &mut self,
        _project: &Project,
    ) -> Result<Option<&mut Contended>, StoreError> {
        Ok(self.store.as_mut())
    }
}

pub struct FakeNames;

impl Names for FakeNames {
    fn task_id(&mut self) -> String {
        "swift-amber".to_string()
    }

    fn project_id(&mut self) -> String {
        unreachable!("the application mints no project id")
    }
}

#[derive(Default)]
pub struct FakeClock {
    ids: u8,
}

impl Clock for FakeClock {
    fn now(&mut self) -> Timestamp {
        Timestamp {
            seconds: 1_781_058_920,
            nanos: 837_009_562,
        }
    }
}

impl CommandIds for FakeClock {
    fn command_id(&mut self) -> Uuid {
        self.ids += 1;
        Uuid::from_bytes([self.ids; 16])
    }
}

/// The wait before a retry, held open when `hold` is set: it says it has
/// begun on `hold.0`, then waits for `hold.1`.
#[derive(Default)]
pub struct HeldDelay {
    pub hold: Option<(oneshot::Sender<()>, oneshot::Receiver<()>)>,
}

impl Delay for HeldDelay {
    async fn sleep(&mut self, _duration: Duration) {
        if let Some((begun, release)) = self.hold.take() {
            let _ = begun.send(());
            let _ = release.await;
        }
    }
}

impl Jitter for HeldDelay {
    fn up_to(&mut self, ceiling: Duration) -> Duration {
        ceiling
    }
}

pub type FakeService = Service<FakeIdentity, FakeStores, FakeNames, FakeClock, HeldDelay>;

/// What each call's service is made with. The first service made takes
/// the hold, if one is set.
#[derive(Default)]
pub struct World {
    pub bound: bool,
    pub conflicts: u32,
    pub hold: Mutex<Option<(oneshot::Sender<()>, oneshot::Receiver<()>)>>,
    pub asked_on: Arc<Mutex<Vec<ThreadId>>>,
    pub made: Mutex<u32>,
}

impl World {
    /// A fresh service, as the composition root makes one per call.
    pub fn service(&self) -> FakeService {
        *self.made.lock().unwrap() += 1;
        let identity = FakeIdentity {
            bound: self.bound,
            asked_on: Arc::clone(&self.asked_on),
        };
        let stores = FakeStores {
            store: None,
            conflicts: self.conflicts,
        };
        let delay = HeldDelay {
            hold: self.hold.lock().unwrap().take(),
        };
        Service::new(identity, stores, FakeNames, FakeClock::default(), delay)
    }
}
