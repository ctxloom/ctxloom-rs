//! The Setup inbound port, surface half (S9; plan 2.1): what wires ctxloom
//! into a project and takes it back out.
//!
//! - [`Setup::materialize`]: a profile set's surfaces, written at rest as an
//!   engine's own native files, any subset of them, and released.
//! - [`Setup::install`], [`Setup::uninstall`], [`Setup::check`],
//!   [`Setup::hooks_list`], [`Setup::set_statusline`],
//!   [`Setup::gitignore_install`]: `manage`.
//! - [`Setup::session_start`], [`Setup::skill_mates`]: the engines' callbacks.
//!
//! The engines are ports (`surface-ports`): this layer decides which engine's
//! claims are delivered, for which kinds, and when; the files are the store's.
//! `init`, `doctor`, `clean` and `auth` are S12's and are not here.

mod config_write;
mod diff;
mod error;
#[cfg(test)]
mod fakes;
mod hooks;
mod manage;
mod materialize;
mod materialize_diff;
mod owner_voice;

use std::rc::Rc;
use std::sync::Arc;

use ctxloom_ports::Scaffold;
use library_application::{Library, Notice};
use surface_ports::{ForeignFiles, Registry, SurfaceStore};

pub use config_write::ConfigWritten;
#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{SetupError, EN_US};
pub use manage::{
    BackendStatus, CheckReport, GitignoreReport, HookRow, HooksEvent, HooksList, InstallPlan,
    InstallReport, InstallRequest, PullReport, SurfaceCurrency, UninstallReport,
};
pub use materialize::{
    EngineOutcome, MaterializeRequest, MaterializeResult, NotCarried, SurfaceSpec,
};
pub use materialize_diff::{DiffRequest, MaterializeDiff};

/// What a command came to besides its answer: the notices it raised.
#[derive(Debug)]
pub struct Done<T> {
    pub value: T,
    pub notices: Vec<Notice>,
}

/// The dependency pull `manage install` ends with, where the program has one.
pub trait Puller {
    /// Pull the dependencies the project's profiles reference; how many were
    /// newly installed.
    fn pull(&self) -> Result<u64, String>;
}

/// The setup use cases over their ports.
pub struct Setup {
    library: Library,
    engines: Registry,
    store: Arc<dyn SurfaceStore>,
    scaffold: Arc<dyn Scaffold>,
    puller: Option<Rc<dyn Puller>>,
    foreign: Option<Arc<dyn ForeignFiles>>,
    /// The project's directory (where `.ctxloom` is), else the working one.
    project_dir: String,
    /// The working directory, which relative targets are relative to.
    cwd: String,
}

impl Setup {
    /// The use cases over `library`, the `engines` composed in (the first the
    /// default), the surface `store` and the project `scaffold`.
    pub fn new(
        library: Library,
        engines: Registry,
        store: Arc<dyn SurfaceStore>,
        scaffold: Arc<dyn Scaffold>,
        project_dir: impl Into<String>,
        cwd: impl Into<String>,
    ) -> Setup {
        Setup {
            library,
            engines,
            store,
            scaffold,
            puller: None,
            foreign: None,
            project_dir: project_dir.into(),
            cwd: cwd.into(),
        }
    }

    /// `manage install` ends by pulling dependencies through `puller`.
    pub fn with_puller(mut self, puller: Rc<dyn Puller>) -> Setup {
        self.puller = Some(puller);
        self
    }

    /// `util config-write` merges into foreign files through `foreign`.
    pub fn with_foreign(mut self, foreign: Arc<dyn ForeignFiles>) -> Setup {
        self.foreign = Some(foreign);
        self
    }

    /// The registered engines' names.
    pub fn engine_names(&self) -> Vec<String> {
        self.engines.names()
    }

    /// `path` made absolute against the working directory.
    fn absolute(&self, path: &str) -> String {
        let joined = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("{}/{path}", self.cwd)
        };
        let mut parts: Vec<&str> = Vec::new();
        for part in joined.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                other => parts.push(other),
            }
        }
        format!("/{}", parts.join("/"))
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
