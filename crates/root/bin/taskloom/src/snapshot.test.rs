//! Acceptance: a list reads one moment of a project's SQLite store, though
//! another process writes and purges between the list's `covers` and its
//! reads of each stream. The interleaving is forced: the list's store
//! runs the other process as soon as its `covers` has answered.

use std::path::PathBuf;

use annals::next::v1::{Cover, EventPage, Snapshot, Uuid};
use annals::next::{AsyncEventStore, AsyncReadStore, AsyncReadUnit, AsyncStore, Slice, StoreError};
use annals_store_sql::sqlite::{SqliteRead, SqliteStore, SqliteUnit};
use harp_names::HarpNames;
use ports::{Project, TaskStores};
use project_registry::{Environment, ProjectRegistry};
use system::System;
use tasks_application::{ListQuery, Reach, Service, TaskService};

use super::{one_document, Env};

/// What the other process does, run once, between a read's `covers` and
/// its reads.
type Meanwhile<'e> = Box<dyn FnOnce() + Send + 'e>;

/// The project's SQLite store, opened by the list's own handle, running
/// `meanwhile` once its `covers` has answered: through a read-only unit,
/// or on the store itself.
struct Interleaved<'e> {
    store: SqliteStore,
    meanwhile: Option<Meanwhile<'e>>,
}

/// A read-only unit over [`Interleaved`]'s store.
struct InterleavedRead<'s, 'e> {
    reads: SqliteRead<'s>,
    meanwhile: &'s mut Option<Meanwhile<'e>>,
}

fn run(meanwhile: &mut Option<Meanwhile<'_>>) {
    if let Some(meanwhile) = meanwhile.take() {
        meanwhile();
    }
}

impl AsyncEventStore for Interleaved<'_> {
    async fn append(
        &mut self,
        stream: &Cover,
        expected: u64,
        pages: &[EventPage],
    ) -> Result<u64, StoreError> {
        self.store.append(stream, expected, pages).await
    }

    async fn read_from(&mut self, stream: &Cover, from: u64) -> Result<Option<Slice>, StoreError> {
        self.store.read_from(stream, from).await
    }

    async fn find_command(
        &mut self,
        stream: &Cover,
        command_id: &Uuid,
    ) -> Result<Vec<EventPage>, StoreError> {
        self.store.find_command(stream, command_id).await
    }

    async fn covers(&mut self, domain: &str) -> Result<Vec<Cover>, StoreError> {
        let covers = self.store.covers(domain).await;
        run(&mut self.meanwhile);
        covers
    }

    async fn read_snapshot(&mut self, stream: &Cover) -> Result<Option<Snapshot>, StoreError> {
        self.store.read_snapshot(stream).await
    }
}

impl AsyncStore for Interleaved<'_> {
    type Unit<'s>
        = SqliteUnit<'s>
    where
        Self: 's;

    async fn begin(&mut self) -> SqliteUnit<'_> {
        self.store.begin().await
    }
}

impl<'e> AsyncReadStore for Interleaved<'e> {
    type Reads<'s>
        = InterleavedRead<'s, 'e>
    where
        Self: 's;

    async fn begin_read(&mut self) -> InterleavedRead<'_, 'e> {
        InterleavedRead {
            reads: self.store.begin_read().await,
            meanwhile: &mut self.meanwhile,
        }
    }
}

impl AsyncReadUnit for InterleavedRead<'_, '_> {
    async fn covers(&mut self, domain: &str) -> Result<Vec<Cover>, StoreError> {
        let covers = self.reads.covers(domain).await;
        run(self.meanwhile);
        covers
    }

    async fn read_from(&mut self, stream: &Cover, from: u64) -> Result<Option<Slice>, StoreError> {
        self.reads.read_from(stream, from).await
    }
}

/// The one project's store, opened read-only on first ask, with the
/// other process's turn armed.
struct Stores<'e> {
    held: Option<Interleaved<'e>>,
    meanwhile: Option<Meanwhile<'e>>,
}

impl<'e> TaskStores for Stores<'e> {
    type Store = Interleaved<'e>;

    async fn open(&mut self, project: &Project) -> Result<&mut Interleaved<'e>, StoreError> {
        Ok(self.open_existing(project).await?.expect("a seeded store"))
    }

    async fn open_existing(
        &mut self,
        project: &Project,
    ) -> Result<Option<&mut Interleaved<'e>>, StoreError> {
        if self.held.is_none() {
            let Some(store) = SqliteStore::open_existing(&project.store).await? else {
                return Ok(None);
            };
            self.held = Some(Interleaved {
                store,
                meanwhile: self.meanwhile.take(),
            });
        }
        Ok(self.held.as_mut())
    }
}

/// Add `args` as a task through the binary; its harp.
fn add(env: &Env, args: &[&str]) -> String {
    let added = env.run(&[&["add"][..], args, &["--json"]].concat());
    assert!(added.ok, "{}", added.stderr);
    one_document(&added.stdout)["harp_id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn test_a_list_reads_one_moment_though_another_process_writes_and_purges_meanwhile() {
    let env = Env::new("snapshot-harp");
    let moving = add(&env, &["Moving"]);
    let pruned = add(&env, &["Pruned", "--status", "Done"]);
    let moved = moving.clone();
    let meanwhile: Meanwhile<'_> = Box::new(|| {
        let ran = env.run(&["prune", "--older-than", "1ns", "--yes", "--json"]);
        assert!(ran.ok, "{}", ran.stderr);
        assert_eq!(one_document(&ran.stdout)["tasks_pruned"], 1);
        let ran = env.run(&["status", &moved, "In Progress", "--json"]);
        assert!(ran.ok, "{}", ran.stderr);
        add(&env, &["Added meanwhile"]);
    });
    let environment = Environment {
        home: Some(env.home.path().to_path_buf()),
        cwd: Ok(PathBuf::from(env.project.path())),
        root: None,
        project_id: None,
    };
    let stores = Stores {
        held: None,
        meanwhile: Some(meanwhile),
    };
    let mut service = Service::new(
        ProjectRegistry::new(environment, HarpNames),
        stores,
        HarpNames,
        System,
        System,
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let all = ListQuery {
        all: true,
        ..ListQuery::default()
    };
    let listing = runtime.block_on(service.list(all)).unwrap();
    assert!(matches!(listing.reach, Reach::Project(_)));
    assert!(
        service.stores.held.as_ref().unwrap().meanwhile.is_none(),
        "the other process ran between the list's covers and its reads"
    );
    let seen: Vec<(&str, &str)> = listing
        .tasks()
        .map(|t| (t.harp_id.as_str(), t.status.as_str()))
        .collect();
    assert_eq!(
        seen,
        [(moving.as_str(), "To Do"), (pruned.as_str(), "Done")],
        "the store as the list's covers saw it: nothing moved, purged or added since"
    );

    let listed = env.run(&["list", "--all", "--json"]);
    assert!(listed.ok, "{}", listed.stderr);
    let after: Vec<String> = one_document(&listed.stdout)
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            format!(
                "{} {}",
                t["text"].as_str().unwrap(),
                t["status"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(after, ["Moving In Progress", "Added meanwhile To Do"]);
}
