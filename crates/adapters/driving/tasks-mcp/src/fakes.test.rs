//! The real use cases over fake ports, for the server's tests: a store
//! whose first commits conflict, kept from one call's service to the
//! next; a wait between retries that can be held open; and an identity
//! that records the thread it was asked on.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use std::time::Duration;

use annals::next::v1::{Cover, EventPage, Snapshot, Uuid};
use annals::next::{
    AsyncEventStore, AsyncReadStore, AsyncStore, AsyncUnitOfWork, InMemoryRead, InMemoryStore,
    InMemoryTx, Ready, Slice, StoreError, Timestamp,
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

/// A work root with a project, or none; or no work root at all, in
/// `rootless`, the one project there is found only by reading every
/// project. Every thread it was asked on is recorded.
pub struct FakeIdentity {
    pub bound: bool,
    pub rootless: Option<String>,
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
        Ok(vec![project()])
    }

    fn rootless_directory(&mut self) -> Result<Option<String>, Report> {
        self.asked();
        Ok(self.rootless.clone())
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

impl AsyncReadStore for Contended {
    type Reads<'s> = Ready<InMemoryRead<'s>>;

    async fn begin_read(&mut self) -> Ready<InMemoryRead<'_>> {
        self.inner.begin_read().await
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

/// One store, made on a write's open; none for a read before one. It is
/// taken from `home` when the service is made and put back when it is
/// dropped, so the next call's service finds what this one wrote.
pub struct FakeStores {
    store: Option<Contended>,
    conflicts: u32,
    home: Arc<Mutex<Option<Contended>>>,
}

impl Drop for FakeStores {
    fn drop(&mut self) {
        if let Some(store) = self.store.take() {
            *self.home.lock().unwrap() = Some(store);
        }
    }
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

/// Harps minted in turn across every call's service: `swift-amber`,
/// then `swift-amber-2`, and so on.
pub struct FakeNames {
    minted: Arc<AtomicU32>,
}

impl Names for FakeNames {
    fn task_id(&mut self) -> String {
        match self.minted.fetch_add(1, Ordering::Relaxed) {
            0 => "swift-amber".to_string(),
            n => format!("swift-amber-{}", n + 1),
        }
    }

    fn project_id(&mut self) -> String {
        unreachable!("the application mints no project id")
    }
}

/// A fixed instant, and command ids unique across every call's service,
/// as the system's are: a command id seen again is a redelivery.
pub struct FakeClock {
    ids: Arc<AtomicU32>,
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
        let id = self.ids.fetch_add(1, Ordering::Relaxed) + 1;
        let mut bytes = [0; 16];
        bytes[..4].copy_from_slice(&id.to_be_bytes());
        Uuid::from_bytes(bytes)
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
    pub rootless: Option<String>,
    pub conflicts: u32,
    pub store: Arc<Mutex<Option<Contended>>>,
    pub hold: Mutex<Option<(oneshot::Sender<()>, oneshot::Receiver<()>)>>,
    pub asked_on: Arc<Mutex<Vec<ThreadId>>>,
    pub made: Mutex<u32>,
    pub minted: Arc<AtomicU32>,
    pub ids: Arc<AtomicU32>,
}

impl World {
    /// A fresh service, as the composition root makes one per call.
    pub fn service(&self) -> FakeService {
        *self.made.lock().unwrap() += 1;
        let identity = FakeIdentity {
            bound: self.bound,
            rootless: self.rootless.clone(),
            asked_on: Arc::clone(&self.asked_on),
        };
        let stores = FakeStores {
            store: self.store.lock().unwrap().take(),
            conflicts: self.conflicts,
            home: Arc::clone(&self.store),
        };
        let delay = HeldDelay {
            hold: self.hold.lock().unwrap().take(),
        };
        let names = FakeNames {
            minted: Arc::clone(&self.minted),
        };
        let clock = FakeClock {
            ids: Arc::clone(&self.ids),
        };
        Service::new(identity, stores, names, clock, delay)
    }
}
