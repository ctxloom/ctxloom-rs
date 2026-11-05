//! Go's `companions.Verify` (`companions.go`): the probe is
//! `<bin> loadout --format yaml`; it must succeed and print something.

use std::sync::Arc;
use std::time::Instant;

use process_env::EnvSnapshot;

use file_system::{Fs, Perms, Rooted};
use os::Mode;

use super::*;

fn env() -> Arc<EnvSnapshot> {
    Arc::new(EnvSnapshot::of([
        ("PATH", "/usr/bin:/bin"),
        ("MARK", "seen"),
    ]))
}

fn script(body: &str) -> (Rooted, String) {
    let dir = Rooted::temp().unwrap();
    let path = os::fixture("/ctxloom-companion-x");
    dir.write_atomic(&path, body.as_bytes(), Perms::Exact(Mode::from_bits(0o755)))
        .unwrap();
    let shown = dir.real_path(&path).unwrap().to_str().unwrap().to_string();
    (dir, shown)
}

#[test]
fn test_a_program_that_prints_a_loadout_answers() {
    let (_dir, path) = script("#!/bin/sh\nprintf 'run:\\n  version: 1.0.0\\n'\n");
    assert_eq!(ProcessProbe::standard(env()).answers(&path), Ok(()));
}

#[test]
fn test_the_probe_is_loadout_format_yaml() {
    let (dir, path) = script("#!/bin/sh\necho \"$@\" > \"$(dirname \"$0\")/args\"\necho x\n");
    ProcessProbe::standard(env()).answers(&path).unwrap();
    assert_eq!(
        dir.read_to_string(&os::fixture("/args")).unwrap(),
        "loadout --format yaml\n"
    );
}

#[test]
fn test_a_program_that_prints_nothing_is_silent() {
    let (_dir, path) = script("#!/bin/sh\nexit 0\n");
    assert_eq!(
        ProcessProbe::standard(env()).answers(&path),
        Err(Unanswered::Silent)
    );
    let (_dir, path) = script("#!/bin/sh\necho\n");
    assert!(
        ProcessProbe::standard(env()).answers(&path).is_ok(),
        "a blank line is output"
    );
}

#[test]
fn test_a_program_that_fails_is_said_to_have_failed() {
    let (_dir, path) = script("#!/bin/sh\necho something\nexit 3\n");
    let Err(Unanswered::Failed(detail)) = ProcessProbe::standard(env()).answers(&path) else {
        panic!("should fail");
    };
    assert!(detail.contains("exit status: 3"), "{detail}");
}

#[test]
fn test_a_program_that_cannot_be_run_has_failed() {
    let Err(Unanswered::Failed(detail)) = ProcessProbe::standard(env()).answers("/no/such/program")
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
    let Err(Unanswered::Failed(detail)) = probe.answers(&path) else {
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
    assert_eq!(ProcessProbe::standard(env()).answers(&path), Ok(()));
}

// `container tooling` reads the loadout a companion prints, not only whether
// it answers (Go: `companions.Verify` and the loadout read, release/0.7).
#[test]
fn the_loadout_text_is_what_the_companion_prints() {
    let (_dir, path) = script("#!/bin/sh\nprintf 'init:\\n  tooling: x\\n'\n");
    let probe = ProcessProbe::standard(env());
    let text = probe.loadout(&path);
    assert_eq!(text, Ok("init:\n  tooling: x\n".to_string()));
}

#[test]
fn test_a_helper_the_program_started_is_killed_with_it() {
    // The helper holds the program's pipe, as a real one would; the limit is
    // long enough for the script to have started it.
    let (dir, path) =
        script("#!/bin/sh\nsleep 30 &\necho $! > \"$(dirname \"$0\")/helper\"\nwait\n");
    let probe = ProcessProbe::new(Duration::from_secs(1), env());
    let started = Instant::now();
    assert!(probe.answers(&path).is_err());
    assert!(started.elapsed() < Duration::from_secs(10));
    let pid = dir.read_to_string(&os::fixture("/helper")).unwrap();
    let stat = format!("/proc/{}/stat", pid.trim());
    // Gone, or a zombie nobody has reaped yet (an orphan waits for init).
    let host =
        file_system::Os::new(os::host(), std::path::Path::new(os::host().flavor().top())).unwrap();
    let state = host
        .read_to_string(std::path::Path::new(&stat))
        .unwrap_or_default();
    assert!(state.is_empty() || state.contains(") Z "), "{state}");
}

#[test]
fn test_a_helper_that_outlives_the_program_does_not_hold_the_probe() {
    let (_dir, path) = script("#!/bin/sh\nsleep 5 &\nprintf 'a:\\n  b: 1\\n'\n");
    let probe =
        ProcessProbe::new(Duration::from_secs(30), env()).drain_grace(Duration::from_millis(50));
    let started = Instant::now();
    assert_eq!(probe.loadout(&path), Ok("a:\n  b: 1\n".to_string()));
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[test]
fn test_the_program_runs_in_exactly_the_probes_environment() {
    let (_dir, path) = script("#!/bin/sh\necho \"${MARK-unset} ${OTHER-unset}\"\n");
    let probe = ProcessProbe::standard(env());
    assert_eq!(probe.loadout(&path), Ok("seen unset\n".to_string()));
}
