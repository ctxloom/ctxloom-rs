//! The adapter against the real, installed `taskloom`, on a throwaway
//! project in a throwaway home: the program's real task log is never opened.
//! Skipped, with its reason printed, where `taskloom` is not on `PATH`.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use file_system::{Fs, Rooted};
use process_env::EnvSnapshot;
use tasks_ports::Tasks;

use super::*;

/// `taskloom` on the process's `PATH`, if it is there.
fn installed() -> Option<(PathBuf, String)> {
    let process = EnvSnapshot::from_process();
    let host = file_system::Os::new(os::host(), std::path::Path::new("/")).ok()?;
    let found = process
        .search_path()
        .into_iter()
        .map(|dir| dir.join("taskloom"))
        .find(|candidate| host.exists(candidate).unwrap_or(false))?;
    let path = process.utf8("PATH")?.to_string();
    Some((found, path))
}

#[test]
fn the_real_taskloom_lists_a_deferred_task_of_a_throwaway_project() {
    let Some((taskloom, path)) = installed() else {
        eprintln!("SKIPPED: taskloom is not on PATH, so the real program is not exercised");
        return;
    };
    if !crate::tests::runs_scripts() {
        return;
    }
    // A home of its own: the project's task log is under it, so the real one
    // is not opened.
    let home = Rooted::temp().unwrap();
    let home_dir = home.root().to_str().unwrap().to_string();
    let root = format!("{home_dir}/root");
    let env = Arc::new(EnvSnapshot::of([
        ("PATH", path.as_str()),
        ("HOME", home_dir.as_str()),
        ("CTXLOOM_ROOT", root.as_str()),
    ]));
    let project = "tasks-poc-throwaway";
    let add = |args: &[&str]| {
        let added = Command::new(&taskloom)
            .args(["tasks", "add"])
            .args(args)
            .args(["--format", "json", "--project", project])
            .env_clear()
            .envs(env.vars())
            .output()
            .unwrap();
        assert!(
            added.status.success(),
            "{}",
            String::from_utf8_lossy(&added.stderr)
        );
    };
    add(&[
        "Revive when the parser lands",
        "--status",
        "Deferred",
        "--trigger",
        "parser PR merged",
    ]);
    add(&["An ordinary task"]);

    let tasks = TaskloomCli::new(env, project).deferred().unwrap();

    assert_eq!(tasks.len(), 1, "only the Deferred task: {tasks:?}");
    assert_eq!(tasks[0].text, "Revive when the parser lands");
    assert_eq!(tasks[0].trigger, "parser PR merged");
    assert!(!tasks[0].harp_id.is_empty());
    assert!(tasks[0].tags.is_empty());
    // This taskloom may not emit deferred_since yet; if it does, it parses.
    let _ = tasks[0].deferred_since;
    // The task log is the throwaway home's.
    let db = format!("{home_dir}/.ctxloom/projects/{project}/state.db");
    let host = file_system::Os::new(os::host(), std::path::Path::new("/")).unwrap();
    assert!(host.exists(std::path::Path::new(&db)).unwrap(), "{db}");
}
