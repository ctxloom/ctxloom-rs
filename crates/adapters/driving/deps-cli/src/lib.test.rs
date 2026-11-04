//! The remote and dependency commands as the program runs them: the command
//! line parsed by this crate's own nouns, the application over in-memory
//! ports, the answer as bytes.

use angzarr_presentation::{Catalog, Format};
use clap::Command;
use deps_application::fakes::{demo, FakeRefresh, Fixture, BUNDLES};

use super::*;

const URL: &str = "file:///srv/origin.git";

struct Ran {
    exit: Result<u8, (String, u8)>,
    out: String,
    err: String,
}

impl Ran {
    fn ok(&self) -> &Ran {
        assert_eq!(self.exit, Ok(0), "out {:?} err {:?}", self.out, self.err);
        self
    }

    fn refused(&self) -> (&str, u8) {
        match &self.exit {
            Err((code, exit)) => (code.as_str(), *exit),
            Ok(code) => panic!("exit {code}: {:?}", self.out),
        }
    }

    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.out).unwrap_or_else(|e| panic!("{e}: {}", self.out))
    }
}

#[derive(Clone, Copy)]
struct How {
    explicit: Option<Format>,
    terminal: bool,
}

const JSON: How = How {
    explicit: Some(Format::Json),
    terminal: false,
};

const TEXT: How = How {
    explicit: Some(Format::Text),
    terminal: false,
};

fn program() -> Command {
    let mut bundle = Command::new("bundle").subcommand_required(false);
    for verb in bundle_verbs() {
        bundle = bundle.subcommand(verb);
    }
    Command::new("ctxloom")
        .subcommand_required(true)
        .subcommands(nouns())
        .subcommand(bundle)
}

fn go(fx: &Fixture, line: &[&str], how: How) -> Ran {
    go_refreshing(fx, &FakeRefresh::default(), line, how)
}

fn go_refreshing(fx: &Fixture, refresh: &FakeRefresh, line: &[&str], how: How) -> Ran {
    let mut argv = vec!["ctxloom"];
    argv.extend_from_slice(line);
    let matches = program()
        .try_get_matches_from(argv)
        .unwrap_or_else(|e| panic!("{line:?} does not parse: {e}"));
    let noun = matches.subcommand_name().expect("a noun").to_string();
    let call = Call {
        stdout_is_terminal: how.terminal,
        ..Call::new(&matches, how.explicit, Format::Json)
    };
    let catalog = Catalog::en_us(&[deps_application::EN_US, deps::EN_US, angzarr_yaml::EN_US])
        .expect("a catalog");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let exit = run(
        &fx.deps, refresh, &noun, &call, &catalog, &mut out, &mut err,
    )
    .map_err(|f| (f.cause.code().to_string(), f.exit));
    Ran {
        exit,
        out: String::from_utf8(out).expect("utf-8"),
        err: String::from_utf8(err).expect("utf-8"),
    }
}

fn demo_ref() -> String {
    Fixture::reference(URL, "demo")
}

fn pulled() -> (Fixture, String) {
    let fx = Fixture::new();
    let sha = fx.origin("origin", URL);
    fx.profile("dev", &[&demo_ref()]);
    go(&fx, &["deps", "pull"], JSON).ok();
    (fx, sha)
}

// ---------------------------------------------------------------- remote

#[test]
fn test_remote_list_json_and_text_and_the_bare_noun() {
    let fx = Fixture::new();
    let empty = go(&fx, &["remote", "list"], JSON);
    assert_eq!(empty.json(), serde_json::json!({"remotes": [], "count": 0}));
    assert_eq!(
        go(&fx, &["remote"], TEXT).ok().out,
        "No remotes configured.\nUse 'ctxloom remote create <name> <url>' to add a remote.\nUse 'ctxloom remote discover' to find public repositories.\n"
    );
    fx.register("zeta", "file:///z.git");
    fx.register("alpha", "file:///a.git");
    go(&fx, &["remote", "default", "zeta"], JSON).ok();
    assert_eq!(
        go(&fx, &["remote", "ls"], JSON).json(),
        serde_json::json!({"remotes": [{"name": "alpha", "url": "file:///a.git"}, {"name": "zeta", "url": "file:///z.git"}], "count": 2, "default": "zeta"})
    );
    assert_eq!(
        go(&fx, &["remote", "list"], TEXT).ok().out,
        "Configured remotes:\n  alpha           file:///a.git\n  zeta            file:///z.git (default)\n"
    );
}

#[test]
fn test_remote_create_says_what_was_added_and_warns_of_a_clone_that_failed() {
    let fx = Fixture::new();
    fx.remotes.serve(URL);
    let created = go(
        &fx,
        &["remote", "create", "origin", URL, "--forge", "git"],
        TEXT,
    );
    assert_eq!(
        created.ok().out,
        format!("Added remote 'origin' \u{2192} {URL}\n")
    );
    assert_eq!(created.err, "");
    let json = go(
        &fx,
        &["remote", "create", "gone", "file:///nonexistent/x.git"],
        JSON,
    );
    json.ok();
    assert_eq!(json.json()["status"], "added");
    assert!(json.json()["warning"]
        .as_str()
        .expect("a warning")
        .starts_with("gone: clone failed: "));
    let said: serde_json::Value = serde_json::from_str(json.err.trim_end()).expect("a json line");
    assert_eq!(said["prog"], "ctxloom");
    assert!(said["warning"]
        .as_str()
        .expect("words")
        .starts_with("gone: clone failed: "));
    let refused = go(&fx, &["remote", "create", "again", URL], TEXT);
    assert_eq!(refused.refused(), ("deps-same-repository", 1));
}

#[test]
fn test_a_warning_under_text_is_words_with_its_fix() {
    let fx = Fixture::new();
    let said = go(
        &fx,
        &["remote", "create", "gone", "file:///nonexistent/x.git"],
        TEXT,
    );
    said.ok();
    assert!(
        said.err
            .starts_with("ctxloom: warning: gone: clone failed: "),
        "{}",
        said.err
    );
}

#[test]
fn test_remote_edit_reads_a_flag_given_even_when_it_is_empty() {
    let fx = Fixture::new();
    fx.register("origin", URL);
    let nothing = go(&fx, &["remote", "edit", "origin"], TEXT);
    assert_eq!(nothing.refused(), ("deps-nothing-to-edit", 1));
    let same = go(&fx, &["remote", "edit", "origin", "--name", "origin"], TEXT);
    assert_eq!(same.ok().out, "remote \"origin\" unchanged\n");
    let rebound = go(
        &fx,
        &["remote", "edit", "origin", "--forge", "github"],
        TEXT,
    );
    assert_eq!(
        rebound.ok().out,
        "Edited remote \"origin\" (forge)\n  forge: git \u{2192} github\n"
    );
    let cleared = go(&fx, &["remote", "edit", "origin", "--forge", ""], TEXT);
    assert_eq!(
        cleared.ok().out,
        "Edited remote \"origin\" (forge)\n  forge: github \u{2192} (resolved from URL host)\n"
    );
    go(&fx, &["remote", "default", "origin"], JSON).ok();
    let moved = go(
        &fx,
        &[
            "remote",
            "edit",
            "origin",
            "--name",
            "up",
            "--url",
            "file:///moved.git",
        ],
        TEXT,
    );
    assert_eq!(
        moved.ok().out,
        format!("Edited remote \"up\" (name, url)\n  name:  origin \u{2192} up\n  url:   {URL} \u{2192} file:///moved.git\n  the default remote followed the rename\n")
    );
    let json = go(&fx, &["remote", "edit", "up", "--forge", "git"], JSON);
    let json = json.ok().json();
    assert_eq!(json["status"], "edited");
    assert_eq!(json["changed"], serde_json::json!(["forge"]));
    assert_eq!(json["before"]["name"], "up");
    assert!(json.get("default_pointer_updated").is_none());
}

#[test]
fn test_remote_remove_previews_and_then_removes() {
    let fx = Fixture::new();
    fx.register("origin", URL);
    let preview = go(&fx, &["remote", "remove", "origin"], TEXT);
    assert_eq!(
        preview.ok().out,
        "Would remove remote \"origin\".\n\nNothing was removed. Re-run with --yes to apply:\n  ctxloom remote remove origin --yes\n"
    );
    assert_eq!(
        go(&fx, &["remote", "rm", "origin"], JSON).json(),
        serde_json::json!({"status": "preview", "applied": false, "target": "remote \"origin\"", "apply": "ctxloom remote remove origin --yes"})
    );
    assert_eq!(
        go(&fx, &["remote", "remove", "origin", "--yes"], TEXT)
            .ok()
            .out,
        "Removed remote 'origin'\n"
    );
    assert_eq!(
        go(&fx, &["remote", "remove", "origin"], TEXT).refused(),
        ("deps-application-no-remote-named", 1)
    );
    assert_eq!(
        go(&fx, &["remote", "remove", "origin", "-y"], JSON).refused(),
        ("deps-remote-not-found", 1)
    );
}

#[test]
fn test_remote_default_sets_clears_and_wants_a_name() {
    let fx = Fixture::new();
    fx.register("origin", URL);
    assert_eq!(
        go(&fx, &["remote", "default", "origin"], TEXT).ok().out,
        "Set default remote to: origin\n"
    );
    assert_eq!(
        go(&fx, &["remote", "default", "--clear"], TEXT).ok().out,
        "Cleared default remote.\n"
    );
    assert_eq!(
        go(&fx, &["remote", "default", "origin"], JSON).json(),
        serde_json::json!({"status": "set", "name": "origin"})
    );
    assert_eq!(
        go(&fx, &["remote", "default", "--clear"], JSON).json(),
        serde_json::json!({"status": "cleared"})
    );
    assert_eq!(
        go(&fx, &["remote", "default"], TEXT).refused(),
        ("deps-application-default-needs-name", 2)
    );
}

#[test]
fn test_remote_show_lists_what_is_published_and_what_is_not() {
    let fx = Fixture::new();
    fx.origin("origin", URL);
    assert_eq!(
        go(&fx, &["remote", "show", "origin", "--recursive=false"], TEXT).ok().out,
        format!("Bundles in origin ({URL}):\n\n  {}\n\nUse one: add its ref to a profile (ctxloom profile create/edit), then ctxloom deps pull\n", demo_ref())
    );
    let json = go(&fx, &["remote", "show", "origin", "-r"], JSON);
    assert_eq!(json.json()["items"][0]["pull_ref"], demo_ref());
    assert_eq!(json.json()["count"], 1);
    fx.remotes.serve("file:///srv/empty.git");
    fx.remotes
        .commit("file:///srv/empty.git", &[("README.md", "x")]);
    fx.register("empty", "file:///srv/empty.git");
    assert_eq!(
        go(&fx, &["remote", "show", "empty"], TEXT).ok().out,
        "No bundles found in empty\n"
    );
    assert_eq!(
        go(&fx, &["remote", "show", "nope"], TEXT).refused(),
        ("deps-application-browse", 1)
    );
}

fn forge_repo(owner: &str, name: &str, stars: u64, description: &str) -> deps_ports::ForgeRepo {
    deps_ports::ForgeRepo {
        owner: owner.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        stars,
        url: format!("https://github.com/{owner}/{name}"),
    }
}

fn two_repositories() -> Fixture {
    let fx = Fixture::new();
    fx.forge.serves(vec![
        forge_repo("bob", "ctxloom-golang", 10, "bob's context"),
        forge_repo("alice", "ctxloom", 3, "alice's context"),
    ]);
    fx
}

// remote_discover.go: the table, under the progress line the stream carries.
#[test]
fn test_remote_discover_prints_the_repositories_as_a_table_and_how_to_add_one() {
    let fx = two_repositories();
    let ran = go(&fx, &["remote", "discover"], TEXT);
    let (bar, rule, cross) = ("\u{2502}", "\u{2500}", "\u{253c}");
    let long = rule.repeat(20);
    let tail = rule.repeat(37);
    assert_eq!(
        ran.ok().out,
        format!(
            "  # {bar} Forge  {bar} Repository         {bar} Stars {bar} Description\n\
             {r4}{cross}{r8}{cross}{long}{cross}{r7}{cross}{tail}\n\
             \x20 1 {bar} GitHub {bar} bob/ctxloom-golang {bar}    10 {bar} bob's context\n\
             \x20 2 {bar} GitHub {bar} alice/ctxloom      {bar}     3 {bar} alice's context\n\
             \n\
             Add one with: ctxloom remote create <name> <url>\n",
            r4 = rule.repeat(4),
            r8 = rule.repeat(8),
            r7 = rule.repeat(7),
        )
    );
    assert_eq!(ran.err, "Searching repositories... found 2\n");
}

// remote_discover.go renderDiscoveredRepos: the Repository column is as wide as
// its widest name, only the Description is cut, and what a stranger wrote
// cannot drive the terminal.
#[test]
fn test_the_table_cuts_a_long_description_and_never_a_name_and_escapes_the_rest() {
    let fx = Fixture::new();
    fx.forge.serves(vec![
        forge_repo(
            "a",
            "ctxloom",
            1,
            "a description far longer than thirty-five bytes",
        ),
        forge_repo("b", "ctxloom-esc", 2, "\u{1b}[31mred"),
    ]);
    let ran = go(&fx, &["remote", "discover"], TEXT);
    let out = ran.ok().out.clone();
    assert!(
        out.contains("a description far longer than th\u{2026}\n"),
        "{out}"
    );
    assert!(out.contains("\\u{1b}[31mred"), "{out}");
    assert!(!out.contains('\u{1b}'), "{out:?}");
}

// Columns are counted in terminal width, so a wide character takes two.
#[test]
fn test_the_table_aligns_wide_characters_by_display_width() {
    let fx = Fixture::new();
    fx.forge.serves(vec![
        forge_repo(
            "a",
            "\u{65e5}\u{672c}\u{8a9e}\u{306e}\u{540d}\u{524d}",
            1,
            "d",
        ),
        forge_repo("b", "ab", 2, "d"),
    ]);
    let out = go(&fx, &["remote", "discover"], TEXT).ok().out.clone();
    assert!(
        out.contains("\u{2502} a/\u{65e5}\u{672c}\u{8a9e}\u{306e}\u{540d}\u{524d} \u{2502}"),
        "{out}"
    );
    assert!(out.contains("\u{2502} b/ab           \u{2502}"), "{out}");
}

#[test]
fn test_remote_discover_json_is_the_repositories_the_count_and_the_errors() {
    let fx = two_repositories();
    let ran = go(&fx, &["remote", "discover", "--stars", "5"], JSON);
    assert_eq!(
        ran.ok().json(),
        serde_json::json!({
            "repositories": [{
                "owner": "bob",
                "name": "ctxloom-golang",
                "description": "bob's context",
                "stars": 10,
                "url": "https://github.com/bob/ctxloom-golang",
                "forge": "github",
                "add_command": "ctxloom remote create ctxloom-golang bob/ctxloom-golang",
            }],
            "count": 1,
            "errors": [],
        })
    );
}

#[test]
fn test_remote_discover_says_when_nothing_matches_and_how_to_widen_the_search() {
    let fx = Fixture::new();
    assert_eq!(
        go(&fx, &["remote", "discover"], TEXT).ok().out,
        "No ctxloom repositories found.\n"
    );
    assert_eq!(
        go(&fx, &["remote", "discover", "golang", "tools"], TEXT)
            .ok()
            .out,
        "No ctxloom repositories found.\nTry a different search term or remove the filter.\n"
    );
}

#[test]
fn test_remote_discover_hands_its_words_flags_and_source_to_the_search() {
    let fx = two_repositories();
    go(
        &fx,
        &[
            "remote", "discover", "golang", "tools", "--stars", "4", "-n", "5", "-s", "github",
        ],
        TEXT,
    )
    .ok();
    let (_, query, limit) = fx.forge.searches().remove(0);
    assert_eq!((query.as_str(), limit), ("golang tools", 5));
    assert_eq!(
        go(&fx, &["remote", "discover", "-s", "gitlab"], TEXT).refused(),
        ("deps-application-discover-source", 2)
    );
}

// A search that failed closes the progress line, warns why, and fails.
#[test]
fn test_a_remote_discover_that_failed_closes_the_progress_line_and_warns_why() {
    let fx = two_repositories();
    fx.forge
        .fail_search("search failed: HTTP 403: rate limit exceeded");
    let ran = go(&fx, &["remote", "discover"], TEXT);
    assert_eq!(ran.refused(), ("deps-application-discover-failed", 1));
    assert_eq!(
        ran.err,
        "Searching repositories...\nctxloom: warning: github: search failed: HTTP 403: rate limit exceeded\n"
    );
    assert_eq!(ran.out, "");
}

// ------------------------------------------------------------------ deps

#[test]
fn test_deps_list_in_both_formats_and_the_bare_noun() {
    let fx = Fixture::new();
    assert_eq!(
        go(&fx, &["deps"], JSON).json(),
        serde_json::json!({"deps": [], "count": 0})
    );
    assert_eq!(
        go(&fx, &["deps", "list"], TEXT).ok().out,
        "No dependencies installed.\nA profile that names a remote bundle gets one; run 'ctxloom deps pull' to install it.\n"
    );
    let (fx, sha) = pulled();
    go(&fx, &["deps", "hold", "origin/demo"], JSON).ok();
    assert_eq!(
        go(&fx, &["deps", "ls"], TEXT).ok().out,
        format!(
            "Installed dependencies:\n  demo                 {}    origin  [held]\n",
            &sha[..7]
        )
    );
    fx.files.set_lock(&format!(
        "schema_version: 3\nbundles:\n  {}:\n    sha: {sha}\n    url: {URL}\n    requested_version: main\n",
        demo_ref()
    ));
    fx.files.set_registry("schema_version: 1\n");
    assert_eq!(
        go(&fx, &["deps"], TEXT).ok().out,
        format!(
            "Installed dependencies:\n  demo                 {}    {URL}  (main)\n",
            &sha[..7]
        )
    );
    let json = go(&fx, &["deps", "list"], JSON).json();
    assert_eq!(json["deps"][0]["constraint"], "main");
    assert_eq!(json["deps"][0]["held"], false);
    assert!(json["deps"][0].get("origin").is_none());
}

#[test]
fn test_deps_pull_text_says_what_it_installed_kept_and_removed() {
    let fx = Fixture::new();
    let sha = fx.origin("origin", URL);
    fx.profile("dev", &[&demo_ref()]);
    let first = go(&fx, &["deps", "pull"], TEXT);
    first.ok();
    assert_eq!(first.err, "Pulling dependencies...\n");
    let want = format!(
        "\nDependencies (1):\n  Installed: 1\nNew pins, with everything each one brings in:\n{} first pin -> unversioned  (new -> {})\n  + fragment demo-frag\n  + file bundle.yaml\n  + file fragments/demo-frag.md\n",
        format_args!("{} ", demo_ref()).to_string().trim_end(),
        &sha[..7]
    )
    .replace(&format!("{}  first", demo_ref()), &format!("{}  first", demo_ref()));
    assert_eq!(
        first.out.replace("  first pin", " first pin"),
        want.replace("  first pin", " first pin")
    );
    let again = go(&fx, &["deps", "pull"], TEXT);
    assert_eq!(
        again.ok().out,
        "\nDependencies (1):\n  Skipped (kept at their locked commit): 1\n    Pull never moves an existing pin and does not ask upstream whether one could move.\n    Run 'ctxloom deps check' to find out.\n"
    );
    let forced = go(&fx, &["deps", "pull", "--force"], TEXT);
    assert!(
        forced.ok().out.contains("  Reinstalled at their pin: 1\n"),
        "{}",
        forced.out
    );
    assert_eq!(
        go(&fx, &["deps", "pull", "-f", "--lock=false"], JSON).json()["reconcile"],
        serde_json::Value::Null
    );
}

#[test]
fn test_a_pull_over_nothing_says_so() {
    let fx = Fixture::new();
    assert_eq!(
        go(&fx, &["deps", "pull"], TEXT).ok().out,
        "No remote dependencies to pull.\n"
    );
    assert_eq!(go(&fx, &["deps", "pull"], JSON).json()["status"], "empty");
}

#[test]
fn test_a_pull_says_what_upstream_withdrew_and_what_it_could_not_check() {
    let (fx, _) = pulled();
    fx.remotes.set_unreachable(URL, true);
    let unreachable = go(&fx, &["deps", "pull"], TEXT);
    let out = unreachable.ok().out.clone();
    assert!(
        out.contains(&format!(
            "\n{URL} could not be reached, so its dependencies were left exactly as they are ("
        )),
        "{out}"
    );
    assert!(out.contains(&format!("  - kept: {}\n  Nothing is removed on the strength of a remote that could not be read.\n", demo_ref())), "{out}");
    assert!(
        unreachable
            .err
            .contains("ctxloom: warning: fetch file:///srv/origin.git: "),
        "{}",
        unreachable.err
    );
    fx.remotes.set_unreachable(URL, false);
    fx.remotes.delete(
        URL,
        &[
            &format!("{BUNDLES}/demo/bundle.yaml"),
            &format!("{BUNDLES}/demo/fragments/demo-frag.md"),
        ],
    );
    let gone = go(&fx, &["deps", "pull"], TEXT);
    assert!(
        gone.ok().out.ends_with(&format!(
            "\nRemoved 1 dependency(ies) no longer published by their remote:\n  - {}\n  Re-adding them upstream and pulling again restores them; nothing authored here was touched.\n",
            demo_ref()
        )),
        "{}",
        gone.out
    );
}

#[test]
fn test_a_pull_names_a_part_of_the_closure_it_could_not_reach_and_a_changed_constraint() {
    let (fx, sha) = pulled();
    fx.register("gone", "file:///nonexistent/gone");
    let kit = Fixture::reference("file:///nonexistent/gone", "kit");
    fx.content.bundle(
        "project",
        &[
            deps_application::fakes::ENVELOPE,
            (
                "profiles/dev.yaml",
                &format!("bundles:\n  - {}@main\n", demo_ref()),
            ),
            (
                "profiles/orphan.yaml",
                &format!("parents:\n  - {kit}#profiles/p\n"),
            ),
        ],
    );
    let ran = go(&fx, &["deps", "pull"], TEXT);
    let out = ran.ok().out.clone();
    assert!(
        out.contains(&format!(
            "  Lock incomplete: could not reach {kit}; their previous lock entries were kept.\n"
        )),
        "{out}"
    );
    assert!(
        out.contains(&format!("  {}: the manifest now asks for main; the pin stays at {} (resolved from the default branch).\n  Only 'ctxloom deps upgrade' moves a pin: run it to see the change, then 'ctxloom deps upgrade --yes' to apply it.\n", demo_ref(), &sha[..7])),
        "{out}"
    );
}

#[test]
fn test_a_pull_that_failed_prints_its_answer_first_then_exits_failed() {
    let fx = Fixture::new();
    fx.remotes.serve(URL);
    fx.remotes
        .commit(URL, &[(&format!("{BUNDLES}/demo/note.txt"), "x")]);
    fx.register("origin", URL);
    fx.profile("dev", &[&demo_ref()]);
    let ran = go(&fx, &["deps", "pull"], TEXT);
    assert_eq!(ran.refused(), ("deps-application-pull-failed", 1));
    assert!(ran.out.contains("  Failed: 1\n"), "{}", ran.out);
    assert!(
        ran.out.contains(&format!("    - {}: ", demo_ref())),
        "{}",
        ran.out
    );
    assert!(
        ran.out
            .contains("      fix: check network/auth and retry (ctxloom deps pull)"),
        "{}",
        ran.out
    );
}

#[test]
fn test_a_pull_says_what_it_dropped_and_pruned() {
    let (fx, _) = pulled();
    fx.origin("other", "file:///srv/other.git");
    let other = Fixture::reference("file:///srv/other.git", "demo");
    fx.profile("dev", &[&demo_ref(), &other]);
    go(&fx, &["deps", "pull"], JSON).ok();
    fx.profile("dev", &[&demo_ref()]);
    let out = go(&fx, &["deps", "pull"], TEXT).ok().out.clone();
    assert!(out.contains(&format!("  Removed {other} from the lockfile: nothing this project composes depends on it any more.\n")), "{out}");
    assert!(
        out.contains(&format!(
            "  Pruned the checkout at {other}: the lockfile no longer names its bundle.\n"
        )),
        "{out}"
    );
}

#[test]
fn test_deps_check_in_its_five_voices() {
    let fx = Fixture::new();
    let nothing = go(&fx, &["deps", "check"], TEXT);
    assert_eq!(
        nothing.ok().out,
        "Nothing is installed, so there is nothing to check.\nInstall this project's closure with: ctxloom deps pull\n"
    );
    assert_eq!(nothing.err, "");
    let (fx, sha) = pulled();
    let current = go(&fx, &["deps", "check"], TEXT);
    assert_eq!(current.ok().out, "All items are up to date!\n");
    assert_eq!(current.err, "Checking 1 items for updates...\n");
    let later = fx.remotes.commit(URL, &demo("1.1.0", "two"));
    assert_eq!(
        go(&fx, &["deps", "check"], TEXT).ok().out,
        format!("Found 1 items with updates available:\n\nBundles:\n  {}  (tracking branch default branch)\n    Current: {} \u{2192} Latest: {}\n\nRun 'ctxloom deps upgrade' to advance these pins.\n", demo_ref(), &sha[..7], &later[..7])
    );
    let json = go(&fx, &["deps", "check"], JSON).json();
    assert_eq!(json["updates"][0]["latest_sha"], later);
    assert_eq!(json["updates"][0]["selector"], "branch");
    assert_eq!(json["missing_default_profiles"], serde_json::json!([]));
    fx.remotes.set_unreachable(URL, true);
    let unreachable = go(&fx, &["deps", "check"], TEXT);
    let out = unreachable.ok().out.clone();
    assert!(
        out.contains(&format!(
            "  {}: not checked \u{2014} {URL} could not be fetched (see warning above)\n",
            demo_ref()
        )),
        "{out}"
    );
    assert!(out.ends_with("No updates found among the entries that could be checked \u{2014} 1 entry(ies) could not be checked (see warnings above).\n"), "{out}");
    assert!(
        unreachable
            .err
            .contains("ctxloom: warning: fetch file:///srv/origin.git: "),
        "{}",
        unreachable.err
    );
}

#[test]
fn test_deps_check_describes_a_range_and_says_what_it_could_not_resolve() {
    let (fx, sha) = pulled();
    fx.files.set_lock(&format!(
        "schema_version: 3\nbundles:\n  {}:\n    sha: {sha}\n    url: {URL}\n    requested_version: ^1.0\n    version: v1.0.0\n    kind: version\n",
        demo_ref()
    ));
    let ran = go(&fx, &["deps", "check"], TEXT);
    ran.ok();
    assert!(
        ran.err.contains("could not resolve \"^1.0\""),
        "{}",
        ran.err
    );
    assert!(
        ran.err.contains("skipping the update check for it"),
        "{}",
        ran.err
    );
    let json = go(&fx, &["deps", "check"], JSON);
    let said: Vec<serde_json::Value> = json
        .err
        .lines()
        .filter(|l| l.starts_with('{'))
        .map(|l| serde_json::from_str(l).expect("json"))
        .collect();
    assert!(
        said.iter().any(|w| w["warning"]
            .as_str()
            .is_some_and(|t| t.contains("could not resolve"))),
        "{said:?}"
    );
}

#[test]
fn test_deps_check_of_one_reference() {
    let (fx, sha) = pulled();
    assert_eq!(
        go(&fx, &["deps", "check", "origin/demo"], TEXT).ok().out,
        format!("origin/demo is up to date (SHA: {})\n", &sha[..7])
    );
    let later = fx.remotes.commit(URL, &demo("1.1.0", "two"));
    assert_eq!(
        go(&fx, &["deps", "check", &demo_ref()], TEXT).ok().out,
        format!("{} has update available:\n  Current: {}\n  Latest:  {}\n\nRun 'ctxloom deps upgrade' to advance it.\n", demo_ref(), &sha[..7], &later[..7])
    );
    let other = Fixture::reference(URL, "other");
    assert_eq!(
        go(&fx, &["deps", "check", &other], TEXT).ok().out,
        format!("{other} not found in lockfile, checking latest version...\n\nRun 'ctxloom deps upgrade' to advance it.\n")
    );
    assert_eq!(
        go(&fx, &["deps", "check", &demo_ref()], JSON).json()["up_to_date"],
        false
    );
    assert_eq!(
        go(&fx, &["deps", "check", "demo"], TEXT).refused(),
        ("deps-application-not-remote-reference", 1)
    );
}

#[test]
fn test_deps_upgrade_previews_applies_and_says_when_there_is_nothing() {
    let (fx, sha) = pulled();
    let current = go(&fx, &["deps", "upgrade"], TEXT);
    assert_eq!(current.ok().out, "Everything is up to date.\n");
    assert_eq!(
        current.err,
        "Resolving latest commits for pinned dependencies...\n"
    );
    let later = fx.remotes.commit(URL, &demo("1.1.0", "two edition"));
    let preview = go(&fx, &["deps", "upgrade"], TEXT);
    assert_eq!(
        preview.ok().out,
        format!("{}  unversioned -> unversioned  ({} -> {})  [compatible]\n  ~ fragment demo-frag\n  ~ file bundle.yaml\n  ~ file fragments/demo-frag.md\n1 pin(s) would move. Re-run with --yes to apply.\n", demo_ref(), &sha[..7], &later[..7])
    );
    let applied = go(&fx, &["deps", "upgrade", "--yes"], TEXT);
    assert!(
        applied.ok().out.ends_with("Applied 1 pin(s).\n"),
        "{}",
        applied.out
    );
    let json = go(&fx, &["deps", "upgrade", "-y"], JSON).json();
    assert_eq!(json["applied"], true);
    assert_eq!(json["changes"], serde_json::json!([]));
    assert_eq!(json["removed"], serde_json::json!([]));
}

#[test]
fn test_deps_upgrade_names_what_it_drops_what_it_could_not_reach_and_what_is_not_declared() {
    let (fx, _) = pulled();
    fx.origin("other", "file:///srv/other.git");
    let other = Fixture::reference("file:///srv/other.git", "demo");
    fx.profile("dev", &[&demo_ref(), &other]);
    go(&fx, &["deps", "pull"], JSON).ok();
    fx.profile("dev", &[&demo_ref()]);
    let preview = go(&fx, &["deps", "upgrade"], TEXT);
    assert_eq!(
        preview.ok().out,
        format!("Would remove {other} from the lockfile: nothing this project composes depends on it any more.\nEverything is up to date.\n0 pin(s) would move. Re-run with --yes to apply.\n")
    );
    let applied = go(&fx, &["deps", "upgrade", "--yes"], TEXT);
    assert_eq!(
        applied.ok().out,
        format!("Removed {other} from the lockfile: nothing this project composes depends on it any more.\nEverything is up to date.\n")
    );
    fx.remotes.set_unreachable(URL, true);
    let incomplete = go(&fx, &["deps", "upgrade"], TEXT);
    assert!(incomplete.ok().out.starts_with("No pins advanced among what could be resolved \u{2014} part of the dependency closure was unreachable this round"), "{}", incomplete.out);
    let empty = Fixture::new();
    let nothing = go(&empty, &["deps", "upgrade"], TEXT);
    assert!(nothing.ok().out.starts_with("Nothing to upgrade: no dependencies are declared here, so there was nothing to check and no lockfile was written.\n  If you expected some"), "{}", nothing.out);
    assert!(
        nothing
            .out
            .ends_with("and 'ctxloom init' sets a new project up.\n"),
        "{}",
        nothing.out
    );
}

#[test]
fn test_hold_and_unhold_say_what_they_did() {
    let (fx, _) = pulled();
    assert_eq!(
        go(&fx, &["deps", "hold", "origin/demo"], TEXT).ok().out,
        "Held \"origin/demo\" at its locked SHA.\n"
    );
    assert_eq!(
        go(&fx, &["deps", "unhold", "origin/demo"], TEXT).ok().out,
        "Released hold on \"origin/demo\"; the next 'ctxloom deps upgrade' may advance it.\n"
    );
    assert_eq!(
        go(&fx, &["deps", "hold", "origin/demo"], JSON).json(),
        serde_json::json!({"status": "held", "name": "origin/demo"})
    );
    fx.files.author("mine", &[("bundle.yaml", "v: 1\n")]);
    let local = go(&fx, &["deps", "hold", "mine"], TEXT);
    assert_eq!(local.ok().out, "");
    assert!(local.err.contains("nothing to hold"), "{}", local.err);
    assert_eq!(
        go(&fx, &["deps", "hold", "typo"], TEXT).refused(),
        ("deps-application-nothing-to-hold", 1)
    );
}

// ---------------------------------------------------------------- bundle

#[test]
fn test_bundle_push_and_move_say_where_the_bundle_went() {
    let fx = Fixture::new();
    fx.files
        .author("authored", &[("bundle.yaml", "description: d\n")]);
    fx.origin("team", "file:///srv/team.git");
    let pushed = go(
        &fx,
        &["bundle", "push", "authored", "team", "-m", "ship it"],
        TEXT,
    );
    let head = fx.remotes.head("file:///srv/team.git");
    assert_eq!(
        pushed.ok().out,
        format!(
            "Pushed .ctxloom/content/bundles/v2/authored to team\nCommit: {}\n",
            &head[..7]
        )
    );
    let json = go(&fx, &["bundle", "push", "authored", "team"], JSON).json();
    assert_eq!(json["status"], "pushed");
    assert_eq!(json["title"], "Update bundle: authored");
    assert_eq!(
        go(&fx, &["bundle", "push", "authored", "team", "--pr"], TEXT).refused(),
        ("deps-application-pull-request-needs-forge", 2)
    );
    fx.register_on("hub", "https://github.com/acme/hub", "github");
    let opened = go(&fx, &["bundle", "push", "authored", "hub", "--pr"], TEXT);
    assert_eq!(
        opened.ok().out,
        "Created pull request: https://github.com/acme/hub/pull/7\n"
    );
    let json = go(&fx, &["bundle", "push", "authored", "hub", "--pr"], JSON).json();
    assert_eq!(json["status"], "pr-created");
    assert_eq!(json["pr_url"], "https://github.com/acme/hub/pull/7");
    assert_eq!(json["create_pr"], true);
    fx.files.directory("other-project");
    let moved = go(
        &fx,
        &["bundle", "move", "authored", "--to", "other-project"],
        TEXT,
    );
    assert_eq!(
        moved.ok().out,
        "Moved /p/.ctxloom/content/bundles/v2/authored/bundle.yaml -> other-project/.ctxloom/content/bundles/v2/authored\n"
    );
    fx.files
        .author("second", &[("bundle.yaml", "description: d\n")]);
    let remote = go(
        &fx,
        &[
            "bundle", "move", "second", "--to", "team", "--force", "-m", "relocate",
        ],
        TEXT,
    );
    let head = fx.remotes.head("file:///srv/team.git");
    assert_eq!(
        remote.ok().out,
        format!("Moved /p/.ctxloom/content/bundles/v2/second/bundle.yaml -> .ctxloom/content/bundles/v2/second (remote team, commit {})\n", &head[..7])
    );
    assert_eq!(
        go(&fx, &["bundle", "move", "ghost", "--to", "team"], JSON).refused(),
        ("deps-application-bundle-not-found", 1)
    );
}

// ---------------------------------------------------------------- format

#[test]
fn test_a_terminal_gets_text_unless_a_format_was_asked_for_and_a_pipe_gets_json() {
    let fx = Fixture::new();
    let terminal = How {
        explicit: None,
        terminal: true,
    };
    assert!(go(&fx, &["remote", "list"], terminal)
        .ok()
        .out
        .starts_with("No remotes configured."));
    let piped = How {
        explicit: None,
        terminal: false,
    };
    assert!(go(&fx, &["remote", "list"], piped)
        .ok()
        .out
        .starts_with('{'));
    let asked = How {
        explicit: Some(Format::Json),
        terminal: true,
    };
    assert!(go(&fx, &["remote", "list"], asked)
        .ok()
        .out
        .starts_with('{'));
}

#[test]
fn test_the_bundle_verbs_are_these_and_only_these() {
    assert!(is_bundle_verb("push") && is_bundle_verb("move"));
    assert!(!is_bundle_verb("list") && !is_bundle_verb(""));
    let names: Vec<String> = bundle_verbs()
        .iter()
        .map(|c| c.get_name().to_string())
        .collect();
    assert_eq!(names, vec!["push", "move"]);
    let nouns: Vec<String> = nouns().iter().map(|c| c.get_name().to_string()).collect();
    assert_eq!(nouns, vec!["remote", "deps"]);
}

#[test]
fn test_every_diagnostic_the_commands_raise_has_words() {
    let catalog = Catalog::en_us(&[deps_application::EN_US, deps::EN_US, angzarr_yaml::EN_US])
        .expect("a catalog");
    for sample in deps_application::diagnostic_samples()
        .iter()
        .chain(deps::diagnostic_samples().iter())
    {
        let message = catalog.message(sample.as_ref());
        assert!(
            !message.is_empty() && message != sample.code(),
            "{}: {message}",
            sample.code()
        );
    }
}

fn row(reason: &'static str) -> deps_application::UncheckedRow {
    deps_application::UncheckedRow {
        reference: "ref".to_string(),
        url: "file:///r.git".to_string(),
        constraint: "^1".to_string(),
        reason,
        error: "why".to_string(),
    }
}

#[test]
fn test_an_entry_that_could_not_be_checked_earns_a_warning_by_the_reason() {
    let words = |reason: &'static str| {
        unchecked_warning(&row(reason)).map(|n| match n {
            Notice::Text { text, .. } => text,
            other => panic!("{other:?}"),
        })
    };
    assert_eq!(
        words("unparseable").as_deref(),
        Some("ref: could not parse reference (why); skipping the update check for it")
    );
    assert_eq!(
        words("unresolvable").as_deref(),
        Some("ref: could not resolve \"^1\" (why); skipping the update check for it")
    );
    assert_eq!(words("not_refreshed"), None);
}

fn report() -> deps_application::CheckReport {
    deps_application::CheckReport {
        entries: 1,
        updates: Vec::new(),
        unchecked: Vec::new(),
        refresh_failures: Vec::new(),
        skipped_empty: 0,
        missing_default_profiles: Vec::new(),
        missing_defaults_error: String::new(),
    }
}

fn update(selector: &str, requested: &str, resolved: &str) -> deps_application::UpdateRow {
    deps_application::UpdateRow {
        reference: "ref".to_string(),
        kind: "bundle",
        current_sha: "a".repeat(40),
        latest_sha: "b".repeat(40),
        selector: selector.to_string(),
        requested: requested.to_string(),
        resolved: resolved.to_string(),
    }
}

fn check_text(report: &deps_application::CheckReport) -> String {
    let mut out = Vec::new();
    text::check(&mut out, report).expect("writes");
    String::from_utf8(out).expect("utf-8")
}

#[test]
fn test_entries_with_no_commit_are_counted_only_when_there_are_some() {
    let mut r = report();
    assert_eq!(check_text(&r), "All items are up to date!\n");
    r.skipped_empty = 2;
    assert_eq!(
        check_text(&r),
        "Skipped 2 entries with empty SHA (run 'ctxloom deps pull' to clean up)\n\nAll items are up to date!\n"
    );
}

#[test]
fn test_an_update_says_what_its_pin_tracks() {
    for (selector, requested, resolved, tracks) in [
        ("branch", "", "", "tracking branch default branch"),
        ("branch", "dev", "", "tracking branch dev"),
        ("version", "^1", "", "range ^1"),
        ("version", "^1", "v1.2.0", "range ^1 \u{2192} v1.2.0"),
        ("sha", "", "", "sha"),
    ] {
        let mut r = report();
        r.updates.push(update(selector, requested, resolved));
        assert!(
            check_text(&r).contains(&format!("  ref  ({tracks})\n")),
            "{selector}: {}",
            check_text(&r)
        );
    }
}

fn writing_hooks() -> FakeRefresh {
    FakeRefresh {
        planned: vec![deps_application::RefreshEngine {
            engine: "claude-code".to_string(),
            wrote: vec!["hooks".to_string()],
            ..deps_application::RefreshEngine::default()
        }],
        ..FakeRefresh::default()
    }
}

// deps.feature: A pull without --yes installs the closure and leaves the
// project's engine files alone
#[test]
fn test_a_pull_without_yes_says_what_the_refresh_would_have_written() {
    let fx = Fixture::new();
    fx.origin("origin", URL);
    fx.profile("dev", &[&demo_ref()]);
    let refresh = writing_hooks();
    let ran = go_refreshing(&fx, &refresh, &["deps", "pull"], TEXT);
    ran.ok();
    assert!(
        ran.out
            .contains("Project refresh skipped: without --yes nothing was written into /project."),
        "{}",
        ran.out
    );
    assert!(
        ran.out.contains("claude-code: would write hooks"),
        "{}",
        ran.out
    );
    assert!(
        ran.out.contains("Apply it with 'ctxloom deps pull --yes', or 'ctxloom materialize --yes --surface hooks'."),
        "{}",
        ran.out
    );
    assert_eq!(*refresh.asked.lock().unwrap(), [false]);
}

#[test]
fn test_a_pull_without_yes_carries_the_skipped_refresh_in_json() {
    let fx = Fixture::new();
    fx.origin("origin", URL);
    fx.profile("dev", &[&demo_ref()]);
    let ran = go_refreshing(&fx, &writing_hooks(), &["deps", "pull"], JSON);
    ran.ok();
    let skipped = &ran.json()["refresh_skipped"];
    assert_eq!(skipped["target"], "/project");
    assert_eq!(skipped["engines"][0]["wrote"][0], "hooks");
    assert_eq!(ran.json()["installed"], 1);
}

// deps.feature: A pull with --yes also refreshes the project's engine files
#[test]
fn test_a_pull_with_yes_asks_for_the_write_and_carries_no_note() {
    let fx = Fixture::new();
    fx.origin("origin", URL);
    fx.profile("dev", &[&demo_ref()]);
    let refresh = writing_hooks();
    let ran = go_refreshing(&fx, &refresh, &["deps", "pull", "--yes"], TEXT);
    ran.ok();
    assert!(!ran.out.contains("Project refresh skipped"), "{}", ran.out);
    assert_eq!(*refresh.asked.lock().unwrap(), [true]);
    let json = go_refreshing(&fx, &writing_hooks(), &["deps", "pull", "--yes"], JSON);
    assert!(json.json().get("refresh_skipped").is_none());
}

// behaviour-changes-0.7.0: a refused consented refresh exits 3, the pull
// having landed.
#[test]
fn test_a_pull_with_yes_whose_refresh_is_refused_exits_3_after_the_pull() {
    let fx = Fixture::new();
    fx.origin("origin", URL);
    fx.profile("dev", &[&demo_ref()]);
    let refresh = FakeRefresh {
        refused: Some("hooks".to_string()),
        ..FakeRefresh::default()
    };
    let ran = go_refreshing(&fx, &refresh, &["deps", "pull", "--yes"], JSON);
    assert_eq!(ran.refused(), ("deps-application-refresh-aborted", 3));
    assert_eq!(ran.json()["installed"], 1);
}
