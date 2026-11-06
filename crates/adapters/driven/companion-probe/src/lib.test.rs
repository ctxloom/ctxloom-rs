//! Go's `companions.Verify` (`companions.go`): the probe is
//! `<bin> loadout --format yaml`; it must succeed and print something.

use std::sync::Arc;
use std::time::Instant;

use process_env::EnvSnapshot;

use file_system::{Fs, Rooted};

use super::*;

fn env() -> Arc<EnvSnapshot> {
    Arc::new(EnvSnapshot::of([
        ("PATH", "/usr/bin:/bin"),
        ("MARK", "seen"),
    ]))
}

/// A `ctxloom-companion-x` that runs `body`, in a directory of its own.
fn script(body: &str) -> (Rooted, String) {
    let dir = Rooted::temp().unwrap();
    let program =
        fixture_program::install_program(&dir, &os::fixture("/ctxloom-companion-x"), body).unwrap();
    (dir, program.to_str().unwrap().to_string())
}

#[test]
fn test_a_program_that_prints_a_loadout_answers() {
    let (_dir, path) = script("#!/bin/sh\nprintf 'run:\\n  version: 1.0.0\\n'\n");
    assert_eq!(
        ProcessProbe::standard(env()).answers(std::slice::from_ref(&path)),
        Ok(())
    );
}

#[test]
fn test_the_probe_is_loadout_format_yaml() {
    let (dir, path) = script("#!/bin/sh\necho \"$@\" > \"$(dirname \"$0\")/args\"\necho x\n");
    ProcessProbe::standard(env())
        .answers(std::slice::from_ref(&path))
        .unwrap();
    assert_eq!(
        dir.read_to_string(&os::fixture("/args")).unwrap(),
        "loadout --format yaml\n"
    );
}

#[test]
fn test_a_program_that_prints_nothing_is_silent() {
    let (_dir, path) = script("#!/bin/sh\nexit 0\n");
    assert_eq!(
        ProcessProbe::standard(env()).answers(std::slice::from_ref(&path)),
        Err(Unanswered::Silent)
    );
    let (_dir, path) = script("#!/bin/sh\necho\n");
    assert!(
        ProcessProbe::standard(env())
            .answers(std::slice::from_ref(&path))
            .is_ok(),
        "a blank line is output"
    );
}

#[test]
fn test_a_program_that_fails_is_said_to_have_failed() {
    let (_dir, path) = script("#!/bin/sh\necho something\nexit 3\n");
    let Err(Unanswered::Exited(detail)) =
        ProcessProbe::standard(env()).answers(std::slice::from_ref(&path))
    else {
        panic!("should fail");
    };
    assert!(detail.contains("exit status: 3"), "{detail}");
}

#[test]
fn test_a_program_that_cannot_be_run_has_failed() {
    let Err(Unanswered::Missing(detail)) =
        ProcessProbe::standard(env()).answers(&["/no/such/program".to_string()])
    else {
        panic!("should fail");
    };
    assert!(detail.contains("No such file"), "{detail}");
}

#[test]
fn test_a_program_that_takes_too_long_is_killed_at_the_limit() {
    let (dir, path) = script("#!/bin/sh\nsleep 20\n");
    let started = Instant::now();
    let probe = ProcessProbe::new(Duration::from_millis(200), env());
    let Err(Unanswered::Failed(detail)) = probe.answers(std::slice::from_ref(&path)) else {
        panic!("should fail");
    };
    assert!(detail.contains("did not answer within"), "{detail}");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "it waited for the program"
    );
    drop(dir);
}

#[test]
fn test_a_program_with_a_great_deal_to_say_does_not_hang_the_probe() {
    let (_dir, path) = script("#!/bin/sh\nyes | head -c 5000000\n");
    assert_eq!(
        ProcessProbe::standard(env()).answers(std::slice::from_ref(&path)),
        Ok(())
    );
}

// `container tooling` reads the loadout a companion prints, not only whether
// it answers (Go: `companions.Verify` and the loadout read, release/0.7).
#[test]
fn the_loadout_text_is_what_the_companion_prints() {
    let (_dir, path) = script("#!/bin/sh\nprintf 'init:\\n  tooling: x\\n'\n");
    let probe = ProcessProbe::standard(env());
    let text = probe.loadout(std::slice::from_ref(&path));
    assert_eq!(text, Ok("init:\n  tooling: x\n".to_string()));
}

#[test]
fn test_a_helper_the_program_started_is_killed_with_it() {
    // The helper holds the program's pipe, as a real one would; the limit is
    // long enough for the script to have started it.
    let (dir, path) =
        script("#!/bin/sh\nsleep 30 &\necho $! > \"$(dirname \"$0\")/helper\"\nwait\n");
    // A loaded machine may not have started the helper by the first limit:
    // ask again with a longer one until it has.
    let started = Instant::now();
    for secs in [1, 5, 25] {
        let probe = ProcessProbe::new(Duration::from_secs(secs), env());
        assert!(probe.answers(std::slice::from_ref(&path)).is_err());
        if dir.exists(&os::fixture("/helper")).unwrap() {
            break;
        }
    }
    assert!(started.elapsed() < Duration::from_secs(40));
    let pid = dir.read_to_string(&os::fixture("/helper")).unwrap();
    assert!(
        run_within::process_gone(pid.trim()),
        "the helper {pid} outlived the program"
    );
}

#[test]
fn test_a_helper_that_outlives_the_program_does_not_hold_the_probe() {
    let (_dir, path) = script("#!/bin/sh\nsleep 5 &\nprintf 'a:\\n  b: 1\\n'\n");
    let probe =
        ProcessProbe::new(Duration::from_secs(30), env()).drain_grace(Duration::from_millis(50));
    let started = Instant::now();
    assert_eq!(
        probe.loadout(std::slice::from_ref(&path)),
        Ok("a:\n  b: 1\n".to_string())
    );
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[test]
fn test_the_program_runs_in_exactly_the_probes_environment() {
    let (_dir, path) = script("#!/bin/sh\necho \"${MARK-unset} ${OTHER-unset}\"\n");
    let probe = ProcessProbe::standard(env());
    assert_eq!(
        probe.loadout(std::slice::from_ref(&path)),
        Ok("seen unset\n".to_string())
    );
}
