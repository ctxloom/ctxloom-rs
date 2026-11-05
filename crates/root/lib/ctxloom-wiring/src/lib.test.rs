// The characterization pins set the process's own environment and working
// directory, to pin what the program takes of them when it reads them once.
#![allow(clippy::disallowed_methods)]

use harp_core::{ADJECTIVES, NOUNS};

use super::{run_id, session_name};

const SESSION: &str = "0197a5c0-1b6e-7a41-8c3d-5e2f4a9b7d10";

#[test]
fn test_a_session_name_is_two_adjectives_and_a_noun() {
    for _ in 0..20 {
        let name = session_name();
        let words: Vec<&str> = name.split('-').collect();
        assert_eq!(words.len(), 3, "{name}");
        assert!(ADJECTIVES.contains(&words[0]) && ADJECTIVES.contains(&words[1]));
        assert!(NOUNS.contains(&words[2]));
    }
}

#[test]
fn test_a_run_id_is_uuid_v5_of_the_session_and_the_incarnation() {
    let id = run_id(SESSION, 1).unwrap();
    assert_eq!(id.get_version_num(), 5);
    assert_eq!(run_id(SESSION, 1).unwrap(), id);
    assert_ne!(run_id(SESSION, 2).unwrap(), id);
}

#[test]
fn test_a_session_that_is_no_uuid_has_no_run() {
    assert!(run_id("jolly-mute-magma", 1).is_err());
}

// ---------------------------------------------------------------- characterization
//
// Pinned before the process's environment is read in one place (A0): every
// `std::env` read the wiring makes, what it does with an unset, empty or
// non-UTF-8 value, and what each is read for.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use ctxloom_ports::{Executables, Source};
use ports::ProjectIdentity;

use std::sync::Arc;

use ::process_env::EnvSnapshot;

use super::{LibError, Wiring};
use crate::fixtures::{canonical, exists, mkdirs, put_mode, remove_dir, root, unix};

/// The process's environment is one: tests that set it take this first.
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A hold on the process's environment and working directory: what a test
/// sets is put back when it ends, however it ends.
struct Env {
    _root: file_system::Rooted,
    saved: Vec<(OsString, Option<OsString>)>,
    cwd: Option<PathBuf>,
    dir: PathBuf,
    _held: std::sync::MutexGuard<'static, ()>,
}

impl Env {
    /// A world of the test's own: `<dir>/home` as HOME, and none of the
    /// variables the wiring reads set.
    fn new(_name: &str) -> Env {
        let held = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let temp = root();
        let dir = temp.root().to_path_buf();
        mkdirs(dir.join("home"));
        mkdirs(dir.join("work"));
        let mut env = Env {
            _root: temp,
            saved: Vec::new(),
            cwd: std::env::current_dir().ok(),
            dir,
            _held: held,
        };
        env.set("HOME", env.dir.join("home"));
        for var in [
            "CTXLOOM_ROOT",
            "CTXLOOM_PROJECT_ID",
            "CTXLOOM_DEGRADED",
            "PATH",
        ] {
            env.unset(var);
        }
        std::env::set_current_dir(env.dir.join("work")).unwrap();
        env
    }

    fn keep(&mut self, var: &str) {
        if !self.saved.iter().any(|(v, _)| v == var) {
            self.saved.push((var.into(), std::env::var_os(var)));
        }
    }

    fn set(&mut self, var: &str, value: impl AsRef<OsStr>) {
        self.keep(var);
        std::env::set_var(var, value);
    }

    fn unset(&mut self, var: &str) {
        self.keep(var);
        std::env::remove_var(var);
    }

    fn work(&self) -> PathBuf {
        self.dir.join("work")
    }

    fn home(&self) -> PathBuf {
        self.dir.join("home")
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        if let Some(cwd) = &self.cwd {
            let _ = std::env::set_current_dir(cwd);
        }
        for (var, value) in &self.saved {
            match value {
                Some(v) => std::env::set_var(var, v),
                None => std::env::remove_var(var),
            }
        }
    }
}

/// The wiring over the process's environment as it is now: read once, as the
/// composition root reads it.
fn from_process() -> Result<Wiring, LibError> {
    Wiring::new(Arc::new(EnvSnapshot::from_process()))
}

/// The library over the process's environment as it is now.
fn wired_library(
    remotes: Option<std::rc::Rc<dyn deps_ports::RemoteLibrary>>,
) -> Result<library_application::Library, LibError> {
    crate::library::build(&from_process()?, remotes)
}

#[test]
fn test_the_home_is_the_home_variable_as_given() {
    let env = Env::new("home");
    let wiring = from_process().ok().unwrap();
    assert_eq!(wiring.home(), env.home());
}

#[test]
fn test_no_home_is_refused_naming_the_variable_whether_unset_or_empty() {
    let mut env = Env::new("nohome");
    let variable = os::host().flavor().home_var().to_string();
    env.unset(&variable);
    assert_eq!(
        from_process().err(),
        Some(LibError::NoHome {
            variable: variable.clone()
        })
    );
    env.set(&variable, "");
    assert_eq!(from_process().err(), Some(LibError::NoHome { variable }));
}

#[test]
fn test_a_home_that_is_not_utf_8_is_taken_as_it_is() {
    let mut env = Env::new("homebytes");
    let odd = os::not_unicode("/h");
    env.set("HOME", &odd);
    let wiring = from_process().ok().unwrap();
    assert_eq!(wiring.home().as_os_str(), odd.as_os_str());
}

#[test]
fn test_the_wiring_is_made_though_the_working_directory_is_gone_and_surroundings_say_dot() {
    let env = Env::new("nocwd");
    let gone = env.dir.join("gone");
    mkdirs(&gone);
    std::env::set_current_dir(&gone).unwrap();
    remove_dir(&gone);
    let wiring = from_process().expect("only what needs the cwd fails");
    assert_eq!(wiring.surroundings().cwd, Path::new("."));
}

#[test]
fn test_the_surroundings_are_the_root_variable_the_cwd_the_home_and_the_temp_dir() {
    let mut env = Env::new("around");
    env.set("TMPDIR", env.dir.join("tmp"));
    let wiring = from_process().ok().unwrap();
    let around = wiring.surroundings();
    assert_eq!(around.root_env, None, "unset");
    assert_eq!(around.cwd, std::env::current_dir().unwrap());
    assert_eq!(around.home, env.home());
    assert_eq!(around.temp_root, env.dir.join("tmp"));
    // The environment is read once: a wiring made before the change does not
    // see it, one made after does.
    env.set("CTXLOOM_ROOT", "");
    assert_eq!(wiring.surroundings().root_env, None, "a snapshot");
    assert_eq!(
        from_process()
            .ok()
            .unwrap()
            .surroundings()
            .root_env
            .as_deref(),
        Some(""),
        "empty is passed on as given: finding the app dir treats it as unset"
    );
    env.set("CTXLOOM_ROOT", "/some/where");
    assert_eq!(
        from_process()
            .ok()
            .unwrap()
            .surroundings()
            .root_env
            .as_deref(),
        Some("/some/where")
    );
}

#[test]
fn test_a_root_variable_that_is_not_utf_8_reads_as_unset() {
    let mut env = Env::new("rootbytes");
    env.set("CTXLOOM_ROOT", os::not_unicode("/"));
    let wiring = from_process().ok().unwrap();
    assert_eq!(wiring.surroundings().root_env, None);
}

#[test]
fn test_locating_with_no_root_and_no_app_dir_is_the_homes() {
    let env = Env::new("locate-home");
    let wiring = from_process().ok().unwrap();
    let (_, app) = wiring.locate();
    assert_eq!(app.path, env.home().join(".ctxloom"));
    assert_eq!(app.source, Source::Home);
    assert_eq!(app.ignored_root, None);
}

#[test]
fn test_locating_walks_up_from_the_cwd_to_the_nearest_app_dir() {
    let env = Env::new("locate-walk");
    mkdirs(env.work().join(".ctxloom"));
    mkdirs(env.work().join("a/b"));
    std::env::set_current_dir(env.work().join("a/b")).unwrap();
    let wiring = from_process().ok().unwrap();
    let (_, app) = wiring.locate();
    assert_eq!(app.source, Source::Project);
    assert_eq!(canonical(&app.path), canonical(env.work().join(".ctxloom")));
}

#[test]
fn test_locating_stops_at_the_temp_dir_the_variable_names() {
    let mut env = Env::new("locate-tmp");
    // the temporary directory is `work` itself, and has an app dir: not a project
    mkdirs(env.work().join(".ctxloom"));
    mkdirs(env.work().join("sub"));
    env.set("TMPDIR", env.work());
    std::env::set_current_dir(env.work().join("sub")).unwrap();
    let wiring = from_process().ok().unwrap();
    let (_, app) = wiring.locate();
    assert_eq!(app.source, Source::Home);
}

#[test]
fn test_locating_honours_a_root_that_is_a_directory_and_says_one_that_is_not() {
    let mut env = Env::new("locate-root");
    env.set("CTXLOOM_ROOT", env.dir.join("home"));
    let wiring = from_process().ok().unwrap();
    let (_, app) = wiring.locate();
    assert_eq!(app.path, env.home().join(".ctxloom"));
    assert_eq!(app.source, Source::Project, "a named root is a project");
    let missing = env.dir.join("missing");
    env.set("CTXLOOM_ROOT", &missing);
    let wiring = from_process().ok().unwrap();
    let (_, app) = wiring.locate();
    assert_eq!(app.source, Source::Home);
    assert_eq!(app.ignored_root, Some(missing.display().to_string()));
}

/// An executable file `name` in `dir`.
fn executable(dir: &Path, name: &str, mode: u32) {
    mkdirs(dir);
    put_mode(dir.join(name), "#!/bin/sh\n", mode);
}

#[test]
fn test_programs_are_found_on_the_path_in_its_order_and_only_if_executable_files() {
    if !unix() {
        return;
    }
    let mut env = Env::new("path");
    let (first, second) = (env.dir.join("first"), env.dir.join("second"));
    executable(&first, "both", 0o644);
    executable(&first, "plain", 0o755);
    executable(&second, "both", 0o755);
    mkdirs(second.join("adir"));
    env.set(
        "PATH",
        std::env::join_paths([&first, &second]).expect("a path"),
    );
    let wiring = from_process().ok().unwrap();
    let search = wiring.path_search();
    assert_eq!(
        search.find("plain"),
        Some(first.join("plain").display().to_string())
    );
    assert_eq!(
        search.find("both"),
        Some(second.join("both").display().to_string()),
        "the first is no executable, so the second's is found"
    );
    assert_eq!(search.find("adir"), None, "a directory is no program");
    assert_eq!(search.find("nosuch"), None);
}

#[test]
fn test_with_no_path_variable_no_program_is_found() {
    let env = Env::new("nopath");
    let wiring = from_process().ok().unwrap();
    assert!(wiring.path_search().dirs.is_empty());
    drop(env);
}

#[test]
fn test_the_project_is_the_one_the_environment_pins_and_its_store_is_state_db_under_the_home() {
    let mut env = Env::new("pinned");
    env.set("CTXLOOM_PROJECT_ID", "pinned-by-env");
    let wiring = from_process().ok().unwrap();
    let mut sessions = wiring.sessions();
    let project = sessions.use_cases.identity.for_write(None).unwrap();
    assert_eq!(project.id, "pinned-by-env");
    assert_eq!(
        project.store,
        env.home().join(".ctxloom/projects/pinned-by-env/state.db"),
        "the identity's store is taskloom's; ctxloom's own is ctxloom.db beside it"
    );
}

#[test]
fn test_an_explicit_project_beats_the_environments() {
    let mut env = Env::new("pinned-arg");
    env.set("CTXLOOM_PROJECT_ID", "pinned-by-env");
    let wiring = from_process().ok().unwrap();
    let mut sessions = wiring.sessions();
    let project = sessions
        .use_cases
        .identity
        .for_write(Some("asked"))
        .unwrap();
    assert_eq!(project.id, "asked");
}

#[test]
fn test_an_empty_project_variable_pins_nothing_and_a_read_finds_no_project() {
    let mut env = Env::new("pinned-empty");
    env.set("CTXLOOM_PROJECT_ID", "");
    let wiring = from_process().ok().unwrap();
    let mut sessions = wiring.sessions();
    assert_eq!(sessions.use_cases.identity.for_read(None).unwrap(), None);
    let minted = sessions.use_cases.identity.for_write(None).unwrap();
    assert!(!minted.id.is_empty());
    assert_eq!(minted.id.split('-').count(), 3, "a minted harp id");
}

#[test]
fn test_a_project_variable_that_is_not_utf_8_pins_nothing() {
    let mut env = Env::new("pinned-bytes");
    env.set("CTXLOOM_PROJECT_ID", os::not_unicode("p"));
    let wiring = from_process().ok().unwrap();
    let mut sessions = wiring.sessions();
    assert_eq!(sessions.use_cases.identity.for_read(None).unwrap(), None);
}

#[test]
fn test_the_root_variable_makes_the_work_root_and_the_project_is_marked_there() {
    let mut env = Env::new("workroot");
    let root = env.dir.join("elsewhere");
    mkdirs(&root);
    env.set("CTXLOOM_ROOT", &root);
    let wiring = from_process().ok().unwrap();
    let mut sessions = wiring.sessions();
    let project = sessions.use_cases.identity.for_write(None).unwrap();
    assert_eq!(
        project.root.as_deref().map(Path::new),
        Some(canonical(&root).as_path())
    );
    assert!(exists(root.join(".project-id")), "marked at the root");
    assert!(
        !exists(env.work().join(".project-id")),
        "not in the working directory"
    );
}

#[test]
fn test_the_libraries_environment_is_the_whole_process_environment_at_the_time_of_wiring() {
    let mut env = Env::new("library-env");
    let library = wired_library(None).ok().unwrap();
    assert!(!library.degraded_by_environment());
    env.set("CTXLOOM_DEGRADED", "1");
    assert!(wired_library(None).ok().unwrap().degraded_by_environment());
    // a snapshot: the one built before does not see it
    assert!(!library.degraded_by_environment());
    for other in ["0", "true", "", "yes"] {
        env.set("CTXLOOM_DEGRADED", other);
        assert!(
            !wired_library(None).ok().unwrap().degraded_by_environment(),
            "{other:?}"
        );
    }
}

#[test]
fn test_a_config_override_in_the_environment_reaches_the_library() {
    let mut env = Env::new("library-override");
    env.set("CTXLOOM_CONFIG_NON_TTY_FORMAT", "text");
    let library = wired_library(None).ok().unwrap();
    assert_eq!(
        library
            .preferred_format(&library_application::Flags::default())
            .as_deref(),
        Some("text")
    );
    env.unset("CTXLOOM_CONFIG_NON_TTY_FORMAT");
    let library = wired_library(None).ok().unwrap();
    assert_eq!(
        library.preferred_format(&library_application::Flags::default()),
        None
    );
}

/// The bug the pin documented (the library's environment was read with
/// `std::env::vars`, which panics on any variable that is not UTF-8, so one
/// such variable anywhere in the process's environment took the whole program
/// down when the library was wired), fixed: the library is wired, the variable
/// is none to it (overrides are text), and every other variable still reaches
/// it.
#[test]
fn test_a_variable_that_is_not_utf_8_does_not_stop_the_library_being_wired() {
    let mut env = Env::new("library-bytes");
    env.set("A0_NOT_UTF8", os::not_unicode(""));
    env.set("CTXLOOM_DEGRADED", "1");
    env.set("CTXLOOM_CONFIG_NON_TTY_FORMAT", "text");
    let library = wired_library(None).expect("wired, though one variable is bytes");
    assert!(library.degraded_by_environment(), "the others are read");
    assert_eq!(
        library
            .preferred_format(&library_application::Flags::default())
            .as_deref(),
        Some("text")
    );
}

#[test]
fn test_opening_a_store_makes_it_under_the_home_and_names_its_database() {
    let env = Env::new("open-store");
    let wiring = from_process().ok().unwrap();
    let path = wiring.open_store("jolly-mute-magma").ok().unwrap();
    assert_eq!(
        path,
        env.home()
            .join(".ctxloom/projects/jolly-mute-magma/ctxloom.db")
    );
    assert!(exists(&path), "the database is made");
}

#[test]
fn test_opening_a_store_for_an_unsafe_project_id_is_refused_and_makes_nothing() {
    use angzarr_diagnostic::Diagnostic;
    let env = Env::new("open-store-unsafe");
    let wiring = from_process().ok().unwrap();
    let error = wiring.open_store("../escape").expect_err("refused");
    assert_eq!(error.code(), "ctxloom-wiring-store");
    assert!(
        matches!(&error, LibError::Store(inner) if inner.code() == "project-dir-id-separator"),
        "{error:?}"
    );
    assert!(!exists(env.home().join(".ctxloom")));
}

#[test]
fn test_the_catalog_has_the_shared_project_dir_messages_and_the_samples_carry_its_error() {
    let sources = crate::catalog_sources();
    assert!(
        sources.contains(&project_dir::EN_US),
        "project-dir's catalog"
    );
    assert!(
        sources.contains(&duration_text::EN_US),
        "duration-text's catalog"
    );
    assert!(sources.contains(&crate::EN_US), "this crate's own");
    assert!(
        sources.contains(&scheduler::EN_US),
        "the scheduler's catalog"
    );
    assert!(
        sources.contains(&scheduler_loop::EN_US),
        "the scheduler loop's catalog"
    );
    let codes: Vec<String> = crate::diagnostic_samples()
        .iter()
        .map(|d| d.code().to_string())
        .collect();
    assert!(
        codes.contains(&"ctxloom-wiring-store".to_string()),
        "{codes:?}"
    );
    assert!(
        codes.len() >= 5 && !crate::catalog_sources().is_empty(),
        "{codes:?}"
    );
}
