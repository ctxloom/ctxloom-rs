//! The `CompanionLoadouts` port over a process: `Prober.ProbeCompanionLoadouts`' four
//! outcomes, over programs the adapter is given.

use std::sync::Arc;
use std::time::Duration;

use file_system::Rooted;
use process_env::EnvSnapshot;

use super::*;

fn companions(programs: &[(&str, &str)], limit: Duration) -> ProcessCompanions {
    let programs = programs
        .iter()
        .map(|(n, p)| ((*n).to_string(), (*p).to_string()))
        .collect();
    ProcessCompanions::new(
        programs,
        ProcessProbe::new(
            limit,
            Arc::new(EnvSnapshot::of([("PATH", "/usr/bin:/bin")])),
        ),
    )
}

/// A program running `body`, as `bin` in a directory of its own.
fn program(bin: &str, body: &str) -> (Rooted, String) {
    let dir = Rooted::temp().unwrap();
    let path =
        fixture_program::install_program(&dir, &os::fixture(&format!("/{bin}")), body).unwrap();
    (dir, path.to_str().unwrap().to_string())
}

const SECS: Duration = Duration::from_secs(5);

#[test]
fn a_name_with_no_program_is_absent() {
    let c = companions(&[], SECS);
    assert_eq!(c.loadout("reprise"), CompanionLoadout::Absent);
}

#[test]
fn a_companion_that_prints_nothing_has_no_loadout() {
    let (_dir, path) = program("x", "#!/bin/sh\nexit 0\n");
    let c = companions(&[("x", &path)], SECS);
    assert_eq!(c.loadout("x"), CompanionLoadout::NoLoadout);
}

#[test]
fn a_companion_that_fails_has_failed() {
    let (_dir, path) = program("x", "#!/bin/sh\necho oops\nexit 3\n");
    let c = companions(&[("x", &path)], SECS);
    let CompanionLoadout::ProbeFailed(detail) = c.loadout("x") else {
        panic!("should fail");
    };
    assert!(detail.contains("exit status: 3"), "{detail}");
}

#[test]
fn a_companion_that_takes_too_long_has_failed() {
    let (_dir, path) = program("x", "#!/bin/sh\nsleep 20\n");
    let c = companions(&[("x", &path)], Duration::from_millis(200));
    let CompanionLoadout::ProbeFailed(detail) = c.loadout("x") else {
        panic!("should fail");
    };
    assert!(detail.contains("did not answer within"), "{detail}");
}

#[test]
fn a_name_is_answered_by_its_own_program() {
    let (_a, one) = program("a", "#!/bin/sh\nprintf 'run:\\n  version: 1\\n'\n");
    let (_b, two) = program("b", "#!/bin/sh\nprintf 'run:\\n  version: 2\\n'\n");
    let c = companions(&[("one", &one), ("two", &two)], SECS);
    assert_eq!(
        c.loadout("two"),
        CompanionLoadout::Loaded("run:\n  version: 2\n".to_string())
    );
    assert_eq!(
        c.loadout("one"),
        CompanionLoadout::Loaded("run:\n  version: 1\n".to_string())
    );
    assert_eq!(c.loadout("three"), CompanionLoadout::Absent);
}
