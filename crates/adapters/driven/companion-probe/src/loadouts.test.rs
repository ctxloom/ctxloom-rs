//! The `CompanionLoadouts` port over a process: Go's name-to-binary rule and
//! `Prober.ProbeCompanionLoadouts`' four outcomes.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use file_system::Rooted;
use process_env::EnvSnapshot;

use super::*;

/// A PATH of named programs.
struct Path(BTreeMap<String, String>);

impl Executables for Path {
    fn find(&self, name: &str) -> Option<String> {
        self.0.get(name).cloned()
    }
}

fn companions(path: &[(&str, &str)], limit: Duration) -> ProcessCompanions {
    let found = path
        .iter()
        .map(|(n, p)| ((*n).to_string(), (*p).to_string()))
        .collect();
    ProcessCompanions::new(
        Arc::new(Path(found)),
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
fn the_first_party_companions_are_their_own_binaries() {
    for name in ["ltk", "taskloom", "reprise"] {
        assert_eq!(binary_name(name), name);
    }
}

#[test]
fn any_other_name_is_ctxloom_companion_dash_the_name() {
    assert_eq!(binary_name("acme"), "ctxloom-companion-acme");
    assert_eq!(binary_name("ltk2"), "ctxloom-companion-ltk2");
    assert_eq!(binary_name("a.b_c-d"), "ctxloom-companion-a.b_c-d");
}

#[test]
fn a_name_is_one_plain_segment_not_starting_like_a_flag() {
    for good in ["a", "A9", "x.y", "x_y", "x-y", "9lives"] {
        assert!(is_plain_name(good), "{good}");
    }
    for bad in [
        "", "-x", ".x", "_x", "a/b", "a b", "a\\b", "..", "a:b", "\u{e9}",
    ] {
        assert!(!is_plain_name(bad), "{bad:?}");
    }
}

#[test]
fn a_companion_not_on_the_path_is_absent() {
    let c = companions(&[], SECS);
    assert_eq!(c.loadout("reprise"), CompanionLoadout::Absent);
}

#[test]
fn the_binary_looked_for_is_the_names() {
    let (_dir, path) = program("x", "#!/bin/sh\nprintf 'run:\\n  version: 1.0.0\\n'\n");
    let c = companions(&[("ctxloom-companion-acme", &path)], SECS);
    assert_eq!(
        c.loadout("acme"),
        CompanionLoadout::Loaded("run:\n  version: 1.0.0\n".to_string())
    );
    // `acme` itself on the PATH is no companion of that name.
    let c = companions(&[("acme", &path)], SECS);
    assert_eq!(c.loadout("acme"), CompanionLoadout::Absent);
}

#[test]
fn a_name_that_is_no_plain_name_is_not_run_and_is_said() {
    let (_dir, path) = program("x", "#!/bin/sh\necho ran\n");
    let c = companions(&[("ctxloom-companion--x", &path)], SECS);
    let CompanionLoadout::ProbeFailed(detail) = c.loadout("-x") else {
        panic!("a bad name should fail");
    };
    assert!(detail.contains("-x"), "{detail}");
}

#[test]
fn a_companion_that_prints_nothing_has_no_loadout() {
    let (_dir, path) = program("x", "#!/bin/sh\nexit 0\n");
    let c = companions(&[("ctxloom-companion-x", &path)], SECS);
    assert_eq!(c.loadout("x"), CompanionLoadout::NoLoadout);
}

#[test]
fn a_companion_that_fails_has_failed() {
    let (_dir, path) = program("x", "#!/bin/sh\necho oops\nexit 3\n");
    let c = companions(&[("ctxloom-companion-x", &path)], SECS);
    let CompanionLoadout::ProbeFailed(detail) = c.loadout("x") else {
        panic!("should fail");
    };
    assert!(detail.contains("exit status: 3"), "{detail}");
}

#[test]
fn a_companion_that_takes_too_long_has_failed() {
    let (_dir, path) = program("x", "#!/bin/sh\nsleep 20\n");
    let c = companions(
        &[("ctxloom-companion-x", &path)],
        Duration::from_millis(200),
    );
    let CompanionLoadout::ProbeFailed(detail) = c.loadout("x") else {
        panic!("should fail");
    };
    assert!(detail.contains("did not answer within"), "{detail}");
}

#[test]
fn the_standard_limit_is_three_seconds() {
    let c = ProcessCompanions::standard(
        Arc::new(Path(BTreeMap::new())),
        Arc::new(EnvSnapshot::of([("PATH", "/usr/bin")])),
    );
    assert_eq!(c.loadout("x"), CompanionLoadout::Absent);
}
