//! ctxloom's wiring: the core's ports over their default adapters, built
//! once for the binary and the tests.
//!
//! - [`session_name`]: a session's harp, `adjective-adjective-noun`, from
//!   taskloom's `harp-names` (its project-id shape) behind the `Names` port.
//! - [`run_id`]: a run's id, UUIDv5 of its session and incarnation (D5 Q4),
//!   so the same incarnation of the same session is the same run.
//! - [`Wiring`]: the process's home and filesystem, and the project stores
//!   opened over them.

mod error;

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use ctxloom_store_sqlite::Stores;
use file_system::{Fs, Os};
use harp_names::HarpNames;
use ports::Names;
use uuid::Uuid;

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{LibError, EN_US};

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
