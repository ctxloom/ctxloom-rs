//! Agents' MCP configurations: where each agent (Claude Code, ...) keeps
//! the MCP servers it starts, and one named server written into or taken
//! out of one: the port behind `manage`.

use std::path::{Path, PathBuf};

use annals_diagnostic::Report;

/// Which of an agent's configurations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    /// The user's, under the home directory.
    User,
    /// A project's, under its directory.
    Project(PathBuf),
}

/// An MCP server as an agent's configuration names it: the command line
/// the agent runs to start it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpServer {
    pub command: String,
    pub args: Vec<String>,
}

/// What writing one server into a configuration did, or would do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rewritten {
    /// Whether the configuration's bytes change.
    pub changed: bool,
    /// The configuration as written (or as it would be).
    pub after: String,
}

/// Agents' MCP configurations.
///
/// Synchronous, and it blocks on the filesystem, as
/// [`crate::ProjectIdentity`] does.
pub trait AgentConfigs {
    /// Every agent known, by name, in a fixed order: what `--engine`
    /// names.
    fn engines(&self) -> Vec<String>;

    /// Whether `engine` looks to be in use at `scope`: its configuration,
    /// or its own directory, is there.
    fn present(&mut self, engine: &str, scope: &Scope) -> bool;

    /// `engine`'s MCP configuration file at `scope`.
    fn config_path(&mut self, engine: &str, scope: &Scope) -> Result<PathBuf, Report>;

    /// Write `server` under `name` into the configuration at `path`, or,
    /// when `server` is `None`, take out the entry under `name`; either way
    /// touching nothing else in the file. An entry already under `name` is
    /// taken out only when it runs `owner` (an executable's base name):
    /// one that runs anything else is someone's own, and an install
    /// refuses rather than overwrite it. A rewrite that changes nothing
    /// writes nothing, and `dry_run` writes nothing either.
    fn register(
        &mut self,
        engine: &str,
        path: &Path,
        name: &str,
        owner: &str,
        server: Option<&McpServer>,
        dry_run: bool,
    ) -> Result<Rewritten, Report>;

    /// Whether the configuration at `path` holds an entry under `name`;
    /// `None` when there is no configuration there.
    fn installed(&mut self, engine: &str, path: &Path, name: &str) -> Result<Option<bool>, Report>;

    /// Whether `command` resolves on this process's PATH: what an agent
    /// starting the server will look for on its own.
    fn resolvable(&mut self, command: &str) -> bool;
}
