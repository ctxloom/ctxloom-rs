//! `manage`: scaffolding the project, taking the harness back out, and
//! reporting what is wired (Go's `internal/adapters/cli/manage.go`).
//!
//! plan supersedes Go (8.2): `manage uninstall` is `materialize --release` at
//! the project's directory for every registered engine; only what the
//! ownership record says ctxloom wrote is removed.

use ctxloom_ports::Put;
use library::model::HOOK_EVENTS;
use library_application::{Flags, PackageRequest};
use serde::Serialize;
use surface_ports::Kind;

use crate::{Done, Setup, SetupError};

/// The comment at the head of `.ctxloom/.gitignore` (Go's
/// `gitignore.NestedComment`).
const NESTED_COMMENT: &str = "# ctxloom's private working state: rebuildable or purely local, and so must\n# never ride a distributable tree. Everything else under .ctxloom/ - config.yaml,\n# remotes.yaml, lock.yaml, content/ - is committed by omission, because it is\n# content or config the project depends on.\n#\n# GENERATED from gitignore.PrivateStatePatterns, relativized to this directory.\n# ctxloom REWRITES THIS FILE WHOLESALE; hand edits are lost. Change that list.\n#\n# COMMIT THIS FILE. Tracking it is what carries the rules with the directory -\n# into every clone and, crucially, into every linked worktree, which is where a\n# rule living only in the superproject's root .gitignore would not reach.";

/// Go's `gitignore.NestedPatterns()`: ctxloom's private state, relative to
/// `.ctxloom/`.
const NESTED_PATTERNS: [&str; 5] = ["/cache", "/sessions", "/project-id", "/state", "*.lock"];

const NESTED_PATH: &str = ".ctxloom/.gitignore";

fn nested_content() -> String {
    let mut out = String::from(NESTED_COMMENT);
    out.push('\n');
    for pattern in NESTED_PATTERNS {
        out.push_str(pattern);
        out.push('\n');
    }
    out
}

/// `manage install`.
#[derive(Clone, Debug, Default)]
pub struct InstallRequest {
    /// The engine to record when scaffolding; none is the default.
    pub engine: Option<String>,
    pub no_pull: bool,
    pub flags: Flags,
}

/// What the install's dependency pull did.
#[derive(Clone, Debug, Serialize)]
pub struct PullReport {
    /// `pulled`, `failed` or `skipped`.
    pub status: String,
    pub installed: u64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub error: String,
}

/// What `manage install --print` says it would do.
#[derive(Clone, Debug, Serialize)]
pub struct InstallPlan {
    pub ctxloom_dir: String,
    pub ctxloom_dir_exists: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub engine: String,
    pub gitignore: String,
}

/// What `manage install` did.
#[derive(Clone, Debug, Serialize)]
pub struct InstallReport {
    pub app_dir: String,
    pub initialized: bool,
    pub gitignore: String,
    pub gitignore_status: String,
    pub pull: PullReport,
}

/// What `manage gitignore install` did.
#[derive(Clone, Debug, Serialize)]
pub struct GitignoreReport {
    pub status: String,
    pub path: String,
    pub nested_path: String,
    pub nested_written: bool,
}

/// What `manage uninstall` did.
#[derive(Clone, Debug, Serialize)]
pub struct UninstallReport {
    pub status: String,
    pub backends: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
    pub note: String,
}

/// One engine's wiring.
#[derive(Clone, Debug, Serialize)]
pub struct BackendStatus {
    pub backend: String,
    pub settings_exists: bool,
    pub hooks_present: bool,
    #[serde(rename = "statusline")]
    pub status_line: bool,
    pub mcp_present: bool,
}

/// A kind ctxloom has delivered, and whether the file still shows it.
#[derive(Clone, Debug, Serialize)]
pub struct SurfaceCurrency {
    pub route: String,
    pub backend: String,
    pub status: String,
}

/// What `manage check` shows.
#[derive(Clone, Debug, Serialize)]
pub struct CheckReport {
    pub work_dir: String,
    pub manage_statusline: bool,
    pub backends: Vec<BackendStatus>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub surfaces: Vec<SurfaceCurrency>,
}

/// One hook of `manage hooks list`.
#[derive(Clone, Debug, Serialize)]
pub struct HookRow {
    pub position: usize,
    /// Where the merge put it before any reorder.
    pub declared: usize,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub command: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub prompt: String,
    #[serde(rename = "type", skip_serializing_if = "String::is_empty")]
    pub kind: String,
    pub source_kind: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source: String,
}

/// One event of `manage hooks list`.
#[derive(Clone, Debug, Serialize)]
pub struct HooksEvent {
    pub event: String,
    pub hooks: Vec<HookRow>,
}

/// What `manage hooks list` shows.
#[derive(Clone, Debug, Serialize)]
pub struct HooksList {
    pub events: Vec<HooksEvent>,
}

impl Setup {
    /// Scaffold `.ctxloom` (if absent), git-ignore ctxloom's private state,
    /// and pull the dependencies the profiles reference. No engine file is
    /// written: a session carries its own, and an explicit materialize writes
    /// the project's.
    pub fn install(&self, request: &InstallRequest) -> Result<Done<InstallReport>, SetupError> {
        let known = self.engines.names();
        if let Some(engine) = &request.engine {
            if !known.contains(engine) {
                return Err(SetupError::UnknownEngine {
                    engine: engine.clone(),
                    known: known.join(", "),
                });
            }
        }
        let exists = self
            .scaffold
            .exists("config.yaml")
            .map_err(SetupError::Cause)?;
        if exists {
            if let Some(engine) = &request.engine {
                return Err(SetupError::EngineNotApplied {
                    engine: engine.clone(),
                });
            }
        }
        let mut notices = Vec::new();
        if !exists {
            let engine = request.engine.clone().unwrap_or_default();
            let created = self.library.config_create(&engine);
            notices.extend(created.notices);
            created.result.map_err(SetupError::cause)?;
        }
        let (status, written) = self.write_nested_gitignore()?;
        let pull = self.pull(request);
        Ok(Done {
            value: InstallReport {
                app_dir: self.scaffold.app_dir(),
                initialized: !exists,
                gitignore: format!("{}/.gitignore", self.project_dir),
                gitignore_status: if written {
                    status
                } else {
                    "unchanged".to_string()
                },
                pull,
            },
            notices,
        })
    }

    /// The steps `manage install` would take, without taking them.
    pub fn install_plan(&self, engine: Option<&str>) -> Result<InstallPlan, SetupError> {
        let exists = self
            .scaffold
            .exists("config.yaml")
            .map_err(SetupError::Cause)?;
        Ok(InstallPlan {
            ctxloom_dir: self.scaffold.app_dir(),
            ctxloom_dir_exists: exists,
            engine: if exists {
                String::new()
            } else {
                engine.unwrap_or("claude-code").to_string()
            },
            gitignore: format!("{}/.gitignore", self.project_dir),
        })
    }

    fn pull(&self, request: &InstallRequest) -> PullReport {
        let report = |status: &str, installed: u64, error: String| PullReport {
            status: status.to_string(),
            installed,
            error,
        };
        match (&self.puller, request.no_pull) {
            (_, true) | (None, _) => report("skipped", 0, String::new()),
            (Some(puller), false) => match puller.pull() {
                Ok(installed) => report("pulled", installed, String::new()),
                Err(error) => report("failed", 0, error),
            },
        }
    }

    /// `.ctxloom/.gitignore` written wholesale when it is not what ctxloom
    /// would write: whether it was.
    fn write_nested_gitignore(&self) -> Result<(String, bool), SetupError> {
        let want = nested_content();
        let held = self
            .store
            .read(&self.project_dir, NESTED_PATH)
            .map_err(SetupError::cause)?;
        if held.as_deref() == Some(want.as_bytes()) {
            return Ok(("unchanged".to_string(), false));
        }
        self.scaffold
            .write(&[Put::file(".gitignore", want)])
            .map_err(SetupError::Cause)?;
        Ok(("updated".to_string(), true))
    }

    /// `manage gitignore install`.
    pub fn gitignore_install(&self) -> Result<GitignoreReport, SetupError> {
        let (status, written) = self.write_nested_gitignore()?;
        Ok(GitignoreReport {
            status,
            path: format!("{}/.gitignore", self.project_dir),
            nested_path: format!("{}/{NESTED_PATH}", self.project_dir),
            nested_written: written,
        })
    }

    /// `manage uninstall`: everything an at-rest delivery wrote into the
    /// project's directory, for every engine, and nothing else.
    pub fn uninstall(&self) -> Result<UninstallReport, SetupError> {
        let mut backends = Vec::new();
        let mut errors = Vec::new();
        for name in self.engines.names() {
            let mut failed = false;
            for kind in Kind::ALL {
                if let Err(e) = self.store.release(&self.project_dir, &name, kind, false) {
                    failed = true;
                    errors.push(format!(
                        "{name}: {}",
                        angzarr_diagnostic::Diagnostic::code(&e)
                    ));
                }
            }
            if !failed {
                backends.push(name);
            }
        }
        Ok(UninstallReport {
            status: if errors.is_empty() {
                "released"
            } else {
                "partial"
            }
            .to_string(),
            backends,
            errors,
            note: "The .ctxloom directory and its contents were left in place.".to_string(),
        })
    }

    /// `manage check`: what is wired, read off the engines' own files.
    pub fn check(&self, flags: &Flags) -> Result<Done<CheckReport>, SetupError> {
        let outcome = self.library.package(flags, &PackageRequest::default());
        let statusline = outcome.result.map(|p| p.statusline).unwrap_or(true);
        let mut backends = Vec::new();
        let mut surfaces = Vec::new();
        for name in self.engines.names() {
            let Some(engine) = self.engines.get(&name) else {
                continue;
            };
            let wiring = engine
                .wiring(self.store.as_ref(), &self.project_dir)
                .map_err(SetupError::cause)?;
            backends.push(BackendStatus {
                backend: name.clone(),
                settings_exists: wiring.settings_exists,
                hooks_present: wiring.hooks_present,
                status_line: wiring.statusline,
                mcp_present: wiring.mcp_present,
            });
            for kind in self
                .store
                .claimed(&self.project_dir, &name)
                .map_err(SetupError::cause)?
            {
                surfaces.push(SurfaceCurrency {
                    route: kind.name().to_string(),
                    backend: name.clone(),
                    status: "materialized".to_string(),
                });
            }
        }
        Ok(Done {
            value: CheckReport {
                work_dir: self.project_dir.clone(),
                manage_statusline: statusline,
                backends,
                surfaces,
            },
            notices: outcome.notices,
        })
    }

    /// `manage hooks list`: the hooks that will fire, per event, in the order
    /// they fire in, with where each came from.
    pub fn hooks_list(
        &self,
        event: Option<&str>,
        profiles: &[String],
        flags: &Flags,
    ) -> Result<Done<HooksList>, SetupError> {
        if let Some(event) = event.filter(|e| !HOOK_EVENTS.contains(e)) {
            return Err(SetupError::UnknownEvent {
                event: event.to_string(),
                known: HOOK_EVENTS.join(", "),
            });
        }
        let outcome = self.library.package(
            flags,
            &PackageRequest {
                profiles: profiles.to_vec(),
            },
        );
        let package = outcome.result.map_err(SetupError::cause)?;
        let events = HOOK_EVENTS
            .iter()
            .filter(|e| event.is_none_or(|wanted| wanted == **e))
            .map(|name| HooksEvent {
                event: (*name).to_string(),
                hooks: package
                    .hooks
                    .iter()
                    .filter(|h| h.event == *name)
                    .map(|h| HookRow {
                        position: h.position,
                        declared: h.position,
                        command: h.hook.command.clone(),
                        args: h.hook.args.clone(),
                        prompt: h.hook.prompt.clone(),
                        kind: h.hook.kind.clone(),
                        source_kind: h.source_kind.clone(),
                        source: h.source.clone(),
                    })
                    .collect(),
            })
            .collect();
        Ok(Done {
            value: HooksList { events },
            notices: outcome.notices,
        })
    }

    /// `manage statusline install|uninstall`: whether ctxloom manages the HUD
    /// statusline; it takes effect at the next materialize or run.
    pub fn set_statusline(&self, enabled: bool, flags: &Flags) -> Result<Done<bool>, SetupError> {
        let outcome = self.library.set_statusline(flags, enabled);
        outcome.result.map_err(SetupError::cause)?;
        Ok(Done {
            value: enabled,
            notices: outcome.notices,
        })
    }
}

#[cfg(test)]
#[path = "manage.test.rs"]
mod tests;
