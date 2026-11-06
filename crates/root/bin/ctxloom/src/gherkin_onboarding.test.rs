//! The steps of Go's `steps_j000800_onboarding.go` (and the Bob half of
//! `steps_j001800_guardrails.go`), as a child of the surfaces World: Carol's
//! team project and Bob's clone of it are `j000700`'s (a bare origin made in
//! the scenario's temporary directory), and every remote a scenario draws on
//! is a local bare repository too.
//!
//! "Bob starts a session" is, as in Go, `materialize default` into `out` for
//! claude-code, run in Bob's checkout.

use std::path::{Path, PathBuf};

use cucumber::{given, then, when};

use super::super::deps::git_in;
use super::super::gherkin_common::CommonWorld;
use super::super::Ran;
use super::{
    j000700_commit, j000700_project, j000700_pull, World, J000700_BUNDLE, J000700_PROFILE,
};
use crate::tests::companion_stub::Stub;
use crate::tests::files;

const TEAM_MARKER: &str = "J000800-TEAM-STANDARDIZED-CONTEXT-MARKER";
const PINNED_MARKER: &str = "J000800-PINNED-UPSTREAM-VERSION-MARKER";
const NEWER_MARKER: &str = "J000800-NEWER-UPSTREAM-VERSION-MARKER";
const COMPANY_MARKER: &str = "J000800-TRENT-COMPANY-CONTENT-MARKER";
const UPSTREAM_BUNDLE: &str = "upstream-bundle";
const COMPANY_BUNDLE: &str = "secure-standards";

/// What the onboarding scenarios keep between steps.
#[derive(Default)]
pub(super) struct State {
    /// The upstream's bare repository, which a later step advances.
    upstream: Option<PathBuf>,
    /// The commit the team's lockfile froze when Carol held the upstream
    /// bundle: what "the versions the team pinned" means.
    pinned_sha: String,
    /// The loadout the team's reprise guidance is, once a step says there is
    /// one.
    reprise_loadout: Option<String>,
    /// The last command Bob ran.
    last: Option<Ran>,
    /// The engine and target of the last `starts a session on`.
    session_on: Option<(String, String)>,
}

impl World {
    /// `ctxloom args` in Bob's checkout (on the `PATH` he has).
    fn bob_runs(&mut self, args: &[&str]) {
        let dir = self.bob_dir().to_path_buf();
        self.onboarding.last = Some(self.common.project.run_in(&dir, args));
    }

    /// Go's `bobMaterialized`: his default profile into `out`, again, and the
    /// `CLAUDE.md` it wrote.
    fn bob_materialized(&mut self) -> String {
        self.bob_runs(&[
            "materialize",
            J000700_PROFILE,
            "--target",
            "out",
            "--backend",
            "claude-code",
        ]);
        let said = self.onboarding.last.as_ref().expect("ran").said();
        read_in(self.bob_dir(), "out/CLAUDE.md")
            .unwrap_or_else(|| panic!("Bob's materialize wrote no out/CLAUDE.md: {said}"))
    }

    fn bob_has(&mut self, marker: &str) {
        let body = self.bob_materialized();
        assert!(
            body.contains(marker),
            "the materialized context for Bob lacks {marker:?}: {body}"
        );
    }

    fn bob_lacks(&mut self, marker: &str) {
        let body = self.bob_materialized();
        assert!(
            !body.contains(marker),
            "the materialized context for Bob has {marker:?} and should not: {body}"
        );
    }

    /// Go's `SeedTreeRemote`: a bare repository whose `main` holds the
    /// bundle `bundle`, with one fragment, `guidance`, carrying `marker`
    /// (and no description: that is a fragment's premise, and would withhold
    /// it from an ordinary assembly).
    fn seed_remote(&self, dir: &str, bundle: &str, marker: &str) -> PathBuf {
        let root = self.scratch().join(dir);
        let (bare, work) = (root.join("remote.git"), root.join("work"));
        files::mkdirs(&root);
        git_in(&root, &["init", "-q", "--bare", "-b", "main", "remote.git"]);
        git_in(&root, &["init", "-q", "-b", "main", "work"]);
        let tree = format!(".ctxloom/content/bundles/v2/{bundle}");
        files::put(
            work.join(format!("{tree}/bundle.yaml")),
            "schema_version: 4\nversion: 1.0.0\n",
        );
        files::put(
            work.join(format!("{tree}/fragments/guidance.md")),
            format!("{marker}\n"),
        );
        git_in(&work, &["add", "-A"]);
        git_in(&work, &["commit", "-q", "-m", "seed"]);
        git_in(
            &work,
            &["remote", "add", "origin", bare.to_str().expect("utf-8")],
        );
        git_in(&work, &["push", "-q", "origin", "main"]);
        bare
    }

    /// Go's `AdvanceTreeRemote`: a new commit that rewrites the fragment.
    fn advance_remote(&self, bare: &Path, bundle: &str, marker: &str) {
        let work = self.scratch().join("advance-work");
        git_in(
            &self.scratch(),
            &[
                "clone",
                "-q",
                bare.to_str().expect("utf-8"),
                work.to_str().expect("utf-8"),
            ],
        );
        files::put(
            work.join(format!(
                ".ctxloom/content/bundles/v2/{bundle}/fragments/guidance.md"
            )),
            format!("{marker}\n"),
        );
        git_in(&work, &["add", "-A"]);
        git_in(&work, &["commit", "-q", "-m", "advance"]);
        git_in(&work, &["push", "-q", "origin", "main"]);
    }

    /// Go's `remote create`, then the profile naming the bundle.
    fn draw_on(&mut self, remote: &str, bare: &Path, bundle: &str) {
        let url = format!("file://{}", bare.display());
        self.common.run_lines(
            &format!(
                "ctxloom remote create {remote} {url} --forge git\nctxloom profile modify {J000700_PROFILE} --add-bundle {remote}/{bundle}"
            ),
            "",
        );
        self.ran().succeeds();
    }
}

fn read_in(dir: &Path, rel: &str) -> Option<String> {
    files::read(dir.join(rel)).ok()
}

/// Go's `j000800LockedSHA`: the commit the lockfile records for `bundle`.
/// The lockfile's entries are keyed by reference, one level in.
fn locked_sha(raw: &str, bundle: &str) -> String {
    let mut inside = false;
    for line in raw.lines() {
        let indent = line.len() - line.trim_start().len();
        if indent == 2 {
            inside = line.trim_end().ends_with(':') && line.contains(bundle);
        } else if inside && indent > 2 {
            if let Some(sha) = line.trim().strip_prefix("sha:") {
                return sha.trim().to_string();
            }
        }
    }
    panic!("the lockfile has no sha for {bundle:?}:\n{raw}");
}

// ------------------------------------------------------------ Background

/// Carol's project, with the team's own first-party fragment in its bundle,
/// committed and pushed.
#[given("the team's project carries the context Carol has standardized on")]
fn team_context(w: &mut World) {
    j000700_project(w);
    w.common.project.write(
        &format!(".ctxloom/content/bundles/v2/{J000700_BUNDLE}/fragments/guidance.md"),
        &format!("{TEAM_MARKER}\n"),
    );
    // A later `deps pull` populates a git cache under .ctxloom/cache/: kept
    // out of the team's history.
    let ignore = w.common.project.read(".gitignore").unwrap_or_default();
    w.common
        .project
        .write(".gitignore", &format!("{ignore}.ctxloom/cache/\n"));
    j000700_commit(w, "author team standards");
}

#[given("Bob has just joined and has never used ctxloom before")]
fn joined(_w: &mut World) {
    // Narrative only: Bob's clone does not exist until he clones.
}

// ---------------------------------------------------------- the shared steps

#[given("Bob clones the project")]
#[when("Bob clones the project")]
fn clones(w: &mut World) {
    j000700_pull(w);
}

#[when("Bob fetches the context the project draws on")]
fn fetches(w: &mut World) {
    w.bob_runs(&["deps", "pull"]);
    w.onboarding.last.as_ref().expect("ran").succeeds();
}

#[when("Bob fetches the context the project draws on, reinstalling every reference")]
fn fetches_forced(w: &mut World) {
    w.bob_runs(&["deps", "pull", "--force"]);
    w.onboarding.last.as_ref().expect("ran").succeeds();
}

#[when("Bob starts a session")]
fn starts(w: &mut World) {
    w.bob_runs(&[
        "materialize",
        J000700_PROFILE,
        "--target",
        "out",
        "--backend",
        "claude-code",
    ]);
}

#[then("his assistant receives the team's standardized context")]
fn receives_team(w: &mut World) {
    w.bob_has(TEAM_MARKER);
}

#[then("his assistant still receives the team's own context, because the project is first-party")]
fn still_receives_team(w: &mut World) {
    w.bob_has(TEAM_MARKER);
}

#[then("nothing fails because of the companion's presence or absence")]
fn nothing_fails(w: &mut World) {
    let ran = w.onboarding.last.as_ref().expect("Bob ran a command");
    assert_eq!(
        ran.code,
        0,
        "Bob's session start should succeed whether or not a companion is installed: {}",
        ran.said()
    );
}

// ------------------------------------------------ the engine outline

#[when(regex = r"^Bob starts a session on (\S+)$")]
fn starts_on(w: &mut World, engine: String) {
    let target = format!("out-{engine}");
    w.bob_runs(&[
        "materialize",
        J000700_PROFILE,
        "--target",
        &target,
        "--backend",
        &engine,
    ]);
    w.onboarding.session_on = Some((engine, target));
}

#[then(
    regex = r"^his assistant receives the team's standardized context in (\S+)'s own native surface$"
)]
fn receives_native(w: &mut World, engine: String) {
    let (_, target) = w
        .onboarding
        .session_on
        .clone()
        .expect("Bob started a session on an engine");
    let file = match engine.as_str() {
        "claude-code" => "CLAUDE.md",
        "mock" => "MOCK_CONTEXT.md",
        other => panic!("no native context file known for {other}"),
    };
    let said = w.onboarding.last.as_ref().expect("ran").said();
    let body = read_in(w.bob_dir(), &format!("{target}/{file}"))
        .unwrap_or_else(|| panic!("{engine}: Bob's {target}/{file} was not written: {said}"));
    assert!(
        body.contains(TEAM_MARKER),
        "the materialized {engine} context for Bob lacks the team's marker: {body}"
    );
}

// ------------------------------------------------------ pinned, not latest

#[given("the project pins the versions of the context it draws from elsewhere")]
fn pins(w: &mut World) {
    let bare = w.seed_remote("upstream", UPSTREAM_BUNDLE, PINNED_MARKER);
    w.draw_on("upstream", &bare, UPSTREAM_BUNDLE);
    w.onboarding.upstream = Some(bare);
    w.common.run_lines(
        &format!("ctxloom deps pull\nctxloom deps hold upstream/{UPSTREAM_BUNDLE}"),
        "",
    );
    w.ran().succeeds();
    // The commit the hold froze: the concrete claim Bob's lockfile keeps or
    // breaks, so the scenario cannot pass by a pull that never resolved
    // anything.
    let raw = w
        .common
        .project
        .read(".ctxloom/lock.yaml")
        .expect("the team's lockfile");
    w.onboarding.pinned_sha = locked_sha(&raw, UPSTREAM_BUNDLE);
    j000700_commit(w, "pin upstream dependency");
}

#[given("an upstream has since published a newer version")]
fn newer(w: &mut World) {
    let bare = w.onboarding.upstream.clone().expect("an upstream");
    w.advance_remote(&bare, UPSTREAM_BUNDLE, NEWER_MARKER);
}

#[then("he receives the pinned versions, the same ones the rest of the team has")]
fn receives_pinned(w: &mut World) {
    assert!(
        !w.onboarding.pinned_sha.is_empty(),
        "the pinning step must run first"
    );
    let raw = read_in(w.bob_dir(), ".ctxloom/lock.yaml").expect("Bob's lockfile");
    let got = locked_sha(&raw, UPSTREAM_BUNDLE);
    assert_eq!(
        got, w.onboarding.pinned_sha,
        "the pull moved Bob's upstream bundle off the team's pin; his lockfile:\n{raw}"
    );
    w.bob_has(PINNED_MARKER);
}

#[then("he does not receive the newer upstream version")]
fn lacks_newer(w: &mut World) {
    w.bob_lacks(NEWER_MARKER);
}

// ------------------------------------------- referenced content survives

#[given("the project references a bundle published by Trent's company")]
fn references(w: &mut World) {
    let bare = w.seed_remote("company", COMPANY_BUNDLE, COMPANY_MARKER);
    w.draw_on("trent-company", &bare, COMPANY_BUNDLE);
    // Not pulled: the reference is committed unresolved, and Bob's own fetch
    // is the first pull, as a reference genuinely arrives at a team.
    j000700_commit(w, "reference trent's company bundle");
}

#[then("his assistant receives the company's content")]
fn receives_company(w: &mut World) {
    w.bob_has(COMPANY_MARKER);
}

// ------------------------------------------------ a companion Bob may not have

const REPRISE_MARKER: &str = "J000800-REPRISE-DEPENDENT-GUIDANCE-MARKER";

#[given(r#"the team's context includes guidance for the "reprise" companion"#)]
fn team_has_reprise_guidance(w: &mut World) {
    w.onboarding.reprise_loadout = Some(format!(
        "run:\n  version: 1.0.0\n  fragments:\n    guidance:\n      content: {REPRISE_MARKER}\n"
    ));
}

/// A companion is registered in the home with the command that runs it.
/// Installed is the stub companion (the compiled `ctxloom-companion-stub`,
/// asked for the team's guidance); not installed is a command whose program is
/// not there, registered all the same, so "nothing fails" is held with a
/// registration that cannot be run.
#[given(regex = r#"^the "(reprise|ltk)" companion is (installed|not installed) on Bob's machine$"#)]
fn companion_presence(w: &mut World, name: String, presence: String) {
    let scratch = w.scratch();
    if presence == "installed" {
        let loadout = w
            .onboarding
            .reprise_loadout
            .clone()
            .unwrap_or_else(|| panic!("no loadout was given for the {name:?} companion"));
        let stub = Stub::install(&scratch.join("companions"), &name, &loadout);
        let args = stub.add_args(&name);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let ran = w.common.project.run_with(&[], &args);
        ran.succeeds();
        return;
    }
    let gone = scratch.join("not-installed").join(&name);
    let held = w
        .common
        .project
        .read_home(".ctxloom/config.yaml")
        .unwrap_or_else(|| "schema_version: 9\n".to_string());
    let entry = format!("  {name}:\n    command: [{}]\n", gone.display());
    let config = if held.contains("\ncompanions:\n") {
        held.replacen("\ncompanions:\n", &format!("\ncompanions:\n{entry}"), 1)
    } else {
        format!("{held}companions:\n{entry}")
    };
    w.common.project.write_home(".ctxloom/config.yaml", &config);
}

#[then("his assistant receives the team's context that does not depend on reprise")]
fn receives_independent_context(w: &mut World) {
    w.bob_has(TEAM_MARKER);
}

#[then(regex = r"^the reprise-dependent guidance (reaches|does not reach) his assistant$")]
fn reprise_guidance(w: &mut World, reaches: String) {
    if reaches == "reaches" {
        w.bob_has(REPRISE_MARKER);
    } else {
        w.bob_lacks(REPRISE_MARKER);
    }
}
