//! Acceptance: `taskloom run`, the real binary in a temporary home, with a
//! stub `ctxloom` alone on its PATH: never a real ctxloom. The stub
//! records the argv it is handed, which is Go's.

use std::path::Path;

use tempfile::TempDir;

use super::{Env, Ran};

/// A directory holding only a stub `ctxloom` that records its arguments,
/// one a line, in `args` beside it, and exits `status`.
fn stub(status: u8) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("ctxloom");
    let record = dir.path().join("args");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nexit {status}\n",
            record.display()
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dir
}

fn recorded(stub: &Path) -> Vec<String> {
    std::fs::read_to_string(stub.join("args"))
        .expect("the stub ran")
        .lines()
        .map(str::to_string)
        .collect()
}

fn run_on(env: &Env, path: &Path, args: &[&str]) -> Ran {
    let path = path.to_str().unwrap();
    env.run_with(args, &[("PATH", path)])
}

#[test]
fn test_run_hands_the_task_to_ctxloom_run_with_gos_argv() {
    let env = Env::new("origin-harp");
    let harp = env.seed("Wire up X")["harp_id"]
        .as_str()
        .unwrap()
        .to_string();
    let ctxloom = stub(0);
    let ran = run_on(&env, ctxloom.path(), &["run", &harp]);
    assert!(ran.ok, "{}", ran.stderr);
    assert_eq!(ran.stdout, "", "stdout is ctxloom's");
    assert_eq!(
        recorded(ctxloom.path()),
        [
            "run".to_string(),
            "--session".to_string(),
            "origin-harp".to_string(),
            "--seed-task".to_string(),
            harp.clone(),
            "--prompt".to_string(),
            format!("Work on this task (`{harp}`): Wire up X"),
        ]
    );

    let ran = run_on(&env, ctxloom.path(), &["run", &harp, "--no-start"]);
    assert!(ran.ok, "{}", ran.stderr);
    let args = recorded(ctxloom.path());
    assert!(
        args.windows(2).any(|w| w == ["--seed-status", "To Do"]),
        "{args:?}"
    );
}

#[test]
fn test_run_says_why_it_launched_nothing() {
    let env = Env::new("run-harp");
    let harp = env.seed("Something")["harp_id"]
        .as_str()
        .unwrap()
        .to_string();

    let ctxloom = stub(3);
    let ran = run_on(&env, ctxloom.path(), &["run", &harp, "--format", "text"]);
    assert!(!ran.ok);
    assert_eq!(ran.stderr, "taskloom: ctxloom run exited with status 3\n");

    let nothing = tempfile::tempdir().unwrap();
    let ran = run_on(&env, nothing.path(), &["run", &harp, "--json"]);
    assert!(!ran.ok);
    assert_eq!(ran.stdout, "", "an error leaves stdout empty");
    assert_eq!(
        super::one_document(&ran.stderr)["code"],
        "tasks-run-no-ctxloom"
    );

    let ran = run_on(&env, ctxloom.path(), &["run", "--format", "text"]);
    assert!(!ran.ok);
    assert_eq!(
        ran.stderr,
        "taskloom: no task harp id given and not a terminal; pass a task harp id (see `taskloom list`)\n"
    );
    let ran = run_on(
        &env,
        ctxloom.path(),
        &["run", "no-such-task", "--format", "text"],
    );
    assert_eq!(ran.stderr, "taskloom: task not found: no-such-task\n");
}
