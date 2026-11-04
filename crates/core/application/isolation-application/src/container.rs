//! `container check|build|prune|scaffold|tooling`: the images isolated
//! agents run in, and the questions that can be asked without a runtime.
//!
//! Go: `cli/container_cmd.go`, `operations/tooling.go` and
//! `isolation/diagnose.go` (release/0.7). `build` and `prune` act on a live
//! runtime and its images; what is hermetic is their refusals: a backend
//! that is none of the engines, a runtime ctxloom does not know, and a
//! machine with none (exit 3).

use ctxloom_ports::UserFiles;
use isolation_ports::{Backends, Companions, ContainerRuntimes};
use serde::Serialize;

use crate::IsolationError;

/// The instructions `container tooling list` emits above the declarations:
/// Go's `resources/prompts/tooling.md`, byte for byte (the owner's words,
/// D24, revision f36e8dfb21).
pub const TOOLING_INSTRUCTIONS: &str = include_str!("../assets/tooling.md");

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

    /// Diagnose container capability for `backend` (the default when none).
    /// Diagnostic only: no probe outcome fails it; a backend that is none of
    /// the engines does.
    pub fn check(&self, backend: Option<&str>) -> Result<CheckReport, IsolationError> {
        let _ = (&self.runtimes, &self.backends, backend);
        todo!("S11 green")
    }

    /// `container build`: validated before any daemon is touched.
    pub fn build(
        &self,
        backend: Option<&str>,
        runtime: Option<&str>,
    ) -> Result<(), IsolationError> {
        let _ = (&self.runtimes, &self.backends, backend, runtime);
        todo!("S11 green")
    }

    /// `container prune`: a `--runtime` that is none of the runtimes ctxloom
    /// knows is refused, and so is a machine with none (exit 3). With one,
    /// the plan over its images is the live lane's, not ported: this always
    /// answers the refusal.
    pub fn prune(&self, runtime: Option<&str>, yes: bool) -> IsolationError {
        let _ = (&self.runtimes, runtime, yes);
        todo!("S11 green")
    }

    /// Give the project a devcontainer seeded from the embedded base, and
    /// never touch an existing one.
    pub fn scaffold(&self) -> Result<ScaffoldReport, IsolationError> {
        let _ = (&self.files, BASE, DEVCONTAINER_JSON);
        todo!("S11 green")
    }

    /// The registered companions' tooling declarations.
    pub fn tooling(&self) -> Result<ToolingReport, IsolationError> {
        let _ = (&self.companions, TOOLING_INSTRUCTIONS);
        todo!("S11 green")
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
