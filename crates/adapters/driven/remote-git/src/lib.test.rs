//! The `Remotes` port over real git, against repositories made in temporary
//! directories (ADR 0015's local-git-remote approach).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use deps_ports::Remotes;
use file_system::Os;

use super::*;

/// git in `dir`, with an identity of its own; its stdout.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", dir)
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.test")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.test")
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// The identity a publish commits as: the user's own configuration, here the
/// environment.
fn identify() {
    for (key, value) in [
        ("GIT_AUTHOR_NAME", "Alice"),
        ("GIT_AUTHOR_EMAIL", "alice@example.test"),
        ("GIT_COMMITTER_NAME", "Alice"),
        ("GIT_COMMITTER_EMAIL", "alice@example.test"),
    ] {
        std::env::set_var(key, value);
    }
}

/// A bare repository and the working clone that pushes to it.
struct Origin {
    root: tempfile::TempDir,
    bare: PathBuf,
    work: PathBuf,
}

impl Origin {
    fn new() -> Origin {
        let root = tempfile::tempdir().expect("a temporary directory");
        let (bare, work) = (root.path().join("remote.git"), root.path().join("work"));
        git(root.path(), &["init", "--bare", "-b", "main", "remote.git"]);
        git(root.path(), &["init", "-b", "main", "work"]);
        git(
            &work,
            &["remote", "add", "origin", bare.to_str().expect("utf-8")],
        );
        git(&bare, &["symbolic-ref", "HEAD", "refs/heads/main"]);
        Origin { root, bare, work }
    }

    fn url(&self) -> String {
        format!("file://{}", self.bare.display())
    }

    /// A commit writing these files (an execute bit where `true`); its sha.
    fn commit(&self, files: &[(&str, &str, bool)]) -> String {
        for (path, text, executable) in files {
            let at = self.work.join(path);
            std::fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
            std::fs::write(&at, text).expect("a file");
            if *executable {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o755))
                    .expect("a mode");
            }
        }
        git(&self.work, &["add", "-A"]);
        git(&self.work, &["commit", "-q", "-m", "commit"]);
        git(&self.work, &["push", "-q", "origin", "main"]);
        git(&self.work, &["rev-parse", "HEAD"])
    }

    fn remove(&self, paths: &[&str]) -> String {
        for path in paths {
            std::fs::remove_file(self.work.join(path)).expect("a file to remove");
        }
        git(&self.work, &["add", "-A"]);
        git(&self.work, &["commit", "-q", "-m", "remove"]);
        git(&self.work, &["push", "-q", "origin", "main"]);
        git(&self.work, &["rev-parse", "HEAD"])
    }

    fn tag(&self, name: &str) {
        git(&self.work, &["tag", name]);
        git(
            &self.work,
            &["push", "-q", "origin", &format!("refs/tags/{name}")],
        );
    }

    /// The remote taken off the air: its repository renamed away.
    fn vanish(&self) {
        std::fs::rename(&self.bare, self.root.path().join("remote.git.gone")).expect("renamed");
    }
}

struct Cache {
    remotes: GitRemotes,
    dir: tempfile::TempDir,
}

fn cache() -> Cache {
    let dir = tempfile::tempdir().expect("a temporary cache");
    let cwd = std::env::current_dir().expect("a working directory");
    let fs = Arc::new(Os::new(os::host(), &cwd).expect("the filesystem"));
    Cache {
        remotes: GitRemotes::new(fs, dir.path().to_path_buf()),
        dir,
    }
}

const BUNDLE: &str = ".ctxloom/content/bundles/v2";

fn demo(origin: &Origin) -> String {
    origin.commit(&[
        (&format!("{BUNDLE}/demo/bundle.yaml"), "version: 1\n", false),
        (&format!("{BUNDLE}/demo/fragments/a.md"), "A\n", false),
        (
            &format!("{BUNDLE}/demo/scripts/run.sh"),
            "#!/bin/sh\n",
            true,
        ),
    ])
}

// ADR 0015 / repo_cache.go: a clone is made once, below the cache root, by the
// repository's host and path; an existing one is not fetched by `ensure`.
#[test]
fn test_ensure_clones_once_and_does_not_fetch_a_clone_it_has() {
    let origin = Origin::new();
    let first = demo(&origin);
    let c = cache();
    c.remotes.ensure(&origin.url()).expect("clones");
    let dir = c.remotes.clone_dir(&origin.url()).expect("a directory");
    assert!(
        dir.starts_with(c.dir.path()) && dir.join(".git").is_dir(),
        "{}",
        dir.display()
    );
    assert!(
        !dir.join(BUNDLE).exists(),
        "no checkout: nothing is read from a working tree"
    );
    let second = origin.commit(&[(&format!("{BUNDLE}/demo/fragments/b.md"), "B\n", false)]);
    c.remotes.ensure(&origin.url()).expect("is there");
    assert_eq!(
        c.remotes
            .resolve_ref(&origin.url(), "main")
            .expect("resolves"),
        first
    );
    c.remotes.refresh(&origin.url()).expect("fetches");
    assert_eq!(
        c.remotes
            .resolve_ref(&origin.url(), "main")
            .expect("resolves"),
        second
    );
}

#[test]
fn test_refresh_clones_what_is_not_there_and_a_failed_clone_leaves_nothing_behind() {
    let origin = Origin::new();
    demo(&origin);
    let c = cache();
    c.remotes.refresh(&origin.url()).expect("clones");
    let missing = format!("file://{}/nowhere.git", origin.root.path().display());
    let refused = c.remotes.ensure(&missing).expect_err("no such repository");
    assert!(refused.starts_with("git clone: "), "{refused}");
    assert!(!c.remotes.clone_dir(&missing).expect("a directory").exists());
    assert!(c.remotes.refresh(&missing).is_err());
}

// deps.feature: A stale local checkout never leaks into what's served
#[test]
fn test_a_branch_is_its_remote_tracking_tip_whatever_the_clone_has_checked_out() {
    let origin = Origin::new();
    let first = demo(&origin);
    let c = cache();
    c.remotes.ensure(&origin.url()).expect("clones");
    let second = origin.commit(&[(&format!("{BUNDLE}/demo/fragments/a.md"), "A2\n", false)]);
    c.remotes.refresh(&origin.url()).expect("fetches");
    let dir = c.remotes.clone_dir(&origin.url()).expect("a directory");
    git(&dir, &["reset", "--hard", &first]);
    assert_eq!(
        c.remotes
            .resolve_ref(&origin.url(), "main")
            .expect("resolves"),
        second
    );
    let tree = c
        .remotes
        .read_bundle(&origin.url(), &second, "demo")
        .expect("reads")
        .expect("a bundle");
    assert_eq!(tree["fragments/a.md"].data, b"A2\n");
}

#[test]
fn test_versions_resolve_as_branch_then_tag_then_commit() {
    let origin = Origin::new();
    let first = demo(&origin);
    origin.tag("v1.0.0");
    let second = origin.commit(&[(&format!("{BUNDLE}/demo/fragments/b.md"), "B\n", false)]);
    git(&origin.work, &["branch", "release", &first]);
    git(
        &origin.work,
        &["push", "-q", "origin", "refs/heads/release"],
    );
    origin.tag("release");
    let c = cache();
    let url = origin.url();
    c.remotes.refresh(&url).expect("clones");
    assert_eq!(
        c.remotes.tags(&url).expect("tags"),
        vec!["release", "v1.0.0"]
    );
    assert_eq!(c.remotes.resolve_ref(&url, "main").expect("branch"), second);
    assert_eq!(c.remotes.resolve_ref(&url, "v1.0.0").expect("tag"), first);
    assert_eq!(
        c.remotes
            .resolve_ref(&url, &second[..9])
            .expect("abbreviated"),
        second
    );
    assert_eq!(c.remotes.resolve_ref(&url, &second).expect("full"), second);
    assert_eq!(c.remotes.resolve_tag(&url, "v1.0.0").expect("tag"), first);
    // A branch and a tag of one name: the generic lookup says the branch, the
    // tag lookup the tag.
    assert_eq!(
        c.remotes.resolve_ref(&url, "release").expect("branch"),
        first
    );
    assert_eq!(c.remotes.resolve_tag(&url, "release").expect("tag"), second);
    for refused in ["nope", "not-hex-zz", "ab"] {
        assert_eq!(
            c.remotes.resolve_ref(&url, refused).expect_err(refused),
            format!("unknown revision {refused}")
        );
    }
    assert_eq!(
        c.remotes.resolve_tag(&url, "main").expect_err("a branch"),
        "unknown tag main"
    );
}

#[test]
fn test_the_default_branch_is_what_the_remote_head_points_at_else_main_or_master() {
    let origin = Origin::new();
    demo(&origin);
    let c = cache();
    c.remotes.ensure(&origin.url()).expect("clones");
    assert_eq!(
        c.remotes.default_branch(&origin.url()).expect("a branch"),
        "main"
    );
    let dir = c.remotes.clone_dir(&origin.url()).expect("a directory");
    // With the remote's HEAD ref gone, the clone's own branches say.
    git(
        &dir,
        &["symbolic-ref", "--delete", "refs/remotes/origin/HEAD"],
    );
    assert_eq!(
        c.remotes.default_branch(&origin.url()).expect("main"),
        "main"
    );
    git(&dir, &["branch", "-m", "main", "master"]);
    git(
        &dir,
        &[
            "update-ref",
            "refs/remotes/origin/master",
            "refs/remotes/origin/main",
        ],
    );
    git(&dir, &["update-ref", "-d", "refs/remotes/origin/main"]);
    assert_eq!(
        c.remotes.default_branch(&origin.url()).expect("master"),
        "master"
    );
    git(&dir, &["update-ref", "-d", "refs/remotes/origin/master"]);
    assert_eq!(
        c.remotes.default_branch(&origin.url()).expect_err("none"),
        "cannot determine the repository's default branch"
    );
}

// remotetree: a bundle's files with the execute bit the publisher committed
#[test]
fn test_a_bundle_is_read_at_a_commit_with_its_bytes_and_modes() {
    let origin = Origin::new();
    let first = demo(&origin);
    let c = cache();
    let url = origin.url();
    c.remotes.ensure(&url).expect("clones");
    let tree = c
        .remotes
        .read_bundle(&url, &first, "demo")
        .expect("reads")
        .expect("a bundle");
    let paths: Vec<&str> = tree.keys().map(String::as_str).collect();
    assert_eq!(
        paths,
        vec!["bundle.yaml", "fragments/a.md", "scripts/run.sh"]
    );
    assert_eq!(tree["fragments/a.md"].data, b"A\n");
    assert!(tree["scripts/run.sh"].executable && !tree["bundle.yaml"].executable);
    assert_eq!(
        c.remotes
            .read_bundle(&url, &first, "absent")
            .expect("reads"),
        None
    );
    assert!(c
        .remotes
        .read_bundle(&url, &"0".repeat(40), "demo")
        .is_err());
    // The tree of an earlier commit is the tree of that commit.
    let second = origin.commit(&[(&format!("{BUNDLE}/demo/fragments/a.md"), "A2\n", false)]);
    c.remotes.refresh(&url).expect("fetches");
    let earlier = c
        .remotes
        .read_bundle(&url, &first, "demo")
        .expect("reads")
        .expect("a bundle");
    assert_eq!(earlier["fragments/a.md"].data, b"A\n");
    let later = c
        .remotes
        .read_bundle(&url, &second, "demo")
        .expect("reads")
        .expect("a bundle");
    assert_eq!(later["fragments/a.md"].data, b"A2\n");
}

#[test]
fn test_binary_bytes_survive_and_a_link_is_not_a_file_of_the_tree() {
    let origin = Origin::new();
    let dir = origin.work.join(BUNDLE).join("demo");
    std::fs::create_dir_all(&dir).expect("a directory");
    std::fs::write(dir.join("bundle.yaml"), "version: 1\n").expect("a file");
    std::fs::write(dir.join("blob.bin"), [0u8, 1, 2, 255, b'\n', 0, b'\n']).expect("a file");
    std::os::unix::fs::symlink("/etc/passwd", dir.join("link")).expect("a link");
    git(&origin.work, &["add", "-A"]);
    git(&origin.work, &["commit", "-q", "-m", "binary"]);
    git(&origin.work, &["push", "-q", "origin", "main"]);
    let sha = git(&origin.work, &["rev-parse", "HEAD"]);
    let c = cache();
    c.remotes.ensure(&origin.url()).expect("clones");
    let tree = c
        .remotes
        .read_bundle(&origin.url(), &sha, "demo")
        .expect("reads")
        .expect("a bundle");
    assert_eq!(tree["blob.bin"].data, vec![0u8, 1, 2, 255, b'\n', 0, b'\n']);
    assert!(!tree.contains_key("link"));
}

#[test]
fn test_bundles_are_listed_and_asked_after_at_the_tip() {
    let origin = Origin::new();
    origin.commit(&[
        (&format!("{BUNDLE}/zeta/bundle.yaml"), "v: 1\n", false),
        (&format!("{BUNDLE}/team/rules/bundle.yaml"), "v: 1\n", false),
        (
            &format!("{BUNDLE}/team/rules/skills/x/bundle.yaml"),
            "v: 1\n",
            false,
        ),
        (&format!("{BUNDLE}/team/notes.md"), "not a bundle\n", false),
        (&format!("{BUNDLE}/alpha/bundle.yaml"), "v: 1\n", false),
        ("README.md", "hi\n", false),
    ]);
    let c = cache();
    let url = origin.url();
    c.remotes.ensure(&url).expect("clones");
    assert_eq!(
        c.remotes.list_bundles(&url).expect("lists"),
        vec!["alpha", "team/rules", "zeta"]
    );
    assert!(c.remotes.has_bundle(&url, "alpha").expect("asks"));
    assert!(c.remotes.has_bundle(&url, "team/rules").expect("asks"));
    assert!(
        !c.remotes.has_bundle(&url, "team").expect("asks"),
        "a directory without a manifest is no bundle"
    );
    assert!(!c.remotes.has_bundle(&url, "absent").expect("asks"));
}

// deps.feature: A bundle deleted upstream: the answer follows the fetch
#[test]
fn test_what_upstream_removed_is_gone_after_a_fetch_and_not_before() {
    let origin = Origin::new();
    demo(&origin);
    let c = cache();
    let url = origin.url();
    c.remotes.ensure(&url).expect("clones");
    origin.remove(&[
        &format!("{BUNDLE}/demo/bundle.yaml"),
        &format!("{BUNDLE}/demo/fragments/a.md"),
        &format!("{BUNDLE}/demo/scripts/run.sh"),
    ]);
    assert!(
        c.remotes.has_bundle(&url, "demo").expect("asks"),
        "the clone has not seen it"
    );
    c.remotes.refresh(&url).expect("fetches");
    assert!(!c.remotes.has_bundle(&url, "demo").expect("asks"));
}

// deps.feature: An unreachable remote
#[test]
fn test_a_remote_that_cannot_be_read_fails_the_fetch_and_the_clone_stays_readable() {
    let origin = Origin::new();
    let first = demo(&origin);
    let c = cache();
    let url = origin.url();
    c.remotes.ensure(&url).expect("clones");
    origin.vanish();
    let refused = c.remotes.refresh(&url).expect_err("unreachable");
    assert!(refused.starts_with("git fetch: "), "{refused}");
    assert!(c
        .remotes
        .read_bundle(&url, &first, "demo")
        .expect("still reads")
        .is_some());
}

#[test]
fn test_what_was_never_cloned_cannot_be_read() {
    let origin = Origin::new();
    demo(&origin);
    let c = cache();
    let url = origin.url();
    let said = c.remotes.tags(&url).expect_err("not cloned");
    assert!(said.ends_with("is not cloned yet"), "{said}");
    assert!(c.remotes.resolve_ref(&url, "main").is_err());
    assert!(c.remotes.read_bundle(&url, "abc", "demo").is_err());
    assert!(c.remotes.list_bundles(&url).is_err());
    assert!(c.remotes.has_bundle(&url, "demo").is_err());
    assert!(c.remotes.default_branch(&url).is_err());
    assert!(c.remotes.resolve_tag(&url, "x").is_err());
}

#[test]
fn test_an_address_that_names_no_repository_has_no_place_in_the_cache() {
    let c = cache();
    for url in ["bundles", "https://", ""] {
        let refused = c.remotes.ensure(url).expect_err(url);
        assert!(refused.starts_with("refusing to cache"), "{refused}");
    }
}

// remote.go resolveLocalRepoURL
#[test]
fn test_a_path_is_the_file_url_of_the_repository_there() {
    let origin = Origin::new();
    let c = cache();
    let url = c
        .remotes
        .local_repository(origin.bare.to_str().expect("utf-8"))
        .expect("a repository");
    assert_eq!(
        url,
        format!(
            "file://{}",
            origin.bare.canonicalize().expect("a path").display()
        )
    );
    assert!(c
        .remotes
        .local_repository(origin.work.to_str().expect("utf-8"))
        .is_ok());
    let refused = c
        .remotes
        .local_repository(origin.root.path().to_str().expect("utf-8"))
        .expect_err("no repository");
    assert!(refused.contains("holds no git repository"), "{refused}");
}

// bundle.feature: Pushing a bundle to a plain git remote lands its tree there
#[test]
fn test_a_publish_lands_files_as_one_commit_beside_the_history() {
    identify();
    let origin = Origin::new();
    let before = demo(&origin);
    let c = cache();
    let files: BTreeMap<String, Vec<u8>> = BTreeMap::from([
        (
            format!("{BUNDLE}/authored/bundle.yaml"),
            b"description: authored\n".to_vec(),
        ),
        (
            format!("{BUNDLE}/authored/fragments/example.md"),
            b"# Example\n".to_vec(),
        ),
    ]);
    let commit = c
        .remotes
        .publish(&origin.url(), &files, "Update bundle: authored")
        .expect("publishes");
    assert_eq!(git(&origin.bare, &["rev-parse", "main"]), commit);
    assert_eq!(
        git(
            &origin.bare,
            &["rev-list", "--count", &format!("{before}..main")]
        ),
        "1"
    );
    assert_eq!(
        git(&origin.bare, &["log", "-1", "--format=%s", "main"]),
        "Update bundle: authored"
    );
    assert_eq!(
        git(&origin.bare, &["log", "-1", "--format=%an", "main"]),
        "Alice"
    );
    assert_eq!(
        git(
            &origin.bare,
            &[
                "show",
                &format!("main:{BUNDLE}/authored/fragments/example.md")
            ]
        ),
        "# Example"
    );
    assert_eq!(
        git(
            &origin.bare,
            &["show", &format!("main:{BUNDLE}/demo/fragments/a.md")]
        ),
        "A"
    );
    // Nothing to change is nothing to publish: the tip stands.
    let again = c
        .remotes
        .publish(&origin.url(), &files, "again")
        .expect("publishes");
    assert_eq!(again, commit);
    assert_eq!(git(&origin.bare, &["rev-list", "--count", "main"]), "2");
    // No working clone is left in the cache.
    let leftovers: Vec<_> = std::fs::read_dir(c.dir.path())
        .expect("the cache")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn test_a_publish_to_a_repository_that_cannot_be_reached_fails_and_leaves_no_clone() {
    identify();
    let origin = Origin::new();
    demo(&origin);
    let c = cache();
    let url = origin.url();
    origin.vanish();
    let files = BTreeMap::from([("a.txt".to_string(), b"a".to_vec())]);
    assert!(c.remotes.publish(&url, &files, "m").is_err());
    let leftovers = std::fs::read_dir(c.dir.path()).expect("the cache").count();
    assert_eq!(leftovers, 0);
}

// gitutil: the repository-location variables are stripped, so an inherited
// GIT_DIR cannot redirect the clone
#[test]
fn test_an_inherited_git_dir_does_not_steer_the_repository() {
    let origin = Origin::new();
    let first = demo(&origin);
    std::env::set_var("GIT_DIR", origin.root.path().join("elsewhere"));
    std::env::set_var("GIT_WORK_TREE", origin.root.path());
    let c = cache();
    c.remotes
        .ensure(&origin.url())
        .expect("clones all the same");
    assert_eq!(
        c.remotes
            .resolve_ref(&origin.url(), "main")
            .expect("resolves"),
        first
    );
}

#[test]
fn test_which_variables_are_the_repository_s_own() {
    let asked = Some(
        ["GIT_DIR", "GIT_WORK_TREE", "GIT_CONFIG_COUNT"]
            .map(String::from)
            .to_vec(),
    );
    for local in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_CONFIG_COUNT",
        "GIT_CONFIG_KEY_0",
        "GIT_CONFIG_VALUE_12",
    ] {
        assert!(command::is_local(local, &asked), "{local}");
    }
    for kept in [
        "GIT_AUTHOR_NAME",
        "GIT_SSH_COMMAND",
        "PATH",
        "HOME",
        "GIT_CONFIG",
    ] {
        assert!(!command::is_local(kept, &asked), "{kept}");
    }
    // Without a counter, no numbered pair is the repository's.
    let without = Some(vec!["GIT_DIR".to_string()]);
    assert!(!command::is_local("GIT_CONFIG_KEY_0", &without));
    // When git cannot be asked, every GIT_ variable goes.
    assert!(command::is_local("GIT_AUTHOR_NAME", &None));
    assert!(!command::is_local("HOME", &None));
}

#[test]
fn test_the_local_variables_git_names_are_found() {
    let names = command::local_variables()
        .clone()
        .expect("git can be asked");
    assert!(names.iter().any(|n| n == "GIT_DIR"), "{names:?}");
}

// A clone clears the place it is made in first: that place is below the cache,
// never the cache itself, an empty path (the working directory) or beyond. The
// filesystem is a rooted one, so that a clone that is wrongly let through
// clears only a temporary root.
#[test]
fn test_a_clone_is_made_only_in_a_place_of_the_cache() {
    let fs = Arc::new(file_system::Rooted::temp().expect("a rooted filesystem"));
    let mode = os::Mode::from_bits(0o755);
    for dir in ["/c/inner", "/c/beside"] {
        fs.create_dir_all(Path::new(dir), mode)
            .expect("a directory");
        fs.write_atomic(
            &Path::new(dir).join("keep.txt"),
            b"mine",
            file_system::Perms::Exact(os::Mode::from_bits(0o644)),
        )
        .expect("a file");
    }
    let remotes = GitRemotes::new(fs.clone(), PathBuf::from("/c/inner"));
    for place in [
        "",
        ".",
        "/c/inner",
        "/c/beside",
        "/c/inner/..",
        "/c/inner/x/../../beside",
        "/c",
    ] {
        let refused = remotes
            .clone_into("file:///nowhere.git", Path::new(place), false)
            .expect_err("refused");
        assert!(
            refused.contains("not a place of the cache"),
            "{place:?}: {refused}"
        );
    }
    for kept in ["/c/inner/keep.txt", "/c/beside/keep.txt"] {
        assert!(fs.exists(Path::new(kept)).unwrap_or(false), "{kept}");
    }
}

// A token for a private repository reaches git only through its environment:
// never argv, never the URL, never an error's text, never the clone's config.
mod credentials {
    use super::*;

    const TOKEN: &str = "ghp_sekrit0123456789"; // gitleaks:allow

    fn with_token(c: Cache) -> Cache {
        Cache {
            remotes: c.remotes.with_credentials(
                Arc::new(|name| (name == "GH_TOKEN").then(|| TOKEN.to_string())),
                Arc::new(|_| vec!["GH_TOKEN".to_string()]),
            ),
            dir: c.dir,
        }
    }

    #[test]
    fn test_the_header_reaches_git_as_configuration_from_the_environment() {
        let c = with_token(cache());
        let credential = c.remotes.credential("https://example.test/o/r");
        let out = command::Git::default()
            .run_with(
                None,
                &["config", "--get", "http.https://example.test/.extraheader"],
                None,
                &credential,
            )
            .expect("git sees the header");
        let seen = String::from_utf8_lossy(&out);
        assert!(seen.starts_with("AUTHORIZATION: basic "), "{seen}");
    }

    #[test]
    fn test_a_failed_clone_names_neither_the_token_nor_its_header() {
        let c = with_token(cache());
        let err = c
            .remotes
            .ensure("https://127.0.0.1:1/o/r")
            .expect_err("nothing listens there");
        assert!(
            !err.contains(TOKEN)
                && !err.contains("basic ")
                && !err.to_lowercase().contains("authorization"),
            "{err}"
        );
        assert!(err.starts_with("git clone"), "{err}");
    }

    #[test]
    fn test_an_unauthenticated_public_clone_is_unchanged() {
        let origin = Origin::new();
        demo(&origin);
        let plain = cache();
        plain.remotes.ensure(&origin.url()).expect("clones");
        let keyed = with_token(cache());
        keyed.remotes.ensure(&origin.url()).expect("clones");
        keyed.remotes.refresh(&origin.url()).expect("fetches");
        let dir = keyed.remotes.clone_dir(&origin.url()).expect("a directory");
        let config = std::fs::read_to_string(dir.join(".git/config")).expect("config");
        assert!(
            !config.contains(TOKEN) && !config.to_lowercase().contains("extraheader"),
            "{config}"
        );
        assert_eq!(
            keyed
                .remotes
                .resolve_ref(&origin.url(), "main")
                .expect("resolves"),
            plain
                .remotes
                .resolve_ref(&origin.url(), "main")
                .expect("resolves"),
        );
        assert!(keyed.remotes.credential(&origin.url()).pairs().is_empty());
    }
}

// S11 hardening: a hung git fails the call at its limit, with the limit named,
// and is never read as an answer about the repository.
mod hung_git {
    use super::*;
    use crate::command::tests::{stub, HANGS};

    const TIMED_OUT: &str = "did not finish within 20ms and was killed";

    /// The cache's directory, over a git that never finishes.
    fn hung(c: &Cache) -> (tempfile::TempDir, GitRemotes) {
        let (dir, git) = stub(HANGS);
        let cwd = std::env::current_dir().expect("a working directory");
        let fs = Arc::new(Os::new(os::host(), &cwd).expect("the filesystem"));
        (
            dir,
            GitRemotes::new(fs, c.dir.path().to_path_buf()).with_git(git),
        )
    }

    #[test]
    fn a_clone_that_hangs_fails_and_leaves_nothing_behind() {
        let origin = Origin::new();
        demo(&origin);
        let c = cache();
        let (_stub, remotes) = hung(&c);
        let why = remotes.ensure(&origin.url()).unwrap_err();
        assert_eq!(why, format!("git clone: {TIMED_OUT}"));
        let dir = remotes.clone_dir(&origin.url()).unwrap();
        assert!(
            !dir.exists(),
            "a partial clone was left at {}",
            dir.display()
        );
    }

    #[test]
    fn a_fetch_that_hangs_fails_and_the_clone_stays_readable() {
        let origin = Origin::new();
        demo(&origin);
        let c = cache();
        c.remotes.ensure(&origin.url()).unwrap();
        let (_stub, remotes) = hung(&c);
        let why = remotes.refresh(&origin.url()).unwrap_err();
        assert_eq!(why, format!("git fetch: {TIMED_OUT}"));
        assert!(c.remotes.tags(&origin.url()).is_ok());
    }

    #[test]
    fn a_directory_whose_git_hangs_is_not_called_repository_free() {
        let origin = Origin::new();
        let c = cache();
        let (_stub, remotes) = hung(&c);
        let why = remotes
            .local_repository(&origin.work.display().to_string())
            .unwrap_err();
        assert_eq!(why, format!("git rev-parse: {TIMED_OUT}"));
    }

    #[test]
    fn a_read_whose_git_hangs_fails_at_the_limit() {
        let origin = Origin::new();
        demo(&origin);
        let c = cache();
        c.remotes.ensure(&origin.url()).unwrap();
        let (_stub, remotes) = hung(&c);
        let why = remotes.tags(&origin.url()).unwrap_err();
        assert_eq!(why, format!("git tag: {TIMED_OUT}"));
    }

    #[test]
    fn a_publish_whose_git_hangs_fails_and_clears_its_working_clone() {
        let origin = Origin::new();
        demo(&origin);
        let c = cache();
        let (_stub, remotes) = hung(&c);
        let files = BTreeMap::from([("a.txt".to_string(), b"a".to_vec())]);
        let why = remotes.publish(&origin.url(), &files, "msg").unwrap_err();
        assert_eq!(why, format!("git clone: {TIMED_OUT}"));
        let left: Vec<_> = std::fs::read_dir(c.dir.path()).unwrap().flatten().collect();
        assert!(left.is_empty(), "{left:?}");
    }
}
