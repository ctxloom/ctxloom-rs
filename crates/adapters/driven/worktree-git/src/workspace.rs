//! The adapter.

use std::path::{Path, PathBuf};

use isolation_ports::{
    Checkout, Failure, Prepared, Removal, Request, Safety, Scratch, Standing, Workspaces,
};

use crate::error::WorktreeError;
use crate::git::run;

/// The header of the block of ctxloom's per-agent config patterns in the
/// repository's shared `info/exclude` (Go: `gitignore.WorktreeComment`).
pub const EXCLUDE_HEADER: &str =
    "# ctxloom per-agent worktree config (isolation; NEVER merge back)";

/// What every checkout directory is named after.
const PREFIX: &str = "ctxloom-wt-";

/// The ref namespace every per-agent branch lives under.
const BRANCH_PREFIX: &str = "ctxloom/";

/// An agent id as one path segment: anything outside `[A-Za-z0-9_.-]` becomes
/// `-`, the ends are trimmed of separators, and an empty result is `agent`.
pub fn sanitize(agent: &str) -> String {
    let mut out = String::new();
    let mut in_run = false;
    for c in agent.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') {
            out.push(c);
            in_run = false;
        } else if !in_run {
            out.push('-');
            in_run = true;
        }
    }
    let trimmed = out.trim_matches(|c| matches!(c, '-' | '.' | '_'));
    if trimmed.is_empty() {
        "agent".to_string()
    } else {
        trimmed.to_string()
    }
}

/// The checkout directory's name for an agent: `ctxloom-wt-<agent>`.
pub fn checkout_name(agent: &str) -> String {
    format!("{PREFIX}{}", sanitize(agent))
}

/// The branch a checkout directory is on: its name with the scratch prefix
/// swapped for the `ctxloom/` namespace. A pure function of the path, so
/// whoever holds the checkout can name the branch with no side table.
pub fn branch_of(checkout: &str) -> String {
    let name = Path::new(checkout)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let rest = name.strip_prefix(PREFIX).unwrap_or(&name);
    format!("{BRANCH_PREFIX}{rest}")
}

/// Workspaces under one home's sessions.
pub struct GitWorkspaces {
    sessions: PathBuf,
}

impl GitWorkspaces {
    /// The sessions under `home` (`<home>/.ctxloom/sessions`).
    pub fn new(home: impl Into<PathBuf>) -> GitWorkspaces {
        GitWorkspaces {
            sessions: home.into().join(".ctxloom").join("sessions"),
        }
    }
}

/// One entry of `git worktree list --porcelain`.
struct Listed {
    path: PathBuf,
    /// The short branch name; none when detached.
    branch: Option<String>,
}

fn worktree_list(repo: &Path) -> Result<Vec<Listed>, String> {
    let out = run(repo, &["worktree", "list", "--porcelain"])?;
    let mut listed = Vec::new();
    for block in out.split("\n\n") {
        let mut path = None;
        let mut branch = None;
        for line in block.lines() {
            if let Some(p) = line.strip_prefix("worktree ") {
                path = Some(PathBuf::from(p));
            } else if let Some(b) = line.strip_prefix("branch refs/heads/") {
                branch = Some(b.to_string());
            }
        }
        if let Some(path) = path {
            listed.push(Listed { path, branch });
        }
    }
    Ok(listed)
}

/// Whether two paths name the same directory: equal, or both resolvable and
/// equal once resolved (a symlinked temp dir or home).
fn same_path(a: &Path, b: &Path) -> bool {
    a == b || matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y)
}

fn is_repo(dir: &Path) -> bool {
    dir.is_dir()
        && run(dir, &["rev-parse", "--is-inside-work-tree"]).is_ok_and(|out| out.trim() == "true")
}

/// The repository's common git directory, absolute.
fn common_dir(dir: &Path) -> Result<PathBuf, String> {
    let out = run(dir, &["rev-parse", "--git-common-dir"])?;
    let path = PathBuf::from(out.trim());
    let path = if path.is_absolute() {
        path
    } else {
        dir.join(path)
    };
    Ok(path.canonicalize().unwrap_or(path))
}

/// Go's `unsafeToRemove`: uncommitted changes, ignored files, or a state git
/// would not answer for. An unreadable state is never read as safe.
fn unsafe_to_remove(dir: &Path) -> Safety {
    match run(dir, &["--no-optional-locks", "status", "--porcelain"]) {
        Ok(out) if out.trim().is_empty() => {}
        _ => {
            return Safety::Unsafe("has uncommitted changes (or unknown state)".to_string());
        }
    }
    match run(dir, &["status", "--porcelain", "--ignored=matching"]) {
        Ok(out) if !out.lines().any(|l| l.starts_with("!!")) => Safety::Clean,
        _ => Safety::Unsafe("holds gitignored/excluded files (or unknown state)".to_string()),
    }
}

fn branch_exists(repo: &Path, branch: &str) -> bool {
    run(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .is_ok()
}

/// Create the checkout: re-enter the branch an earlier run left, or carve it
/// from the project's HEAD.
fn add_checkout(project: &Path, path: &Path, branch: &str) -> Result<(), String> {
    let shown = path.display().to_string();
    if branch_exists(project, branch) {
        run(project, &["worktree", "add", &shown, branch])
    } else {
        run(project, &["worktree", "add", "-b", branch, &shown, "HEAD"])
    }
    .map(drop)
}

/// Go's `reuseExistingCheckout`: whether the agent's path is its own earlier
/// checkout (registered on the agent's branch), to be re-entered; anything
/// else there is a refusal.
fn reuse(project: &Path, path: &Path, branch: &str) -> Result<bool, WorktreeError> {
    if !path.exists() {
        return Ok(false);
    }
    let refuse = |why: String| WorktreeError::Unadoptable {
        path: path.display().to_string(),
        branch: branch.to_string(),
        why,
    };
    let listed =
        worktree_list(project).map_err(|e| refuse(format!("git worktree list failed: {e}")))?;
    match listed.iter().find(|w| same_path(&w.path, path)) {
        Some(w) if w.branch.as_deref() == Some(branch) => Ok(true),
        Some(w) => Err(refuse(format!(
            "it is registered on branch {}",
            w.branch.as_deref().unwrap_or("(detached)")
        ))),
        None => Err(refuse(
            "it is not a registered git worktree of the project".to_string(),
        )),
    }
}

/// Hide the per-agent config from the project's git status: the block of
/// patterns in the shared common-dir `info/exclude`, appended where missing
/// (idempotent, and shared-safe among concurrent agents). Best-effort.
fn exclude_config(project: &Path, patterns: &[String]) {
    let Ok(common) = common_dir(project) else {
        return;
    };
    let info = common.join("info");
    let file = info.join("exclude");
    let existing = std::fs::read_to_string(&file).unwrap_or_default();
    let held: Vec<&str> = existing.lines().map(str::trim).collect();
    let missing: Vec<&String> = patterns
        .iter()
        .filter(|p| !held.contains(&p.as_str()))
        .collect();
    if missing.is_empty() {
        return;
    }
    let mut text = existing.clone();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    if !held.contains(&EXCLUDE_HEADER) {
        text.push_str(EXCLUDE_HEADER);
        text.push('\n');
    }
    for pattern in missing {
        text.push_str(pattern);
        text.push('\n');
    }
    if std::fs::create_dir_all(&info).is_ok() {
        let _ = std::fs::write(&file, text);
    }
}

/// Where the repository TRACKS a config path the run will overwrite, set the
/// skip-worktree bit, so it cannot merge back. Best-effort.
fn skip_tracked(checkout: &Path, patterns: &[String]) {
    if patterns.is_empty() {
        return;
    }
    let mut args = vec!["ls-files", "-z", "--"];
    args.extend(patterns.iter().map(String::as_str));
    let Ok(out) = run(checkout, &args) else {
        return;
    };
    for file in out.split('\0').filter(|f| !f.is_empty()) {
        let _ = run(checkout, &["update-index", "--skip-worktree", file]);
    }
}

/// The checkouts directly under a session's work directory. A missing work
/// directory is none; any other failure to read it is an error.
fn checkouts_in(work: &Path, harp: &str) -> Result<Vec<Scratch>, Failure> {
    let entries = match std::fs::read_dir(work) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(Box::new(WorktreeError::Unreadable {
                path: work.display().to_string(),
                why: e.to_string(),
            }))
        }
    };
    let mut found: Vec<Scratch> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| e.file_name().to_string_lossy().starts_with(PREFIX))
        .map(|e| Scratch {
            harp: harp.to_string(),
            path: e.path().display().to_string(),
        })
        .collect();
    found.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(found)
}

/// Whether `harp` is one path segment that could name a session.
fn is_name(harp: &str) -> bool {
    !harp.is_empty() && harp != "." && harp != ".." && !harp.contains(['/', '\\'])
}

/// Remove a nested or target worktree through git, never forced.
fn remove_one(repo: &Path, path: &Path) -> Result<(), String> {
    run(repo, &["worktree", "remove", &path.display().to_string()]).map(drop)
}

/// A merged branch holds nothing the repository does not, and left behind
/// every agent run adds one; an unmerged one holds commits nothing else has
/// and is kept. Any doubt keeps it.
fn delete_merged_branch(repo: &Path, branch: &str) {
    let merged = run(
        repo,
        &["branch", "--merged", "HEAD", "--format=%(refname:short)"],
    );
    if merged.is_ok_and(|out| out.lines().any(|l| l.trim() == branch)) {
        let _ = run(repo, &["branch", "-d", branch]);
    }
}

/// The worktrees strictly under `target`, deepest first.
fn nested_under(listed: &[Listed], target: &Path) -> Vec<PathBuf> {
    let mut prefixes = vec![target.to_path_buf()];
    if let Ok(resolved) = target.canonicalize() {
        if resolved != target {
            prefixes.push(resolved);
        }
    }
    let mut nested: Vec<PathBuf> = listed
        .iter()
        .map(|w| w.path.clone())
        .filter(|p| {
            prefixes
                .iter()
                .any(|prefix| p != prefix && p.starts_with(prefix))
        })
        .collect();
    nested.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    nested
}

impl Workspaces for GitWorkspaces {
    fn prepare(&self, request: &Request) -> Result<Prepared, Failure> {
        let project = Path::new(&request.project_dir);
        if !is_repo(project) {
            return Ok(Prepared::Unavailable(format!(
                "worktree isolation: {} is not a git repository",
                request.project_dir
            )));
        }
        let path = Path::new(&request.work_dir).join(checkout_name(&request.agent));
        let branch = branch_of(&path.display().to_string());
        let reused = reuse(project, &path, &branch).map_err(|e| Box::new(e) as Failure)?;
        if !reused {
            if let Err(why) = add_checkout(project, &path, &branch) {
                return Ok(Prepared::Unavailable(format!("worktree add: {why}")));
            }
        }
        exclude_config(project, &request.artifact_patterns);
        skip_tracked(&path, &request.artifact_patterns);
        Ok(Prepared::Checkout(Checkout {
            path: path.display().to_string(),
            branch,
            repo_dir: request.project_dir.clone(),
            reused,
        }))
    }

    fn session_known(&self, harp: &str) -> Result<bool, Failure> {
        Ok(is_name(harp) && self.sessions.join(harp).is_dir())
    }

    fn scratch(&self, harp: Option<&str>) -> Result<Vec<Scratch>, Failure> {
        if let Some(harp) = harp {
            if !is_name(harp) {
                return Ok(Vec::new());
            }
            return checkouts_in(&self.sessions.join(harp).join("work"), harp);
        }
        let Ok(entries) = std::fs::read_dir(&self.sessions) else {
            return Ok(Vec::new());
        };
        let mut all = Vec::new();
        for entry in entries.flatten() {
            if !entry.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            // A session whose work directory cannot be read has nothing to
            // sweep; only a single-session question reports it.
            if let Ok(found) = checkouts_in(&entry.path().join("work"), &name) {
                all.extend(found);
            }
        }
        all.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(all)
    }

    fn inspect(&self, path: &str) -> Standing {
        let dir = Path::new(path);
        match common_dir(dir) {
            Err(why) => Standing::Unresolved(why),
            Ok(common) => Standing::Resolved {
                repo_dir: common
                    .parent()
                    .map_or_else(|| common.clone(), Path::to_path_buf)
                    .display()
                    .to_string(),
                safety: unsafe_to_remove(dir),
            },
        }
    }

    fn remove(&self, repo_dir: &str, path: &str) -> Removal {
        let (repo, target) = (Path::new(repo_dir), Path::new(path));
        let listed = match worktree_list(repo) {
            Ok(listed) => listed,
            Err(why) => {
                return Removal::Left(format!(
                    "cannot list worktrees; left in place to avoid destroying nested work: {why}"
                ))
            }
        };
        for inner in nested_under(&listed, target) {
            if let Safety::Unsafe(why) = unsafe_to_remove(&inner) {
                return Removal::Left(format!(
                    "nested worktree {} {why}; left in place to preserve it",
                    inner.display()
                ));
            }
            if let Err(why) = remove_one(repo, &inner) {
                return Removal::Left(format!(
                    "cannot remove nested worktree {}: {why}",
                    inner.display()
                ));
            }
        }
        if let Safety::Unsafe(why) = unsafe_to_remove(target) {
            return Removal::Left(format!("it {why}; left in place to preserve WIP"));
        }
        if let Err(why) = remove_one(repo, target) {
            return Removal::Left(format!("cannot remove it: {why}"));
        }
        let _ = run(repo, &["worktree", "prune"]);
        delete_merged_branch(repo, &branch_of(path));
        if target.exists() {
            Removal::Left("it is still on disk after git removed it".to_string())
        } else {
            Removal::Removed
        }
    }
}
