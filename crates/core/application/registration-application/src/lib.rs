//! The registration use cases, Go taskloom's `manage`: write one MCP
//! server (taskloom's, `taskloom mcp`) into agents' configurations,
//! take it back out, and report where it is registered. The agents and
//! their files are behind [`AgentConfigs`]; this crate decides which
//! agents a call touches and what it answers.
//!
//! An install with no agent named touches every agent in use at the
//! scope, and refuses when there is none; one named is written, its
//! configuration made if need be. An uninstall with none in use answers
//! nothing to do. Ownership of an entry already there is the port's to
//! prove, from the executable it runs: the server's command, as a bare
//! name.

mod error;

use std::path::{Path, PathBuf};

use annals_diagnostic::Report;
pub use ports::{AgentConfigs, McpServer, Rewritten, Scope};

pub use error::{diagnostic_samples, RegistrationError, EN_US};

/// The name taskloom's server is registered under, Go's `TaskloomName`.
pub const TASKLOOM: &str = "taskloom";

/// taskloom's server: the bare command, resolved on the agent's own PATH
/// when it starts the server, so a registration survives the binary
/// moving (Go's `TaskloomServer`).
pub fn taskloom_server() -> McpServer {
    McpServer {
        command: TASKLOOM.to_string(),
        args: vec!["mcp".to_string()],
    }
}

/// An install: of the agent named, or of every one in use at `scope`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Install {
    pub engine: Option<String>,
    pub scope: Scope,
    /// Compute every configuration, write none.
    pub print_only: bool,
}

/// An uninstall: from the agent named, or from every one in use at
/// `scope`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Uninstall {
    pub engine: Option<String>,
    pub scope: Scope,
}

/// One agent's configuration, as a registration left it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub engine: String,
    pub config: PathBuf,
    pub rewritten: Rewritten,
}

/// What an install did: each agent's configuration, and whether the
/// server's command resolves here (an agent resolves it on its own PATH,
/// so a registration of a command that does not is worth a warning, not
/// a refusal).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installed {
    pub resolvable: bool,
    pub outcomes: Vec<Outcome>,
}

/// Whether one configuration holds the server.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Registered,
    NotRegistered,
    /// The configuration is there and cannot be read: said, never
    /// dropped, for it is not the same answer as no configuration.
    Unreadable(Report),
}

/// One configuration there is, as a check found it.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub engine: String,
    /// "user" or "project".
    pub scope: &'static str,
    pub path: PathBuf,
    pub state: State,
}

/// Where the server is registered: every agent's configuration at both
/// scopes that exists, and whether its command resolves here.
#[derive(Clone, Debug, PartialEq)]
pub struct Checked {
    pub resolvable: bool,
    pub found: Vec<Found>,
}

/// The use cases, registering `server` under `name`.
pub struct Registrations<A> {
    pub configs: A,
    pub name: String,
    pub server: McpServer,
}

impl<A: AgentConfigs> Registrations<A> {
    /// taskloom's server, under its name.
    pub fn taskloom(configs: A) -> Registrations<A> {
        Registrations {
            configs,
            name: TASKLOOM.to_string(),
            server: taskloom_server(),
        }
    }

    /// Write the server into the configurations `request` names.
    pub fn install(&mut self, request: &Install) -> Result<Installed, RegistrationError> {
        let engines = self.engines(request.engine.as_deref(), &request.scope)?;
        if engines.is_empty() {
            return Err(RegistrationError::NoBackends {
                known: self.known(),
            });
        }
        let resolvable = self.configs.resolvable(&self.server.command);
        let server = self.server.clone();
        let outcomes = self.rewrite(&engines, &request.scope, Some(&server), request.print_only)?;
        Ok(Installed {
            resolvable,
            outcomes,
        })
    }

    /// Take the server out of the configurations `request` names; none
    /// when no agent is in use there.
    pub fn uninstall(&mut self, request: &Uninstall) -> Result<Vec<Outcome>, RegistrationError> {
        let engines = self.engines(request.engine.as_deref(), &request.scope)?;
        self.rewrite(&engines, &request.scope, None, false)
    }

    /// Where the server is registered, the project scope's configurations
    /// those under `dir`.
    pub fn check(&mut self, dir: &Path) -> Checked {
        let resolvable = self.configs.resolvable(&self.server.command);
        let mut found = Vec::new();
        for engine in self.configs.engines() {
            let scopes = [
                ("user", Scope::User),
                ("project", Scope::Project(dir.to_path_buf())),
            ];
            for (label, scope) in scopes {
                let Ok(path) = self.configs.config_path(&engine, &scope) else {
                    continue;
                };
                let state = match self.configs.installed(&engine, &path, &self.name) {
                    Ok(None) => continue,
                    Ok(Some(true)) => State::Registered,
                    Ok(Some(false)) => State::NotRegistered,
                    Err(report) => State::Unreadable(report),
                };
                found.push(Found {
                    engine: engine.clone(),
                    scope: label,
                    path,
                    state,
                });
            }
        }
        Checked { resolvable, found }
    }

    /// The agents a call touches: the one named, or every one in use at
    /// `scope`.
    fn engines(
        &mut self,
        named: Option<&str>,
        scope: &Scope,
    ) -> Result<Vec<String>, RegistrationError> {
        let known = self.configs.engines();
        if let Some(name) = named {
            if !known.iter().any(|e| e == name) {
                return Err(RegistrationError::UnknownEngine {
                    engine: name.to_string(),
                    known: known.join(", "),
                });
            }
            return Ok(vec![name.to_string()]);
        }
        Ok(known
            .into_iter()
            .filter(|engine| self.configs.present(engine, scope))
            .collect())
    }

    fn known(&self) -> String {
        self.configs.engines().join(", ")
    }

    /// Write `server` (or its removal) into each of `engines`'
    /// configurations at `scope`, in order, stopping at the first that
    /// fails.
    fn rewrite(
        &mut self,
        engines: &[String],
        scope: &Scope,
        server: Option<&McpServer>,
        dry_run: bool,
    ) -> Result<Vec<Outcome>, RegistrationError> {
        let mut outcomes = Vec::new();
        for engine in engines {
            let failed = |error: Report| RegistrationError::EngineFailed {
                engine: engine.clone(),
                error,
            };
            let config = self.configs.config_path(engine, scope).map_err(failed)?;
            let rewritten = self
                .configs
                .register(
                    engine,
                    &config,
                    &self.name,
                    &self.server.command,
                    server,
                    dry_run,
                )
                .map_err(failed)?;
            outcomes.push(Outcome {
                engine: engine.clone(),
                config,
                rewritten,
            });
        }
        Ok(outcomes)
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
