//! The checks that read only the text they are given.

use serde::Serialize;

use crate::hooks::Stale;

/// A row's verdict.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Warn,
    Fail,
    /// A stated limit: not a verdict, so never `ok`.
    Info,
}

/// One row of the report.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct Check {
    pub marker: &'static str,
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
    /// What to do about a warning or failure.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub remedy: String,
}

fn row(
    marker: &'static str,
    name: &'static str,
    status: Status,
    detail: &str,
    remedy: &str,
) -> Check {
    Check {
        marker,
        name,
        status,
        detail: detail.to_string(),
        remedy: remedy.to_string(),
    }
}

/// `SETUP-MARKER-e5`: the project's `.ctxloom` and the config in it. A config
/// that will not parse is a finding, never a reason to stop the diagnosis.
pub fn setup_marker(has_project: bool, config: Option<&str>) -> Check {
    const M: &str = "DOCTOR-CHECK-SETUP-MARKER-e5";
    const N: &str = "setup marker";
    if !has_project {
        return row(
            M,
            N,
            Status::Warn,
            "no .ctxloom directory here or above: this is not a ctxloom project",
            "ctxloom init",
        );
    }
    match config.map(angzarr_yaml::parse) {
        None => row(
            M,
            N,
            Status::Warn,
            ".ctxloom is there but holds no config.yaml",
            "ctxloom config create",
        ),
        Some(Err(error)) => row(
            M,
            N,
            Status::Warn,
            &format!(".ctxloom/config.yaml does not parse: {error}"),
            "ctxloom config edit",
        ),
        Some(Ok(_)) => row(M, N, Status::Ok, ".ctxloom and its config.yaml read", ""),
    }
}

/// `DEPS-a1`: what the machine needs whatever the project is.
pub fn deps(git_found: bool, gitconfig: Option<&str>) -> Check {
    const M: &str = "DOCTOR-CHECK-DEPS-a1";
    const N: &str = "system dependencies";
    if !git_found {
        return row(
            M,
            N,
            Status::Warn,
            "git is not on the PATH: bundles and remotes need it",
            "install git",
        );
    }
    if !has_identity(gitconfig.unwrap_or_default()) {
        return row(
            M,
            N,
            Status::Warn,
            "git is there but has no identity (user.name and user.email): a commit would fail",
            "git config --global user.name \"Your Name\" && git config --global user.email you@example.com",
        );
    }
    row(
        M,
        N,
        Status::Ok,
        "git is on the PATH and has an identity",
        "",
    )
}

/// Whether a git config text sets `user.name` and `user.email`.
fn has_identity(gitconfig: &str) -> bool {
    let (mut in_user, mut name, mut email) = (false, false, false);
    for line in gitconfig.lines().map(str::trim) {
        if let Some(section) = line.strip_prefix('[') {
            in_user = section
                .trim_end_matches(']')
                .trim()
                .eq_ignore_ascii_case("user");
        } else if in_user {
            let key = line.split('=').next().unwrap_or_default().trim();
            let set = line.contains('=');
            name |= set && key.eq_ignore_ascii_case("name");
            email |= set && key.eq_ignore_ascii_case("email");
        }
    }
    name && email
}

/// `AGENTS-b2`: every configured agent names the llm it runs.
pub fn agents(config: Option<&str>) -> Check {
    const M: &str = "DOCTOR-CHECK-AGENTS-b2";
    const N: &str = "agents";
    let Some(Ok(value)) = config.map(angzarr_yaml::parse) else {
        return row(
            M,
            N,
            Status::Info,
            "no readable config to read agents from",
            "",
        );
    };
    let agents = match value.get("agents") {
        Some(angzarr_yaml::Value::Map(agents)) => agents.clone(),
        _ => Vec::new(),
    };
    if agents.is_empty() {
        return row(M, N, Status::Ok, "no agents are configured", "");
    }
    let unbound: Vec<&str> = agents
        .iter()
        .filter(|(_, agent)| {
            !matches!(agent.get("llm"), Some(angzarr_yaml::Value::Scalar(llm)) if !llm.is_empty())
        })
        .map(|(name, _)| name.as_str())
        .collect();
    if unbound.is_empty() {
        return row(
            M,
            N,
            Status::Ok,
            &format!("{} agent(s) configured, each naming its llm", agents.len()),
            "",
        );
    }
    row(
        M,
        N,
        Status::Warn,
        &format!(
            "agent(s) {} name no llm, so none resolves",
            unbound.join(", ")
        ),
        "ctxloom agent edit <name> --llm <llm>",
    )
}

/// `VERSION-c3`.
pub fn version(version: &str) -> Check {
    row(
        "DOCTOR-CHECK-VERSION-c3",
        "version",
        Status::Ok,
        &format!("ctxloom {version}"),
        "",
    )
}

/// `HOOKS-TRUST-d4`: whether ctxloom's hooks are registered with the engine.
pub fn hooks(settings: &[(String, String)]) -> Check {
    const M: &str = "DOCTOR-CHECK-HOOKS-TRUST-d4";
    const N: &str = "hooks";
    let registered = settings
        .iter()
        .any(|(_, text)| crate::hooks::registered(text));
    if registered {
        return row(
            M,
            N,
            Status::Ok,
            "ctxloom's hooks are registered with the engine",
            "",
        );
    }
    row(
        M,
        N,
        Status::Info,
        "no ctxloom hook is registered in the engine's settings; a launch carries them in its own session home",
        "ctxloom materialize",
    )
}

/// `GITIGNORE-f6`: a blanket `.ctxloom` ignore rule predates content moving
/// into `.ctxloom/`; left in place it un-tracks the project's own content.
pub fn gitignore(text: Option<&str>) -> Check {
    const M: &str = "DOCTOR-CHECK-GITIGNORE-f6";
    const N: &str = ".gitignore posture";
    let blanket = text
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .find(|line| {
            matches!(
                *line,
                ".ctxloom"
                    | ".ctxloom/"
                    | "/.ctxloom"
                    | "/.ctxloom/"
                    | ".ctxloom/*"
                    | "/.ctxloom/*"
                    | ".ctxloom/**"
                    | "/.ctxloom/**"
            )
        });
    match blanket {
        Some(rule) => row(
            M,
            N,
            Status::Warn,
            &format!(
                ".gitignore has the blanket rule `{rule}`: it un-tracks the project's own content under .ctxloom, so `git add` reports nothing and a content repository publishes an empty tree"
            ),
            "ctxloom manage gitignore install",
        ),
        None => row(M, N, Status::Ok, "no blanket .ctxloom ignore rule", ""),
    }
}

/// `MCP-INVOCATION-g7`: a settings file whose ctxloom entry LAUNCHES a
/// server. ctxloom ships no stdio MCP server, so the engine waits on a
/// handshake that never arrives. No record of this machine writing the entry
/// is read yet (S9 ports the records): the remedy is the by-hand one.
pub fn mcp_invocation(files: &[(String, String)]) -> Check {
    const M: &str = "DOCTOR-CHECK-MCP-INVOCATION-g7";
    const N: &str = "MCP invocation";
    let bad: Vec<&str> = files
        .iter()
        .filter(|(_, text)| crate::hooks::launches_ctxloom_mcp(text))
        .map(|(path, _)| path.as_str())
        .collect();
    let Some(first) = bad.first() else {
        return row(
            M,
            N,
            Status::Ok,
            "no settings file launches a ctxloom MCP server",
            "",
        );
    };
    row(
        M,
        N,
        Status::Warn,
        &format!(
            "the ctxloom entry from {first} launches `ctxloom mcp serve`, which does not exist: the session comes up without ctxloom's tools and nothing says why"
        ),
        &format!(
            "this machine holds no record of writing that entry, so delete it from {first} by hand; a session renders its own endpoint"
        ),
    )
}

/// `STALE-HOOKS-n5`: hook entries running a `ctxloom hook` subcommand this
/// ctxloom does not have.
pub fn stale_hooks(stale: &[(String, Stale)]) -> Check {
    const M: &str = "DOCTOR-CHECK-STALE-HOOKS-n5";
    const N: &str = "stale hook entries";
    let Some((file, first)) = stale.first() else {
        return row(
            M,
            N,
            Status::Ok,
            "no hook entry runs a ctxloom subcommand that is gone",
            "",
        );
    };
    let mut detail = format!(
        "{} runs `ctxloom hook {}` ({file}), which this ctxloom does not have; the engine runs it at every {} and it fails",
        first.event, first.sub, first.event
    );
    if stale.len() > 1 {
        detail.push_str(&format!("; {} entries in all", stale.len()));
    }
    row(M, N, Status::Warn, &detail, "ctxloom doctor --fix")
}

/// `INGESTION-q7`: the one thing no check can confirm.
pub fn ingestion() -> Check {
    row(
        "DOCTOR-CHECK-INGESTION-q7",
        "delivered versus ingested",
        Status::Info,
        "ctxloom writes the context onto the engine's surface, but whether the engine reads it happens in a process ctxloom does not own",
        "",
    )
}
