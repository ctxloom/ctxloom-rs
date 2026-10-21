//! The adapter against Go: every case in fixtures/go-manage is what Go
//! taskloom's own `manage` did to that file (scripts/go-manage-fixtures.sh
//! ran it), and the adapter must leave the same bytes, refuse where Go
//! refused, and copy a file aside where Go did. Every case runs in
//! temporary homes; nothing outside them is read or written.

use std::path::{Path, PathBuf};

use annals_diagnostic::Diagnostic;
use ports::{AgentConfigs, McpServer, Scope};
use tempfile::TempDir;

use super::{go_join, records, AgentConfigFiles, CLAUDE_CODE};

fn server() -> McpServer {
    McpServer {
        command: "taskloom".to_string(),
        args: vec!["mcp".to_string()],
    }
}

/// One Go case.
struct Case {
    name: String,
    dir: PathBuf,
}

impl Case {
    fn all() -> Vec<Case> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/go-manage");
        let mut cases: Vec<Case> = std::fs::read_dir(&root)
            .expect("the Go cases")
            .map(|e| {
                let dir = e.unwrap().path();
                Case {
                    name: dir.file_name().unwrap().to_string_lossy().into_owned(),
                    dir,
                }
            })
            .collect();
        cases.sort_by(|a, b| a.name.cmp(&b.name));
        assert!(cases.len() > 20, "the cases are there");
        cases
    }

    fn file(&self, name: &str) -> Option<Vec<u8>> {
        std::fs::read(self.dir.join(name)).ok()
    }

    fn project(&self) -> bool {
        self.dir.join("scope").exists()
    }

    /// An outcome file: its exit code and the rest.
    fn outcome(&self, name: &str) -> (i32, String) {
        let text = String::from_utf8(self.file(name).expect(name)).unwrap();
        let (code, rest) = text.split_once('\n').unwrap_or((&text, ""));
        (code.parse().unwrap(), rest.to_string())
    }
}

/// A fresh home and project directory, the case's file at its place with
/// `content`, and the adapter over them, with nothing on its PATH.
struct Home {
    _home: TempDir,
    _dir: TempDir,
    _bin: TempDir,
    configs: AgentConfigFiles,
    path: PathBuf,
}

impl Home {
    fn new(case: &Case, content: Option<&[u8]>) -> Home {
        let (home, dir, bin) = (
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
        );
        let mut configs = AgentConfigFiles {
            home: Some(home.path().to_path_buf()),
            path: Some(bin.path().as_os_str().to_owned()),
            waits: file_lock::Waits::DEFAULT,
        };
        let scope = if case.project() {
            Scope::Project(dir.path().to_path_buf())
        } else {
            Scope::User
        };
        let path = configs.config_path(CLAUDE_CODE, &scope).unwrap();
        if let Some(content) = content {
            std::fs::write(&path, content).unwrap();
        }
        Home {
            _home: home,
            _dir: dir,
            _bin: bin,
            configs,
            path,
        }
    }

    fn register(&mut self, server: Option<&McpServer>) -> Result<bool, annals_diagnostic::Report> {
        let path = self.path.clone();
        self.configs
            .register(CLAUDE_CODE, &path, "taskloom", "taskloom", server, false)
            .map(|r| r.changed)
    }

    fn content(&self) -> Option<Vec<u8>> {
        std::fs::read(&self.path).ok()
    }

    fn backups(&self) -> Vec<Vec<u8>> {
        let dir = self.path.parent().unwrap();
        let prefix = format!(
            "{}.corrupt-",
            self.path.file_name().unwrap().to_string_lossy()
        );
        std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| {
                let e = e.unwrap();
                let name = e.file_name().to_string_lossy().into_owned();
                name.starts_with(&prefix)
                    .then(|| std::fs::read(e.path()).unwrap())
            })
            .collect()
    }
}

/// `outcome` (Go's exit code and stderr) held against what the adapter
/// answered and left: the file `want` after a success, the file untouched
/// and copied aside (or not) after a refusal.
fn hold(
    case: &Case,
    step: &str,
    home: &Home,
    answered: &Result<bool, annals_diagnostic::Report>,
    outcome: (i32, String),
    before: Option<&[u8]>,
    want: Option<Vec<u8>>,
) {
    let label = format!("{} {step}", case.name);
    let (code, stderr) = outcome;
    match answered {
        Ok(changed) => {
            assert_eq!(code, 0, "{label}: Go refused ({stderr}), Rust did not");
            assert_eq!(
                home.content()
                    .map(|c| String::from_utf8_lossy(&c).into_owned()),
                want.map(|w| String::from_utf8_lossy(&w).into_owned()),
                "{label}: the bytes"
            );
            let removed = stderr.contains("removed MCP server");
            let nothing = stderr.contains("nothing to remove");
            if removed || nothing {
                assert_eq!(*changed, removed, "{label}: changed");
            }
            assert!(home.backups().is_empty(), "{label}: no copy");
        }
        Err(report) => {
            assert_eq!(code, 1, "{label}: Rust refused ({report}), Go did not");
            assert_eq!(home.content().as_deref(), before, "{label}: left as it was");
            let copied = stderr.contains("backed up to");
            assert_eq!(
                report.code() == "agent-configs-corrupt",
                copied,
                "{label}: {report}"
            );
            let want: Vec<Vec<u8>> = if copied {
                vec![before.unwrap_or_default().to_vec()]
            } else {
                vec![]
            };
            assert_eq!(home.backups(), want, "{label}: the copy");
        }
    }
}

#[test]
fn test_an_install_does_to_each_file_what_gos_did() {
    for case in Case::all() {
        let before = case.file("before.json");
        let mut home = Home::new(&case, before.as_deref());
        let answered = home.register(Some(&server()));
        let want = case.file("install.json");
        hold(
            &case,
            "install",
            &home,
            &answered,
            case.outcome("install.out"),
            before.as_deref(),
            want,
        );
    }
}

#[test]
fn test_an_uninstall_with_no_record_does_what_gos_did_with_none() {
    for case in Case::all() {
        let Some(installed) = case
            .file("install.json")
            .or_else(|| case.file("before.json"))
        else {
            continue;
        };
        let mut home = Home::new(&case, Some(&installed));
        let answered = home.register(None);
        hold(
            &case,
            "uninstall",
            &home,
            &answered,
            case.outcome("uninstall.out"),
            Some(&installed),
            case.file("uninstall.json"),
        );
    }
}

#[test]
fn test_an_uninstall_after_gos_install_does_what_gos_uninstall_did() {
    let mut held = 0;
    for case in Case::all() {
        let (Some(record), Some(installed)) = (case.file("record.yaml"), case.file("install.json"))
        else {
            continue;
        };
        let mut home = Home::new(&case, Some(&installed));
        let target = home.path.display().to_string();
        let records = home
            .configs
            .home
            .clone()
            .unwrap()
            .join(".ctxloom/records/taskloom");
        std::fs::create_dir_all(&records).unwrap();
        let record = String::from_utf8(record)
            .unwrap()
            .replace("$TARGET", &target);
        let name = format!(
            "{}__20261021T000000.000000000Z.hew-record.yaml",
            records::flat_name(&target)
        );
        std::fs::write(records.join(name), record).unwrap();
        let answered = home.register(None);
        assert!(answered.is_ok(), "{}: {answered:?}", case.name);
        assert_eq!(
            home.content()
                .map(|c| String::from_utf8_lossy(&c).into_owned()),
            case.file("recorded.json")
                .map(|c| String::from_utf8_lossy(&c).into_owned()),
            "{}: Go's own uninstall",
            case.name
        );
        held += 1;
    }
    assert!(held > 10, "{held} cases with Go's record");
}

#[test]
fn test_a_check_reads_each_file_as_gos_did() {
    for case in Case::all() {
        let before = case.file("before.json");
        let mut home = Home::new(&case, before.as_deref());
        let path = home.path.clone();
        let found = home.configs.installed(CLAUDE_CODE, &path, "taskloom");
        let (code, stdout) = case.outcome("check.out");
        assert_eq!(code, 0);
        let row = stdout.lines().nth(1).unwrap_or_default();
        let label = &case.name;
        match found {
            Ok(None) => assert_eq!(row, "", "{label}"),
            Ok(Some(true)) => assert!(row.contains(" registered ("), "{label}: {row}"),
            Ok(Some(false)) => assert!(row.contains(" not registered ("), "{label}: {row}"),
            Err(report) => {
                assert!(row.contains(" unreadable: "), "{label}: {row}");
                assert_eq!(report.code(), "agent-configs-parse", "{label}");
            }
        }
    }
}

// ------------------------------------------------------------ the rest

fn files() -> (TempDir, AgentConfigFiles) {
    let home = tempfile::tempdir().unwrap();
    let configs = AgentConfigFiles {
        home: Some(home.path().to_path_buf()),
        path: None,
        waits: file_lock::Waits::DEFAULT,
    };
    (home, configs)
}

#[test]
fn test_claude_code_is_the_one_agent_and_its_files_are_gos() {
    let (home, mut configs) = files();
    assert_eq!(configs.engines(), ["claude-code"]);
    assert_eq!(
        configs.config_path(CLAUDE_CODE, &Scope::User).unwrap(),
        home.path().join(".claude.json")
    );
    assert_eq!(
        configs
            .config_path(CLAUDE_CODE, &Scope::Project(PathBuf::from(".")))
            .unwrap(),
        PathBuf::from(".mcp.json"),
        "Go's filepath.Join"
    );
    let unknown = configs.config_path("codex", &Scope::User).unwrap_err();
    assert_eq!(unknown.code(), "agent-configs-unknown-engine");
    let path = home.path().join(".claude.json");
    let refused = configs
        .register("codex", &path, "taskloom", "taskloom", None, false)
        .unwrap_err();
    assert_eq!(refused.code(), "agent-configs-unknown-engine");
    let homeless = AgentConfigFiles {
        home: None,
        path: None,
        waits: file_lock::Waits::DEFAULT,
    }
    .config_path(CLAUDE_CODE, &Scope::User)
    .unwrap_err();
    assert_eq!(homeless.code(), "agent-configs-no-home");
}

#[test]
fn test_an_agent_is_present_where_its_file_or_its_directory_is() {
    let (home, mut configs) = files();
    let project = tempfile::tempdir().unwrap();
    let scope = Scope::Project(project.path().to_path_buf());
    assert!(!configs.present(CLAUDE_CODE, &Scope::User));
    assert!(!configs.present(CLAUDE_CODE, &scope));
    std::fs::create_dir(home.path().join(".claude")).unwrap();
    assert!(configs.present(CLAUDE_CODE, &Scope::User));
    std::fs::write(project.path().join(".mcp.json"), "{}").unwrap();
    assert!(configs.present(CLAUDE_CODE, &scope));
    assert!(!configs.present("codex", &Scope::User));
    let homeless = AgentConfigFiles {
        home: None,
        path: None,
        waits: file_lock::Waits::DEFAULT,
    };
    assert!(!homeless.clone().present(CLAUDE_CODE, &Scope::User));
    let other = tempfile::tempdir().unwrap();
    std::fs::write(other.path().join(".claude.json"), "{}").unwrap();
    let mut configs = AgentConfigFiles {
        home: Some(other.path().to_path_buf()),
        path: None,
        waits: file_lock::Waits::DEFAULT,
    };
    assert!(configs.present(CLAUDE_CODE, &Scope::User));
}

#[test]
fn test_a_dry_run_answers_the_document_and_writes_nothing() {
    let (home, mut configs) = files();
    let path = home.path().join(".claude.json");
    std::fs::write(&path, "{}").unwrap();
    let rewritten = configs
        .register(
            CLAUDE_CODE,
            &path,
            "taskloom",
            "taskloom",
            Some(&server()),
            true,
        )
        .unwrap();
    assert!(rewritten.changed);
    assert_eq!(
        rewritten.after,
        r#"{ "mcpServers": { "taskloom": { "args": ["mcp"], "command": "taskloom" } } }"#
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");
}

#[test]
fn test_a_rewrite_that_changes_nothing_writes_nothing() {
    let (home, mut configs) = files();
    let path = home.path().join(".claude.json");
    configs
        .register(
            CLAUDE_CODE,
            &path,
            "taskloom",
            "taskloom",
            Some(&server()),
            false,
        )
        .unwrap();
    let written = std::fs::metadata(&path).unwrap().modified().unwrap();
    std::fs::write(home.path().join("marker"), "").unwrap();
    let again = configs
        .register(
            CLAUDE_CODE,
            &path,
            "taskloom",
            "taskloom",
            Some(&server()),
            false,
        )
        .unwrap();
    assert!(!again.changed);
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        written
    );
    let entries: Vec<_> = std::fs::read_dir(home.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .filter(|name| name != ".ctxloom")
        .collect();
    assert_eq!(
        entries.len(),
        2,
        "no file staged and left behind: {entries:?}"
    );
}

#[cfg(unix)]
#[test]
fn test_a_write_keeps_the_files_permissions_and_a_new_file_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let (home, mut configs) = files();
    let made = home.path().join("deeper/.mcp.json");
    configs
        .register(
            CLAUDE_CODE,
            &made,
            "taskloom",
            "taskloom",
            Some(&server()),
            false,
        )
        .unwrap();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&made), 0o600);
    let kept = home.path().join(".claude.json");
    std::fs::write(&kept, "{}").unwrap();
    std::fs::set_permissions(&kept, std::fs::Permissions::from_mode(0o640)).unwrap();
    configs
        .register(
            CLAUDE_CODE,
            &kept,
            "taskloom",
            "taskloom",
            Some(&server()),
            false,
        )
        .unwrap();
    assert_eq!(mode(&kept), 0o640);
}

#[cfg(unix)]
#[test]
fn test_a_file_that_cannot_be_read_or_written_says_so() {
    let (home, mut configs) = files();
    let dir = home.path().join("a-directory");
    std::fs::create_dir(&dir).unwrap();
    let unreadable = configs
        .register(
            CLAUDE_CODE,
            &dir,
            "taskloom",
            "taskloom",
            Some(&server()),
            false,
        )
        .unwrap_err();
    assert_eq!(unreadable.code(), "agent-configs-read");
    let found = configs
        .installed(CLAUDE_CODE, &dir, "taskloom")
        .unwrap_err();
    assert_eq!(found.code(), "agent-configs-read");

    use std::os::unix::fs::PermissionsExt;
    let locked = home.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::write(locked.join(".mcp.json"), "{}").unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o500)).unwrap();
    let unwritable = configs.register(
        CLAUDE_CODE,
        &locked.join(".mcp.json"),
        "taskloom",
        "taskloom",
        Some(&server()),
        false,
    );
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(unwritable.unwrap_err().code(), "agent-configs-write");
    assert_eq!(
        std::fs::read_to_string(locked.join(".mcp.json")).unwrap(),
        "{}"
    );
}

#[test]
fn test_an_entry_running_another_program_is_refused_by_name() {
    let (home, mut configs) = files();
    let path = home.path().join(".claude.json");
    let src = r#"{"mcpServers": {"taskloom": {"command": "mywrapper"}}}"#;
    std::fs::write(&path, src).unwrap();
    let refused = configs
        .register(
            CLAUDE_CODE,
            &path,
            "taskloom",
            "taskloom",
            Some(&server()),
            false,
        )
        .unwrap_err();
    assert_eq!(refused.code(), "agent-configs-foreign");
    assert!(refused.to_string().contains("named taskloom"), "{refused}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), src);
}

#[test]
fn test_a_command_resolves_where_an_executable_of_its_name_is_on_the_path() {
    let bin = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let mut configs = AgentConfigFiles {
        home: None,
        path: Some(std::env::join_paths([other.path(), bin.path()]).unwrap()),
        waits: file_lock::Waits::DEFAULT,
    };
    assert!(!configs.resolvable("taskloom"));
    let program = bin.path().join("taskloom");
    std::fs::write(&program, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert!(!configs.resolvable("taskloom"), "not executable");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    assert!(configs.resolvable("taskloom"));
    std::fs::create_dir(other.path().join("dir")).unwrap();
    assert!(!configs.resolvable("dir"), "a directory runs nothing");
    let mut pathless = AgentConfigFiles {
        home: None,
        path: None,
        waits: file_lock::Waits::DEFAULT,
    };
    assert!(!pathless.resolvable("taskloom"));
}

#[test]
fn test_a_path_is_joined_and_cleaned_as_gos() {
    for (dir, want) in [
        (".", ".mcp.json"),
        ("./", ".mcp.json"),
        ("/home/me/", "/home/me/.mcp.json"),
        ("/home/me/./work/../x", "/home/me/x/.mcp.json"),
        ("../up", "../up/.mcp.json"),
        ("/..", "/.mcp.json"),
        ("a/../..", "../.mcp.json"),
    ] {
        assert_eq!(
            go_join(Path::new(dir), ".mcp.json"),
            PathBuf::from(want),
            "{dir}"
        );
    }
    assert_eq!(go_join(Path::new(""), ""), PathBuf::from("."));
}

#[test]
fn test_a_write_stages_beside_a_staging_file_left_by_another() {
    let (home, mut configs) = files();
    let path = home.path().join(".claude.json");
    // Staging names are `.<base>.<pid>.<n>.tmp`, and the base is
    // `.claude.json`: the first two are taken.
    let left = home
        .path()
        .join(format!("..claude.json.{}.0.tmp", std::process::id()));
    std::fs::write(&left, "someone's").unwrap();
    let next = home
        .path()
        .join(format!("..claude.json.{}.1.tmp", std::process::id()));
    std::fs::write(&next, "another's").unwrap();
    configs
        .register(
            CLAUDE_CODE,
            &path,
            "taskloom",
            "taskloom",
            Some(&server()),
            false,
        )
        .unwrap();
    assert_eq!(std::fs::read_to_string(&left).unwrap(), "someone's");
    assert_eq!(std::fs::read_to_string(&next).unwrap(), "another's");
    assert!(std::fs::read_to_string(&path).unwrap().contains("taskloom"));
}

#[test]
fn test_staging_gives_up_after_a_thousand_names_taken() {
    let dir = tempfile::tempdir().unwrap();
    let name = |n: u32| {
        dir.path()
            .join(format!(".f.{}.{n}.tmp", std::process::id()))
    };
    for n in 0..1000 {
        std::fs::write(name(n), "").unwrap();
    }
    let (staged, _) = super::fresh(dir.path(), "f").unwrap();
    assert_eq!(staged, name(1000), "the thousandth retry is the last");
    let refused = super::fresh(dir.path(), "f").unwrap_err();
    assert_eq!(refused.kind(), std::io::ErrorKind::AlreadyExists);
    assert!(!name(1001).exists(), "no retry past the thousandth");
}

// ------------------------------------------------------------ the lock

/// Whether some holder keeps `path` from being locked exclusively now.
fn held(path: &Path) -> bool {
    match std::fs::File::open(path) {
        Ok(file) => fs4::FileExt::try_lock(&file).is_err(),
        Err(_) => false,
    }
}

/// A home, a project directory, and the adapter over them waiting `wait`
/// for a held lock; the project's `.mcp.json`, and the sidecar Go's
/// `sessions.WithFileLock` takes for it, held here as another process
/// holding it would.
struct Locking {
    home: TempDir,
    _dir: TempDir,
    configs: AgentConfigFiles,
    path: PathBuf,
    sidecar: file_lock::path::Sidecar,
    holder: Option<std::fs::File>,
}

impl Locking {
    fn new(wait: std::time::Duration) -> Locking {
        let (home, dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let configs = AgentConfigFiles {
            home: Some(home.path().to_path_buf()),
            path: None,
            waits: file_lock::Waits {
                lock: wait,
                reap_interlock: wait,
            },
        };
        let path = dir.path().join(".mcp.json");
        let sidecar = file_lock::path::sidecar_for(home.path(), &path);
        assert_eq!(
            sidecar.lock.parent().unwrap(),
            home.path().join(".ctxloom/locks"),
            "Go's home lock directory"
        );
        std::fs::create_dir_all(sidecar.lock.parent().unwrap()).unwrap();
        let holder = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&sidecar.lock)
            .unwrap();
        fs4::FileExt::lock(&holder).unwrap();
        Locking {
            home,
            _dir: dir,
            configs,
            path,
            sidecar,
            holder: Some(holder),
        }
    }
}

#[test]
fn test_a_registration_waits_while_another_process_holds_the_files_lock_then_writes() {
    let mut at = Locking::new(std::time::Duration::from_secs(60));
    let interlock = at.sidecar.interlock.clone().unwrap();
    let (path, mut configs) = (at.path.clone(), at.configs.clone());
    let registering = std::thread::spawn(move || {
        configs.register(
            CLAUDE_CODE,
            &path,
            "taskloom",
            "taskloom",
            Some(&server()),
            false,
        )
    });
    // The registration has begun its lock once it holds the reap interlock
    // (shared); from then until the sidecar is let go it can write nothing.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !held(&interlock) {
        assert!(std::time::Instant::now() < deadline, "never began its lock");
        assert!(!registering.is_finished(), "{:?}", registering.join());
        std::thread::yield_now();
    }
    assert!(!at.path.exists(), "nothing written while the lock is held");
    at.holder = None;
    let rewritten = registering.join().unwrap().expect("written once let go");
    assert!(rewritten.changed);
    assert!(std::fs::read_to_string(&at.path)
        .unwrap()
        .contains("\"taskloom\""));
    assert!(!held(&at.sidecar.lock), "released after the write");
    assert!(!held(&interlock));
}

#[test]
fn test_a_registration_whose_file_stays_locked_past_the_wait_is_refused_and_writes_nothing() {
    let mut at = Locking::new(std::time::Duration::from_millis(30));
    std::fs::write(&at.path, "{}").unwrap();
    for server in [Some(server()), None] {
        let path = at.path.clone();
        let refused = at
            .configs
            .register(
                CLAUDE_CODE,
                &path,
                "taskloom",
                "taskloom",
                server.as_ref(),
                false,
            )
            .unwrap_err();
        assert_eq!(refused.code(), "file-lock-timeout");
        assert_eq!(std::fs::read_to_string(&at.path).unwrap(), "{}");
    }
    drop(at.holder.take());
    let path = at.path.clone();
    at.configs
        .register(
            CLAUDE_CODE,
            &path,
            "taskloom",
            "taskloom",
            Some(&server()),
            false,
        )
        .expect("written once let go");
    assert!(std::fs::read_to_string(&at.path)
        .unwrap()
        .contains("\"taskloom\""));
    drop(at.home);
}

#[test]
fn test_a_registration_with_no_home_has_no_lock_to_take_and_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut homeless = AgentConfigFiles {
        home: None,
        path: None,
        waits: file_lock::Waits::DEFAULT,
    };
    let path = dir.path().join(".mcp.json");
    let refused = homeless
        .register(
            CLAUDE_CODE,
            &path,
            "taskloom",
            "taskloom",
            Some(&server()),
            true,
        )
        .unwrap_err();
    assert_eq!(refused.code(), "agent-configs-no-home");
    assert!(!path.exists());
}
