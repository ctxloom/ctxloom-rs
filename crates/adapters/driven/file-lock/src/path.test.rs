//! Where a lock lives, as Go names it.

use std::path::{Path, PathBuf};

use super::{absolute, clean, flat_name, sidecar_for, valid_harp, Sidecar};

const SCRATCH: &str =
    "/tmp/claude-1000/-home-babbitt-workspace-ctxloom-ctxloom-rs/74fb18c8-0651-4a53-83c8-19f8a2e256cc/scratchpad";

#[test]
fn test_a_flat_name_is_gos() {
    assert_eq!(
        flat_name(&format!("{SCRATCH}/tmp.L0iKUu4t5Y/home/.claude.json")),
        "ctxloom-rs__74fb18c8-0651-4a53-83c8-19f8a2e256cc__scratchpad__tmp.L0iKUu4t5Y__home__.claude.json__d07667d34aae78ed"
    );
    assert_eq!(
        flat_name(&format!("{SCRATCH}/tmp.qBAVn0XNgO/home/.claude.json")),
        "ctxloom-rs__74fb18c8-0651-4a53-83c8-19f8a2e256cc__scratchpad__tmp.qBAVn0XNgO__home__.claude.json__1865b30e2a6a57b4"
    );
}

#[test]
fn test_a_flat_name_escapes_what_ntfs_refuses_and_cuts_on_a_character() {
    let name = flat_name("C:/a|b\u{1}");
    assert!(name.starts_with("C%3A__a%7Cb%01__"), "{name}");
    assert_eq!(name.len(), "C%3A__a%7Cb%01__".len() + 16);
    let deep = format!("/{}", "\u{e9}".repeat(60));
    let name = flat_name(&deep);
    let tail = &name[..name.len() - 18];
    assert!(tail.len() <= 96, "{}", tail.len());
    assert!(tail.chars().all(|c| c == '\u{e9}'), "{tail}");
}

/// Go's `FlatName` gave each of these (its body run on them verbatim).
#[test]
fn test_a_flat_name_cut_inside_a_character_steps_forward_as_gos() {
    assert_eq!(
        flat_name(&format!("/{}x", "\u{e9}".repeat(60))),
        format!("{}x__e3ac857167b96b7b", "\u{e9}".repeat(47))
    );
    assert_eq!(
        flat_name(&"a".repeat(96)),
        format!("{}__ee4caa5518a866f3", "a".repeat(96))
    );
    assert_eq!(
        flat_name(&"a".repeat(97)),
        format!("{}__2a2c60fe96afac91", "a".repeat(96))
    );
}

#[test]
fn test_a_path_is_cleaned_as_gos_filepath_clean_cleans_it() {
    assert_eq!(clean(Path::new("/a/./b/../c/")), Path::new("/a/c"));
    assert_eq!(clean(Path::new("/../x")), Path::new("/x"));
    assert_eq!(clean(Path::new("../a/../../b")), Path::new("../../b"));
    assert_eq!(clean(Path::new("a/..")), Path::new("."));
    assert_eq!(clean(Path::new("")), Path::new("."));
}

#[test]
fn test_a_relative_path_is_made_absolute_against_the_working_directory() {
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(
        absolute(Path::new("a/./b/../.mcp.json")).unwrap(),
        cwd.join("a/.mcp.json")
    );
    assert_eq!(absolute(Path::new("/x/../y")).unwrap(), Path::new("/y"));
}

/// Go's own `paths.HomePathFor` and `LockReapInterlockPath`, run with
/// HOME at /home/u, gave each of these.
#[test]
fn test_a_file_outside_every_session_is_locked_in_the_home_lock_directory() {
    let home = Path::new("/home/u");
    assert_eq!(
        sidecar_for(home, Path::new("/home/u/work/.mcp.json")),
        Sidecar {
            lock: PathBuf::from(
                "/home/u/.ctxloom/locks/__home__u__work__.mcp.json__54e9de5e9d028e9c.lock"
            ),
            interlock: Some(PathBuf::from("/home/u/.ctxloom/locks/reap.interlock")),
        }
    );
    assert_eq!(
        sidecar_for(home, Path::new("/home/u/.claude.json")).lock,
        Path::new("/home/u/.ctxloom/locks/__home__u__.claude.json__93b778aeca6eb169.lock")
    );
}

#[test]
fn test_a_file_inside_a_session_is_locked_in_that_sessions_own_lock_directory() {
    let home = Path::new("/home/u");
    let target = Path::new("/home/u/.ctxloom/sessions/jolly-mute-magma/work/.mcp.json");
    assert_eq!(
        sidecar_for(home, target),
        Sidecar {
            lock: PathBuf::from(
                "/home/u/.ctxloom/sessions/jolly-mute-magma/locks/work__.mcp.json__2087cb7edc82acdc.lock"
            ),
            interlock: None,
        }
    );
}

#[test]
fn test_the_sessions_root_a_harp_dir_and_the_files_beside_them_are_in_no_session() {
    let home = Path::new("/home/u");
    assert_eq!(
        sidecar_for(home, Path::new("/home/u/.ctxloom/sessions/jolly-mute-magma")).lock,
        Path::new(
            "/home/u/.ctxloom/locks/__home__u__.ctxloom__sessions__jolly-mute-magma__a7979502be344972.lock"
        )
    );
    assert_eq!(
        sidecar_for(home, Path::new("/home/u/.ctxloom/sessions/ bad/a")).lock,
        Path::new(
            "/home/u/.ctxloom/locks/__home__u__.ctxloom__sessions__ bad__a__cbeb694342752469.lock"
        )
    );
    for outside in [
        "/home/u/.ctxloom/sessions",
        "/home/u/.ctxloom/sessions/jolly-mute-magma",
        "/home/u/.ctxloom/sessionsx/jolly/a",
        "/home/u/.ctxloom/sessions/ bad/a",
    ] {
        let sidecar = sidecar_for(home, Path::new(outside));
        assert_eq!(
            sidecar.lock,
            Path::new(&format!(
                "/home/u/.ctxloom/locks/{}.lock",
                flat_name(outside)
            )),
            "{outside}"
        );
        assert!(sidecar.interlock.is_some(), "{outside}");
    }
}

#[test]
fn test_a_harp_is_one_path_component_with_visible_edges() {
    for good in ["jolly-mute-magma", "a b", "x"] {
        assert!(valid_harp(good), "{good:?}");
    }
    for bad in [
        "", ".", "..", "a/b", "a\\b", "a:b", "a\u{1}b", "a\u{7f}", "\u{1f}", " a", "a ",
    ] {
        assert!(!valid_harp(bad), "{bad:?}");
    }
}

#[cfg(unix)]
#[test]
fn test_a_session_file_under_a_linked_sessions_root_names_the_lock_as_home_spells_it() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let real = dir.path().join("elsewhere");
    std::fs::create_dir_all(home.join(".ctxloom")).unwrap();
    std::fs::create_dir_all(real.join("jolly-mute-magma/work")).unwrap();
    std::os::unix::fs::symlink(&real, home.join(".ctxloom/sessions")).unwrap();
    let real = std::fs::canonicalize(&real).unwrap();
    let sidecar = sidecar_for(&home, &real.join("jolly-mute-magma/work/.mcp.json"));
    assert_eq!(
        sidecar,
        Sidecar {
            lock: home.join(format!(
                ".ctxloom/sessions/jolly-mute-magma/locks/{}.lock",
                flat_name("work/.mcp.json")
            )),
            interlock: None,
        }
    );
    // Outside the resolved root as well as the spelled one.
    assert!(sidecar_for(&home, &dir.path().join("x/.mcp.json"))
        .interlock
        .is_some());
}
