//! The `CompanionLoadouts` port over a process: `Prober.ProbeCompanionLoadouts`'
//! outcomes, over the command a registration holds.

use std::sync::Arc;
use std::time::Duration;

use file_system::{Fs, Rooted};
use process_env::EnvSnapshot;

use super::*;

fn companions(limit: Duration) -> ProcessCompanions {
    ProcessCompanions::new(ProcessProbe::new(
        limit,
        Arc::new(EnvSnapshot::of([("PATH", "/usr/bin:/bin")])),
    ))
}

fn registered(command: &[&str]) -> RegisteredCompanion {
    RegisteredCompanion {
        name: "x".to_string(),
        command: command.iter().map(ToString::to_string).collect(),
    }
}

/// A program running `body`, in a directory of its own.
fn program(body: &str) -> (Rooted, String) {
    let dir = Rooted::temp().unwrap();
    let path = fixture_program::install_program(&dir, &os::fixture("/prog"), body).unwrap();
    (dir, path.to_str().unwrap().to_string())
}

const SECS: Duration = Duration::from_secs(5);

#[test]
fn a_program_that_prints_a_loadout_loads() {
    let (_dir, path) = program("#!/bin/sh\nprintf 'run:\\n  version: 1\\n'\n");
    assert_eq!(
        companions(SECS).loadout(&registered(&[&path])),
        CompanionLoadout::Loaded("run:\n  version: 1\n".to_string())
    );
}

#[test]
fn the_registered_arguments_come_before_the_protocols() {
    let (dir, path) = program("#!/bin/sh\necho \"$@\" > \"$(dirname \"$0\")/args\"\necho x\n");
    companions(SECS).loadout(&registered(&[&path, "--as", "acme"]));
    assert_eq!(
        dir.read_to_string(&os::fixture("/args")).unwrap(),
        "--as acme loadout --format yaml\n"
    );
}

#[test]
fn a_program_that_does_not_exist_is_absent() {
    assert_eq!(
        companions(SECS).loadout(&registered(&["/no/such/program"])),
        CompanionLoadout::Absent
    );
}

#[test]
fn a_bare_name_is_left_to_the_system_to_find() {
    // `true` exits 0 printing nothing: found by the system's own lookup, and
    // it printed no loadout.
    assert_eq!(
        companions(SECS).loadout(&registered(&["true"])),
        CompanionLoadout::ProbeFailed("it printed no loadout".to_string())
    );
}

#[test]
fn a_program_that_exits_with_a_status_offers_no_loadout() {
    let (_dir, path) = program("#!/bin/sh\necho oops\nexit 3\n");
    assert_eq!(
        companions(SECS).loadout(&registered(&[&path])),
        CompanionLoadout::NoLoadout
    );
}

#[test]
fn a_program_that_prints_nothing_has_failed() {
    let (_dir, path) = program("#!/bin/sh\nexit 0\n");
    assert_eq!(
        companions(SECS).loadout(&registered(&[&path])),
        CompanionLoadout::ProbeFailed("it printed no loadout".to_string())
    );
}

#[test]
fn a_program_killed_by_a_signal_has_failed() {
    let (_dir, path) = program("#!/bin/sh\nkill -9 $$\n");
    let CompanionLoadout::ProbeFailed(detail) = companions(SECS).loadout(&registered(&[&path]))
    else {
        panic!("should fail");
    };
    assert!(detail.contains("signal"), "{detail}");
}

#[test]
fn a_program_that_cannot_be_started_has_failed() {
    // A directory is there and cannot be run.
    let dir = Rooted::temp().unwrap();
    let path = dir.root().to_str().unwrap().to_string();
    let CompanionLoadout::ProbeFailed(detail) = companions(SECS).loadout(&registered(&[&path]))
    else {
        panic!("should fail");
    };
    assert!(detail.contains("ermission"), "{detail}");
}

#[test]
fn a_program_that_takes_too_long_has_failed() {
    let (_dir, path) = program("#!/bin/sh\nsleep 20\n");
    let CompanionLoadout::ProbeFailed(detail) =
        companions(Duration::from_millis(200)).loadout(&registered(&[&path]))
    else {
        panic!("should fail");
    };
    assert!(detail.contains("did not answer within"), "{detail}");
}

#[test]
fn an_empty_command_has_failed() {
    let CompanionLoadout::ProbeFailed(detail) = companions(SECS).loadout(&registered(&[])) else {
        panic!("should fail");
    };
    assert!(detail.contains("empty"), "{detail}");
}
