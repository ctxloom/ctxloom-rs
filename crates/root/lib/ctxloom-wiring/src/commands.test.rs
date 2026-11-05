//! The program's nouns: the library's, with the publishing verbs added to
//! `bundle`, and the remote and dependency ones.

use super::*;
use std::path::PathBuf;

fn verbs(noun: &Command) -> Vec<String> {
    noun.get_subcommands()
        .map(|c| c.get_name().to_string())
        .collect()
}

#[test]
fn test_the_nouns_are_the_librarys_and_the_dependency_ones() {
    let nouns = Commands::new().nouns();
    let names: Vec<&str> = nouns.iter().map(Command::get_name).collect();
    for noun in ["bundle", "fragment", "profile", "search", "remote", "deps"] {
        assert!(names.contains(&noun), "{noun} in {names:?}");
    }
    assert_eq!(names.iter().filter(|n| **n == "bundle").count(), 1);
}

#[test]
fn test_only_the_bundle_noun_gains_the_publishing_verbs() {
    let nouns = Commands::new().nouns();
    let find = |name: &str| nouns.iter().find(|n| n.get_name() == name).expect(name);
    let bundle = verbs(find("bundle"));
    assert!(bundle.contains(&"push".to_string()) && bundle.contains(&"move".to_string()));
    assert!(
        bundle.contains(&"list".to_string()),
        "the library's verbs stay: {bundle:?}"
    );
    for other in ["fragment", "profile"] {
        let verbs = verbs(find(other));
        assert!(
            !verbs.contains(&"push".to_string()) && !verbs.contains(&"move".to_string()),
            "{other}: {verbs:?}"
        );
    }
}

/// A process-bound world: HOME and the working directory in a directory of
/// the test's own, the root and project variables unset. The environment is
/// the process's, so one test holds it at a time and puts it back.
struct World {
    dir: PathBuf,
    saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
    cwd: PathBuf,
    _held: std::sync::MutexGuard<'static, ()>,
}

static WORLD: std::sync::Mutex<()> = std::sync::Mutex::new(());

impl World {
    fn new(name: &str) -> World {
        let held = WORLD.lock().unwrap_or_else(|e| e.into_inner());
        let dir =
            std::env::temp_dir().join(format!("ctxloom-wiring-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("home")).unwrap();
        std::fs::create_dir_all(dir.join("work")).unwrap();
        let cwd = std::env::current_dir().unwrap();
        let vars = [
            "HOME",
            "CTXLOOM_ROOT",
            "CTXLOOM_PROJECT_ID",
            "CLAUDE_CODE_OAUTH_TOKEN",
        ];
        let saved = vars.iter().map(|v| (*v, std::env::var_os(v))).collect();
        std::env::set_var("HOME", dir.join("home"));
        std::env::remove_var("CTXLOOM_ROOT");
        std::env::remove_var("CTXLOOM_PROJECT_ID");
        std::env::remove_var("CLAUDE_CODE_OAUTH_TOKEN");
        std::env::set_current_dir(dir.join("work")).unwrap();
        World {
            dir,
            saved,
            cwd,
            _held: held,
        }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.cwd);
        for (var, value) in &self.saved {
            match value {
                Some(v) => std::env::set_var(var, v),
                None => std::env::remove_var(var),
            }
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Run `line` through the extension; its exit, stdout and stderr.
fn run_line(commands: &mut Commands, line: &[&str]) -> (u8, String, String) {
    let program = Command::new("ctxloom")
        .subcommand_required(true)
        .subcommands(commands.nouns());
    let argv = std::iter::once("ctxloom").chain(line.iter().copied());
    let matches = program.try_get_matches_from(argv).unwrap();
    let noun = matches.subcommand_name().unwrap().to_string();
    let json = angzarr_presentation::Format::Json;
    let call = Call::new(&matches, Some(json), json);
    let catalog = Catalog::en_us(&[crate::EN_US]).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = commands
        .run(&noun, &call, &catalog, &mut out, &mut err)
        .unwrap_or_else(|f| panic!("failed: {}", f.cause));
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

/// The names of the files under `dir`, relative to it, sorted.
fn files_under(dir: &std::path::Path) -> Vec<String> {
    fn walk(root: &std::path::Path, at: &std::path::Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.push(path.strip_prefix(root).unwrap().display().to_string());
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

#[test]
fn test_the_engines_composed_in_are_claude_first_then_the_mocks() {
    assert_eq!(engines().names(), ["claude-code", "mock", "mock-launch"]);
}

#[test]
fn test_a_library_command_is_run_over_the_home() {
    let _world = World::new("library");
    let (code, out, err) = run_line(&mut Commands::new(), &["bundle", "list"]);
    assert_eq!((code, out.as_str(), err.as_str()), (0, "[]\n", ""));
}

#[test]
fn test_a_setup_command_is_run_over_the_home() {
    let world = World::new("setup");
    let (code, out, _) = run_line(&mut Commands::new(), &["clean"]);
    assert_eq!(code, 0);
    assert!(out.contains("\"applied\": false"), "{out}");
    let home = world.dir.join("home").display().to_string();
    assert!(
        out.contains(&format!("\"path\": \"{home}/.ctxloom/cache/bundles\"")),
        "{out}"
    );
}

// The sweep's purge age is read from the configuration of the project the
// process is in (`session_purge_age`), through the wired sessions.
#[test]
fn test_the_wired_sessions_read_the_purge_age_from_the_project_config() {
    use reclaim_application::SweepConfig;
    let world = World::new("purge-age");
    let mut sessions = crate::Wiring::from_process().unwrap().sessions();
    assert_eq!(sessions.purge_age().unwrap(), None);
    std::fs::create_dir_all(world.dir.join("work/.ctxloom")).unwrap();
    std::fs::write(
        world.dir.join("work/.ctxloom/config.yaml"),
        "schema_version: 8\nsession_purge_age: 45d\n",
    )
    .unwrap();
    let mut sessions = crate::Wiring::from_process().unwrap().sessions();
    assert_eq!(sessions.purge_age().unwrap(), Some("45d".to_string()));
}

// auth.feature, over the process's own environment.
#[test]
fn test_auth_status_reads_the_launch_environment_and_never_its_values() {
    let _world = World::new("auth");
    let (code, out, _) = run_line(&mut Commands::new(), &["auth", "status"]);
    assert_eq!(code, 0);
    assert!(out.contains("\"present\": false"), "{out}");
    std::env::set_var("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat01-fixture-not-real"); // gitleaks:allow
    let (code, out, err) = run_line(&mut Commands::new(), &["auth", "status"]);
    assert_eq!(code, 0);
    assert!(out.contains("\"present\": true"), "{out}");
    assert!(!out.contains("fixture-not-real") && !err.contains("fixture-not-real"));
}

// doctor.feature: the credentials row is read from the same environment.
#[test]
fn test_doctor_fails_without_the_token_and_passes_with_it() {
    let _world = World::new("doctor-credentials");
    let (code, out, _) = run_line(&mut Commands::new(), &["doctor"]);
    assert_eq!(code, 3);
    assert!(out.contains("DOCTOR-CHECK-CREDENTIALS-m5"), "{out}");
    std::env::set_var("CLAUDE_CODE_OAUTH_TOKEN", "x");
    let (code, _, _) = run_line(&mut Commands::new(), &["doctor"]);
    assert_eq!(code, 0);
}

#[test]
fn test_a_dependency_command_is_run_over_the_home() {
    let _world = World::new("deps");
    let (code, out, _) = run_line(&mut Commands::new(), &["remote", "list"]);
    assert_eq!(code, 0);
    assert!(!out.is_empty());
}

#[test]
fn test_the_hooks_answer_through_the_engines_codec() {
    use hook_cli::HookService;
    let _world = World::new("hooks");
    let mut commands = Commands::new();
    let hooks = commands.hooks().ok().unwrap();
    assert_eq!(
        hooks.session_start("claude-code", "{}").ok().unwrap(),
        "{}\n"
    );
    assert_eq!(
        hooks
            .skill_mates("claude-code", r#"{"skill":"x"}"#)
            .ok()
            .unwrap(),
        "{}\n"
    );
    assert!(hooks.session_start("nosuch", "{}").is_err());
    assert!(hooks.skill_mates("nosuch", "{}").is_err());
}

/// What a refresh preview names: each engine, what it would write and release.
fn planned(refresh: &Refresh) -> Vec<(String, Vec<String>, Vec<String>)> {
    refresh
        .engines
        .iter()
        .map(|e| (e.engine.clone(), e.wrote.clone(), e.released.clone()))
        .collect()
}

fn strings(of: &[&str]) -> Vec<String> {
    of.iter().map(ToString::to_string).collect()
}

#[test]
fn test_a_refresh_without_consent_previews_the_project_and_writes_nothing() {
    let _world = World::new("preview");
    let here = std::env::current_dir().unwrap().display().to_string();
    let mut commands = Commands::new();
    let wired = commands.wired().ok().unwrap();
    let refresh = SetupRefresh {
        setup: &wired.surface,
        flags: library_application::Flags::default(),
    };
    let preview = refresh.refresh(false).ok().unwrap();
    assert_eq!(preview.target, here);
    // the context of an engine that carries it on the launch is left alone
    assert_eq!(
        planned(&preview),
        [(
            "claude-code".to_string(),
            strings(&["settings", "hooks"]),
            vec![]
        )]
    );
    assert!(files_under(std::path::Path::new(&here)).is_empty());
}

#[test]
fn test_a_refresh_is_asked_for_the_flags_of_the_command() {
    let _world = World::new("flags");
    let mut commands = Commands::new();
    let wired = commands.wired().ok().unwrap();
    let refresh = SetupRefresh {
        setup: &wired.surface,
        flags: library_application::Flags {
            config_set: vec!["llm.configs.m.type=mock".to_string()],
            ..library_application::Flags::default()
        },
    };
    let preview = refresh.refresh(false).ok().unwrap();
    let engines: Vec<String> = planned(&preview).into_iter().map(|e| e.0).collect();
    assert_eq!(engines, ["mock"]);
}

#[test]
fn test_a_consented_refresh_writes_the_project_and_answers_its_target() {
    let _world = World::new("write");
    let here = std::env::current_dir().unwrap();
    let mut commands = Commands::new();
    let wired = commands.wired().ok().unwrap();
    let refresh = SetupRefresh {
        setup: &wired.surface,
        flags: library_application::Flags::default(),
    };
    let done = refresh.refresh(true).ok().unwrap();
    assert_eq!(done.target, here.display().to_string());
    assert!(done.engines.is_empty());
    assert!(!files_under(&here).is_empty());
}

const ORIGIN: &str = "file:///srv/origin.git";

/// The pull a manage install ends with, over fakes: `dev` drawing on
/// origin's demo bundle, which is not yet installed.
fn unpulled() -> (Arc<deps_application::fakes::FakeFiles>, DepsPuller) {
    use deps_application::fakes::Fixture;
    let fx = Fixture::new();
    fx.origin("origin", ORIGIN);
    fx.profile("dev", &[&Fixture::reference(ORIGIN, "demo")]);
    let Fixture { files, deps, .. } = fx;
    (files, DepsPuller(Rc::new(deps)))
}

#[test]
fn test_the_pull_answers_how_many_it_installed() {
    let (_, puller) = unpulled();
    assert_eq!(puller.pull(), Ok(1));
    assert_eq!(puller.pull(), Ok(0));
}

#[test]
fn test_a_pull_that_could_not_install_is_an_error_not_a_count() {
    let (files, puller) = unpulled();
    files.fail_installs(true);
    assert_eq!(puller.pull(), Err("1 could not be installed".to_string()));
}

#[test]
fn test_a_pull_that_is_refused_answers_the_refusal_code() {
    use deps_application::fakes::Fixture;
    let fx = Fixture::new();
    fx.profile(
        "dev",
        &[&Fixture::reference("file:///srv/stranger.git", "demo")],
    );
    let puller = DepsPuller(Rc::new(fx.deps));
    let said = puller.pull().unwrap_err();
    assert!(said.contains("not-registered"), "{said}");
}
