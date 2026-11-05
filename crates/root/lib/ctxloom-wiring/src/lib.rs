//! ctxloom's wiring: the core's ports over their default adapters, built
//! once for the binary and the tests.
//!
//! - [`session_name`]: a session's harp, `adjective-adjective-noun`, from
//!   taskloom's `harp-names` (its project-id shape) behind the `Names` port.
//! - [`run_id`]: a run's id, UUIDv5 of its session and incarnation (D5 Q4),
//!   so the same incarnation of the same session is the same run.
//! - [`Wiring`]: the process's home and filesystem, and the project stores
//!   opened over them.

mod commands;
mod core_link;
mod error;
mod isolation;
mod library;
#[cfg(feature = "test-support")]
mod seed;
mod setup;
mod sweep_config;

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use config_files::{find_app_dir, AppDir, ConfigFilesFs, Surroundings};
use ctxloom_store_sqlite::{SqliteStores, Stores};
use file_system::{Fs, Os};
use harp_names::HarpNames;
use isolation_application::{IsolationError, Report, WorktreeUse, Worktrees};
use liveness::LockFiles;
use ports::Names;
use project_registry::{Environment, ProjectRegistry};
use reclaim_application::SweepConfig;
use session_files::{GoDirs, SessionDirs};
use sessions_application::{
    AppError, BindRequest, Bound, ImportReport, ImportRequest, ImportService, Importer,
    KeepRequest, Kept, ListQuery, Removal, RemoveRequest, RenameRequest, Renamed, SearchQuery,
    Service, SessionService, ShowQuery, TranscriptQuery, TranscriptRow, View,
};
use system::System;

use crate::library::PathSearch;
use uuid::Uuid;
use worktree_git::GitWorkspaces;

pub use commands::{engines, Commands, Hooks};
pub use core_link::InProcessCore;
#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{LibError, EN_US};
pub use library::LibraryCommands;
#[cfg(feature = "test-support")]
pub use seed::Seed;
pub use sweep_config::SweepSettings;

/// A newly minted session's harp: two adjectives and a noun.
pub fn session_name() -> String {
    HarpNames.project_id()
}

/// The run id of `incarnation` of `session`: UUIDv5, in the session's
/// namespace, of the incarnation's decimal digits.
pub fn run_id(session: &str, incarnation: u64) -> Result<Uuid, LibError> {
    let namespace = Uuid::parse_str(session).map_err(|e| LibError::BadSession {
        session: session.to_string(),
        detail: e.to_string(),
    })?;
    Ok(Uuid::new_v5(&namespace, incarnation.to_string().as_bytes()))
}

/// The sessions use cases over their default adapters: the project registry
/// (taskloom's, so a project is the same one in both programs), the project's
/// SQLite store, the session directories, the lock files, and the system's
/// clock, ids and timer.
pub type SessionUseCases =
    Service<ProjectRegistry<HarpNames>, SqliteStores, SessionDirs, LockFiles, System, System>;

/// The import over the same adapters, and Go's session directories.
pub type ImportUseCase = Importer<ProjectRegistry<HarpNames>, SqliteStores, GoDirs, System, System>;

/// Everything the `session` commands and the session hooks call: the sessions
/// use cases and the import, one value to the command line.
pub struct Sessions {
    pub use_cases: SessionUseCases,
    pub importer: ImportUseCase,
    /// The scratch worktrees sessions left behind (S11).
    pub worktrees: Worktrees<GitWorkspaces, LockFiles>,
    /// `session_purge_age`, for a sweep given no `--purge-older-than`.
    pub settings: SweepSettings,
}

impl SweepConfig for Sessions {
    fn purge_age(&mut self) -> Result<Option<String>, ctxloom_ports::Failure> {
        self.settings.purge_age()
    }
}

impl WorktreeUse for Sessions {
    fn worktrees(&mut self, harp: Option<&str>, apply: bool) -> Result<Report, IsolationError> {
        self.worktrees.worktrees(harp, apply)
    }
}

impl SessionService for Sessions {
    async fn list(&mut self, query: ListQuery) -> Result<Vec<View>, AppError> {
        self.use_cases.list(query).await
    }

    async fn search(&mut self, query: SearchQuery) -> Result<Vec<View>, AppError> {
        self.use_cases.search(query).await
    }

    async fn show(&mut self, query: ShowQuery) -> Result<View, AppError> {
        self.use_cases.show(query).await
    }

    async fn keep(&mut self, request: KeepRequest) -> Result<Kept, AppError> {
        self.use_cases.keep(request).await
    }

    async fn remove(&mut self, request: RemoveRequest) -> Result<Removal, AppError> {
        self.use_cases.remove(request).await
    }

    async fn rename(&mut self, request: RenameRequest) -> Result<Renamed, AppError> {
        self.use_cases.rename(request).await
    }

    async fn transcripts(
        &mut self,
        query: TranscriptQuery,
    ) -> Result<Vec<TranscriptRow>, AppError> {
        self.use_cases.transcripts(query).await
    }

    async fn transcript(&mut self, harp: &str) -> Result<String, AppError> {
        self.use_cases.transcript(harp).await
    }

    async fn bind(&mut self, request: BindRequest) -> Result<Bound, AppError> {
        self.use_cases.bind(request).await
    }
}

impl ImportService for Sessions {
    async fn import(&mut self, request: ImportRequest) -> Result<ImportReport, AppError> {
        self.importer.import(request).await
    }
}

/// Every catalog the binary's diagnostics are rendered from, beside its
/// frontends' own: angzarr's (the kernel's reasons, the store's), the
/// platform's, taskloom's project registry's, and this program's adapters and
/// use cases'.
pub fn catalog_sources() -> Vec<angzarr_presentation::Source> {
    let mut sources = vec![
        EN_US,
        angzarr_embedded::angzarr_proto::REASONS_EN_US,
        angzarr_embedded::EN_US,
        angzarr_embedded::angzarr_ports::EN_US,
        angzarr_embedded::angzarr_store_sqlite::EN_US,
        file_lock::EN_US,
        file_system::EN_US,
        project_registry::EN_US,
        ctxloom_store_sqlite::EN_US,
        session_files::EN_US,
    ];
    sources.extend(sessions_application::CATALOGS);
    sources.extend(isolation_application::CATALOGS);
    sources.extend([worktree_git::EN_US, container_cli::EN_US]);
    sources
}

/// The process's home and filesystem.
pub struct Wiring {
    fs: Arc<dyn Fs>,
    home: PathBuf,
}

impl Wiring {
    /// The wiring as the process gives it: its home (HOME, or USERPROFILE on
    /// Windows), and the real filesystem over its working directory (with
    /// none when it cannot be read: only what needs it fails).
    pub fn from_process() -> Result<Wiring, LibError> {
        let host = os::host();
        let variable = host.flavor().home_var();
        let home = std::env::var_os(variable)
            .filter(|home| !home.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| LibError::NoHome {
                variable: variable.to_string(),
            })?;
        let fs = match std::env::current_dir().and_then(|cwd| Os::new(host.clone(), &cwd)) {
            Ok(fs) => fs,
            Err(why) => Os::without_cwd(host, why),
        };
        Ok(Wiring {
            fs: Arc::new(fs),
            home,
        })
    }

    pub(crate) fn fs(&self) -> Arc<dyn Fs> {
        Arc::clone(&self.fs)
    }

    pub(crate) fn home(&self) -> &std::path::Path {
        &self.home
    }

    /// Where the process is: the root variable, the working directory, the
    /// home and the temporary root.
    pub(crate) fn surroundings(&self) -> Surroundings {
        Surroundings {
            root_env: std::env::var("CTXLOOM_ROOT").ok(),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            home: self.home.clone(),
            temp_root: std::env::temp_dir(),
        }
    }

    /// The surroundings, and the app dir found in them.
    pub(crate) fn locate(&self) -> (Surroundings, AppDir) {
        let around = self.surroundings();
        let app = find_app_dir(self.fs.as_ref(), &around);
        (around, app)
    }

    /// The programs on the process's `PATH`, found through the filesystem.
    pub(crate) fn path_search(&self) -> PathSearch {
        PathSearch {
            fs: Arc::clone(&self.fs),
            dirs: std::env::var_os("PATH")
                .map(|path| std::env::split_paths(&path).collect())
                .unwrap_or_default(),
        }
    }

    /// The sessions use cases over the default adapters, in this process's
    /// home and working directory, reading CTXLOOM_ROOT and CTXLOOM_PROJECT_ID
    /// as the project registry does.
    pub fn sessions(&self) -> Sessions {
        let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
        let environment = Environment {
            home: Some(self.home.clone()),
            root: var("CTXLOOM_ROOT").map(PathBuf::from),
            project_id: var("CTXLOOM_PROJECT_ID").and_then(|v| v.into_string().ok()),
        };
        let registry =
            || ProjectRegistry::new(Arc::clone(&self.fs), environment.clone(), HarpNames);
        let stores = || SqliteStores::new(Arc::clone(&self.fs), self.home.clone());
        Sessions {
            use_cases: Service::new(
                registry(),
                stores(),
                SessionDirs::new(Arc::clone(&self.fs), self.home.clone()),
                LockFiles::new(Arc::clone(&self.fs), self.home.clone()),
                System,
                System,
            ),
            importer: Importer::new(
                registry(),
                stores(),
                GoDirs::new(Arc::clone(&self.fs), self.home.clone()),
                System,
                System,
            ),
            worktrees: Worktrees::new(
                GitWorkspaces::new(self.home.clone()),
                LockFiles::new(Arc::clone(&self.fs), self.home.clone()),
            ),
            settings: SweepSettings::new(
                Arc::new(ConfigFilesFs::new(
                    Arc::clone(&self.fs),
                    self.locate().1,
                    &self.home,
                )),
                std::env::vars().collect(),
            ),
        }
    }

    /// `project`'s store opened (made, if need be) under the home; the path
    /// of its database.
    pub fn open_store(&self, project: &str) -> Result<PathBuf, LibError> {
        let stores = Stores::new(Arc::clone(&self.fs), self.home.clone());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e: io::Error| LibError::NoRuntime {
                detail: e.to_string(),
            })?;
        runtime
            .block_on(stores.open(project))
            .map_err(LibError::Store)?;
        Ok(stores.path(project))
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
