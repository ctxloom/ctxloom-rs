//! The workspaces adapter, over real git in temporary directories. Ported
//! from Go's `worktree_test.go`, `worktree_integration_test.go`,
//! `worktree_reap_test.go` and j002200_isolation.feature's workspace rows
//! (release/0.7, f36e8dfb21).

use std::path::{Path, PathBuf};
use std::process::Command;

use isolation_ports::{Prepared, Removal, Request, Safety, Standing, Workspaces};

use super::{
    branch_of, checkout_name, is_name, nested_under, same_path, sanitize, GitWorkspaces, Listed,
    EXCLUDE_HEADER,
};

const PATTERNS: [&str; 5] = [
    ".ctxloom/cache/",
    ".ctxloom/project-id",
    ".claude/",
    ".mcp.json",
    "CLAUDE.md",
];

/// git with a clean identity and none of the caller's environment.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", dir)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// A home with a git-backed project.
struct Fixture {
    _root: tempfile::TempDir,
    home: PathBuf,
    project: PathBuf,
}

impl Fixture {
    fn new() -> Fixture {
        let root = tempfile::tempdir().expect("a temporary directory");
        let home = root.path().join("home");
        let project = root.path().join("project");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&project).unwrap();
        git(&project, &["init", "-q"]);
        std::fs::write(project.join("README.md"), "hello\n").unwrap();
        git(&project, &["add", "-A"]);
        git(&project, &["commit", "-q", "-m", "initial"]);
        Fixture {
            _root: root,
            home,
            project,
        }
    }

    fn adapter(&self) -> GitWorkspaces {
        GitWorkspaces::new(&self.home)
    }

    fn work_dir(&self, harp: &str) -> PathBuf {
        self.home.join(".ctxloom/sessions").join(harp).join("work")
    }

    fn request(&self, harp: &str, agent: &str) -> Request {
        Request {
            project_dir: self.project.display().to_string(),
            work_dir: self.work_dir(harp).display().to_string(),
            agent: agent.to_string(),
            artifact_patterns: PATTERNS.iter().map(ToString::to_string).collect(),
        }
    }

    fn prepared(&self, harp: &str, agent: &str) -> isolation_ports::Checkout {
        match self.adapter().prepare(&self.request(harp, agent)) {
            Ok(Prepared::Checkout(checkout)) => checkout,
            other => panic!("no checkout: {other:?}"),
        }
    }
}

// Go: worktree_test.go sanitizeAgentID.
#[test]
fn an_agent_id_becomes_one_safe_path_segment() {
    assert_eq!(sanitize("member-1"), "member-1");
    assert_eq!(sanitize("a/b c"), "a-b-c");
    assert_eq!(sanitize("--x.."), "x");
    assert_eq!(sanitize(""), "agent");
    assert_eq!(sanitize("///"), "agent");
}

// Go: worktreeBranchName: the scratch prefix swapped for the ref namespace.
#[test]
fn the_branch_derives_from_the_checkout_name_alone() {
    assert_eq!(checkout_name("reviewer"), "ctxloom-wt-reviewer");
    assert_eq!(
        branch_of("/home/.ctxloom/sessions/x/work/ctxloom-wt-reviewer"),
        "ctxloom/reviewer"
    );
}

// j002200 "The same run lands in the workspace its axis dictates": a
// worktree is a checkout of its own, on its own branch, in the session's work
// directory and never under the project.
#[test]
fn a_worktree_run_gets_its_own_branch_in_the_sessions_work_dir() {
    let f = Fixture::new();
    let checkout = f.prepared("amber-quiet-heron", "reviewer");
    let expected = f.work_dir("amber-quiet-heron").join("ctxloom-wt-reviewer");
    assert_eq!(Path::new(&checkout.path), expected);
    assert_eq!(checkout.branch, "ctxloom/reviewer");
    assert!(!checkout.reused);
    assert!(!Path::new(&checkout.path).starts_with(&f.project));
    let branch = git(
        Path::new(&checkout.path),
        &["rev-parse", "--abbrev-ref", "HEAD"],
    );
    assert_eq!(branch.trim(), "ctxloom/reviewer");
    assert!(Path::new(&checkout.path).join("README.md").exists());
}

// j002200 "Two isolated runs share no workspace between them".
#[test]
fn two_isolated_runs_share_no_workspace() {
    let f = Fixture::new();
    let one = f.prepared("amber-quiet-heron", "task-one");
    let two = f.prepared("brisk-copper-moth", "task-two");
    assert_ne!(one.path, two.path);
    for checkout in [&one, &two] {
        assert!(!Path::new(&checkout.path).starts_with(&f.project));
    }
}

// j002200 "Asking for a worktree outside git runs in the live project
// directory instead of refusing".
#[test]
fn a_worktree_outside_git_is_unavailable_not_a_refusal() {
    let f = Fixture::new();
    let plain = f.home.join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let mut request = f.request("amber-quiet-heron", "reviewer");
    request.project_dir = plain.display().to_string();
    match f.adapter().prepare(&request) {
        Ok(Prepared::Unavailable(why)) => {
            assert!(why.contains("not a git repository"), "{why}");
        }
        other => panic!("{other:?}"),
    }
}

// j002200 "A worktree run leaves the project tree clean": the config the run
// writes is hidden by the shared exclude, in the project and in the checkout.
#[test]
fn the_per_agent_config_never_appears_in_the_projects_git_status() {
    let f = Fixture::new();
    let checkout = f.prepared("amber-quiet-heron", "reviewer");
    let wt = Path::new(&checkout.path);
    std::fs::create_dir_all(wt.join(".claude")).unwrap();
    std::fs::write(wt.join(".claude/settings.json"), "{}").unwrap();
    std::fs::write(wt.join("CLAUDE.md"), "context").unwrap();
    std::fs::write(wt.join(".mcp.json"), "{}").unwrap();
    std::fs::create_dir_all(wt.join(".ctxloom/cache")).unwrap();
    std::fs::write(wt.join(".ctxloom/cache/x"), "x").unwrap();
    assert_eq!(git(&f.project, &["status", "--porcelain"]).trim(), "");
    assert_eq!(git(wt, &["status", "--porcelain"]).trim(), "");
    let exclude = std::fs::read_to_string(f.project.join(".git/info/exclude")).unwrap();
    assert!(exclude.contains(EXCLUDE_HEADER), "{exclude}");
    for pattern in PATTERNS {
        assert!(exclude.contains(pattern), "{pattern} in {exclude}");
    }
}

// Go: excludeConfigFromMerge is idempotent and shared-safe.
#[test]
fn preparing_twice_writes_the_exclude_block_once() {
    let f = Fixture::new();
    f.prepared("amber-quiet-heron", "one");
    f.prepared("amber-quiet-heron", "two");
    let exclude = std::fs::read_to_string(f.project.join(".git/info/exclude")).unwrap();
    assert_eq!(exclude.matches(EXCLUDE_HEADER).count(), 1, "{exclude}");
    assert_eq!(exclude.matches("CLAUDE.md").count(), 1, "{exclude}");
}

// Go: reuseExistingCheckout (sizable-antler): an earlier run's own checkout,
// still registered on the agent's branch, is re-entered in place.
#[test]
fn a_resume_re_enters_the_agents_own_checkout() {
    let f = Fixture::new();
    let first = f.prepared("amber-quiet-heron", "reviewer");
    std::fs::write(Path::new(&first.path).join("wip.txt"), "in flight").unwrap();
    let again = f.prepared("amber-quiet-heron", "reviewer");
    assert!(again.reused);
    assert_eq!(again.path, first.path);
    assert!(Path::new(&again.path).join("wip.txt").exists());
}

// Go: a path collision is not proof of ownership: anything else at the path
// is a refusal, not a degrade, and is left exactly as it was.
#[test]
fn something_else_at_the_agents_path_is_refused_and_left_alone() {
    let f = Fixture::new();
    let path = f.work_dir("amber-quiet-heron").join("ctxloom-wt-reviewer");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("mine.txt"), "mine").unwrap();
    let refused = f
        .adapter()
        .prepare(&f.request("amber-quiet-heron", "reviewer"))
        .unwrap_err();
    assert_eq!(refused.code(), "worktree-git-unadoptable");
    assert_eq!(
        std::fs::read_to_string(path.join("mine.txt")).unwrap(),
        "mine"
    );
}

// Go: findWorkWorktrees: only ctxloom-wt-* directories directly under a
// session's work dir; toolchain scratch and foreign trees are not candidates.
#[test]
fn scratch_lists_only_ctxloom_checkouts_under_session_work_dirs() {
    let f = Fixture::new();
    let a = f.prepared("amber-quiet-heron", "a");
    let b = f.prepared("brisk-copper-moth", "b");
    std::fs::create_dir_all(f.work_dir("amber-quiet-heron").join("ctxloom-tmp-a")).unwrap();
    std::fs::write(f.work_dir("amber-quiet-heron").join("ctxloom-wt-file"), "x").unwrap();
    let foreign = f.home.join("workspace/worktrees/proj--stale");
    git(
        &f.project,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "stale",
            &foreign.display().to_string(),
        ],
    );
    let ws = f.adapter();
    let all = ws.scratch(None).unwrap();
    let paths: Vec<&str> = all.iter().map(|s| s.path.as_str()).collect();
    assert_eq!(paths.len(), 2, "{paths:?}");
    assert!(paths.contains(&a.path.as_str()) && paths.contains(&b.path.as_str()));
    assert!(!paths.iter().any(|p| p.contains("stale")));
    let one = ws.scratch(Some("amber-quiet-heron")).unwrap();
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].harp, "amber-quiet-heron");
}

#[test]
fn scratch_of_a_home_where_nothing_ever_ran_is_empty() {
    let f = Fixture::new();
    assert!(f.adapter().scratch(None).unwrap().is_empty());
    assert!(f.adapter().scratch(Some("nobody")).unwrap().is_empty());
}

#[test]
fn a_session_is_known_by_its_directory() {
    let f = Fixture::new();
    std::fs::create_dir_all(f.home.join(".ctxloom/sessions/amber-swift-owl")).unwrap();
    let ws = f.adapter();
    assert!(ws.session_known("amber-swift-owl").unwrap());
    assert!(!ws.session_known("nobody").unwrap());
    assert!(!ws.session_known("../escape").unwrap());
}

// Go: unsafeToRemove: dirty, untracked and ignored-only content all make a
// tree unsafe; an unreadable state is never clean.
#[test]
fn inspect_reads_clean_dirty_and_ignored_only_trees() {
    let f = Fixture::new();
    let clean = f.prepared("amber-quiet-heron", "clean");
    let dirty = f.prepared("amber-quiet-heron", "dirty");
    let ignored = f.prepared("amber-quiet-heron", "ignored");
    std::fs::write(Path::new(&dirty.path).join("in-flight.go"), "// wip\n").unwrap();
    std::fs::write(Path::new(&ignored.path).join("CLAUDE.md"), "ignored\n").unwrap();
    let ws = f.adapter();
    assert_eq!(
        ws.inspect(&clean.path),
        Standing::Resolved {
            repo_dir: f.project.canonicalize().unwrap().display().to_string(),
            safety: Safety::Clean
        }
    );
    match ws.inspect(&dirty.path) {
        Standing::Resolved {
            safety: Safety::Unsafe(why),
            ..
        } => assert!(why.contains("uncommitted changes"), "{why}"),
        other => panic!("{other:?}"),
    }
    match ws.inspect(&ignored.path) {
        Standing::Resolved {
            safety: Safety::Unsafe(why),
            ..
        } => assert!(why.contains("gitignored"), "{why}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn inspect_of_a_directory_in_no_repository_is_unresolved() {
    let f = Fixture::new();
    let plain = f.home.join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    assert!(matches!(
        f.adapter().inspect(&plain.display().to_string()),
        Standing::Unresolved(_)
    ));
}

// Go: ReapWorktrees: a clean checkout is removed, its registration pruned,
// and its branch deleted when merged.
#[test]
fn removing_a_clean_checkout_removes_it_its_registration_and_its_merged_branch() {
    let f = Fixture::new();
    let checkout = f.prepared("amber-quiet-heron", "clean");
    let removal = f.adapter().remove(&checkout.repo_dir, &checkout.path);
    assert_eq!(removal, Removal::Removed);
    assert!(!Path::new(&checkout.path).exists());
    let listed = git(&f.project, &["worktree", "list", "--porcelain"]);
    assert!(!listed.contains("ctxloom-wt-clean"), "{listed}");
    let branches = git(&f.project, &["branch", "--list", "ctxloom/clean"]);
    assert_eq!(
        branches.trim(),
        "",
        "a merged branch holds nothing the repository does not"
    );
}

// Go: an unmerged branch holds commits nothing else has, and is kept.
#[test]
fn an_unmerged_branch_is_kept() {
    let f = Fixture::new();
    let checkout = f.prepared("amber-quiet-heron", "work");
    let wt = Path::new(&checkout.path);
    std::fs::write(wt.join("feature.txt"), "feature").unwrap();
    git(wt, &["add", "-A"]);
    git(wt, &["commit", "-q", "-m", "feature"]);
    assert_eq!(
        f.adapter().remove(&checkout.repo_dir, &checkout.path),
        Removal::Removed
    );
    let branches = git(&f.project, &["branch", "--list", "ctxloom/work"]);
    assert!(branches.contains("ctxloom/work"), "{branches}");
}

// j001300 "THE SAFETY SCENARIO": uncommitted work is spared IN PLACE, its
// bytes still on disk and still registered with git.
#[test]
fn removing_a_checkout_with_uncommitted_work_leaves_it_in_place() {
    let f = Fixture::new();
    let checkout = f.prepared("amber-quiet-heron", "wip");
    let wip = Path::new(&checkout.path).join("in-flight.go");
    std::fs::write(&wip, "// WIP-MARKER\n").unwrap();
    let removal = f.adapter().remove(&checkout.repo_dir, &checkout.path);
    assert!(matches!(removal, Removal::Left(_)), "{removal:?}");
    assert_eq!(std::fs::read_to_string(&wip).unwrap(), "// WIP-MARKER\n");
    let listed = git(&f.project, &["worktree", "list", "--porcelain"]);
    assert!(listed.contains("ctxloom-wt-wip"), "{listed}");
}

// Go: teardownWorktree: a dirty nested worktree aborts the whole teardown.
#[test]
fn a_dirty_nested_worktree_keeps_its_parent_in_place() {
    let f = Fixture::new();
    let parent = f.prepared("amber-quiet-heron", "parent");
    let inner = Path::new(&parent.path).join("inner");
    git(
        Path::new(&parent.path),
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "inner-branch",
            &inner.display().to_string(),
        ],
    );
    std::fs::write(inner.join("nested-wip.txt"), "nested").unwrap();
    let removal = f.adapter().remove(&parent.repo_dir, &parent.path);
    assert!(matches!(removal, Removal::Left(_)), "{removal:?}");
    assert!(inner.join("nested-wip.txt").exists());
}

fn exclude_of(f: &Fixture) -> String {
    std::fs::read_to_string(f.project.join(".git/info/exclude")).unwrap()
}

#[test]
fn a_directory_outside_git_says_it_is_not_a_repository() {
    let f = Fixture::new();
    let plain = f.home.join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let mut request = f.request("amber-quiet-heron", "reviewer");
    request.project_dir = plain.display().to_string();
    let why = match f.adapter().prepare(&request) {
        Ok(Prepared::Unavailable(why)) => why,
        other => panic!("{other:?}"),
    };
    assert!(why.starts_with("worktree isolation: "), "{why}");
}

#[test]
fn a_branch_an_earlier_run_left_is_re_entered() {
    let f = Fixture::new();
    git(&f.project, &["branch", "ctxloom/reviewer"]);
    let checkout = f.prepared("amber-quiet-heron", "reviewer");
    assert_eq!(checkout.branch, "ctxloom/reviewer");
    assert!(!checkout.reused, "the checkout is new; the branch is not");
}

#[test]
fn a_checkout_registered_on_another_branch_is_refused() {
    let f = Fixture::new();
    let path = f.work_dir("amber-quiet-heron").join("ctxloom-wt-reviewer");
    git(
        &f.project,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "someone-elses",
            &path.display().to_string(),
        ],
    );
    let refused = f
        .adapter()
        .prepare(&f.request("amber-quiet-heron", "reviewer"))
        .unwrap_err();
    assert_eq!(refused.code(), "worktree-git-unadoptable");
    assert!(refused.to_string().contains("someone-elses"), "{refused}");
}

#[test]
fn the_exclude_block_starts_a_fresh_file_without_a_blank_line() {
    let f = Fixture::new();
    std::fs::write(f.project.join(".git/info/exclude"), "").unwrap();
    f.prepared("amber-quiet-heron", "one");
    assert!(
        exclude_of(&f).starts_with(EXCLUDE_HEADER),
        "{}",
        exclude_of(&f)
    );
}

#[test]
fn the_exclude_block_follows_an_unterminated_line_on_a_line_of_its_own() {
    let f = Fixture::new();
    std::fs::write(f.project.join(".git/info/exclude"), "mine").unwrap();
    f.prepared("amber-quiet-heron", "one");
    assert!(
        exclude_of(&f).starts_with(&format!("mine\n{EXCLUDE_HEADER}\n")),
        "{}",
        exclude_of(&f)
    );
}

#[test]
fn the_exclude_block_follows_a_terminated_line_with_no_blank_between() {
    let f = Fixture::new();
    std::fs::write(f.project.join(".git/info/exclude"), "mine\n").unwrap();
    f.prepared("amber-quiet-heron", "one");
    assert!(
        exclude_of(&f).starts_with(&format!("mine\n{EXCLUDE_HEADER}\n")),
        "{}",
        exclude_of(&f)
    );
}

// A path the repository tracks that the run will overwrite cannot merge back.
#[test]
fn a_tracked_config_path_is_marked_skip_worktree() {
    let f = Fixture::new();
    std::fs::write(f.project.join("CLAUDE.md"), "tracked").unwrap();
    git(&f.project, &["add", "-A"]);
    git(&f.project, &["commit", "-q", "-m", "tracked config"]);
    let checkout = f.prepared("amber-quiet-heron", "reviewer");
    let wt = Path::new(&checkout.path);
    let flags = git(wt, &["ls-files", "-v", "CLAUDE.md"]);
    assert!(flags.starts_with("S "), "{flags}");
    std::fs::write(wt.join("CLAUDE.md"), "overwritten by the run").unwrap();
    assert_eq!(git(wt, &["status", "--porcelain"]).trim(), "");
}

#[test]
fn a_work_dir_that_cannot_be_read_is_an_error_for_its_own_session() {
    let f = Fixture::new();
    let work = f.work_dir("amber-quiet-heron");
    std::fs::create_dir_all(work.parent().unwrap()).unwrap();
    std::fs::write(&work, "not a directory").unwrap();
    let error = f.adapter().scratch(Some("amber-quiet-heron")).unwrap_err();
    assert_eq!(error.code(), "worktree-git-unreadable");
}

#[test]
fn only_a_single_path_segment_names_a_session() {
    for bad in ["", ".", "..", "a/b", "a\\b"] {
        assert!(!is_name(bad), "{bad:?}");
    }
    assert!(is_name("amber-quiet-heron"));
    let f = Fixture::new();
    assert!(!f.adapter().session_known("..").unwrap());
    assert!(f.adapter().scratch(Some("..")).unwrap().is_empty());
}

#[test]
fn paths_are_the_same_when_equal_or_when_they_resolve_alike() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real");
    std::fs::create_dir(&real).unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let gone = dir.path().join("gone");
    assert!(same_path(&gone, &gone), "equal, though neither exists");
    assert!(same_path(&real, &link));
    assert!(!same_path(&real, &gone));
}

#[test]
fn worktrees_nested_under_a_symlinked_target_are_found_by_their_resolved_paths() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real");
    std::fs::create_dir(&real).unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let listed = vec![
        Listed {
            path: real.clone(),
            branch: None,
        },
        Listed {
            path: real.join("inner"),
            branch: None,
        },
        Listed {
            path: dir.path().join("elsewhere"),
            branch: None,
        },
    ];
    assert_eq!(nested_under(&listed, &link), vec![real.join("inner")]);
}

#[test]
fn a_clean_nested_worktree_is_removed_with_its_parent() {
    let f = Fixture::new();
    let parent = f.prepared("amber-quiet-heron", "parent");
    let inner = Path::new(&parent.path).join("inner");
    git(
        Path::new(&parent.path),
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "inner-branch",
            &inner.display().to_string(),
        ],
    );
    assert_eq!(
        f.adapter().remove(&parent.repo_dir, &parent.path),
        Removal::Removed
    );
    assert!(!inner.exists());
    let listed = git(&f.project, &["worktree", "list", "--porcelain"]);
    assert!(!listed.contains("inner"), "{listed}");
}

// S11 hardening: a hung git stops the run with a diagnostic, and never reads
// as "not a repository", "clean" or "removed".
mod hung_git {
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    use super::*;
    use crate::{Git, GitError};

    /// A workspace whose git is a script that never finishes, with a tiny
    /// limit, over the fixture's home.
    fn hung(fixture: &Fixture) -> (tempfile::TempDir, GitWorkspaces) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("git");
        std::fs::write(&path, "#!/bin/sh\nexec sleep 30\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let git = Git::new(path, Duration::from_millis(20));
        // Run once, so the script is past the moment another thread's fork
        // may hold it open for writing (ETXTBSY); every use after is plain.
        while matches!(git.run(dir.path(), &["warm"]), Err(GitError::Said(why)) if why.contains("busy"))
        {
            std::thread::yield_now();
        }
        (dir, fixture.adapter().with_git(git))
    }

    #[test]
    fn preparing_with_a_hung_git_fails_with_the_timeout_not_a_degrade() {
        let fixture = Fixture::new();
        let (_stub, workspaces) = hung(&fixture);
        let request = fixture.request("amber-quiet-heron", "member-1");
        let Err(failure) = workspaces.prepare(&request) else {
            panic!("a hung git was read as an answer");
        };
        assert_eq!(failure.code(), "worktree-git-git-timed-out");
        assert!(!fixture
            .work_dir("amber-quiet-heron")
            .join(checkout_name("member-1"))
            .exists());
    }

    #[test]
    fn inspecting_with_a_hung_git_resolves_nothing() {
        let fixture = Fixture::new();
        let (_stub, workspaces) = hung(&fixture);
        let standing = workspaces.inspect(&fixture.project.display().to_string());
        let Standing::Unresolved(why) = standing else {
            panic!("a hung git resolved a repository: {standing:?}");
        };
        assert!(why.contains("did not finish within 20ms"), "{why}");
    }

    #[test]
    fn removing_with_a_hung_git_leaves_the_checkout_in_place() {
        let fixture = Fixture::new();
        let checkout = fixture.prepared("amber-quiet-heron", "member-1");
        let (_stub, workspaces) = hung(&fixture);
        let Removal::Left(why) = workspaces.remove(&checkout.repo_dir, &checkout.path) else {
            panic!("a hung git removed a checkout");
        };
        assert!(why.contains("did not finish within 20ms"), "{why}");
        assert!(Path::new(&checkout.path).is_dir());
    }
}
