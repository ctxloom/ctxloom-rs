//! `ctxloom init` (S12).
//!
//! BUILT: `init` on a directory that already has a `.ctxloom` (what every
//! re-run does): it says so, changes none of the configuration, and
//! registers the `--remote` repositories as `personal`, `personal-2`, ...
//! (a re-run once read and threw them away); and `init prompt`, the setup
//! interview body.
//!
//! NOT BUILT: `init` of a directory with no `.ctxloom`. It scaffolds, seeds
//! and CLONES the trusted `ctxloom-default` remote over the network, pulls
//! what its default profile resolves through, and hands the lot to the
//! engine for the setup session. The clone is S3's seeded default, and the
//! launch is S9's: it is refused, saying so, rather than half done. Also
//! not here: minting the project identity (taskloom's registry owns it; a
//! re-run only reads it), the dependency pull a re-run ends with, and the
//! agent-token check (S5's credentials).

mod error;

use std::rc::Rc;
use std::sync::Arc;

pub use ctxloom_ports::{Scaffold, UserFiles};
use deps_application::{CreateRequest, Deps, Notice};
use serde::Serialize;

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{InitError, EN_US};

/// The setup interview's body: ctxloom's own words, copied byte for byte
/// from Go ctxloom's `resources/commands/ctxloom-init.md` (release/0.7,
/// f36e8dfb21; the file's blob is 702251f0877f), its front matter dropped.
/// D24: the owner's text, never edited here. It names `agent_run`, an MCP tool
/// whose place in the cut list (D8) is still open.
pub const SETUP_INTERVIEW: &str = include_str!("../resources/setup-interview.md");

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
}

impl Init {
    pub fn new(scaffold: Arc<dyn Scaffold>, files: Arc<dyn UserFiles>, deps: Rc<Deps>) -> Init {
        Init {
            scaffold,
            files,
            deps,
        }
    }

    pub fn init(&self, request: &InitRequest) -> Result<Initialized, InitError> {
        let app_dir = self.scaffold.app_dir();
        let existed = self.files.is_dir(&app_dir).map_err(InitError::Files)?;
        if !existed {
            return Err(InitError::NewProjectNotBuilt { app_dir });
        }
        let mut done = Initialized {
            app_dir,
            existed,
            remotes: Vec::new(),
            notices: Vec::new(),
        };
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
