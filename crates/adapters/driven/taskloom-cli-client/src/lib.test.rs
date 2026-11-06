//! The adapter against a stub `taskloom`: a script in a directory of its own,
//! printing canned output, so each of taskloom's answers (and its silences)
//! is shown without the real program. One test of the real program is
//! `real.test.rs`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use angzarr_diagnostic::{Diagnostic, Report};
use file_system::{Fs, Perms, Rooted};
use os::Mode;
use process_env::EnvSnapshot;
use tasks_ports::{DeferredTask, Tasks, TasksError};

use super::*;

const SHELL_PATH: &str = "/usr/bin:/bin";

pub(crate) fn runs_scripts() -> bool {
    os::host().flavor() == os::Flavor::Unix
}

/// An environment of exactly these variables.
fn env(vars: &[(&str, &str)]) -> Arc<EnvSnapshot> {
    Arc::new(EnvSnapshot::of(vars.iter().copied()))
}

/// A program that runs `body`: `/taskloom` in `dir`. The path of the program
/// on the host.
fn program(dir: &Rooted, body: &[u8]) -> std::path::PathBuf {
    let body = std::str::from_utf8(body).unwrap();
    fixture_program::install_program(dir, &os::fixture("/taskloom"), body).unwrap()
}

/// A `taskloom` that is a script in a directory of its own, and a client for
/// project `p1` running it.
fn stub(body: &str) -> (Rooted, TaskloomCli) {
    let dir = Rooted::temp().unwrap();
    let real = program(&dir, body.as_bytes());
    let shown = real.to_str().unwrap();
    let cli = TaskloomCli::new(env(&[(BIN_VAR, shown), ("PATH", SHELL_PATH)]), "p1")
        .with_timeout(Duration::from_secs(20));
    (dir, cli)
}

fn prints(json: &str) -> String {
    format!("#!/bin/sh\ncat <<'EOF'\n{json}\nEOF\n")
}

fn read(dir: &Rooted, name: &str) -> String {
    dir.read_to_string(&os::fixture(&format!("/{name}")))
        .unwrap()
}

const SAMPLE: &str = r#"[
  {
    "harp_id": "unlearned-irregular",
    "text": "Revive when parser lands",
    "status": "Deferred",
    "checked": false,
    "text_hash": "77e4c4d6a35e",
    "trigger": "parser PR merged",
    "origin_session": "wild-live-blend",
    "tags": ["queue:20261104", "touches:cli"],
    "created_at": "2026-11-05T04:59:42.613303229Z",
    "derived_priority": null,
    "deferred_since": "2026-03-01T10:02:00Z"
  },
  {
    "harp_id": "catchy-showman",
    "text": "Second\nline",
    "status": "Deferred",
    "checked": false,
    "text_hash": "a116c9ed46d6",
    "trigger": "owner says go",
    "origin_session": "wild-live-blend",
    "tags": [],
    "created_at": "2026-11-05T04:59:42.629107707Z",
    "derived_priority": 3
  }
]"#;

#[test]
fn deferred_tasks_are_read_with_and_without_deferred_since() {
    if !runs_scripts() {
        return;
    }
    let (_dir, cli) = stub(&prints(SAMPLE));
    let tasks = cli.deferred().unwrap();
    assert_eq!(
        tasks,
        vec![
            DeferredTask {
                harp_id: "unlearned-irregular".into(),
                text: "Revive when parser lands".into(),
                trigger: "parser PR merged".into(),
                tags: vec!["queue:20261104".into(), "touches:cli".into()],
                deferred_since: Some(Duration::from_secs(1_772_359_320)),
            },
            DeferredTask {
                harp_id: "catchy-showman".into(),
                text: "Second\nline".into(),
                trigger: "owner says go".into(),
                tags: vec![],
                deferred_since: None,
            },
        ]
    );
}

#[test]
fn a_null_deferred_since_and_missing_optional_fields_are_none_and_empty() {
    if !runs_scripts() {
        return;
    }
    let (_dir, cli) = stub(&prints(
        r#"[{"harp_id":"a-b","text":"t","deferred_since":null}]"#,
    ));
    assert_eq!(
        cli.deferred().unwrap(),
        vec![DeferredTask {
            harp_id: "a-b".into(),
            text: "t".into(),
            trigger: String::new(),
            tags: vec![],
            deferred_since: None,
        }]
    );
}

#[test]
fn an_empty_list_is_no_tasks() {
    if !runs_scripts() {
        return;
    }
    let (_dir, cli) = stub(&prints("[]"));
    assert_eq!(cli.deferred().unwrap(), vec![]);
}

#[test]
fn taskloom_is_asked_for_the_deferred_tasks_of_the_project_as_json() {
    if !runs_scripts() {
        return;
    }
    let (dir, cli) = stub(
        "#!/bin/sh\nfor a in \"$@\"; do echo \"$a\"; done > \"$(dirname \"$0\")/args\"\necho '[]'\n",
    );
    cli.deferred().unwrap();
    assert_eq!(
        read(&dir, "args"),
        "tasks\nlist\n--status\nDeferred\n--format\njson\n--project\np1\n"
    );
}

#[test]
fn taskloom_runs_in_exactly_the_injected_environment() {
    if !runs_scripts() {
        return;
    }
    let dir = Rooted::temp().unwrap();
    let real = program(
        &dir,
        b"#!/bin/sh\nenv | sort > \"$(dirname \"$0\")/seen\"\necho '[]'\n",
    );
    let shown = real.to_str().unwrap();
    let cli = TaskloomCli::new(
        env(&[
            (BIN_VAR, shown),
            ("PATH", SHELL_PATH),
            ("CTXLOOM_ROOT", "/injected"),
        ]),
        "p1",
    );
    let tasks = cli.deferred();
    tasks.unwrap();
    let seen = read(&dir, "seen");
    assert!(seen.contains("CTXLOOM_ROOT=/injected\n"), "{seen}");
    assert!(seen.contains(&format!("TASKLOOM_BIN={shown}\n")), "{seen}");
    assert!(
        !seen.contains("HOME="),
        "the process's own is not passed: {seen}"
    );
}

#[test]
fn without_taskloom_bin_taskloom_is_looked_for_on_the_injected_path() {
    if !runs_scripts() {
        return;
    }
    let (dir, found) = stub(&prints(SAMPLE));
    let bin_dir = dir.real_path(&os::fixture("/")).unwrap();
    let cli = TaskloomCli::new(
        env(&[("PATH", &format!("{}:{SHELL_PATH}", bin_dir.display()))]),
        "p1",
    );
    assert_eq!(cli.deferred().unwrap(), found.deferred().unwrap());
    // An empty TASKLOOM_BIN names nothing.
    let cli = TaskloomCli::new(
        env(&[
            (BIN_VAR, ""),
            ("PATH", &format!("{}:{SHELL_PATH}", bin_dir.display())),
        ]),
        "p1",
    );
    assert_eq!(cli.deferred().unwrap().len(), 2);
}

#[test]
fn taskloom_not_found_is_said_with_the_program_asked_for() {
    let cli = TaskloomCli::new(env(&[(BIN_VAR, "/no/such/taskloom")]), "p1");
    assert_eq!(
        cli.deferred(),
        Err(TasksError::NotFound {
            program: "/no/such/taskloom".into()
        })
    );
    let cli = TaskloomCli::new(env(&[("PATH", "/no/such/dir")]), "p1");
    assert_eq!(
        cli.deferred(),
        Err(TasksError::NotFound {
            program: "taskloom".into()
        })
    );
}

#[test]
fn a_program_that_cannot_be_started_is_unrunnable_not_missing() {
    if !runs_scripts() {
        return;
    }
    let dir = Rooted::temp().unwrap();
    let path = os::fixture("/taskloom");
    dir.write_atomic(&path, b"#!/bin/sh\n", Perms::Exact(Mode::FILE))
        .unwrap();
    let real = dir.real_path(&path).unwrap();
    let cli = TaskloomCli::new(env(&[(BIN_VAR, real.to_str().unwrap())]), "p1");
    let Err(TasksError::Unrunnable { program, why }) = cli.deferred() else {
        panic!("should be unrunnable");
    };
    assert_eq!(program, real.to_str().unwrap());
    assert!(why.contains("Permission denied"), "{why}");
}

#[test]
fn a_failing_taskloom_with_its_envelope_is_passed_through_as_the_cause() {
    if !runs_scripts() {
        return;
    }
    let (_dir, cli) = stub(
        "#!/bin/sh\ncat >&2 <<'EOF'\n{\n  \"code\": \"tag-rules-tag-rules\",\n  \"args\": {\"findings\": \"not-permitted area:cli\", \"count\": 2},\n  \"message\": \"the store's tag rules refuse this write\"\n}\nEOF\nexit 2\n",
    );
    let Err(TasksError::Refused {
        program,
        exit,
        cause,
    }) = cli.deferred()
    else {
        panic!("should be refused");
    };
    assert!(program.ends_with("/taskloom"), "{program}");
    assert_eq!(exit, "exit status: 2");
    assert_eq!(
        cause,
        Report::new("tag-rules-tag-rules")
            .arg("count", "2")
            .arg("findings", "not-permitted area:cli")
            .arg("message", "the store's tag rules refuse this write")
    );
}

#[test]
fn an_envelope_after_other_words_on_standard_error_is_still_the_cause() {
    if !runs_scripts() {
        return;
    }
    let (_dir, cli) = stub(
        "#!/bin/sh\necho 'taskloom: project p1' >&2\necho '{\"code\":\"x-y\",\"args\":{},\"message\":\"m\"}' >&2\nexit 1\n",
    );
    let Err(TasksError::Refused { cause, .. }) = cli.deferred() else {
        panic!("should be refused");
    };
    assert_eq!(cause.code(), "x-y");
}

#[test]
fn a_failing_taskloom_without_an_envelope_is_failed_with_its_words() {
    if !runs_scripts() {
        return;
    }
    for words in ["boom: no such project", "{not json", "{\"code\": 3}", ""] {
        let (_dir, cli) = stub(&format!("#!/bin/sh\necho '{words}' >&2\nexit 7\n"));
        let Err(TasksError::Failed {
            exit,
            stderr,
            program,
        }) = cli.deferred()
        else {
            panic!("should fail: {words}");
        };
        assert_eq!(exit, "exit status: 7");
        assert_eq!(stderr, words);
        assert!(program.ends_with("/taskloom"), "{program}");
    }
}

#[test]
fn a_failure_is_judged_by_the_exit_not_by_what_is_printed() {
    if !runs_scripts() {
        return;
    }
    let (_dir, cli) = stub("#!/bin/sh\necho '[]'\nexit 1\n");
    assert!(matches!(cli.deferred(), Err(TasksError::Failed { .. })));
    // And a success that also says something on standard error is a success.
    let (_dir, cli) = stub("#!/bin/sh\necho 'taskloom: project p1' >&2\necho '[]'\n");
    assert_eq!(cli.deferred().unwrap(), vec![]);
}

#[test]
fn output_that_is_not_a_task_list_is_unparsable() {
    if !runs_scripts() {
        return;
    }
    for body in [
        "not json at all",
        "",
        "[{\"harp_id\": ",
        "{\"harp_id\":\"a\"}",
        "[{\"text\":\"no id\"}]",
        "[{\"harp_id\":\"a\",\"text\":\"t\",\"deferred_since\":\"yesterday\"}]",
        "[{\"harp_id\":\"a\",\"text\":\"t\",\"deferred_since\":\"1969-12-31T23:59:59Z\"}]",
    ] {
        let (_dir, cli) = stub(&prints(body));
        let Err(TasksError::Unparsable { program, why }) = cli.deferred() else {
            panic!("should be unparsable: {body}");
        };
        assert!(program.ends_with("/taskloom"), "{program}");
        assert!(!why.is_empty());
    }
}

#[test]
fn a_bad_timestamp_names_its_task() {
    if !runs_scripts() {
        return;
    }
    let (_dir, cli) = stub(&prints(
        r#"[{"harp_id":"a-b","text":"t","deferred_since":"yesterday"}]"#,
    ));
    let Err(TasksError::Unparsable { why, .. }) = cli.deferred() else {
        panic!("should be unparsable");
    };
    assert!(why.contains("a-b") && why.contains("yesterday"), "{why}");
}

#[test]
fn output_larger_than_a_pipe_does_not_stall_taskloom() {
    if !runs_scripts() {
        return;
    }
    // About 400 KiB of tasks: far past a pipe's buffer.
    let one = r#"{"harp_id":"a-b","text":"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"}"#;
    let (_dir, cli) = stub(&format!(
        "#!/bin/sh\necho '['\ni=0\nwhile [ $i -lt 3000 ]; do echo '{one},'; i=$((i+1)); done\necho '{one}]'\n"
    ));
    assert_eq!(cli.deferred().unwrap().len(), 3001);
}

#[test]
fn a_taskloom_that_hangs_is_killed_and_reaped_at_the_limit() {
    if !runs_scripts() {
        return;
    }
    let (dir, cli) = stub("#!/bin/sh\necho $$ > \"$(dirname \"$0\")/pid\"\nexec sleep 30\n");
    let cli = cli.with_timeout(Duration::from_millis(400));
    let started = Instant::now();
    let Err(TasksError::TimedOut { program, timeout }) = cli.deferred() else {
        panic!("should time out");
    };
    let took = started.elapsed();
    assert!(program.ends_with("/taskloom"), "{program}");
    assert_eq!(timeout, "400ms");
    assert!(took >= Duration::from_millis(400), "{took:?}");
    assert!(took < Duration::from_secs(10), "{took:?}");
    // Reaped, not left a zombie: its entry in /proc is gone.
    let pid = read(&dir, "pid").trim().to_string();
    let host = file_system::Os::new(os::host(), std::path::Path::new("/")).unwrap();
    assert!(
        !host
            .exists(std::path::Path::new(&format!("/proc/{pid}")))
            .unwrap(),
        "pid {pid} still has an entry"
    );
}

#[test]
fn the_default_limit_is_thirty_seconds() {
    assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(30));
    let cli = TaskloomCli::new(env(&[]), "p1");
    assert_eq!(cli.timeout, DEFAULT_TIMEOUT);
}

#[test]
fn deferred_since_keeps_its_fraction_and_honors_its_offset() {
    if !runs_scripts() {
        return;
    }
    let (_dir, cli) = stub(&prints(
        r#"[{"harp_id":"a-b","text":"t","deferred_since":"2026-11-05T06:59:42.5+02:00"}]"#,
    ));
    assert_eq!(
        cli.deferred().unwrap()[0].deferred_since,
        Some(Duration::new(1_793_854_782, 500_000_000))
    );
}

#[test]
fn a_helper_taskloom_started_is_killed_with_it() {
    if !runs_scripts() {
        return;
    }
    // The helper holds taskloom's pipes; the limit is long enough for the
    // script to have started it.
    let (dir, cli) = stub("#!/bin/sh\nsleep 30 &\necho $! > \"$(dirname \"$0\")/helper\"\nwait\n");
    // A loaded machine may not have started the helper by the first limit:
    // ask again with a longer one until it has.
    let started = std::time::Instant::now();
    for secs in [1, 5, 25] {
        let cli = cli.clone().with_timeout(Duration::from_secs(secs));
        assert!(matches!(cli.deferred(), Err(TasksError::TimedOut { .. })));
        if dir.exists(&os::fixture("/helper")).unwrap() {
            break;
        }
    }
    assert!(started.elapsed() < Duration::from_secs(40));
    let pid = read(&dir, "helper");
    let host =
        file_system::Os::new(os::host(), std::path::Path::new(os::host().flavor().top())).unwrap();
    let stat = std::path::Path::new("/proc").join(pid.trim()).join("stat");
    // Gone, or a zombie nobody has reaped yet (an orphan waits for init).
    let state = host.read_to_string(&stat).unwrap_or_default();
    assert!(state.is_empty() || state.contains(") Z "), "{state}");
}

#[test]
fn a_helper_that_outlives_taskloom_does_not_hold_the_caller() {
    if !runs_scripts() {
        return;
    }
    let (_dir, cli) = stub("#!/bin/sh\nsleep 5 &\necho '[]'\n");
    let cli = cli.with_drain_grace(Duration::from_millis(50));
    let started = std::time::Instant::now();
    assert_eq!(cli.deferred().unwrap(), vec![]);
    assert!(started.elapsed() < Duration::from_secs(4));
}
