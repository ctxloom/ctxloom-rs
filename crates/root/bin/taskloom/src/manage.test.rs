//! Acceptance: `taskloom manage`, the real binary, only ever in a
//! temporary home with an empty PATH: it writes Go's bytes into
//! `~/.claude.json` and a project's `.mcp.json`, says what Go says, and
//! reads back what it wrote. Nothing outside the temporary home and
//! project directory is read or written.

use std::path::Path;

use tempfile::TempDir;

use super::{one_document, Env, Ran};

/// The Go case `name`'s file `file`.
fn go(name: &str, file: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../adapters/driven/agent-configs/fixtures/go-manage")
        .join(name)
        .join(file);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// `taskloom manage <args>` in `env`, nothing on its PATH.
fn manage(env: &Env, path: &TempDir, args: &[&str]) -> Ran {
    let mut all = vec!["manage"];
    all.extend_from_slice(args);
    env.run_with(&all, &[("PATH", path.path().to_str().unwrap())])
}

/// Go's stderr for the case, its exit code dropped, `$HOME` and `$DIR`
/// put back.
fn go_said(name: &str, file: &str, env: &Env) -> String {
    let text = String::from_utf8(go(name, file)).unwrap();
    let (_, said) = text.split_once('\n').unwrap();
    said.replace("$HOME", &env.home.path().display().to_string())
        .replace("$DIR", &env.project.path().display().to_string())
}

#[test]
fn test_install_and_uninstall_write_gos_bytes_and_say_gos_words() {
    let env = Env::new("manage-harp");
    let empty = tempfile::tempdir().unwrap();
    let config = env.home.path().join(".claude.json");
    std::fs::write(&config, go("indented-other-server", "before.json")).unwrap();

    let ran = manage(&env, &empty, &["install", "--format", "text"]);
    assert!(ran.ok, "{}", ran.stderr);
    assert_eq!(ran.stdout, "");
    assert_eq!(
        ran.stderr,
        go_said("indented-other-server", "install.out", &env)
    );
    assert_eq!(
        std::fs::read(&config).unwrap(),
        go("indented-other-server", "install.json")
    );

    let ran = manage(&env, &empty, &["check", "--format", "text"]);
    assert_eq!(
        ran.stdout,
        format!(
            "binary: taskloom NOT on PATH\nclaude-code  user     registered ({})\n",
            config.display()
        )
    );

    let ran = manage(&env, &empty, &["uninstall", "--format", "text"]);
    assert!(ran.ok, "{}", ran.stderr);
    assert_eq!(
        ran.stderr,
        go_said("indented-other-server", "uninstall.out", &env)
    );
    assert_eq!(
        std::fs::read(&config).unwrap(),
        go("indented-other-server", "before.json")
    );
}

#[test]
fn test_an_engine_named_makes_its_file_and_project_is_a_switch() {
    let env = Env::new("manage-project-harp");
    let empty = tempfile::tempdir().unwrap();
    let ran = manage(&env, &empty, &["install", "--format", "text"]);
    assert!(!ran.ok);
    assert_eq!(
        ran.stderr,
        "taskloom: no agent backends detected; name one with --engine (claude-code)\n"
    );
    let ran = manage(
        &env,
        &empty,
        &["install", "--engine", "claude-code", "--json"],
    );
    assert!(ran.ok, "{}", ran.stderr);
    assert_eq!(one_document(&ran.stdout)["configs"][0]["written"], true);
    assert_eq!(
        std::fs::read(env.home.path().join(".claude.json")).unwrap(),
        go("absent", "install.json")
    );

    std::fs::write(
        env.project.path().join(".mcp.json"),
        go("project-scope", "before.json"),
    )
    .unwrap();
    let ran = manage(&env, &empty, &["install", "--project", "--format", "text"]);
    assert!(ran.ok, "{}", ran.stderr);
    assert!(
        ran.stderr.ends_with("  config: .mcp.json\n"),
        "{}",
        ran.stderr
    );
    assert_eq!(
        std::fs::read(env.project.path().join(".mcp.json")).unwrap(),
        go("project-scope", "install.json")
    );
    let ran = manage(&env, &empty, &["check", "--json"]);
    let doc = one_document(&ran.stdout);
    assert_eq!(doc["binary_on_path"], false);
    assert_eq!(doc["configs"].as_array().unwrap().len(), 2);

    // Every other command's --project still names a project.
    let listed = env.run(&["list", "--project", "jolly-mute-magma", "--json"]);
    assert!(listed.ok, "{}", listed.stderr);
}

#[test]
fn test_a_corrupt_file_is_copied_aside_and_left_alone() {
    let env = Env::new("manage-corrupt-harp");
    let empty = tempfile::tempdir().unwrap();
    let config = env.home.path().join(".claude.json");
    let broken = go("corrupt", "before.json");
    std::fs::write(&config, &broken).unwrap();
    let ran = manage(&env, &empty, &["install", "--json"]);
    assert!(!ran.ok);
    assert_eq!(ran.stdout, "");
    let envelope = one_document(&ran.stderr);
    assert_eq!(envelope["code"], "registration-application-engine-failed");
    assert_eq!(envelope["args"]["engine"], "claude-code");
    assert_eq!(envelope["args"]["error"]["code"], "agent-configs-corrupt");
    assert_eq!(std::fs::read(&config).unwrap(), broken);
    let copies: Vec<_> = std::fs::read_dir(env.home.path())
        .unwrap()
        .filter_map(|e| {
            let e = e.unwrap();
            e.file_name()
                .to_string_lossy()
                .starts_with(".claude.json.corrupt-")
                .then(|| std::fs::read(e.path()).unwrap())
        })
        .collect();
    assert_eq!(copies, [broken]);
}
