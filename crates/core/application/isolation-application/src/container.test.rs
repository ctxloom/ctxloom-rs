//! The container use cases. Ported from Go's cli/container.feature rules
//! (release/0.7, f36e8dfb21): check, build's validation, prune's two
//! refusals, scaffold, and tooling collection.
//!
//! plan supersedes Go: the image is named `ctxloom-agent-<backend>` (no
//! content address: the image build is the live lane's, plan 5.6), and prune
//! and build with a runtime present refuse as not ported rather than act.

use std::cell::RefCell;
use std::collections::BTreeMap;

use ctxloom_ports::{Failure, Put, UserFiles};
use isolation_ports::{
    Backends, Companions, ContainerRuntimes, Failure as IsolationFailure, RuntimeFacts, Tooling,
};

use super::container::{BASE, DEVCONTAINER_JSON};
use super::{Containers, IsolationError, TOOLING_INSTRUCTIONS};

struct Machine {
    markers: Vec<String>,
    runtimes: Vec<RuntimeFacts>,
    images: Vec<(String, String)>,
}

impl ContainerRuntimes for Machine {
    fn in_container(&self) -> Vec<String> {
        self.markers.clone()
    }

    fn installed(&self) -> Vec<RuntimeFacts> {
        self.runtimes.clone()
    }

    fn image_present(&self, runtime: &str, image: &str) -> bool {
        self.images.iter().any(|(r, i)| r == runtime && i == image)
    }
}

struct Engines {
    names: Vec<&'static str>,
    default: Option<&'static str>,
}

impl Backends for Engines {
    fn names(&self) -> Vec<String> {
        self.names.iter().map(ToString::to_string).collect()
    }

    fn default_backend(&self) -> Option<String> {
        self.default.map(str::to_string)
    }
}

struct Registered(Vec<Tooling>);

impl Companions for Registered {
    fn tooling(&self) -> Result<Vec<Tooling>, IsolationFailure> {
        Ok(self.0.clone())
    }
}

/// A project's files, in memory.
#[derive(Default)]
struct Files(RefCell<BTreeMap<String, Vec<u8>>>);

impl Files {
    fn with(paths: &[&str]) -> Files {
        let files = Files::default();
        for path in paths {
            files
                .0
                .borrow_mut()
                .insert(path.to_string(), b"mine".to_vec());
        }
        files
    }
}

impl UserFiles for &Files {
    fn exists(&self, path: &str) -> Result<bool, Failure> {
        let files = self.0.borrow();
        Ok(files.contains_key(path) || files.keys().any(|k| k.starts_with(&format!("{path}/"))))
    }

    fn is_dir(&self, path: &str) -> Result<bool, Failure> {
        Ok(self
            .0
            .borrow()
            .keys()
            .any(|k| k.starts_with(&format!("{path}/"))))
    }

    fn read_file(&self, path: &str) -> Result<Vec<u8>, Failure> {
        Ok(self.0.borrow().get(path).cloned().unwrap_or_default())
    }

    fn read_tree(&self, _dir: &str) -> Result<Vec<Put>, Failure> {
        Ok(Vec::new())
    }

    fn write_file(&self, path: &str, bytes: &[u8], overwrite: bool) -> Result<bool, Failure> {
        let mut files = self.0.borrow_mut();
        if !overwrite && files.contains_key(path) {
            return Ok(false);
        }
        files.insert(path.to_string(), bytes.to_vec());
        Ok(true)
    }

    fn write_tree(&self, _dir: &str, _files: &[Put]) -> Result<(), Failure> {
        Ok(())
    }
}

fn no_machine() -> Machine {
    Machine {
        markers: Vec::new(),
        runtimes: Vec::new(),
        images: Vec::new(),
    }
}

fn engines() -> Engines {
    Engines {
        names: vec!["claude-code", "mock"],
        default: Some("claude-code"),
    }
}

fn containers(
    machine: Machine,
    engines: Engines,
    files: &Files,
) -> Containers<Machine, Engines, Registered, &Files> {
    Containers::new(machine, engines, Registered(Vec::new()), files)
}

// "An unknown backend is a usage error, and the real ones are named".
#[test]
fn check_of_an_unknown_backend_is_refused_naming_the_real_ones() {
    let files = Files::default();
    let error = containers(no_machine(), engines(), &files)
        .check(Some("totally-bogus-engine"))
        .unwrap_err();
    assert!(matches!(
        &error,
        IsolationError::UnknownBackend { backend, available }
            if backend == "totally-bogus-engine" && available.contains("claude-code")
    ));
    assert_eq!(error.exit_code(), 2);
}

// "The text report labels every axis": with no runtime every axis still
// answers, and no probe outcome fails the command.
#[test]
fn check_with_no_runtime_still_answers_every_axis() {
    let files = Files::default();
    let report = containers(no_machine(), engines(), &files)
        .check(Some("mock"))
        .unwrap();
    assert_eq!(report.backend, "mock");
    assert!(!report.in_container);
    assert_eq!(report.runtime, "none");
    assert!(!report.reachable);
    assert!(
        report.shared_fs.starts_with("unprobed"),
        "{}",
        report.shared_fs
    );
    assert!(
        report
            .guidance
            .iter()
            .any(|g| g.contains("no container runtime")),
        "{:?}",
        report.guidance
    );
}

// "With no backend named, the text report still names one": the engine
// default; "(unresolved)" only when there is none at all.
#[test]
fn check_with_no_backend_resolves_the_default() {
    let files = Files::default();
    let report = containers(no_machine(), engines(), &files)
        .check(None)
        .unwrap();
    assert_eq!(report.backend, "claude-code");
    let none = Engines {
        names: Vec::new(),
        default: None,
    };
    let report = containers(no_machine(), none, &files).check(None).unwrap();
    assert_eq!(report.backend, "(unresolved)");
}

// "The capability check is diagnostic-only": the markers, the reachable
// runtime and the agent image's presence are reported as found.
#[test]
fn check_reports_the_runtime_the_markers_and_the_image() {
    let files = Files::default();
    let machine = Machine {
        markers: vec!["/.dockerenv".to_string()],
        runtimes: vec![RuntimeFacts {
            name: "podman".to_string(),
            reachable: true,
        }],
        images: vec![("podman".to_string(), "ctxloom-agent-mock".to_string())],
    };
    let report = containers(machine, engines(), &files)
        .check(Some("mock"))
        .unwrap();
    assert!(report.in_container);
    assert_eq!(report.markers, vec!["/.dockerenv".to_string()]);
    assert_eq!(report.runtime, "podman");
    assert!(report.reachable);
    assert_eq!(report.image, "ctxloom-agent-mock");
    assert!(report.image_present);
}

// "Build validates its backend before touching any daemon".
#[test]
fn build_of_an_unknown_backend_is_refused_before_any_daemon_is_touched() {
    let files = Files::default();
    let error = containers(no_machine(), engines(), &files)
        .build(Some("totally-bogus-engine"), None)
        .unwrap_err();
    assert!(matches!(error, IsolationError::UnknownBackend { .. }));
}

// prune: "A --runtime ctxloom does not know is refused, naming the ones it
// does".
#[test]
fn prune_of_an_unknown_runtime_is_refused_naming_the_known_ones() {
    let files = Files::default();
    let error = containers(no_machine(), engines(), &files).prune(Some("rkt"), false);
    assert!(matches!(
        &error,
        IsolationError::UnknownRuntime { runtime, known }
            if runtime == "rkt" && known == "docker|podman"
    ));
    assert_eq!(error.exit_code(), 2);
}

// prune: "With no container runtime on this machine, prune exits 3 rather
// than reporting a clean sweep".
#[test]
fn prune_with_no_runtime_exits_three() {
    let files = Files::default();
    for yes in [false, true] {
        let error = containers(no_machine(), engines(), &files).prune(None, yes);
        assert!(matches!(error, IsolationError::NoPruneRuntime));
        assert_eq!(error.exit_code(), 3);
    }
}

// scaffold: "Scaffolding writes a devcontainer seeded from the default base".
#[test]
fn scaffold_writes_a_devcontainer_seeded_from_the_default_base() {
    let files = Files::default();
    let report = containers(no_machine(), engines(), &files)
        .scaffold()
        .unwrap();
    assert_eq!(report.dir, ".devcontainer");
    let written = files.0.borrow();
    assert_eq!(
        written.get(".devcontainer/Dockerfile").map(Vec::as_slice),
        Some(BASE.as_bytes())
    );
    assert_eq!(
        written
            .get(".devcontainer/devcontainer.json")
            .map(Vec::as_slice),
        Some(DEVCONTAINER_JSON.as_bytes())
    );
    assert!(BASE.contains("FROM"));
}

// scaffold: "An existing devcontainer is refused and left untouched" and "A
// root .devcontainer.json is a devcontainer too".
#[test]
fn scaffold_refuses_an_existing_devcontainer_and_writes_nothing() {
    for existing in [".devcontainer/Dockerfile", ".devcontainer.json"] {
        let files = Files::with(&[existing]);
        let error = containers(no_machine(), engines(), &files)
            .scaffold()
            .unwrap_err();
        assert!(
            matches!(&error, IsolationError::DevcontainerExists { path } if path.contains(".devcontainer")),
            "{error:?}"
        );
        assert_eq!(
            files.0.borrow().len(),
            1,
            "nothing was written beside {existing}"
        );
        assert_eq!(
            files.0.borrow()[existing],
            b"mine".to_vec(),
            "{existing} survives"
        );
    }
}

// tooling: the instructions are the owner's words, with the registered
// companions' declarations beside them.
#[test]
fn tooling_carries_the_instructions_and_the_declarations() {
    let files = Files::default();
    let registered = Registered(vec![Tooling {
        source: "tooled".to_string(),
        content: "TOOLING-DECL-TOOLED".to_string(),
    }]);
    let report = Containers::new(no_machine(), engines(), registered, &files)
        .tooling()
        .unwrap();
    assert_eq!(report.instructions, TOOLING_INSTRUCTIONS);
    assert_eq!(report.declarations.len(), 1);
    assert_eq!(report.declarations[0].source, "tooled");
    assert_eq!(report.declarations[0].content, "TOOLING-DECL-TOOLED");
}

#[test]
fn tooling_with_nothing_declared_has_no_declarations() {
    let files = Files::default();
    let report = containers(no_machine(), engines(), &files)
        .tooling()
        .unwrap();
    assert!(report.declarations.is_empty());
    assert!(!report.instructions.is_empty());
}

fn podman() -> Machine {
    Machine {
        markers: Vec::new(),
        runtimes: vec![RuntimeFacts {
            name: "podman".to_string(),
            reachable: true,
        }],
        images: Vec::new(),
    }
}

// Build with no runtime is refused as prune is: exit 3, not a clean build.
#[test]
fn build_with_no_runtime_exits_three() {
    let files = Files::default();
    let error = containers(no_machine(), engines(), &files)
        .build(Some("mock"), None)
        .unwrap_err();
    assert!(matches!(error, IsolationError::NoBuildRuntime));
    assert_eq!(error.exit_code(), 3);
}

// plan 5.6: the image build and the prune plan are the live lane's. With a
// runtime they refuse as not ported, never as a success that did nothing.
#[test]
fn build_and_prune_with_a_runtime_refuse_as_not_ported() {
    let files = Files::default();
    let built = containers(podman(), engines(), &files)
        .build(Some("mock"), None)
        .unwrap_err();
    assert!(matches!(built, IsolationError::NotPorted { .. }));
    let pruned = containers(podman(), engines(), &files).prune(None, true);
    assert!(matches!(pruned, IsolationError::NotPorted { .. }));
    assert_eq!(pruned.exit_code(), 1);
}

fn machine_of(runtimes: &[(&str, bool)]) -> Machine {
    Machine {
        markers: Vec::new(),
        runtimes: runtimes
            .iter()
            .map(|(name, reachable)| RuntimeFacts {
                name: name.to_string(),
                reachable: *reachable,
            })
            .collect(),
        images: Vec::new(),
    }
}

// Only a runtime that answers can build, and `--runtime` narrows to that one.
#[test]
fn build_considers_only_reachable_runtimes_narrowed_by_name() {
    let files = Files::default();
    let build = |runtimes: &[(&str, bool)], only: Option<&str>| {
        containers(machine_of(runtimes), engines(), &files)
            .build(Some("mock"), only)
            .unwrap_err()
    };
    let down = [("docker", false)];
    assert!(matches!(build(&down, None), IsolationError::NoBuildRuntime));
    let mixed = [("docker", false), ("podman", true)];
    assert!(matches!(
        build(&mixed, Some("docker")),
        IsolationError::NoBuildRuntime
    ));
    assert!(matches!(
        build(&mixed, Some("podman")),
        IsolationError::NotPorted { .. }
    ));
    assert!(matches!(
        build(&mixed, None),
        IsolationError::NotPorted { .. }
    ));
}

// The check names an installed runtime whose daemon is down, and probes nothing.
#[test]
fn check_of_a_runtime_whose_daemon_is_down_says_so_and_probes_no_image() {
    let files = Files::default();
    let report = containers(machine_of(&[("docker", false)]), engines(), &files)
        .check(Some("mock"))
        .unwrap();
    assert_eq!(report.runtime, "docker");
    assert!(!report.reachable);
    assert_eq!(
        report.shared_fs,
        "unprobed: the docker daemon did not answer"
    );
    assert!(report.image.is_empty());
    assert!(!report.image_present);
    assert_eq!(report.guidance.len(), 1);
}

// The use-case trait delegates to the real operations.
#[test]
fn the_container_use_trait_delegates() {
    use super::ContainerUse;
    let files = Files::default();
    let c = containers(podman(), engines(), &files);
    assert!(matches!(
        ContainerUse::build(&c, Some("mock"), None),
        Err(IsolationError::NotPorted { .. })
    ));
}
