//! Ported from Go's `internal/adapters/configload` tests (`findappdir_test.go`,
//! `layering_test.go`) at 860336c4e.

use file_system::{Perms, Rooted};
use os::Mode;

use angzarr_diagnostic::Diagnostic;

use super::*;

fn rooted() -> Arc<Rooted> {
    Arc::new(Rooted::temp().expect("a temporary root"))
}

fn dir(fs: &Rooted, path: &str) {
    fs.create_dir_all(Path::new(path), Mode::from_bits(0o755))
        .unwrap();
}

fn file(fs: &Rooted, path: &str, text: &str) {
    dir(fs, Path::new(path).parent().unwrap().to_str().unwrap());
    fs.write_atomic(
        Path::new(path),
        text.as_bytes(),
        Perms::Exact(Mode::from_bits(0o644)),
    )
    .unwrap();
}

fn around(root_env: Option<&str>, cwd: &str) -> Surroundings {
    Surroundings {
        root_env: root_env.map(ToString::to_string),
        cwd: PathBuf::from(cwd),
        home: PathBuf::from("/home/u"),
        temp_root: PathBuf::from("/tmp"),
    }
}

// findappdir_test.go: TestFindAppDirCtxloomRoot / resolves_under_valid_root_without_creating_it
#[test]
fn test_a_valid_ctxloom_root_names_the_project_and_nothing_is_created() {
    let fs = rooted();
    dir(&fs, "/work/proj");
    let found = find_app_dir(&*fs, &around(Some("/work/proj"), "/elsewhere"));
    assert_eq!(found.path, Path::new("/work/proj/.ctxloom"));
    assert_eq!(
        found.source,
        Source::Project,
        "a named root resolves as a project, not the home fallback"
    );
    assert!(
        !fs.exists(Path::new("/work/proj/.ctxloom")).unwrap(),
        "resolving config is a read: whatever first writes there makes it"
    );
}

// findappdir_test.go: invalid_root_falls_through_and_creates_nothing
#[test]
fn test_an_invalid_ctxloom_root_is_ignored_and_said() {
    let fs = rooted();
    let found = find_app_dir(&*fs, &around(Some("/work/missing"), "/elsewhere"));
    assert_ne!(found.path, Path::new("/work/missing/.ctxloom"));
    assert_eq!(found.ignored_root.as_deref(), Some("/work/missing"));
    assert!(!fs.exists(Path::new("/work/missing")).unwrap());
    // An empty CTXLOOM_ROOT is unset: nothing to say.
    let empty = find_app_dir(&*fs, &around(Some(""), "/elsewhere"));
    assert_eq!(empty.ignored_root, None);
}

// appdir.go walkUpForAppDir: the nearest .ctxloom up from the working directory
#[test]
fn test_the_project_is_the_nearest_ctxloom_up_from_the_working_directory() {
    let fs = rooted();
    dir(&fs, "/work/proj/.ctxloom");
    dir(&fs, "/work/proj/sub/deeper");
    let found = find_app_dir(&*fs, &around(None, "/work/proj/sub/deeper"));
    assert_eq!(found.path, Path::new("/work/proj/.ctxloom"));
    assert_eq!(found.source, Source::Project);
    // A .ctxloom that is a file is no app dir.
    let fs = rooted();
    file(&fs, "/work/proj/.ctxloom", "a file");
    dir(&fs, "/work/proj/sub");
    let found = find_app_dir(&*fs, &around(None, "/work/proj/sub"));
    assert_eq!(found.source, Source::Home);
}

// findappdir_test.go: TestFindAppDirBareTempDirDoesNotEscapeToSharedTempRoot:
// the walk stops at the temporary directory, whose own .ctxloom (shared
// scratch space) is never a project's
#[test]
fn test_the_walk_up_stops_at_the_temporary_directory() {
    let fs = rooted();
    dir(&fs, "/tmp/.ctxloom");
    dir(&fs, "/tmp/work/bare");
    let found = find_app_dir(&*fs, &around(None, "/tmp/work/bare"));
    assert_ne!(
        found.path,
        Path::new("/tmp/.ctxloom"),
        "the shared temp root's .ctxloom is no project's"
    );
    assert_eq!(found.source, Source::Home);
    assert_eq!(found.path, Path::new("/home/u/.ctxloom"));
}

// findappdir_test.go: TestLoad_EmptyHomeNonRepoCwd_CreatesNothing: the home
// fallback is the app dir, and finding it leaves no trace
#[test]
fn test_with_no_project_the_home_is_the_app_dir_and_nothing_is_created() {
    let fs = rooted();
    dir(&fs, "/work/bare");
    let found = find_app_dir(&*fs, &around(None, "/work/bare"));
    assert_eq!(
        found,
        AppDir {
            path: PathBuf::from("/home/u/.ctxloom"),
            source: Source::Home,
            ignored_root: None
        }
    );
    assert!(!fs.exists(Path::new("/home/u")).unwrap());
}

fn files_of(fs: Arc<Rooted>, app: AppDir) -> ConfigFilesFs {
    ConfigFilesFs::new(fs, app, Path::new("/home/u"))
}

fn project_app() -> AppDir {
    AppDir {
        path: PathBuf::from("/work/proj/.ctxloom"),
        source: Source::Project,
        ignored_root: None,
    }
}

// layering_test.go: TestLoad_ProjectInheritsHomeKeys: the layers are the
// home's, then the project's
#[test]
fn test_the_layers_are_the_homes_then_the_projects() {
    let fs = rooted();
    file(
        &fs,
        "/home/u/.ctxloom/config.yaml",
        "schema_version: 8\neditor:\n  command: home\n",
    );
    file(
        &fs,
        "/work/proj/.ctxloom/config.yaml",
        "schema_version: 8\nworkspace: worktree\n",
    );
    let layers = files_of(fs, project_app()).layers().unwrap();
    let paths: Vec<&str> = layers.iter().map(|l| l.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "/home/u/.ctxloom/config.yaml",
            "/work/proj/.ctxloom/config.yaml"
        ]
    );
    assert!(layers[0].text.contains("command: home"));
    assert!(layers[1].text.contains("workspace: worktree"));
}

// loadConfigLayer: a layer that is not there contributes nothing
#[test]
fn test_a_layer_that_is_not_there_is_left_out() {
    let fs = rooted();
    file(
        &fs,
        "/work/proj/.ctxloom/config.yaml",
        "schema_version: 8\n",
    );
    let layers = files_of(fs.clone(), project_app()).layers().unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].path, "/work/proj/.ctxloom/config.yaml");
    let none = files_of(rooted(), project_app()).layers().unwrap();
    assert!(none.is_empty());
}

// layering_test.go: TestLoad_NoProjectFound_HomeIsSingleSource and
// TestResolveConfigLayerPaths_DedupsWhenHomeEqualsProject
#[test]
fn test_with_no_project_the_home_is_the_only_layer_and_never_twice() {
    let fs = rooted();
    file(&fs, "/home/u/.ctxloom/config.yaml", "schema_version: 8\n");
    let home_app = AppDir {
        path: PathBuf::from("/home/u/.ctxloom"),
        source: Source::Home,
        ignored_root: None,
    };
    let layers = files_of(fs.clone(), home_app).layers().unwrap();
    assert_eq!(layers.len(), 1);
    // A project whose app dir IS the home's: one file, one layer.
    let same = AppDir {
        path: PathBuf::from("/home/u/.ctxloom"),
        source: Source::Project,
        ignored_root: None,
    };
    assert_eq!(files_of(fs, same).layers().unwrap().len(), 1);
}

// layers.go: the app dir shown
#[test]
fn test_the_app_dir_is_shown_as_found() {
    assert_eq!(
        files_of(rooted(), project_app()).app_dir(),
        "/work/proj/.ctxloom"
    );
}

// a config that is not text is refused naming it
#[test]
fn test_a_config_that_is_not_text_is_refused_naming_it() {
    let fs = rooted();
    dir(&fs, "/work/proj/.ctxloom");
    fs.write_atomic(
        Path::new("/work/proj/.ctxloom/config.yaml"),
        &[0xff, 0xfe, 0x00],
        Perms::Exact(Mode::from_bits(0o644)),
    )
    .unwrap();
    let err = files_of(fs, project_app()).layers().expect_err("not UTF-8");
    assert_eq!(err.code(), "config-files-not-text");
}

// layering_test.go: a CTXLOOM_ROOT that named nothing is said, by what it
// said; with none to say there is nothing to say.
#[test]
fn test_the_ignored_root_is_reported_as_found() {
    let ignored = AppDir {
        ignored_root: Some("/nowhere".to_string()),
        ..project_app()
    };
    assert_eq!(
        files_of(rooted(), ignored).ignored_root().as_deref(),
        Some("/nowhere")
    );
    assert_eq!(files_of(rooted(), project_app()).ignored_root(), None);
}

// loadConfigLayer: a file that is there and cannot be read is an error naming
// it and the OS's words, not a layer left out.
#[test]
fn test_a_config_that_cannot_be_read_is_refused_with_the_systems_words() {
    let fs = rooted();
    dir(&fs, "/work/proj/.ctxloom/config.yaml");
    let err = files_of(fs, project_app())
        .layers()
        .expect_err("a directory is no file");
    assert_eq!(err.code(), "config-files-read");
    let args = err.args();
    assert_eq!(args[0].0, "path");
    assert_eq!(args[1].0, "ext_err_txt");
    assert!(err.to_string().contains("config-files-read"));
}

// The errors say their code and their args.
#[test]
fn test_the_errors_carry_their_args_and_their_samples_are_all_of_them() {
    let read = ConfigFileError::Read {
        path: "/p".to_string(),
        detail: "denied".to_string(),
    };
    assert_eq!(read.args().len(), 2);
    assert!(read.to_string().contains("config-files-read"));
    let not_text = ConfigFileError::NotText {
        path: "/p".to_string(),
    };
    assert_eq!(not_text.args().len(), 1);
    assert!(not_text.to_string().contains("config-files-not-text"));
    let codes: Vec<String> = diagnostic_samples()
        .iter()
        .map(|d| d.code().to_string())
        .collect();
    assert_eq!(
        codes,
        [
            "config-files-read",
            "config-files-not-text",
            "config-files-write"
        ]
    );
}

// ---------------------------------------------------------------- the write side (S2)

#[test]
fn test_each_layers_file_is_where_it_is_shown() {
    let files = files_of(rooted(), project_app());
    assert_eq!(
        files.layer_path(Scope::Project),
        "/work/proj/.ctxloom/config.yaml"
    );
    assert_eq!(
        files.layer_path(Scope::Home),
        "/home/u/.ctxloom/config.yaml"
    );
    // With no project there is one file for both.
    let home_only = files_of(
        rooted(),
        AppDir {
            path: PathBuf::from("/home/u/.ctxloom"),
            source: Source::Home,
            ignored_root: None,
        },
    );
    assert_eq!(
        home_only.layer_path(Scope::Project),
        "/home/u/.ctxloom/config.yaml"
    );
    assert_eq!(
        home_only.layer_path(Scope::Home),
        "/home/u/.ctxloom/config.yaml"
    );
}

#[test]
fn test_a_layer_is_read_by_scope_and_none_where_it_is_not_there() {
    let fs = rooted();
    file(&fs, "/home/u/.ctxloom/config.yaml", "home: 1\n");
    let files = files_of(fs, project_app());
    assert_eq!(
        files.layer(Scope::Home).unwrap(),
        Some(LayerText {
            path: "/home/u/.ctxloom/config.yaml".to_string(),
            text: "home: 1\n".to_string()
        })
    );
    assert_eq!(files.layer(Scope::Project).unwrap(), None);
}

#[test]
fn test_a_layer_that_is_not_text_or_cannot_be_read_is_refused() {
    let fs = rooted();
    dir(&fs, "/work/proj/.ctxloom");
    fs.write_atomic(
        Path::new("/work/proj/.ctxloom/config.yaml"),
        &[0xff, 0xfe, 0x00],
        Perms::Exact(Mode::from_bits(0o644)),
    )
    .unwrap();
    dir(&fs, "/home/u/.ctxloom/config.yaml");
    let files = files_of(fs, project_app());
    assert_eq!(
        files.layer(Scope::Project).unwrap_err().code(),
        "config-files-not-text"
    );
    assert_eq!(
        files.layer(Scope::Home).unwrap_err().code(),
        "config-files-read"
    );
}

#[test]
fn test_a_layer_is_written_whole_and_its_directory_made() {
    let fs = rooted();
    let files = files_of(fs.clone(), project_app());
    files
        .write_layer(Scope::Project, "schema_version: 8\n")
        .unwrap();
    assert_eq!(
        fs.read(Path::new("/work/proj/.ctxloom/config.yaml"))
            .unwrap(),
        b"schema_version: 8\n"
    );
    assert_eq!(
        fs.metadata(Path::new("/work/proj/.ctxloom/config.yaml"))
            .unwrap()
            .mode,
        Some(Mode::from_bits(0o644))
    );
    files.write_layer(Scope::Project, "x: 1\n").unwrap();
    assert_eq!(
        fs.read(Path::new("/work/proj/.ctxloom/config.yaml"))
            .unwrap(),
        b"x: 1\n"
    );
    let names: Vec<String> = fs
        .read_dir(Path::new("/work/proj/.ctxloom"))
        .unwrap()
        .into_iter()
        .map(|e| e.name.to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["config.yaml"], "no temporary file is left");
}

#[test]
fn test_the_home_layer_is_written_in_the_home_and_the_projects_is_left_alone() {
    let fs = rooted();
    file(&fs, "/work/proj/.ctxloom/config.yaml", "project: 1\n");
    let files = files_of(fs.clone(), project_app());
    files
        .write_layer(Scope::Home, "companions:\n  - acme\n")
        .unwrap();
    assert_eq!(
        fs.read(Path::new("/home/u/.ctxloom/config.yaml")).unwrap(),
        b"companions:\n  - acme\n"
    );
    assert_eq!(
        fs.read(Path::new("/work/proj/.ctxloom/config.yaml"))
            .unwrap(),
        b"project: 1\n"
    );
}

#[test]
fn test_a_rewrite_keeps_the_mode_the_file_had() {
    let fs = rooted();
    file(&fs, "/work/proj/.ctxloom/config.yaml", "a: 1\n");
    fs.set_permissions(
        Path::new("/work/proj/.ctxloom/config.yaml"),
        Mode::from_bits(0o600),
    )
    .unwrap();
    files_of(fs.clone(), project_app())
        .write_layer(Scope::Project, "a: 2\n")
        .unwrap();
    assert_eq!(
        fs.metadata(Path::new("/work/proj/.ctxloom/config.yaml"))
            .unwrap()
            .mode,
        Some(Mode::from_bits(0o600))
    );
}

#[test]
fn test_a_layer_that_cannot_be_written_is_refused_with_the_systems_words() {
    let fs = rooted();
    // A directory where the file would go.
    dir(&fs, "/work/proj/.ctxloom/config.yaml");
    let err = files_of(fs, project_app())
        .write_layer(Scope::Project, "a: 1\n")
        .unwrap_err();
    assert_eq!(err.code(), "config-files-write");
    assert_eq!(err.args()[0].0, "path");
    assert_eq!(err.args()[1].0, "ext_err_txt");
}

// ---------------------------------------------------------------- the scaffold (S2)

fn scaffold() -> (Arc<Rooted>, ScaffoldFs) {
    let fs = rooted();
    dir(&fs, "/work/proj");
    let scaffold = ScaffoldFs::new(fs.clone(), Path::new("/work/proj"));
    (fs, scaffold)
}

// cli/init.go resolveAppDir(false): under the working directory
#[test]
fn test_a_new_project_is_made_under_the_working_directory() {
    let (_fs, scaffold) = scaffold();
    assert_eq!(scaffold.app_dir(), "/work/proj/.ctxloom");
}

#[test]
fn test_the_scaffold_writes_its_files_and_their_directories() {
    let (fs, scaffold) = scaffold();
    assert!(!scaffold.exists("config.yaml").unwrap());
    scaffold
        .write(&[
            Put::file("config.yaml", "schema_version: 8\n"),
            Put::file("content/bundles/v2/project/bundle.yaml", "b"),
        ])
        .unwrap();
    assert!(scaffold.exists("config.yaml").unwrap());
    assert!(scaffold.exists("content/bundles/v2/project").unwrap());
    assert!(!scaffold.exists("remotes.yaml").unwrap());
    assert_eq!(
        fs.read(Path::new(
            "/work/proj/.ctxloom/content/bundles/v2/project/bundle.yaml"
        ))
        .unwrap(),
        b"b"
    );
    let meta = fs
        .metadata(Path::new("/work/proj/.ctxloom/config.yaml"))
        .unwrap();
    assert_eq!(meta.mode, Some(Mode::from_bits(0o644)));
    assert_eq!(
        fs.metadata(Path::new("/work/proj/.ctxloom")).unwrap().mode,
        Some(Mode::from_bits(0o755))
    );
}

#[test]
fn test_the_scaffold_refuses_a_path_that_leaves_the_project() {
    let (fs, scaffold) = scaffold();
    for bad in [
        "../x",
        "/abs",
        "a/../../b",
        "",
        ".",
        "a//b",
        "./a",
        "a/./b",
        "a\0b",
        "..",
    ] {
        let err = scaffold
            .write(&[Put::file("ok", "1"), Put::file(bad, "evil")])
            .unwrap_err();
        assert_eq!(err.code(), "config-files-write", "{bad:?}");
    }
    assert!(!fs.exists(Path::new("/work/proj/.ctxloom/ok")).unwrap());
    assert!(scaffold.exists("../x").is_err());
    for bad in ["", "/abs", "a\0b", "a//b", "a/./b", "a/../b"] {
        assert!(scaffold.exists(bad).is_err(), "{bad:?}");
    }
    assert!(scaffold.exists("fine/path").is_ok());
}

#[test]
fn test_the_scaffold_names_the_systems_words_when_it_cannot_write() {
    let (fs, scaffold) = scaffold();
    dir(&fs, "/work/proj/.ctxloom/config.yaml");
    let err = scaffold
        .write(&[Put::file("config.yaml", "a")])
        .unwrap_err();
    assert_eq!(err.code(), "config-files-write");
    assert_eq!(err.args()[1].0, "ext_err_txt");
}

// ---------------------------------------------------------------- characterization
//
// Pinned before the root rule may move (A0): every branch of finding the app
// dir, and what it carries along.

#[test]
fn test_a_valid_root_beats_a_nearer_app_dir() {
    let fs = rooted();
    dir(&fs, "/named");
    dir(&fs, "/work/proj/.ctxloom");
    let found = find_app_dir(&*fs, &around(Some("/named"), "/work/proj"));
    assert_eq!(found.path, Path::new("/named/.ctxloom"));
    assert_eq!(found.source, Source::Project);
    assert_eq!(found.ignored_root, None);
}

#[test]
fn test_a_root_that_is_a_file_is_ignored_as_one_that_is_missing() {
    let fs = rooted();
    file(&fs, "/a-file", "x");
    let found = find_app_dir(&*fs, &around(Some("/a-file"), "/elsewhere"));
    assert_eq!(found.ignored_root.as_deref(), Some("/a-file"));
    assert_eq!(found.source, Source::Home);
    // a relative name that is no directory is ignored the same way
    let found = find_app_dir(&*fs, &around(Some("rel/none"), "/elsewhere"));
    assert_eq!(found.ignored_root.as_deref(), Some("rel/none"));
}

#[test]
fn test_an_ignored_root_is_carried_to_a_project_found_by_walking_and_to_the_home() {
    let fs = rooted();
    dir(&fs, "/work/proj/.ctxloom");
    let walked = find_app_dir(&*fs, &around(Some("/missing"), "/work/proj/sub"));
    assert_eq!(
        walked,
        AppDir {
            path: PathBuf::from("/work/proj/.ctxloom"),
            source: Source::Project,
            ignored_root: Some("/missing".to_string()),
        }
    );
    let home = find_app_dir(&*fs, &around(Some("/missing"), "/bare"));
    assert_eq!(
        home,
        AppDir {
            path: PathBuf::from("/home/u/.ctxloom"),
            source: Source::Home,
            ignored_root: Some("/missing".to_string()),
        }
    );
}

#[test]
fn test_the_nearest_app_dir_wins_and_the_working_directory_counts() {
    let fs = rooted();
    dir(&fs, "/work/.ctxloom");
    dir(&fs, "/work/proj/.ctxloom");
    dir(&fs, "/work/proj/sub/.ctxloom");
    let found = find_app_dir(&*fs, &around(None, "/work/proj/sub"));
    assert_eq!(found.path, Path::new("/work/proj/sub/.ctxloom"));
    let found = find_app_dir(&*fs, &around(None, "/work/proj/sub/deeper"));
    assert_eq!(found.path, Path::new("/work/proj/sub/.ctxloom"));
}

#[test]
fn test_the_walk_stops_at_the_temporary_directory_itself_and_below_it_only() {
    let fs = rooted();
    dir(&fs, "/tmp/.ctxloom");
    // standing in the temp dir: its own app dir is not a project's
    let found = find_app_dir(&*fs, &around(None, "/tmp"));
    assert_eq!(found.source, Source::Home);
    // a project below it is found, though the temp dir has one too
    dir(&fs, "/tmp/p/.ctxloom");
    let found = find_app_dir(&*fs, &around(None, "/tmp/p/deep"));
    assert_eq!(found.path, Path::new("/tmp/p/.ctxloom"));
    // the temp dir's spelling does not matter
    let mut spelled = around(None, "/tmp/work");
    spelled.temp_root = PathBuf::from("/tmp/");
    assert_eq!(find_app_dir(&*fs, &spelled).source, Source::Home);
}

#[test]
fn test_the_walk_is_not_stopped_by_the_home_so_the_homes_app_dir_is_a_project() {
    // Go parity, not a defect: Go's walkUpForAppDir (ctxloom
    // internal/adapters/configload/appdir.go:81) stops only at the temp
    // directory, never at the home, so a working directory under the home
    // finds the home's own `.ctxloom` on the way up, as a Project, not a
    // Home. Do not stop the walk at the home without changing Go first.
    // (The tests use an in-memory fs, and the wiring's real-fs tests give
    // themselves a HOME and walk no further than the temp directory, so a
    // TMPDIR under the real $HOME never reaches the real `~/.ctxloom`.)
    let fs = rooted();
    dir(&fs, "/home/u/.ctxloom");
    dir(&fs, "/home/u/code/x");
    let found = find_app_dir(&*fs, &around(None, "/home/u/code/x"));
    assert_eq!(found.path, Path::new("/home/u/.ctxloom"));
    assert_eq!(found.source, Source::Project);
}

#[test]
fn test_an_empty_root_is_unset_and_a_root_only_counts_as_a_directory() {
    let fs = rooted();
    dir(&fs, "/work/proj/.ctxloom");
    let found = find_app_dir(&*fs, &around(Some(""), "/work/proj"));
    assert_eq!(found.path, Path::new("/work/proj/.ctxloom"));
    assert_eq!(found.ignored_root, None);
}

#[test]
fn test_the_names_of_the_app_dir_and_its_config_file() {
    assert_eq!(APP_DIR_NAME, ".ctxloom");
    assert_eq!(CONFIG_FILE_NAME, "config.yaml");
}
