//! The runtimes on a `PATH`. The runtimes here are scripts that answer (or
//! do not) as a daemon would, so no real runtime is needed or touched
//! (Go: `hermetic_runtimes_test.go`, `runtime_test.go`).

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use isolation_ports::{ContainerRuntimes, RuntimeFacts};

use super::HostRuntimes;

fn script(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn runtimes(dirs: &[&Path]) -> HostRuntimes {
    HostRuntimes::new(
        dirs.iter().map(PathBuf::from).collect(),
        Duration::from_secs(5),
    )
}

// j002200/container.feature "no container runtime is installed": a PATH with
// neither binary has no runtime.
#[test]
fn no_runtime_on_the_path_means_none_installed() {
    let empty = tempfile::tempdir().unwrap();
    assert!(runtimes(&[empty.path()]).installed().is_empty());
    assert!(runtimes(&[]).installed().is_empty());
}

#[test]
fn an_installed_runtime_is_reachable_when_info_answers() {
    let bin = tempfile::tempdir().unwrap();
    script(bin.path(), "docker", "[ \"$1\" = info ] && exit 0; exit 1");
    assert_eq!(
        runtimes(&[bin.path()]).installed(),
        vec![RuntimeFacts {
            name: "docker".to_string(),
            reachable: true
        }]
    );
}

#[test]
fn an_installed_runtime_whose_daemon_is_down_is_listed_unreachable() {
    let bin = tempfile::tempdir().unwrap();
    script(bin.path(), "podman", "exit 1");
    assert_eq!(
        runtimes(&[bin.path()]).installed(),
        vec![RuntimeFacts {
            name: "podman".to_string(),
            reachable: false
        }]
    );
}

// A runtime that never answers is bounded, not waited on.
#[test]
fn a_runtime_that_never_answers_is_unreachable_within_the_bound() {
    let bin = tempfile::tempdir().unwrap();
    script(bin.path(), "docker", "exec sleep 30");
    let slow = HostRuntimes::new(vec![bin.path().to_path_buf()], Duration::from_millis(200));
    let started = std::time::Instant::now();
    let found = slow.installed();
    assert!(started.elapsed() < Duration::from_secs(10));
    assert_eq!(found.len(), 1);
    assert!(!found[0].reachable);
}

// docker is preferred over podman (Go: runtimeCandidates order).
#[test]
fn docker_comes_before_podman() {
    let bin = tempfile::tempdir().unwrap();
    script(bin.path(), "podman", "exit 0");
    script(bin.path(), "docker", "exit 0");
    let names: Vec<String> = runtimes(&[bin.path()])
        .installed()
        .into_iter()
        .map(|r| r.name)
        .collect();
    assert_eq!(names, vec!["docker", "podman"]);
}

// A file that is not executable is not a runtime.
#[test]
fn a_file_that_cannot_be_run_is_not_a_runtime() {
    let bin = tempfile::tempdir().unwrap();
    std::fs::write(bin.path().join("docker"), "not a program").unwrap();
    assert!(runtimes(&[bin.path()]).installed().is_empty());
}

#[test]
fn an_image_is_present_when_the_runtime_inspects_it() {
    let bin = tempfile::tempdir().unwrap();
    script(
        bin.path(),
        "docker",
        "[ \"$1 $2 $3\" = \"image inspect ctxloom-agent-mock\" ] && exit 0; exit 1",
    );
    let found = runtimes(&[bin.path()]);
    assert!(found.image_present("docker", "ctxloom-agent-mock"));
    assert!(!found.image_present("docker", "ctxloom-agent-other"));
    assert!(!found.image_present("podman", "ctxloom-agent-mock"));
}
