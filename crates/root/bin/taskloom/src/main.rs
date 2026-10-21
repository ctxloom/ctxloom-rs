//! taskloom: per-project task tracking. The composition root: it reads the
//! process's environment, wires the tasks use cases to their adapters (the
//! project registry, harp names, annals-store-sql's SQLite store, the
//! system clock and tokio's timer), and hands the command line to the CLI.
//! It holds no logic of its own.

use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use annals::next::v1::Uuid;
use annals::next::{StoreError, Timestamp};
use annals_diagnostic::Report;
use annals_store_sql::sqlite::SqliteStore;
use harp_names::HarpNames;
use ports::{Clock, CommandIds, Delay, Jitter, Project, TaskStores};
use project_registry::{Environment, ProjectRegistry, RegistryError};
use tasks_application::Service;
use tasks_cli::Context;

/// The project's SQLite store, opened on first use and kept for the call.
#[derive(Default)]
struct SqliteStores {
    open: Option<(PathBuf, SqliteStore)>,
}

impl TaskStores for SqliteStores {
    type Store = SqliteStore;

    async fn open(&mut self, project: &Project) -> Result<&mut SqliteStore, StoreError> {
        let held = self.open.as_ref().map(|(path, _)| path);
        if held != Some(&project.store) {
            if let Some(dir) = project.store.parent() {
                std::fs::create_dir_all(dir).map_err(|e| {
                    StoreError::Unavailable(Report::of(&RegistryError::Io {
                        path: dir.display().to_string(),
                        detail: e.to_string(),
                    }))
                })?;
            }
            let store = SqliteStore::open(&project.store).await?;
            self.open = Some((project.store.clone(), store));
        }
        Ok(&mut self.open.as_mut().expect("opened above").1)
    }
}

/// The system's clock, ids, timer and randomness.
struct System;

impl Clock for System {
    fn now(&mut self) -> Timestamp {
        let since = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Timestamp {
            seconds: i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
            nanos: i32::try_from(since.subsec_nanos()).unwrap_or(0),
        }
    }
}

impl CommandIds for System {
    fn command_id(&mut self) -> Uuid {
        Uuid::from_bytes(*uuid::Uuid::now_v7().as_bytes())
    }
}

impl Delay for System {
    async fn sleep(&mut self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

impl Jitter for System {
    fn up_to(&mut self, ceiling: Duration) -> Duration {
        let nanos = u64::try_from(ceiling.as_nanos()).unwrap_or(u64::MAX);
        Duration::from_nanos(rand::random_range(0..=nanos))
    }
}

fn main() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("{}: cannot start: {e}", tasks_cli::PROG);
            return ExitCode::FAILURE;
        }
    };
    let env = Environment {
        home: std::env::var_os("HOME").map(PathBuf::from),
        cwd: std::env::current_dir().unwrap_or_default(),
        root: std::env::var_os("CTXLOOM_ROOT").map(PathBuf::from),
        project_id: std::env::var("CTXLOOM_PROJECT_ID").ok(),
    };
    let identity = ProjectRegistry::new(env, HarpNames);
    let mut service = Service::new(identity, SqliteStores::default(), HarpNames, System, System);
    let context = Context {
        session: std::env::var("CTXLOOM_SESSION_HARP").unwrap_or_default(),
        stdout_is_terminal: io::stdout().is_terminal(),
    };
    let code = runtime.block_on(tasks_cli::run(
        std::env::args_os(),
        &context,
        &mut service,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    ));
    ExitCode::from(code)
}

#[cfg(test)]
#[path = "main.test.rs"]
mod tests;
