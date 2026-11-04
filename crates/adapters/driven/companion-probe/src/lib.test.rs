//! Go's `companions.Verify` (`companions.go`): the probe is
//! `<bin> loadout --format yaml`; it must succeed and print something.

use std::os::unix::fs::PermissionsExt;
use std::time::Instant;

use super::*;

fn script(body: &str) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ctxloom-companion-x");
    std::fs::write(&path, body).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let shown = path.to_str().unwrap().to_string();
    (dir, shown)
}

/// The probe's answer, asked again while the system says the script is busy:
/// another test's fork may hold the file open for writing a moment (ETXTBSY).
fn settled(probe: &ProcessProbe, path: &str) -> Result<(), Unanswered> {
    for _ in 0..50 {
        match probe.answers(path) {
            Err(Unanswered::Failed(why)) if why.contains("busy") => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            other => return other,
        }
    }
    probe.answers(path)
}

#[test]
fn test_a_program_that_prints_a_loadout_answers() {
    let (_dir, path) = script("#!/bin/sh\nprintf 'run:\\n  version: 1.0.0\\n'\n");
    assert_eq!(settled(&ProcessProbe::standard(), &path), Ok(()));
}

#[test]
fn test_the_probe_is_loadout_format_yaml() {
    let (dir, path) = script("#!/bin/sh\necho \"$@\" > \"$(dirname \"$0\")/args\"\necho x\n");
    settled(&ProcessProbe::standard(), &path).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("args")).unwrap(),
        "loadout --format yaml\n"
    );
}

#[test]
fn test_a_program_that_prints_nothing_is_silent() {
    let (_dir, path) = script("#!/bin/sh\nexit 0\n");
    assert_eq!(
        settled(&ProcessProbe::standard(), &path),
        Err(Unanswered::Silent)
    );
    let (_dir, path) = script("#!/bin/sh\necho\n");
    assert!(
        settled(&ProcessProbe::standard(), &path).is_ok(),
        "a blank line is output"
    );
}

#[test]
fn test_a_program_that_fails_is_said_to_have_failed() {
    let (_dir, path) = script("#!/bin/sh\necho something\nexit 3\n");
    let Err(Unanswered::Failed(detail)) = settled(&ProcessProbe::standard(), &path) else {
        panic!("should fail");
    };
    assert!(detail.contains("exit status: 3"), "{detail}");
}

#[test]
fn test_a_program_that_cannot_be_run_has_failed() {
    let Err(Unanswered::Failed(detail)) = ProcessProbe::standard().answers("/no/such/program")
    else {
        panic!("should fail");
    };
    assert!(detail.contains("No such file"), "{detail}");
}

#[test]
fn test_a_program_that_takes_too_long_is_killed_at_the_limit() {
    let (dir, path) = script("#!/bin/sh\nsleep 20\n");
    let started = Instant::now();
    let probe = ProcessProbe::new(Duration::from_millis(200));
    let Err(Unanswered::Failed(detail)) = settled(&probe, &path) else {
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
    assert_eq!(settled(&ProcessProbe::standard(), &path), Ok(()));
}

// `container tooling` reads the loadout a companion prints, not only whether
// it answers (Go: `companions.Verify` and the loadout read, release/0.7).
#[test]
fn the_loadout_text_is_what_the_companion_prints() {
    let (_dir, path) = script("#!/bin/sh\nprintf 'init:\\n  tooling: x\\n'\n");
    let probe = ProcessProbe::standard();
    let mut text = probe.loadout(&path);
    for _ in 0..50 {
        if !matches!(&text, Err(Unanswered::Failed(why)) if why.contains("busy")) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
        text = probe.loadout(&path);
    }
    assert_eq!(text, Ok("init:\n  tooling: x\n".to_string()));
}
