//! The runtimes on a `PATH`. The runtimes here are scripts that answer (or
//! do not) as a daemon would, so no real runtime is needed or touched
//! (Go: `hermetic_runtimes_test.go`, `runtime_test.go`).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use file_system::{Fs, Perms, Rooted};
use isolation_ports::{ContainerRuntimes, RuntimeFacts};
use os::Mode;
use process_env::EnvSnapshot;

use super::HostRuntimes;

/// A filesystem of the test's own, and a `bin` directory in it.
fn machine() -> (Arc<Rooted>, PathBuf) {
    let fs = Arc::new(Rooted::temp().unwrap());
    let bin = os::fixture("/bin");
    fs.create_dir_all(&bin, Mode::DIR).unwrap();
    (fs, bin)
}

fn script(fs: &Rooted, dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    let text = format!("#!/bin/sh\n{body}\n");
    fs.write_atomic(&path, text.as_bytes(), Perms::Exact(Mode::from_bits(0o755)))
        .unwrap();
}

/// The snapshot the runtimes' markers are read from: none of its own.
fn env() -> Arc<EnvSnapshot> {
    Arc::new(EnvSnapshot::of(Vec::<(&str, &str)>::new()))
}

fn runtimes(fs: &Arc<Rooted>, dirs: &[&Path]) -> HostRuntimes {
    HostRuntimes::new(
        fs.clone(),
        env(),
        dirs.iter().map(PathBuf::from).collect(),
        Duration::from_secs(5),
    )
}

// j002200/container.feature "no container runtime is installed": a PATH with
// neither binary has no runtime.
#[test]
fn no_runtime_on_the_path_means_none_installed() {
    let (fs, empty) = machine();
    assert!(runtimes(&fs, &[&empty]).installed().is_empty());
    assert!(runtimes(&fs, &[]).installed().is_empty());
}

#[test]
fn an_installed_runtime_is_reachable_when_info_answers() {
    let (fs, bin) = machine();
    script(&fs, &bin, "docker", "[ \"$1\" = info ] && exit 0; exit 1");
    assert_eq!(
        runtimes(&fs, &[&bin]).installed(),
        vec![RuntimeFacts {
            name: "docker".to_string(),
            reachable: true
        }]
    );
}

#[test]
fn an_installed_runtime_whose_daemon_is_down_is_listed_unreachable() {
    let (fs, bin) = machine();
    script(&fs, &bin, "podman", "exit 1");
    assert_eq!(
        runtimes(&fs, &[&bin]).installed(),
        vec![RuntimeFacts {
            name: "podman".to_string(),
            reachable: false
        }]
    );
}

// A runtime that never answers is bounded, not waited on.
#[test]
fn a_runtime_that_never_answers_is_unreachable_within_the_bound() {
    let (fs, bin) = machine();
    script(&fs, &bin, "docker", "exec sleep 30");
    let slow = HostRuntimes::new(
        fs.clone(),
        env(),
        vec![bin.clone()],
        Duration::from_millis(200),
    );
    let started = std::time::Instant::now();
    let found = slow.installed();
    assert!(started.elapsed() < Duration::from_secs(10));
    assert_eq!(found.len(), 1);
    assert!(!found[0].reachable);
}

// docker is preferred over podman (Go: runtimeCandidates order).
#[test]
fn docker_comes_before_podman() {
    let (fs, bin) = machine();
    script(&fs, &bin, "podman", "exit 0");
    script(&fs, &bin, "docker", "exit 0");
    let names: Vec<String> = runtimes(&fs, &[&bin])
        .installed()
        .into_iter()
        .map(|r| r.name)
        .collect();
    assert_eq!(names, vec!["docker", "podman"]);
}

// A file that is not executable is not a runtime.
#[test]
fn a_file_that_cannot_be_run_is_not_a_runtime() {
    let (fs, bin) = machine();
    fs.write_atomic(
        &bin.join("docker"),
        b"not a program",
        Perms::Exact(Mode::FILE),
    )
    .unwrap();
    assert!(runtimes(&fs, &[&bin]).installed().is_empty());
}

#[test]
fn an_image_is_present_when_the_runtime_inspects_it() {
    let (fs, bin) = machine();
    script(
        &fs,
        &bin,
        "docker",
        "[ \"$1 $2 $3\" = \"image inspect ctxloom-agent-mock\" ] && exit 0; exit 1",
    );
    let found = runtimes(&fs, &[&bin]);
    assert!(found.image_present("docker", "ctxloom-agent-mock"));
    assert!(!found.image_present("docker", "ctxloom-agent-other"));
    assert!(!found.image_present("podman", "ctxloom-agent-mock"));
}

// A directory called docker is no program, whatever its mode.
#[test]
fn a_directory_is_not_a_runtime() {
    let (fs, bin) = machine();
    fs.create_dir_all(&bin.join("docker"), Mode::DIR).unwrap();
    assert!(runtimes(&fs, &[&bin]).installed().is_empty());
}

// An empty PATH entry is not the working directory.
#[test]
fn the_path_of_the_environment_is_split_without_empty_entries() {
    let (fs, _) = machine();
    let env = Arc::new(EnvSnapshot::of([(
        "PATH",
        std::env::join_paths(["/a", "", "/b"]).unwrap(),
    )]));
    let found = HostRuntimes::from_env(fs.clone(), env.clone());
    let expected = vec![PathBuf::from("/a"), PathBuf::from("/b")];
    assert_eq!(found.dirs, expected);
    assert!(found.dirs.iter().all(|d| !d.as_os_str().is_empty()));
    assert_eq!(found.answer_within, Duration::from_secs(10));
    assert_eq!(found.drain_grace, Duration::from_secs(2));
}

// The runtime is run in the runtimes' environment, and only that one.
#[test]
fn a_runtime_runs_in_exactly_the_environment_it_was_given() {
    let (fs, bin) = machine();
    script(
        &fs,
        &bin,
        "docker",
        "[ \"${MARK-unset}\" = seen ] && [ \"${OTHER-unset}\" = unset ]",
    );
    let with = |vars: &[(&str, &str)]| {
        HostRuntimes::new(
            fs.clone(),
            Arc::new(EnvSnapshot::of(vars.iter().copied())),
            vec![bin.clone()],
            Duration::from_secs(5),
        )
        .installed()
    };
    assert!(with(&[("MARK", "seen")])[0].reachable);
    assert!(!with(&[("MARK", "seen"), ("OTHER", "x")])[0].reachable);
    assert!(!with(&[])[0].reachable);
}

// What a runtime started goes with it when it is killed.
#[test]
fn a_helper_a_runtime_started_is_killed_with_it() {
    let (fs, bin) = machine();
    script(
        &fs,
        &bin,
        "docker",
        "sleep 30 &\necho $! > \"$(dirname \"$0\")/helper\"\nwait",
    );
    // A loaded machine may not have started the helper by the first limit:
    // ask again with a longer one until it has.
    let started = std::time::Instant::now();
    for secs in [1, 5, 25] {
        let slow = HostRuntimes::new(
            fs.clone(),
            env(),
            vec![bin.clone()],
            Duration::from_secs(secs),
        );
        assert!(!slow.installed()[0].reachable);
        if fs.exists(&bin.join("helper")).unwrap() {
            break;
        }
    }
    assert!(started.elapsed() < Duration::from_secs(40));
    let pid = fs.read_to_string(&bin.join("helper")).unwrap();
    let stat = Path::new("/proc").join(pid.trim()).join("stat");
    let host = file_system::Os::new(os::host(), Path::new(os::host().flavor().top())).unwrap();
    // Gone, or a zombie nobody has reaped yet (an orphan waits for init).
    let state = host.read_to_string(&stat).unwrap_or_default();
    assert!(state.is_empty() || state.contains(") Z "), "{state}");
}

// A helper that outlives an answering runtime does not hold the caller.
#[test]
fn a_helper_that_outlives_the_runtime_does_not_hold_the_caller() {
    let (fs, bin) = machine();
    script(&fs, &bin, "docker", "sleep 5 &\nexit 0");
    let found = HostRuntimes::new(
        fs.clone(),
        env(),
        vec![bin.clone()],
        Duration::from_secs(30),
    )
    .drain_grace(Duration::from_millis(50));
    let started = std::time::Instant::now();
    assert!(found.installed()[0].reachable);
    assert!(started.elapsed() < Duration::from_secs(4));
}
