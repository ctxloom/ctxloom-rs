//! `ctxloom init` (S12).
//!
//! BUILT: `init` on a directory that already has a `.ctxloom` (what every
//! re-run does): it says so, changes none of the configuration, and
//! registers the `--remote` repositories as `personal`, `personal-2`, ...
//! (a re-run once read and threw them away); and `init prompt`, the setup
//! interview body.
//!
//! `init` of a directory with no `.ctxloom`: it scaffolds the project (the
//! `Create` port, as `config create` does), which seeds the trusted
//! `ctxloom-default` remote and the default profile inheriting from it, then
//! pulls what that profile resolves through. A pull that fails (no network)
//! is said and does not undo the scaffold. It writes nothing outside
//! `.ctxloom`: the engine files wait for a session home.
//!
//! NOT BUILT: handing the project to the engine for the setup session on a
//! terminal (S9's launch), the interview's questions, minting the project
//! identity (taskloom's registry owns it; a re-run only reads it), and the
//! agent-token check (S5's credentials).

mod error;

use std::rc::Rc;
use std::sync::Arc;

pub use ctxloom_ports::{Failure, Scaffold, UserFiles};
use deps_application::{CreateRequest, Deps, Notice};
use serde::Serialize;

pub use error::InitError;

/// The setup interview's body: ctxloom's own words, copied byte for byte
/// from Go ctxloom's `resources/commands/ctxloom-init.md` (release/0.7,
/// f36e8dfb21; the file's blob is 702251f0877f), its front matter dropped.
/// D24: the owner's text, never edited here. It names `agent_run`, an MCP tool
/// whose place in the cut list (D8) is still open.
pub const SETUP_INTERVIEW: &str = include_str!("../resources/setup-interview.md");

/// Scaffolds a project's `.ctxloom`: the config, the remotes registry and the
/// seeded default profile; it refuses a directory that has a config.
pub trait Create {
    fn create(&self) -> Result<(), Failure>;
}

/// What `init` is asked.
#[derive(Clone, Debug, Default)]
pub struct InitRequest {
    /// `--remote`, repeatable.
    pub remotes: Vec<String>,
    /// `--forge`: the forge every `--remote` is bound to.
    pub forge: String,
}

/// A remote `init` registered.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct RemoteAdded {
    pub name: String,
    pub url: String,
}

/// What an `init` came to.
#[derive(Debug, Serialize)]
pub struct Initialized {
    pub app_dir: String,
    /// The directory was there already.
    pub existed: bool,
    pub remotes: Vec<RemoteAdded>,
    /// The dependencies the pull of a new project installed.
    pub pulled: u64,
    /// What went wrong without stopping init (a remote that would not
    /// register), for the error stream.
    #[serde(skip)]
    pub notices: Vec<Notice>,
}

/// The names the `--remote` repositories are registered under.
fn personal_name(index: usize) -> String {
    match index {
        0 => "personal".to_string(),
        n => format!("personal-{}", n + 1),
    }
}

/// The init use case.
pub struct Init {
    scaffold: Arc<dyn Scaffold>,
    files: Arc<dyn UserFiles>,
    deps: Rc<Deps>,
    create: Box<dyn Create>,
}

impl Init {
    pub fn new(
        scaffold: Arc<dyn Scaffold>,
        files: Arc<dyn UserFiles>,
        deps: Rc<Deps>,
        create: Box<dyn Create>,
    ) -> Init {
        Init {
            scaffold,
            files,
            deps,
            create,
        }
    }

    pub fn init(&self, request: &InitRequest) -> Result<Initialized, InitError> {
        let app_dir = self.scaffold.app_dir();
        let existed = self.files.is_dir(&app_dir).map_err(InitError::Files)?;
        let mut done = Initialized {
            app_dir,
            existed,
            remotes: Vec::new(),
            pulled: 0,
            notices: Vec::new(),
        };
        if !existed {
            self.create.create().map_err(InitError::Create)?;
            let pulled = self.deps.pull(false, true);
            done.notices.extend(pulled.notices);
            match pulled.result {
                Ok(report) => done.pulled = report.installed,
                Err(error) => done.notices.push(Notice::text(format!(
                    "could not pull the seeded dependencies: {error}"
                ))),
            }
        }
        for (index, url) in request.remotes.iter().enumerate() {
            let outcome = self.deps.remote_create(&CreateRequest {
                name: personal_name(index),
                url: url.clone(),
                forge: request.forge.clone(),
            });
            done.notices.extend(outcome.notices);
            match outcome.result {
                Ok(created) => done.remotes.push(RemoteAdded {
                    name: created.name,
                    url: created.url,
                }),
                Err(error) => done.notices.push(Notice::text(format!(
                    "failed to add remote \"{}\" ({url}): {error}",
                    personal_name(index)
                ))),
            }
        }
        Ok(done)
    }
}

/// `init prompt`: the interview body, as Go prints it (a line after it).
pub fn prompt() -> String {
    format!("{SETUP_INTERVIEW}\n")
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
