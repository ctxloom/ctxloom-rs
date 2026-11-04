//! `container check|build|prune|scaffold|tooling`: the images isolated
//! agents run in, and the questions that can be asked without a runtime.
//!
//! Go: `cli/container_cmd.go`, `operations/tooling.go` and
//! `isolation/diagnose.go` (release/0.7). `build` and `prune` act on a live
//! runtime and its images; what is hermetic is their refusals: a backend
//! that is none of the engines, a runtime ctxloom does not know, and a
//! machine with none (exit 3).

use ctxloom_ports::UserFiles;
use isolation_ports::{Backends, Companions, ContainerRuntimes, RuntimeFacts, RUNTIMES};
use serde::Serialize;

use crate::IsolationError;

/// The instructions `container tooling list` emits above the declarations:
/// Go's `resources/prompts/tooling.md`, byte for byte (the owner's words,
/// D24, revision f36e8dfb21).
pub const TOOLING_INSTRUCTIONS: &str = include_str!("../assets/tooling.md");

/// The project's devcontainer directory.
const DEVCONTAINER: &str = ".devcontainer";

/// The embedded default base the scaffold seeds the devcontainer's Dockerfile
/// from: Go's `container/base/Containerfile`, byte for byte.
pub(crate) const BASE: &str = include_str!("../assets/base.Containerfile");

/// The `.devcontainer/devcontainer.json` a scaffold writes: build the
/// Dockerfile beside it, nothing else.
pub(crate) const DEVCONTAINER_JSON: &str =
    "{\n  \"build\": {\n    \"dockerfile\": \"Dockerfile\"\n  }\n}\n";

/// The container-capability report (`container check`).
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct CheckReport {
    /// The backend the report is about; `(unresolved)` when none could be
    /// resolved. Shown in the text report's heading, not in json.
    #[serde(skip)]
    pub backend: String,
    pub in_container: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<String>,
    /// `docker`, `podman` or `none`.
    pub runtime: String,
    pub reachable: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub image: String,
    pub image_present: bool,
    /// `ok`, `mismatch: ...` or `unprobed: ...`.
    pub shared_fs: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub guidance: Vec<String>,
}

/// What a scaffold wrote.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ScaffoldReport {
    /// The `.devcontainer` directory.
    pub dir: String,
}

/// One companion's declaration.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct Declaration {
    pub source: String,
    pub content: String,
}

/// The tooling the registered companions declare, with the instructions for
/// the LLM that folds it in.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct ToolingReport {
    pub instructions: String,
    pub declarations: Vec<Declaration>,
}

/// The container use cases over the machine's runtimes, the engines, the
/// registered companions and the project's files.
pub struct Containers<R, B, C, U> {
    runtimes: R,
    backends: B,
    companions: C,
    files: U,
}

impl<R: ContainerRuntimes, B: Backends, C: Companions, U: UserFiles> Containers<R, B, C, U> {
    pub fn new(runtimes: R, backends: B, companions: C, files: U) -> Containers<R, B, C, U> {
        Containers {
            runtimes,
            backends,
            companions,
            files,
        }
    }

    /// The backend named, validated against the engines; or the default.
    fn backend(&self, named: Option<&str>) -> Result<Option<String>, IsolationError> {
        let Some(named) = named else {
            return Ok(self.backends.default_backend());
        };
        let mut names = self.backends.names();
        if names.iter().any(|n| n == named) {
            return Ok(Some(named.to_string()));
        }
        names.sort();
        Err(IsolationError::UnknownBackend {
            backend: named.to_string(),
            available: names.join(", "),
        })
    }

    /// The runtimes that answer, narrowed by a `--runtime` that must be one
    /// ctxloom knows.
    fn reachable(&self, runtime: Option<&str>) -> Result<Vec<RuntimeFacts>, IsolationError> {
        if let Some(runtime) = runtime {
            if !RUNTIMES.contains(&runtime) {
                return Err(IsolationError::UnknownRuntime {
                    runtime: runtime.to_string(),
                    known: RUNTIMES.join("|"),
                });
            }
        }
        Ok(self
            .runtimes
            .installed()
            .into_iter()
            .filter(|r| r.reachable && runtime.is_none_or(|only| only == r.name))
            .collect())
    }

    /// Diagnose container capability for `backend` (the default when none).
    /// Diagnostic only: no probe outcome fails it; a backend that is none of
    /// the engines does.
    pub fn check(&self, backend: Option<&str>) -> Result<CheckReport, IsolationError> {
        let backend = self.backend(backend)?;
        let markers = self.runtimes.in_container();
        let installed = self.runtimes.installed();
        let chosen = installed
            .iter()
            .find(|r| r.reachable)
            .or_else(|| installed.first());
        let mut report = CheckReport {
            backend: backend
                .clone()
                .unwrap_or_else(|| "(unresolved)".to_string()),
            in_container: !markers.is_empty(),
            markers,
            runtime: "none".to_string(),
            reachable: false,
            image: String::new(),
            image_present: false,
            shared_fs: "unprobed: no runtime".to_string(),
            guidance: Vec::new(),
        };
        let Some(runtime) = chosen else {
            report.guidance.push(
                "no container runtime is reachable: `runtime: container-rootless` / `runtime: container-rootful` agents abort startup (exit 3), even under --degraded - install docker or podman and start it, or declare `runtime: host` on an agent that should run unsandboxed"
                    .to_string(),
            );
            return Ok(report);
        };
        report.runtime = runtime.name.clone();
        report.reachable = runtime.reachable;
        if !runtime.reachable {
            report.shared_fs = format!("unprobed: the {} daemon did not answer", runtime.name);
            report.guidance.push(format!(
                "{} is installed but its daemon did not answer: start it, or `runtime: container-rootless` / `runtime: container-rootful` agents abort startup (exit 3), even under --degraded",
                runtime.name
            ));
            return Ok(report);
        }
        if let Some(backend) = backend {
            report.image = format!("ctxloom-agent-{backend}");
            report.image_present = self.runtimes.image_present(&runtime.name, &report.image);
            if !report.image_present {
                report.guidance.push(format!(
                    "agent image {} is not present; a containerized run builds it on first use (or run `ctxloom container build {backend}`)",
                    report.image
                ));
            }
        }
        report.shared_fs =
            "unprobed: the shared-filesystem marker probe runs in the live lane, not here"
                .to_string();
        Ok(report)
    }

    /// `container build`: validated before any daemon is touched.
    pub fn build(
        &self,
        backend: Option<&str>,
        runtime: Option<&str>,
    ) -> Result<(), IsolationError> {
        self.backend(backend)?;
        if self.reachable(runtime)?.is_empty() {
            return Err(IsolationError::NoBuildRuntime);
        }
        Err(IsolationError::NotPorted {
            command: "container build".to_string(),
        })
    }

    /// `container prune`: a `--runtime` that is none of the runtimes ctxloom
    /// knows is refused, and so is a machine with none (exit 3). With one,
    /// the plan over its images is the live lane's, not ported: this always
    /// answers the refusal.
    pub fn prune(&self, runtime: Option<&str>, yes: bool) -> IsolationError {
        let _ = yes;
        match self.reachable(runtime) {
            Err(refusal) => refusal,
            Ok(available) if available.is_empty() => IsolationError::NoPruneRuntime,
            Ok(_) => IsolationError::NotPorted {
                command: "container prune".to_string(),
            },
        }
    }

    /// Give the project a devcontainer seeded from the embedded base, and
    /// never touch an existing one.
    pub fn scaffold(&self) -> Result<ScaffoldReport, IsolationError> {
        let adapter = IsolationError::Adapter;
        for existing in [DEVCONTAINER, ".devcontainer.json"] {
            if self.files.exists(existing).map_err(adapter)? {
                return Err(IsolationError::DevcontainerExists {
                    path: existing.to_string(),
                });
            }
        }
        for (rel, text) in [
            ("Dockerfile", BASE),
            ("devcontainer.json", DEVCONTAINER_JSON),
        ] {
            let path = format!("{DEVCONTAINER}/{rel}");
            // Nothing is replaced: a file that appeared meanwhile is the
            // human's.
            if !self
                .files
                .write_file(&path, text.as_bytes(), false)
                .map_err(IsolationError::Adapter)?
            {
                return Err(IsolationError::DevcontainerExists { path });
            }
        }
        Ok(ScaffoldReport {
            dir: DEVCONTAINER.to_string(),
        })
    }

    /// The registered companions' tooling declarations.
    pub fn tooling(&self) -> Result<ToolingReport, IsolationError> {
        let declarations = self
            .companions
            .tooling()
            .map_err(IsolationError::Adapter)?
            .into_iter()
            .map(|t| Declaration {
                source: t.source,
                content: t.content,
            })
            .collect();
        Ok(ToolingReport {
            instructions: TOOLING_INSTRUCTIONS.to_string(),
            declarations,
        })
    }
}

/// What the `container` commands call.
pub trait ContainerUse {
    fn check(&self, backend: Option<&str>) -> Result<CheckReport, IsolationError>;
    fn build(&self, backend: Option<&str>, runtime: Option<&str>) -> Result<(), IsolationError>;
    fn prune(&self, runtime: Option<&str>, yes: bool) -> IsolationError;
    fn scaffold(&self) -> Result<ScaffoldReport, IsolationError>;
    fn tooling(&self) -> Result<ToolingReport, IsolationError>;
}

impl<R: ContainerRuntimes, B: Backends, C: Companions, U: UserFiles> ContainerUse
    for Containers<R, B, C, U>
{
    fn check(&self, backend: Option<&str>) -> Result<CheckReport, IsolationError> {
        Containers::check(self, backend)
    }

    fn build(&self, backend: Option<&str>, runtime: Option<&str>) -> Result<(), IsolationError> {
        Containers::build(self, backend, runtime)
    }

    fn prune(&self, runtime: Option<&str>, yes: bool) -> IsolationError {
        Containers::prune(self, runtime, yes)
    }

    fn scaffold(&self) -> Result<ScaffoldReport, IsolationError> {
        Containers::scaffold(self)
    }

    fn tooling(&self) -> Result<ToolingReport, IsolationError> {
        Containers::tooling(self)
    }
}
