//! The Deps inbound port (S3): the registered remotes, the dependency closure
//! and everything that moves it, and publishing a bundle, over the git, file
//! and content ports. A command is a method of [`Deps`] that answers a view
//! (data, in the shape the command prints) and the notices it raised on the
//! way; the driving adapter renders both.
//!
//! - A remote is an address. Registering one is the trust act, and the
//!   consent to publish to it; it installs nothing and holds no credential.
//! - The closure is every remote bundle the project's profiles follow to; the
//!   lock pins each to a commit. `pull` makes the installation match
//!   upstream and never moves a pin; `upgrade` is the one thing that does,
//!   and shows what each move brings in first.
//! - Absence is authority only from a repository this run proved it could
//!   read.

mod check;
mod closure;
mod discover;
mod error;
/// In-memory ports, for this crate's tests and a driving adapter's.
#[cfg(any(test, feature = "test-support"))]
pub mod fakes;
mod library;
mod listing;
mod publish;
mod pull;
mod registry;
mod upgrade;
mod views;

use std::sync::Arc;

use ctxloom_ports::ContentRepo;
use deps::lock::{self, Lock};
use deps::registry::Registry;
use deps_ports::{DepsFiles, Endpoint, Forge, Opened, PullRequest, Remotes, Why};

pub use discover::DiscoverRequest;
#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{AppError, Notice, EN_US};
pub use publish::{MoveRequest, PushRequest};
pub use registry::CreateRequest;
pub use views::*;

/// What a command came to: its answer or its error, and what it noticed.
pub struct Outcome<T> {
    pub result: Result<T, AppError>,
    pub notices: Vec<Notice>,
}

/// The dependency commands, over their ports.
pub struct Deps {
    remotes: Arc<dyn Remotes>,
    files: Arc<dyn DepsFiles>,
    content: Arc<dyn ContentRepo>,
    forge: Arc<dyn Forge>,
}

/// The forge of a build that wired none: it finds nothing and opens nothing,
/// and says so.
struct NoForge;

impl Forge for NoForge {
    fn search_repositories(
        &self,
        _: &Endpoint,
        _: &str,
        _: usize,
    ) -> Result<Vec<deps_ports::ForgeRepo>, Why> {
        Err("this build has no forge adapter".to_string())
    }

    fn open_pull_request(&self, _: &Endpoint, _: &PullRequest) -> Result<Opened, Why> {
        Err("this build has no forge adapter".to_string())
    }
}

impl Deps {
    pub fn new(
        remotes: Arc<dyn Remotes>,
        files: Arc<dyn DepsFiles>,
        content: Arc<dyn ContentRepo>,
    ) -> Deps {
        Deps {
            remotes,
            files,
            content,
            forge: Arc::new(NoForge),
        }
    }

    /// The same, with a forge's API behind `remote discover` and `bundle push
    /// --pr`.
    pub fn with_forge(mut self, forge: Arc<dyn Forge>) -> Deps {
        self.forge = forge;
        self
    }

    /// The registry; none written yet is an empty one.
    pub(crate) fn registry(&self) -> Result<Registry, AppError> {
        let path = self.files.registry_path();
        match self.files.read_registry() {
            Ok(Some(text)) => Registry::decode(&text, &path).map_err(AppError::cause),
            Ok(None) => Ok(Registry::empty()),
            Err(detail) => Err(AppError::File { what: path, detail }),
        }
    }

    pub(crate) fn save_registry(&self, registry: &Registry) -> Result<(), AppError> {
        self.files
            .write_registry(&registry.encode())
            .map_err(|detail| AppError::File {
                what: self.files.registry_path(),
                detail,
            })
    }

    /// The lock and its text; none written yet is an empty one. A lock that
    /// cannot be read is refused and stays as it is.
    pub(crate) fn lock(&self) -> Result<(Lock, Option<String>), AppError> {
        let path = self.files.lock_path();
        match self.files.read_lock() {
            Ok(Some(text)) => Ok((
                Lock::decode(&text, &path).map_err(AppError::cause)?,
                Some(text),
            )),
            Ok(None) => Ok((Lock::default(), None)),
            Err(detail) => Err(AppError::File { what: path, detail }),
        }
    }

    /// Check that `lock` may be written over `current`, writing nothing.
    pub(crate) fn check_lock(
        &self,
        current: Option<&str>,
        lock: &Lock,
        allow_empty: bool,
    ) -> Result<(), AppError> {
        lock::guard_write(current, lock, allow_empty, &self.files.lock_path())
            .map_err(AppError::cause)
    }

    /// Write `lock` over `current` after the guard, and only if it differs:
    /// a rewrite at unchanged pins touches nothing.
    pub(crate) fn save_lock(
        &self,
        current: Option<&str>,
        lock: &Lock,
        allow_empty: bool,
    ) -> Result<(), AppError> {
        self.check_lock(current, lock, allow_empty)?;
        let text = lock.encode();
        if current == Some(text.as_str()) {
            return Ok(());
        }
        self.files
            .write_lock(&text)
            .map_err(|detail| AppError::File {
                what: self.files.lock_path(),
                detail,
            })
    }
}
