//! `ctxloom auth status` (S12), and the rules the doctor's credentials check
//! shares with it.
//!
//! ctxloom never collects, stores or mints a credential: a run reads the
//! engine's token from the environment it is launched in. This reports, per
//! engine, whether that credential is exported, and never its value: the only
//! thing the [`ExportedVars`] port can say of a variable is whether it is
//! exported, so a value cannot reach any output from here.

use std::sync::Arc;

pub use ctxloom_ports::ExportedVars;
use serde::Serialize;

/// The mode every agent ctxloom spawns authenticates in.
const TOKEN: &str = "token";

/// How the human supplies claude's token. It is minted with claude's own flow
/// and kept in their environment or secret manager: Anthropic does not allow
/// a third party to collect, store or intermediate Claude.ai credentials, so
/// no ctxloom command takes one.
const CLAUDE_REMEDY: &str = "run `claude setup-token` and export CLAUDE_CODE_OAUTH_TOKEN (or store it in your secret manager)";

/// One engine's credential in token mode.
struct Rule {
    engine: &'static str,
    var: &'static str,
    /// A run is refused without it. An optional one (opencode's endpoint key)
    /// leaves the run keyless.
    required: bool,
    remedy: &'static str,
}

/// The engines that read a credential from the environment, by name.
const RULES: [Rule; 2] = [
    Rule {
        engine: "claude-code",
        var: "CLAUDE_CODE_OAUTH_TOKEN",
        required: true,
        remedy: CLAUDE_REMEDY,
    },
    Rule {
        engine: "opencode",
        var: "CTXLOOM_OPENCODE_API_KEY",
        required: false,
        remedy: "",
    },
];

/// One row of `auth status`: whether an engine's credential is exported, the
/// variable it is read from when it is (never its value), and the fix when it
/// is not.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct Row {
    pub engine: &'static str,
    pub mode: &'static str,
    pub present: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub vars: Vec<&'static str>,
    #[serde(skip_serializing_if = "str::is_empty")]
    pub remedy: &'static str,
}

/// Whether one configured engine could authenticate a run.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Verdict {
    pub engine: &'static str,
    pub mode: &'static str,
    pub var: &'static str,
    /// False only where a required credential is not exported.
    pub ready: bool,
    pub remedy: &'static str,
}

/// The use case, over the environment it is launched in.
pub struct Auth {
    env: Arc<dyn ExportedVars + Send + Sync>,
}

impl Auth {
    pub fn new(env: Arc<dyn ExportedVars + Send + Sync>) -> Auth {
        Auth { env }
    }

    /// Per engine that reads a credential from the environment: present with
    /// its variable, or missing with the fix. An optional credential that is
    /// not exported has no row (the run is keyless). Engines in name order.
    pub fn status(&self) -> Vec<Row> {
        RULES
            .iter()
            .filter_map(|rule| {
                let present = self.env.is_exported(rule.var);
                if !present && !rule.required {
                    return None;
                }
                Some(Row {
                    engine: rule.engine,
                    mode: TOKEN,
                    present,
                    vars: if present { vec![rule.var] } else { Vec::new() },
                    remedy: if present { "" } else { rule.remedy },
                })
            })
            .collect()
    }

    /// For each of `engines` that reads a credential from the environment
    /// (each once), whether it could authenticate a run in `mode`. A login
    /// shared in place (`login`) reads nothing from the environment; any
    /// other mode is the default, `token`.
    pub fn needed(&self, engines: &[String], mode: &str) -> Vec<Verdict> {
        if mode == "login" {
            return Vec::new();
        }
        RULES
            .iter()
            .filter(|rule| engines.iter().any(|e| e == rule.engine))
            .map(|rule| Verdict {
                engine: rule.engine,
                mode: TOKEN,
                var: rule.var,
                ready: !rule.required || self.env.is_exported(rule.var),
                remedy: rule.remedy,
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
