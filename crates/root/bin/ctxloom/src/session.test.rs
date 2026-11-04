//! Acceptance: the `session` commands, through the real binary, ported from
//! Go's tests/acceptance/features/cli/session.feature (release/0.7), with the
//! recall journey (j001200) and the closeout's destroyers (j001300) where they
//! touch what slice S4 ports.
//!
//! THE TEST-ONLY SEED this file proposes (built with `test-support` only;
//! Go's fixtures write a session's sidecar, and the Rust store has no sidecar
//! to write): `ctxloom test seed-session <harp>` mints the session in the
//! cwd's project store (through the domain, bypassing the name claim, which
//! is the owner's design), and writes what a fixture writes beside it:
//!
//! - `--project <id>`: another project (default the cwd's);
//! - `--started-ago <secs>` (default 86400) and `--ended-ago <secs>`: the
//!   instants of the mint and of the last run's end; every file it writes
//!   under the session directory is backdated to the end, as Go's
//!   `backdateSessionDir` does, so a session that ENDED then looks untouched
//!   since;
//! - `--lock free` makes the session's lock file, nothing holding it (`--lock
//!   none`, the default, makes none): a test holds it itself to be "running";
//! - `--transcript` records the canonical transcript Go's fixture records
//!   (one user prompt, "Seeded prompt for <harp>.");
//! - `--keep`, `--oneshot`, `--native-id <id> --native-transcript <path>`;
//! - `--other-file <rel>` (repeatable): an authored file in the session
//!   directory.

use std::fs::File;
use std::path::PathBuf;

use serde_json::Value;

use super::{Env, Ran};

const HARP: &str = "amber-swift-owl";

/// Everything a run said, as one text: stdout then stderr.
fn said(ran: &Ran) -> String {
    format!("{}{}", ran.stdout, ran.stderr)
}

/// A home with sessions seeded in it.
struct Home {
    env: Env,
}

impl Home {
    fn new() -> Home {
        Home { env: Env::new() }
    }

    fn run(&self, args: &[&str]) -> Ran {
        self.env.run(args)
    }

    /// Seed `harp` with `flags`; it must succeed.
    fn seed(&self, harp: &str, flags: &[&str]) {
        let mut args = vec!["test", "seed-session", harp];
        args.extend_from_slice(flags);
        let ran = self.env.run(&args);
        assert_eq!(ran.code, 0, "seed {harp} {flags:?}: {}", ran.said());
    }

    /// A finished session with a transcript (Go: "a finished session ... with
    /// a transcript"): its lock free, it ended 60 days ago.
    fn finished(&self, harp: &str) {
        self.seed(
            harp,
            &[
                "--lock",
                "free",
                "--transcript",
                "--started-ago",
                "6048000",
                "--ended-ago",
                "5184000",
            ],
        );
    }

    fn session_dir(&self, harp: &str) -> PathBuf {
        self.env.home().join(".ctxloom/sessions").join(harp)
    }

    fn transcript_file(&self, harp: &str) -> PathBuf {
        self.session_dir(harp).join("transcripts/transcript.jsonl")
    }

    /// `<home>/Documents/ctxloom/<project>/<harp>`: where a seeded session's
    /// output goes (Go: `defaultOutputDirIn`).
    fn output_dir(&self, harp: &str) -> PathBuf {
        let project = self.env.home().file_name().expect("a name");
        self.env
            .home()
            .join("Documents/ctxloom")
            .join(project)
            .join(harp)
    }

    /// A summary an older ctxloom left in the session's output dir.
    fn leave_summary(&self, harp: &str) -> PathBuf {
        let dir = self.output_dir(harp);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("essence.md");
        std::fs::write(&file, "an older ctxloom's summary\n").unwrap();
        file
    }

    /// Hold `harp`'s lock as a running session does, until dropped.
    fn running(&self, harp: &str) -> File {
        let path = self
            .env
            .home()
            .join(".ctxloom/sessions")
            .join(format!("{harp}.lock"));
        let file = File::open(path).expect("a lock file: seed with --lock free");
        file.lock().expect("the lock is free to take");
        file
    }
}

fn rows(ran: &Ran) -> Vec<Value> {
    assert_eq!(ran.code, 0, "{}", ran.said());
    ran.json().as_array().expect("an array").clone()
}

fn row<'a>(rows: &'a [Value], harp: &str) -> &'a Value {
    rows.iter()
        .find(|r| r["harp"] == harp)
        .unwrap_or_else(|| panic!("no row for {harp} in {rows:?}"))
}

fn harps(rows: &[Value]) -> Vec<String> {
    rows.iter()
        .map(|r| r["harp"].as_str().unwrap().to_string())
        .collect()
}

// ------------------------------------------------------ "Reading the record"

// session.feature: "The bare noun lists what has been recorded".
#[test]
fn test_the_bare_noun_lists() {
    let home = Home::new();
    home.seed(HARP, &["--transcript"]);
    let ran = home.run(&["session"]);
    assert_eq!(ran.code, 0, "{}", ran.said());
    assert!(said(&ran).contains(HARP), "{}", ran.said());
}

// session.feature: "--all reaches past the current project".
#[test]
fn test_all_reaches_past_the_current_project() {
    let home = Home::new();
    home.seed(HARP, &[]);
    home.seed("calm-dusty-wren", &["--project", "brisk-tall-oak"]);
    let here = rows(&home.run(&["session", "list"]));
    assert_eq!(harps(&here), [HARP]);
    let everywhere = rows(&home.run(&["session", "list", "--all"]));
    let mut got = harps(&everywhere);
    got.sort();
    assert_eq!(got, ["amber-swift-owl", "calm-dusty-wren"]);
}

// session.feature: "A fresh project has recorded nothing", three examples: no
// --format at all takes the derived default off a terminal (json); an explicit
// one wins in both directions.
#[test]
fn test_a_fresh_project_has_recorded_nothing() {
    let home = Home::new();
    for (flags, json) in [
        (&[][..], true),
        (&["--format", "json"], true),
        (&["--format", "text"], false),
    ] {
        let mut args = flags.to_vec();
        args.extend(["session", "list"]);
        let ran = home.run(&args);
        assert_eq!(ran.code, 0, "{flags:?}: {}", ran.said());
        if json {
            assert_eq!(ran.json(), serde_json::json!([]), "{flags:?}");
        } else {
            assert_eq!(ran.stdout, "(no sessions)\n", "{flags:?}");
        }
    }
}

// session.feature: "Reading the record touches nothing": a read of a fresh
// home creates no store.
#[test]
fn test_reading_creates_nothing() {
    let home = Home::new();
    let before = std::fs::read_dir(home.env.home()).unwrap().count();
    for args in [
        &["session", "list"][..],
        &["session", "list", "--all"],
        &["session", "search", "x"],
        &["session", "transcript", "list"],
    ] {
        assert_eq!(home.run(args).code, 0, "{args:?}");
    }
    assert_eq!(std::fs::read_dir(home.env.home()).unwrap().count(), before);
}

// session.feature: "Showing a session prints its record, titled by its first
// prompt".
#[test]
fn test_showing_a_session_prints_its_record_titled_by_its_first_prompt() {
    let home = Home::new();
    home.seed(HARP, &["--transcript"]);
    let shown = home.run(&["session", "show", HARP, "--format", "text"]);
    assert_eq!(shown.code, 0, "{}", shown.said());
    assert!(
        shown.stdout.contains("Seeded prompt for amber-swift-owl."),
        "{}",
        shown.said()
    );
    assert!(shown.stdout.contains("engine:"), "{}", shown.said());
    let listed = rows(&home.run(&["session", "list", "--format", "json"]));
    assert_eq!(
        row(&listed, HARP)["summary"],
        "Seeded prompt for amber-swift-owl."
    );
}

// session.feature: "The listing takes each session's state from its lock, not
// its end stamp": the index says ended; the lock says running.
#[test]
fn test_the_listing_takes_each_sessions_state_from_its_lock() {
    let home = Home::new();
    home.seed(
        "amber-swift-owl",
        &["--lock", "free", "--transcript", "--ended-ago", "7200"],
    );
    home.seed(
        "brisk-copper-moth",
        &["--lock", "free", "--transcript", "--ended-ago", "7200"],
    );
    home.seed("calm-dusty-wren", &["--transcript", "--ended-ago", "7200"]);
    let _running = home.running("amber-swift-owl");
    let listed = rows(&home.run(&["session", "list", "--all", "--format", "json"]));
    assert_eq!(row(&listed, "amber-swift-owl")["state"], "live");
    assert!(
        row(&listed, "amber-swift-owl").get("end").is_none(),
        "a live session's end is blank"
    );
    assert_eq!(row(&listed, "brisk-copper-moth")["state"], "ended");
    assert!(row(&listed, "brisk-copper-moth").get("end").is_some());
    assert_eq!(
        row(&listed, "calm-dusty-wren")["state"],
        "unknown",
        "no lock file"
    );
    let text = home.run(&["session", "list", "--all", "--format", "text"]);
    assert_eq!(text.code, 0, "{}", text.said());
    assert!(text.stdout.contains("STATE"), "{}", text.stdout);
}

// session.feature: "--state keeps only the sessions in that state", both
// listings.
#[test]
fn test_state_keeps_only_the_sessions_in_that_state() {
    let home = Home::new();
    home.seed(
        "amber-swift-owl",
        &["--lock", "free", "--transcript", "--ended-ago", "7200"],
    );
    home.seed(
        "brisk-copper-moth",
        &["--lock", "free", "--transcript", "--ended-ago", "7200"],
    );
    home.seed("calm-dusty-wren", &["--transcript", "--ended-ago", "7200"]);
    let _running = home.running("amber-swift-owl");
    for (leaf, state, kept, dropped) in [
        (
            "list",
            "live",
            "amber-swift-owl",
            ["brisk-copper-moth", "calm-dusty-wren"],
        ),
        (
            "list",
            "ended",
            "brisk-copper-moth",
            ["amber-swift-owl", "calm-dusty-wren"],
        ),
        (
            "list",
            "unknown",
            "calm-dusty-wren",
            ["amber-swift-owl", "brisk-copper-moth"],
        ),
        (
            "search",
            "live",
            "amber-swift-owl",
            ["brisk-copper-moth", "calm-dusty-wren"],
        ),
        (
            "search",
            "ended",
            "brisk-copper-moth",
            ["amber-swift-owl", "calm-dusty-wren"],
        ),
    ] {
        let mut args = vec![
            "session", leaf, "--all", "--state", state, "--format", "json",
        ];
        if leaf == "search" {
            args.insert(2, "seeded");
        }
        let listed = rows(&home.run(&args));
        assert_eq!(row(&listed, kept)["state"], state, "{leaf} {state}");
        for harp in dropped {
            assert!(
                !harps(&listed).contains(&harp.to_string()),
                "{leaf} {state}: {harp}"
            );
        }
    }
}

// session.feature: "--state refuses a word that is not a state".
#[test]
fn test_state_refuses_a_word_that_is_not_a_state() {
    let home = Home::new();
    let ran = home.run(&["session", "list", "--state", "running"]);
    assert_ne!(ran.code, 0, "{}", ran.said());
    assert!(
        said(&ran).contains("live, ended, unknown"),
        "{}",
        ran.said()
    );
}

// ----------------------------------------------------------------- j001200

// j001200: "A phrase from the question she asked finds the session that
// answered it", beside an unrelated session the search could have returned.
#[test]
fn test_a_phrase_from_the_first_prompt_finds_the_session() {
    let home = Home::new();
    home.seed(HARP, &["--transcript"]);
    home.seed("brisk-copper-moth", &["--transcript"]);
    let found = rows(&home.run(&["session", "search", "amber-swift-owl.", "--all"]));
    assert_eq!(harps(&found), [HARP]);
    let by_title = rows(&home.run(&["session", "search", "Seeded", "prompt", "--all"]));
    assert_eq!(by_title.len(), 2);
}

// j001200: "A query that matches nothing says so rather than printing nothing".
#[test]
fn test_a_query_that_matches_nothing_says_so() {
    let home = Home::new();
    home.seed(HARP, &["--transcript"]);
    let text = home.run(&[
        "session",
        "search",
        "J001200-MATCHES-ABSOLUTELY-NOTHING",
        "--format",
        "text",
    ]);
    assert_eq!(text.code, 0, "{}", text.said());
    assert_eq!(text.stdout.trim(), "(no sessions)");
    let json = home.run(&[
        "session",
        "search",
        "J001200-MATCHES-ABSOLUTELY-NOTHING",
        "--format",
        "json",
    ]);
    assert_eq!(json.json(), serde_json::json!([]));
}

// ------------------------------------------------------------------- keep

// session.feature: "A kept transcript is refused at any age, and the listing
// says it is kept" (the listing and the destroyer; --undo releases it).
#[test]
fn test_keep_marks_the_listing_and_undo_releases_it() {
    let home = Home::new();
    home.finished(HARP);
    let kept = home.run(&["session", "keep", HARP]);
    assert_eq!(kept.code, 0, "{}", kept.said());
    let listed = rows(&home.run(&["session", "list", "--all", "--format", "json"]));
    assert_eq!(row(&listed, HARP)["kept"], true);
    let removal = home.run(&["session", "remove", HARP, "--yes"]);
    assert_ne!(removal.code, 0, "{}", removal.said());
    assert!(
        said(&removal).contains("ctxloom session keep amber-swift-owl --undo"),
        "{}",
        removal.said()
    );
    assert!(home.transcript_file(HARP).exists());
    assert_eq!(home.run(&["session", "keep", HARP, "--undo"]).code, 0);
    let listed = rows(&home.run(&["session", "list", "--all", "--format", "json"]));
    assert!(row(&listed, HARP).get("kept").is_none());
    let removal = home.run(&["session", "remove", HARP, "--yes"]);
    assert_eq!(removal.code, 0, "{}", removal.said());
    assert!(!home.transcript_file(HARP).exists());
}

#[test]
fn test_keeping_a_session_nobody_recorded_fails_naming_it() {
    let home = Home::new();
    let ran = home.run(&["session", "keep", "no-such-harp"]);
    assert_ne!(ran.code, 0, "{}", ran.said());
    assert!(said(&ran).contains("no-such-harp"), "{}", ran.said());
}

// ----------------------------------------------------------------- remove

// session.feature: "Delete reports before it removes".
#[test]
fn test_remove_reports_before_it_removes() {
    let home = Home::new();
    home.finished(HARP);
    let ran = home.run(&["session", "remove", HARP]);
    assert_eq!(ran.code, 0, "{}", ran.said());
    let all = said(&ran);
    assert!(all.contains("removed nothing"), "{all}");
    assert!(
        all.contains("ctxloom session remove amber-swift-owl --yes"),
        "{all}"
    );
    assert!(home.transcript_file(HARP).exists());
    let listed = home.run(&["session", "list", "--all"]);
    assert!(said(&listed).contains(HARP), "{}", listed.said());
}

// session.feature: "--yes removes the index entry and the transcript", and
// leaves the output dir (the human's).
#[test]
fn test_remove_yes_takes_the_index_entry_and_the_transcript_and_leaves_the_output_dir() {
    let home = Home::new();
    home.finished(HARP);
    let essence = home.leave_summary(HARP);
    let ran = home.run(&["session", "remove", HARP, "--yes"]);
    assert_eq!(ran.code, 0, "{}", ran.said());
    assert!(!home.transcript_file(HARP).exists());
    assert!(essence.exists(), "the output dir is the human's");
    let listed = home.run(&["session", "list", "--all"]);
    assert!(!said(&listed).contains(HARP), "{}", listed.said());
}

// session.feature: "A transcript within its retention is refused by every
// destroyer": a session that finished an hour ago.
#[test]
fn test_remove_within_the_retention_is_refused() {
    let home = Home::new();
    home.seed(
        "brisk-copper-moth",
        &["--lock", "free", "--transcript", "--ended-ago", "3600"],
    );
    let ran = home.run(&["session", "remove", "brisk-copper-moth", "--yes"]);
    assert_ne!(ran.code, 0, "{}", ran.said());
    assert!(said(&ran).contains("retention"), "{}", ran.said());
    assert!(home.transcript_file("brisk-copper-moth").exists());
}

// session.feature: "A session the lock cannot prove finished is refused, and
// the escape is named", and "--even-if-live sweeps it anyway".
#[test]
fn test_remove_of_a_session_the_lock_cannot_prove_dead_is_refused_and_the_escape_is_named() {
    let home = Home::new();
    home.seed(
        HARP,
        &[
            "--transcript",
            "--started-ago",
            "6048000",
            "--ended-ago",
            "5184000",
        ],
    );
    let ran = home.run(&["session", "remove", HARP, "--yes"]);
    assert_ne!(ran.code, 0, "{}", ran.said());
    assert!(
        said(&ran).contains("cannot be proven dead"),
        "{}",
        ran.said()
    );
    assert!(
        said(&ran).contains("ctxloom session remove amber-swift-owl --yes --even-if-live"),
        "{}",
        ran.said()
    );
    assert!(home.transcript_file(HARP).exists());
    let anyway = home.run(&["session", "remove", HARP, "--yes", "--even-if-live"]);
    assert_eq!(anyway.code, 0, "{}", anyway.said());
    assert!(!home.transcript_file(HARP).exists());
}

// --------------------------------------------------------------- transcript

// session.feature: "The bare sub-noun lists which sessions have a transcript".
#[test]
fn test_the_bare_transcript_lists() {
    let home = Home::new();
    home.seed(HARP, &["--transcript"]);
    let ran = home.run(&["session", "transcript"]);
    assert_eq!(ran.code, 0, "{}", ran.said());
    assert!(said(&ran).contains(HARP), "{}", ran.said());
}

// session.feature: "A session with no captured transcript is still named".
#[test]
fn test_a_session_with_no_captured_transcript_is_still_named() {
    let home = Home::new();
    home.seed(HARP, &[]);
    let ran = home.run(&["session", "transcript", "list", "--format", "text"]);
    assert_eq!(ran.code, 0, "{}", ran.said());
    assert!(ran.stdout.contains(HARP), "{}", ran.said());
    assert!(ran.stdout.contains("false"), "{}", ran.said());
}

// session.feature: "The harp is a positional, and an unknown one fails".
#[test]
fn test_the_transcript_listing_of_an_unknown_harp_fails() {
    let home = Home::new();
    let ran = home.run(&["session", "transcript", "list", "no-such-harp"]);
    assert_ne!(ran.code, 0, "{}", ran.said());
    assert!(said(&ran).contains("no-such-harp"), "{}", ran.said());
}

// session.feature: "Show prints the whole transcript and exits, ready for a
// pipe": every line of stdout is a JSON object.
#[test]
fn test_transcript_show_prints_the_whole_transcript_for_a_pipe() {
    let home = Home::new();
    home.seed(HARP, &["--transcript"]);
    let ran = home.run(&["session", "transcript", "show", HARP]);
    assert_eq!(ran.code, 0, "{}", ran.said());
    assert!(!ran.stdout.is_empty());
    for line in ran.stdout.lines() {
        let value: Value = serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}"));
        assert!(value.is_object(), "{line}");
    }
    assert!(
        ran.stdout.contains("Seeded prompt for amber-swift-owl."),
        "{}",
        ran.stdout
    );
}

// Go: errNoCapturedTranscript.
#[test]
fn test_transcript_show_of_a_session_with_no_transcript_fails() {
    let home = Home::new();
    home.seed(HARP, &[]);
    let ran = home.run(&["session", "transcript", "show", HARP]);
    assert_ne!(ran.code, 0, "{}", ran.said());
    assert_eq!(
        ran.stdout, "",
        "nothing but the transcript is written to stdout"
    );
}

// ---------------------------------------------------------------------- edit

// session.feature: "The old rename verb is gone, not hidden", and "Watching
// moved under the transcript it watches".
#[test]
fn test_the_old_rename_verb_is_gone() {
    let home = Home::new();
    home.seed(HARP, &[]);
    let ran = home.run(&["session", "rename", HARP, "bright-keen-hawk"]);
    assert_eq!(ran.code, 2, "{}", ran.said());
    assert!(said(&ran).contains("unknown command"), "{}", ran.said());
}

// session.feature: "A bare edit refuses, because there is no document to
// open".
#[test]
fn test_a_bare_edit_refuses_and_names_the_field_it_takes() {
    let home = Home::new();
    home.seed(HARP, &[]);
    let ran = home.run(&["session", "edit", HARP]);
    assert_ne!(ran.code, 0, "{}", ran.said());
    assert!(
        said(&ran).contains("no editable document"),
        "{}",
        ran.said()
    );
    assert!(said(&ran).contains("--name"), "{}", ran.said());
}

// session.feature: "Editing a session nobody recorded fails".
#[test]
fn test_editing_a_session_nobody_recorded_fails_naming_it() {
    let home = Home::new();
    let ran = home.run(&[
        "session",
        "edit",
        "no-such-harp",
        "--name",
        "bright-keen-hawk",
    ]);
    assert_ne!(ran.code, 0, "{}", ran.said());
    assert!(said(&ran).contains("no-such-harp"), "{}", ran.said());
}

// session.feature: "--name renames the harp and moves its output folder".
//
// GO-DERIVED, BLOCKED ON THE OWNER'S D5 RULING: a rename claims the new name
// and frees the old (plan 3.1), and the claim streams await review. Red until
// then: `session edit --name` says renaming is unavailable.
#[test]
fn test_edit_name_renames_the_harp_and_moves_its_output_folder() {
    let home = Home::new();
    home.finished(HARP);
    let essence = home.leave_summary(HARP);
    let ran = home.run(&["session", "edit", HARP, "--name", "bright-keen-hawk"]);
    assert_eq!(ran.code, 0, "{}", ran.said());
    assert!(said(&ran).contains("bright-keen-hawk"), "{}", ran.said());
    assert!(home
        .output_dir("bright-keen-hawk")
        .join("essence.md")
        .exists());
    assert!(!essence.exists());
    let listed = rows(&home.run(&["session", "list", "--all"]));
    assert!(harps(&listed).contains(&"bright-keen-hawk".to_string()));
    assert!(!harps(&listed).contains(&HARP.to_string()));
}

// ---------------------------------------------------- session_hooks.feature

const SESSION_START: &str = r#"{"session_id":"vendor-session-42","transcript_path":"/tmp/vendor/transcript.jsonl","hook_event_name":"SessionStart"}"#;

// session_hooks.feature: "Binding a session records the harp both in the
// transcript and in the index": the marker on stdout (parsed ALONE as JSON),
// and the index binds the harp to the session.
#[test]
fn test_binding_a_session_records_the_harp_in_the_transcript_and_in_the_index() {
    let home = Home::new();
    home.seed("brisk-copper-moth", &[]);
    let ran = home.env.run_hook(
        &["hook", "session-bind", "--engine", "claude-code"],
        SESSION_START,
        &[("CTXLOOM_SESSION_HARP", "brisk-copper-moth")],
    );
    assert_eq!(ran.code, 0, "{}", ran.said());
    let reply = ran.json();
    assert!(reply["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap()
        .contains("brisk-copper-moth"));
    let shown = home.run(&["session", "show", "brisk-copper-moth", "--format", "json"]);
    assert_eq!(shown.json()["native_session"], "vendor-session-42");
}

// Go: TestBindSession_RotationRepointsTheBinding, through the hook: /clear
// fires SessionStart again with a new session.
#[test]
fn test_a_second_bind_with_a_new_session_rotates_the_binding() {
    let home = Home::new();
    home.seed("brisk-copper-moth", &[]);
    let vars = [("CTXLOOM_SESSION_HARP", "brisk-copper-moth")];
    let bind = |payload: &str| {
        home.env.run_hook(
            &["hook", "session-bind", "--engine", "claude-code"],
            payload,
            &vars,
        )
    };
    assert_eq!(bind(SESSION_START).code, 0);
    assert_eq!(bind(SESSION_START).code, 0, "a repeat is a no-op");
    let second = r#"{"session_id":"vendor-session-43","transcript_path":"/tmp/vendor/second.jsonl","hook_event_name":"SessionStart"}"#;
    assert_eq!(bind(second).code, 0);
    let shown = home
        .run(&["session", "show", "brisk-copper-moth", "--format", "json"])
        .json();
    assert_eq!(shown["native_session"], "vendor-session-43");
    assert_eq!(shown["rotations"].as_array().unwrap().len(), 1);
    assert_eq!(shown["rotations"][0]["native_id"], "vendor-session-42");
}

// Go: operations.BindSession no-ops a harp absent from the index: the hook
// never fails an engine's startup over it.
#[test]
fn test_binding_a_harp_nobody_recorded_still_emits_the_marker_and_exits_0() {
    let home = Home::new();
    let ran = home.env.run_hook(
        &["hook", "session-bind", "--engine", "claude-code"],
        SESSION_START,
        &[("CTXLOOM_SESSION_HARP", "no-such-harp")],
    );
    assert_eq!(ran.code, 0, "{}", ran.said());
    assert!(ran.json()["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap()
        .contains("no-such-harp"));
}

// ------------------------------------------------------------ session import
//
// THE OWNER'S D5 RULINGS, not Go's: the imported session's stable id is
// UUIDv5(project, harp) and its name is claimed first. Red, and WITHHELD from
// implementation until the owner reviews them. What is Go's is the listing:
// "the same listing Go gives for it" (plan S4). The fixtures are Go's
// session.yaml, schema 1 to 3, copied under `fixtures/go-sidecar/` and
// planted in a scratch home: never the real ~/.ctxloom.

const GO_SIDECARS: [(&str, &str); 3] = [
    (
        "amber-swift-owl",
        include_str!("../fixtures/go-sidecar/schema1.yaml"),
    ),
    (
        "brisk-copper-moth",
        include_str!("../fixtures/go-sidecar/schema2.yaml"),
    ),
    (
        "calm-dusty-wren",
        include_str!("../fixtures/go-sidecar/schema3.yaml"),
    ),
];

impl Home {
    /// Plant Go's three fixture sessions as Go lays them out, for the
    /// project this home is (the seed registers it).
    fn go_sessions(&self) -> Vec<(PathBuf, String)> {
        self.seed("registered-project-marker", &[]);
        let work_root = std::fs::canonicalize(self.env.home()).unwrap();
        GO_SIDECARS
            .iter()
            .map(|(harp, text)| {
                let dir = self.session_dir(harp);
                std::fs::create_dir_all(&dir).unwrap();
                let body = text.replace("/work/proj", &work_root.display().to_string());
                std::fs::write(dir.join("session.yaml"), &body).unwrap();
                (dir.join("session.yaml"), body)
            })
            .collect()
    }
}

#[test]
fn test_import_gives_the_listing_go_gives_for_each_sidecar_generation() {
    let home = Home::new();
    let planted = home.go_sessions();
    let ran = home.run(&["session", "import"]);
    assert_eq!(ran.code, 0, "{}", ran.said());
    let report = ran.json();
    assert_eq!(report["imported"].as_array().unwrap().len(), 3, "{report}");
    assert_eq!(report["left"].as_array().unwrap().len(), 0, "{report}");

    let listed = rows(&home.run(&["session", "list", "--all", "--format", "json"]));
    let owl = row(&listed, "amber-swift-owl");
    assert_eq!(owl["engine"], "claude-code");
    assert_eq!(owl["start"], "2026-04-10T00:00:00Z");
    assert_eq!(owl["end"], "2026-04-10T01:00:00Z");
    assert!(owl.get("kept").is_none());
    let moth = row(&listed, "brisk-copper-moth");
    assert_eq!(moth["kept"], true);
    assert_eq!(moth["start"], "2026-05-02T08:30:00.481Z");
    let wren = row(&listed, "calm-dusty-wren");
    assert_eq!(wren["kept"], true);
    assert_eq!(wren["purged"], true);

    // Go's directories are read, never written.
    for (path, body) in planted {
        assert_eq!(std::fs::read_to_string(path).unwrap(), body);
    }
}

#[test]
fn test_a_rerun_of_import_adds_nothing() {
    let home = Home::new();
    home.go_sessions();
    assert_eq!(home.run(&["session", "import"]).code, 0);
    let before = home
        .run(&["session", "list", "--all", "--format", "json"])
        .stdout;
    let again = home.run(&["session", "import"]);
    assert_eq!(again.code, 0, "{}", again.said());
    let report = again.json();
    assert_eq!(report["imported"].as_array().unwrap().len(), 0, "{report}");
    assert_eq!(report["already"].as_array().unwrap().len(), 3, "{report}");
    assert_eq!(
        home.run(&["session", "list", "--all", "--format", "json"])
            .stdout,
        before
    );
}

// D5 Q1: the imported session's id is UUIDv5(project, harp).
#[test]
fn test_an_imported_session_has_the_derived_stable_id() {
    let home = Home::new();
    home.go_sessions();
    home.run(&["session", "import"]);
    let shown = home
        .run(&["session", "show", "amber-swift-owl", "--format", "json"])
        .json();
    let id = shown["id"].as_str().unwrap();
    assert_eq!(id.as_bytes()[14], b'5', "a version 5 UUID: {id}");
}
