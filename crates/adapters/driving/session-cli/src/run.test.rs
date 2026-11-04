//! The `session` commands against a fake of the application: what each
//! flag asks for, and what each answer looks like in json and in text.
//! Ported from Go's `session_row_test.go`, `session_query_test.go`,
//! `session_state_test.go` and session.feature (release/0.7); the rows are
//! D16's.

use angzarr_presentation::Format;
use clap::{Args, Command, FromArgMatches};
use serde_json::{json, Value};
use session_index::{Captured, SessionRecord, State, View};
use sessions_application::{
    dead_letter_sample, AppError, BindRequest, Bound, Class, FilePlan, ImportReport, ImportRequest,
    ImportService, Imported, KeepRequest, Kept, Left, ListQuery, PlanItem, Refusal, Removal,
    RemoveRequest, RenameRequest, Renamed, SearchQuery, SessionService, ShowQuery, TranscriptQuery,
    TranscriptRow, Why,
};

use super::run;
use crate::Session;

// 2026-07-17T12:00:00Z.
const STARTED: i64 = 1_784_289_600;

fn ts(seconds: i64) -> sessions_application::Timestamp {
    sessions_application::Timestamp { seconds, nanos: 0 }
}

/// What the fake answers with and what it was asked.
#[derive(Default)]
struct Fake {
    views: Vec<View>,
    rows: Vec<TranscriptRow>,
    transcript: String,
    removal: Option<Removal>,
    rename: Option<Result<Renamed, AppError>>,
    dead_letters: bool,
    list: Vec<ListQuery>,
    search: Vec<SearchQuery>,
    show: Vec<ShowQuery>,
    keep: Vec<KeepRequest>,
    remove: Vec<RemoveRequest>,
    renamed: Vec<RenameRequest>,
    transcripts: Vec<TranscriptQuery>,
    report: Option<ImportReport>,
    imports: usize,
}

impl ImportService for Fake {
    async fn import(&mut self, _request: ImportRequest) -> Result<ImportReport, AppError> {
        self.imports += 1;
        Ok(self.report.clone().unwrap_or_default())
    }
}

impl SessionService for Fake {
    async fn list(&mut self, query: ListQuery) -> Result<Vec<View>, AppError> {
        self.list.push(query);
        Ok(self.views.clone())
    }

    async fn search(&mut self, query: SearchQuery) -> Result<Vec<View>, AppError> {
        self.search.push(query);
        Ok(self.views.clone())
    }

    async fn show(&mut self, query: ShowQuery) -> Result<View, AppError> {
        self.show.push(query.clone());
        self.views
            .first()
            .cloned()
            .ok_or(AppError::NoSession { harp: query.harp })
    }

    async fn keep(&mut self, request: KeepRequest) -> Result<Kept, AppError> {
        self.keep.push(request.clone());
        Ok(Kept {
            harp: request.harp,
            kept: request.keep,
            changed: true,
            dead_letters: if self.dead_letters {
                vec![dead_letter_sample("sessions")]
            } else {
                Vec::new()
            },
        })
    }

    async fn remove(&mut self, request: RemoveRequest) -> Result<Removal, AppError> {
        self.remove.push(request);
        self.removal.clone().ok_or(AppError::NoSession {
            harp: "x".to_string(),
        })
    }

    async fn rename(&mut self, request: RenameRequest) -> Result<Renamed, AppError> {
        self.renamed.push(request);
        self.rename.clone().unwrap_or(Err(AppError::NoSession {
            harp: "x".to_string(),
        }))
    }

    async fn transcripts(
        &mut self,
        query: TranscriptQuery,
    ) -> Result<Vec<TranscriptRow>, AppError> {
        self.transcripts.push(query);
        Ok(self.rows.clone())
    }

    async fn transcript(&mut self, _harp: &str) -> Result<String, AppError> {
        Ok(self.transcript.clone())
    }

    async fn bind(&mut self, _request: BindRequest) -> Result<Bound, AppError> {
        Ok(Bound::Unchanged)
    }
}

/// The parsed `session ...` of `words`.
fn parsed(words: &[&str]) -> Session {
    let command = Session::augment_args(Command::new("session")).no_binary_name(true);
    let matches = command
        .try_get_matches_from(words.iter().map(|w| w.to_string()))
        .unwrap_or_else(|e| panic!("{e}"));
    Session::from_arg_matches(&matches).unwrap()
}

struct Ran {
    out: String,
    result: Result<crate::Done, crate::Failure>,
}

fn go(fake: &mut Fake, format: Format, words: &[&str]) -> Ran {
    let session = parsed(words);
    let mut out = Vec::new();
    let result = run(&session, fake, format, &mut out);
    Ran {
        out: String::from_utf8(out).unwrap(),
        result,
    }
}

fn json_of(ran: &Ran) -> Value {
    assert!(
        ran.result.is_ok(),
        "{:?}",
        ran.result
            .as_ref()
            .err()
            .map(|f| f.error.code().to_string())
    );
    serde_json::from_str(&ran.out).unwrap_or_else(|e| panic!("{e}: {:?}", ran.out))
}

fn view(harp: &str, state: State) -> View {
    View {
        record: SessionRecord {
            id: "01010101-0101-0101-0101-010101010101".to_string(),
            harp: harp.to_string(),
            project_dir: "/work/proj".to_string(),
            engine: "claude-code".to_string(),
            started_at: ts(STARTED),
            ..SessionRecord::default()
        },
        state,
        title: Some(format!("Seeded prompt for {harp}.")),
        captured: None,
    }
}

fn keys(value: &Value) -> Vec<String> {
    value.as_object().unwrap().keys().cloned().collect()
}

// ------------------------------------------------------------------------ list

// Go: TestSessionRow_JSONShape: harp/summary/state/start, plus end, purged and
// kept once set; D16 adds the transcript and the engine.
#[test]
fn a_row_has_harp_summary_state_start_and_the_engine() {
    let mut fake = Fake {
        views: vec![view("amber-swift-owl", State::Ended)],
        ..Fake::default()
    };
    let rows = json_of(&go(&mut fake, Format::Json, &["list"]));
    let row = &rows[0];
    let mut got = keys(row);
    got.sort();
    assert_eq!(got, ["engine", "harp", "start", "state", "summary"]);
    assert_eq!(row["harp"], "amber-swift-owl");
    assert_eq!(row["summary"], "Seeded prompt for amber-swift-owl.");
    assert_eq!(row["state"], "ended");
    assert_eq!(row["start"], "2026-07-17T12:00:00Z");
    assert_eq!(row["engine"], "claude-code");
}

// Go: end, purged and kept "are omitted when unset"; the transcript when it
// was captured.
#[test]
fn end_purged_kept_and_the_transcript_ride_the_row_once_set() {
    let mut v = view("amber-swift-owl", State::Ended);
    v.record.ended_at = Some(ts(STARTED + 3600));
    v.record.purged_at = Some(ts(STARTED + 7200));
    v.record.kept = true;
    v.captured = Some(Captured {
        path: "/home/.ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl".to_string(),
        bytes: 9,
    });
    let mut fake = Fake {
        views: vec![v],
        ..Fake::default()
    };
    let rows = json_of(&go(&mut fake, Format::Json, &["list"]));
    let row = &rows[0];
    assert_eq!(row["end"], "2026-07-17T13:00:00Z");
    assert_eq!(row["purged"], true);
    assert_eq!(row["kept"], true);
    assert_eq!(
        row["transcript_path"],
        "/home/.ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl"
    );
    assert!(row["summary"].as_str().unwrap().contains("kept"));
    assert!(row["summary"].as_str().unwrap().contains("purged"));
}

// session.feature: "The listing takes each session's state from its lock, not
// its end stamp": a live session has no end though its record carries one.
#[test]
fn a_live_session_has_no_end_in_json() {
    let mut live = view("amber-swift-owl", State::Live);
    live.record.ended_at = Some(ts(STARTED + 3600));
    let mut ended = view("brisk-copper-moth", State::Ended);
    ended.record.ended_at = Some(ts(STARTED + 3600));
    let mut fake = Fake {
        views: vec![live, ended],
        ..Fake::default()
    };
    let rows = json_of(&go(&mut fake, Format::Json, &["list", "--all"]));
    assert_eq!(rows[0]["state"], "live");
    assert!(rows[0].get("end").is_none());
    assert!(rows[1].get("end").is_some());
}

// session.feature: `session list --all --format text` has a STATE column;
// session_state_test.go: "the table has a STATE column, and the live row prints
// no END while the others print theirs".
#[test]
fn the_text_table_has_a_state_column_and_a_blank_end_while_live() {
    let mut live = view("amber-swift-owl", State::Live);
    live.record.ended_at = Some(ts(STARTED + 3600));
    let mut ended = view("brisk-copper-moth", State::Ended);
    ended.record.ended_at = Some(ts(STARTED + 3600));
    let mut fake = Fake {
        views: vec![live, ended],
        ..Fake::default()
    };
    let ran = go(&mut fake, Format::Text, &["list", "--all"]);
    assert!(ran.result.is_ok());
    let lines: Vec<&str> = ran.out.lines().collect();
    for header in ["HARP", "SUMMARY", "STATE", "START", "END"] {
        assert!(lines[0].contains(header), "{header} in {:?}", lines[0]);
    }
    let live_line = lines
        .iter()
        .find(|l| l.contains("amber-swift-owl"))
        .unwrap();
    let ended_line = lines
        .iter()
        .find(|l| l.contains("brisk-copper-moth"))
        .unwrap();
    assert!(
        live_line.contains("live") && !live_line.contains("13:00:00"),
        "{live_line}"
    );
    assert!(
        ended_line.contains("ended") && ended_line.contains("2026-07-17 13:00:00"),
        "{ended_line}"
    );
    assert!(live_line.contains("2026-07-17 12:00:00"));
}

// session.feature: "A fresh project has recorded nothing": three examples, json
// with no flag off a terminal, json and text; empty is an array, empty text is
// the placeholder.
#[test]
fn an_empty_listing_is_an_empty_array_or_the_placeholder() {
    let mut fake = Fake::default();
    let ran = go(&mut fake, Format::Json, &["list"]);
    assert_eq!(json_of(&ran), json!([]));
    let ran = go(&mut fake, Format::Text, &["list"]);
    assert_eq!(ran.out, "(no sessions)\n");
}

// A bare namespace runs its default verb (D28; Go: groupNodeDefault).
#[test]
fn a_bare_session_lists() {
    let mut fake = Fake {
        views: vec![view("amber-swift-owl", State::Ended)],
        ..Fake::default()
    };
    let ran = go(&mut fake, Format::Json, &[]);
    assert_eq!(json_of(&ran)[0]["harp"], "amber-swift-owl");
    assert_eq!(fake.list, [ListQuery::default()]);
}

#[test]
fn all_and_state_reach_the_use_case() {
    let mut fake = Fake::default();
    go(
        &mut fake,
        Format::Json,
        &["list", "--all", "--state", "ended"],
    );
    assert_eq!(
        fake.list,
        [ListQuery {
            all: true,
            state: Some(State::Ended),
            project: None,
        }]
    );
}

// session.feature: "--state refuses a word that is not a state".
#[test]
fn a_state_that_is_not_one_is_refused_naming_the_states() {
    let mut fake = Fake::default();
    for words in [
        &["list", "--state", "running"][..],
        &["search", "x", "--state", "running"],
    ] {
        let ran = go(&mut fake, Format::Json, words);
        let failure = ran.result.unwrap_err();
        assert_eq!(failure.exit, 2);
        assert_eq!(failure.error.code(), "session-cli-bad-state");
        let states = failure
            .error
            .args()
            .into_iter()
            .find(|(n, _)| *n == "states")
            .map(|(_, a)| format!("{a:?}"))
            .unwrap();
        assert!(states.contains("live, ended, unknown"), "{states}");
    }
    assert!(
        fake.list.is_empty() && fake.search.is_empty(),
        "refused before it asked anything"
    );
}

// ---------------------------------------------------------------------- search

// Go: runSessionQuery.
#[test]
fn a_search_carries_its_words_and_scope() {
    let mut fake = Fake {
        views: vec![view("amber-swift-owl", State::Ended)],
        ..Fake::default()
    };
    let ran = go(
        &mut fake,
        Format::Json,
        &[
            "search",
            "lockfile",
            "migration",
            "--all",
            "--state",
            "live",
        ],
    );
    assert_eq!(json_of(&ran)[0]["harp"], "amber-swift-owl");
    assert_eq!(
        fake.search,
        [SearchQuery {
            list: ListQuery {
                all: true,
                state: Some(State::Live),
                project: None
            },
            words: vec!["lockfile".to_string(), "migration".to_string()],
        }]
    );
}

// j001200: "A query that matches nothing says so rather than printing
// nothing": the bytes, not the exit code (Go: TestRunSessionQuery_NoMatches).
#[test]
fn a_search_that_matches_nothing_says_so() {
    let mut fake = Fake::default();
    let ran = go(
        &mut fake,
        Format::Text,
        &["search", "J001200-MATCHES-ABSOLUTELY-NOTHING"],
    );
    assert!(ran.result.is_ok());
    assert_eq!(ran.out.trim(), "(no sessions)");
    let ran = go(&mut fake, Format::Json, &["search", "nothing"]);
    assert_eq!(ran.out.trim(), "[]");
}

// ------------------------------------------------------------------------ show

// Go: renderSessionShow.
#[test]
fn show_prints_the_record_one_fact_per_line() {
    let mut v = view("amber-swift-owl", State::Ended);
    v.record.native_id = "native-1".to_string();
    v.record.ended_at = Some(ts(STARTED + 3600));
    v.record.purged_at = Some(ts(STARTED + 7200));
    v.record.kept = true;
    let mut fake = Fake {
        views: vec![v],
        ..Fake::default()
    };
    let ran = go(&mut fake, Format::Text, &["show", "amber-swift-owl"]);
    assert!(ran.result.is_ok());
    assert_eq!(
        ran.out,
        "amber-swift-owl\n\
         Seeded prompt for amber-swift-owl.\n\
         \n\
         state:    ended\n\
         engine:   claude-code\n\
         native:   native-1\n\
         project:  /work/proj\n\
         started:  2026-07-17 12:00:00\n\
         ended:    2026-07-17 13:00:00\n\
         purged:   its transcript was destroyed\n\
         kept:     its transcript is kept at any age (`ctxloom session keep --undo` releases it)\n"
    );
    assert_eq!(fake.show[0].harp, "amber-swift-owl");
}

// Go: "absent facts left out rather than printed blank", and a title-less
// session says so: j001200's "Show prints what the session was about".
#[test]
fn show_leaves_out_what_is_absent_and_names_a_session_with_no_title() {
    let mut v = view("amber-swift-owl", State::Live);
    v.title = None;
    v.record.ended_at = Some(ts(STARTED + 3600));
    let mut fake = Fake {
        views: vec![v],
        ..Fake::default()
    };
    let ran = go(&mut fake, Format::Text, &["show", "amber-swift-owl"]);
    assert!(ran.out.contains("(no summary)"), "{}", ran.out);
    for absent in ["native:", "ended:", "purged:", "kept:"] {
        assert!(!ran.out.contains(absent), "{absent} in {}", ran.out);
    }
}

#[test]
fn show_in_json_is_the_record() {
    let mut v = view("amber-swift-owl", State::Ended);
    v.record.native_id = "native-1".to_string();
    let mut fake = Fake {
        views: vec![v],
        ..Fake::default()
    };
    let doc = json_of(&go(&mut fake, Format::Json, &["show", "amber-swift-owl"]));
    assert_eq!(doc["harp"], "amber-swift-owl");
    assert_eq!(doc["state"], "ended");
    assert_eq!(doc["engine"], "claude-code");
    assert_eq!(doc["native_session"], "native-1");
    assert_eq!(doc["project"], "/work/proj");
    assert_eq!(doc["title"], "Seeded prompt for amber-swift-owl.");
    assert_eq!(doc["started_at"], "2026-07-17T12:00:00Z");
    assert_eq!(doc["id"], "01010101-0101-0101-0101-010101010101");
}

#[test]
fn a_failure_of_the_use_case_is_the_commands_failure_with_its_exit_code() {
    let mut fake = Fake::default();
    let failure = go(&mut fake, Format::Json, &["show", "no-such-harp"])
        .result
        .unwrap_err();
    assert_eq!(failure.error.code(), "sessions-application-no-session");
    assert_eq!(failure.exit, 1);
}

// ------------------------------------------------------------------------ keep

// Go: runSessionKeep: `{harp, kept}`, and the two sentences.
#[test]
fn keep_and_undo_say_where_the_mark_stands() {
    let mut fake = Fake::default();
    let ran = go(&mut fake, Format::Json, &["keep", "amber-swift-owl"]);
    assert_eq!(
        json_of(&ran),
        json!({"harp": "amber-swift-owl", "kept": true})
    );
    let ran = go(&mut fake, Format::Text, &["keep", "amber-swift-owl"]);
    assert_eq!(
        ran.out,
        "amber-swift-owl is marked keep: its transcript is kept at any age\n"
    );
    let ran = go(
        &mut fake,
        Format::Text,
        &["keep", "amber-swift-owl", "--undo"],
    );
    assert_eq!(
        ran.out,
        "amber-swift-owl is no longer marked keep: its transcript is reclaimable once session_transcript_retention has passed since it ended\n"
    );
    assert_eq!(
        fake.keep.iter().map(|k| k.keep).collect::<Vec<_>>(),
        [true, true, false]
    );
}

#[test]
fn dead_letters_are_said_loudly_beside_the_answer() {
    let mut fake = Fake {
        dead_letters: true,
        ..Fake::default()
    };
    let done = go(&mut fake, Format::Json, &["keep", "amber-swift-owl"])
        .result
        .unwrap();
    assert_eq!(done.lines.len(), 1);
    assert!(done.lines[0].contains("DEAD_LETTER"), "{}", done.lines[0]);
}

// ------------------------------------------------------------------------ edit

// session.feature: "A bare edit refuses, because there is no document to open".
#[test]
fn a_bare_edit_refuses_and_names_the_field_it_takes() {
    let mut fake = Fake::default();
    let failure = go(&mut fake, Format::Json, &["edit", "amber-swift-owl"])
        .result
        .unwrap_err();
    assert_eq!(failure.error.code(), "session-cli-edit-nothing");
    assert_eq!(failure.exit, 2);
    assert!(fake.renamed.is_empty());
}

// Go: sessionEditResult and "renamed X → Y".
#[test]
fn edit_name_renames_and_says_both_names() {
    let mut fake = Fake {
        rename: Some(Ok(Renamed {
            from: "amber-swift-owl".to_string(),
            to: "bright-keen-hawk".to_string(),
            output_dir: "/o/bright-keen-hawk".to_string(),
            dead_letters: Vec::new(),
        })),
        ..Fake::default()
    };
    let ran = go(
        &mut fake,
        Format::Json,
        &["edit", "amber-swift-owl", "--name", "bright-keen-hawk"],
    );
    assert_eq!(
        json_of(&ran),
        json!({"harp": "bright-keen-hawk", "previous_harp": "amber-swift-owl", "fields": ["name"], "output_dir": "/o/bright-keen-hawk"})
    );
    let ran = go(
        &mut fake,
        Format::Text,
        &["edit", "amber-swift-owl", "--name", "bright-keen-hawk"],
    );
    assert_eq!(
        ran.out,
        "renamed amber-swift-owl \u{2192} bright-keen-hawk\n"
    );
    assert_eq!(fake.renamed[0].to, "bright-keen-hawk");
}

// `--name ""` is a caller asking for an invalid name and must reach the
// validator, not look like not passing the flag (Go: Flags().Changed).
#[test]
fn an_empty_name_reaches_the_use_case() {
    let mut fake = Fake::default();
    let ran = go(
        &mut fake,
        Format::Json,
        &["edit", "amber-swift-owl", "--name", ""],
    );
    assert_eq!(fake.renamed.len(), 1);
    assert_eq!(fake.renamed[0].to, "");
    assert!(ran.result.is_err());
}

// ---------------------------------------------------------------------- remove

fn plan(applied: bool) -> Removal {
    Removal {
        scope: Default::default(),
        harp: "amber-swift-owl".to_string(),
        applied,
        index_entry_removed: applied,
        files: FilePlan {
            destroy: vec![PlanItem {
                path: "/h/.ctxloom/sessions/amber-swift-owl/transcripts/transcript.jsonl"
                    .to_string(),
                rel: "transcripts/transcript.jsonl".to_string(),
                class: Class::Machine,
                bytes: 9,
                action: "destroy",
                reason: None,
            }],
            keep: vec![PlanItem {
                path: "/h/.ctxloom/sessions/amber-swift-owl/notes.md".to_string(),
                rel: "notes.md".to_string(),
                class: Class::Authored,
                bytes: 4,
                action: "keep",
                reason: Some("authored: never destroyed".to_string()),
            }],
            bytes_freed: if applied { 9 } else { 0 },
        },
        refusal: None,
        dead_letters: Vec::new(),
    }
}

// Go: sessionRemoveResult.
#[test]
fn a_removal_report_is_the_plan_and_says_nothing_was_removed() {
    let mut fake = Fake {
        removal: Some(plan(false)),
        ..Fake::default()
    };
    let ran = go(&mut fake, Format::Json, &["remove", "amber-swift-owl"]);
    let doc = json_of(&ran);
    assert_eq!(doc["harp"], "amber-swift-owl");
    assert_eq!(doc["applied"], false);
    assert_eq!(doc["index_entry_removed"], false);
    let files = &doc["files"];
    assert_eq!(files["destroy"][0]["rel"], "transcripts/transcript.jsonl");
    assert_eq!(files["destroy"][0]["class"], "machine");
    assert_eq!(files["destroy"][0]["action"], "destroy");
    assert_eq!(files["keep"][0]["class"], "authored");
    assert_eq!(files["keep"][0]["reason"], "authored: never destroyed");
    assert_eq!(files["bytes_freed"], 0);
    let notices = ran.result.unwrap().notices;
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].code(), "sessions-application-removed-nothing");
    let command = notices[0]
        .args()
        .into_iter()
        .find(|(n, _)| *n == "command")
        .map(|(_, a)| format!("{a:?}"))
        .unwrap();
    assert!(
        command.contains("ctxloom session remove amber-swift-owl --yes"),
        "{command}"
    );
    assert!(!fake.remove[0].yes);
}

// Go: renderSessionRemove: the file plan's summary line, its table, and one
// line for the index entry.
#[test]
fn a_removal_report_in_text_names_each_file_and_the_index_entry() {
    let mut fake = Fake {
        removal: Some(plan(false)),
        ..Fake::default()
    };
    let ran = go(&mut fake, Format::Text, &["remove", "amber-swift-owl"]);
    let lines: Vec<&str> = ran.out.lines().collect();
    assert_eq!(
        lines[0],
        "session amber-swift-owl (transcript) \u{2014} would destroy 1 file(s) (0 byte(s) freed)"
    );
    for header in ["PATH", "CLASS", "ACTION", "BYTES", "REASON"] {
        assert!(lines[1].contains(header), "{header}: {}", lines[1]);
    }
    assert!(ran.out.contains("transcripts/transcript.jsonl"));
    assert!(ran.out.contains("notes.md") && ran.out.contains("authored: never destroyed"));
    assert_eq!(
        *lines.last().unwrap(),
        "index entry amber-swift-owl: would be removed"
    );
}

#[test]
fn an_applied_removal_says_destroyed_and_adds_no_notice() {
    let mut fake = Fake {
        removal: Some(plan(true)),
        ..Fake::default()
    };
    let ran = go(
        &mut fake,
        Format::Text,
        &["remove", "amber-swift-owl", "--yes"],
    );
    assert!(ran.out.starts_with(
        "session amber-swift-owl (transcript) \u{2014} destroyed 1 file(s) (9 byte(s) freed)"
    ));
    assert!(ran.out.ends_with("index entry amber-swift-owl: removed\n"));
    assert!(ran.result.unwrap().notices.is_empty());
    assert!(fake.remove[0].yes);
}

#[test]
fn even_if_live_reaches_the_use_case_and_the_applying_command_repeats_it() {
    let mut fake = Fake {
        removal: Some(plan(false)),
        ..Fake::default()
    };
    let ran = go(
        &mut fake,
        Format::Json,
        &["remove", "amber-swift-owl", "--even-if-live"],
    );
    assert!(fake.remove[0].even_if_live);
    let notices = ran.result.unwrap().notices;
    let command = format!("{:?}", notices[0].args());
    assert!(command.contains("--yes --even-if-live"), "{command}");
}

// Go: a refusal prints the plan, then exits refused (2): "an action verb that
// was asked for something and did not do it".
#[test]
fn a_refusal_prints_the_plan_then_fails_refused() {
    let mut refused = plan(false);
    refused.refusal = Some(Refusal::Retained {
        harp: "amber-swift-owl".to_string(),
        detail: "it ended an hour ago".to_string(),
    });
    let mut fake = Fake {
        removal: Some(refused),
        ..Fake::default()
    };
    let ran = go(
        &mut fake,
        Format::Json,
        &["remove", "amber-swift-owl", "--yes"],
    );
    let doc: Value = serde_json::from_str(&ran.out).unwrap();
    assert_eq!(doc["applied"], false, "the plan is still printed");
    let failure = ran.result.unwrap_err();
    assert_eq!(failure.exit, 2);
    assert_eq!(
        failure.error.code(),
        "sessions-application-refused-retained"
    );
}

// ------------------------------------------------------------------ transcript

fn rows() -> Vec<TranscriptRow> {
    vec![
        TranscriptRow {
            harp: "amber-swift-owl".to_string(),
            captured: true,
            bytes: 9,
            path: Some("/h/t.jsonl".to_string()),
        },
        TranscriptRow {
            harp: "brisk-copper-moth".to_string(),
            captured: false,
            bytes: 0,
            path: None,
        },
    ]
}

// Go: sessionTranscriptReport.
#[test]
fn the_transcript_listing_names_every_session_captured_or_not() {
    let mut fake = Fake {
        rows: rows(),
        ..Fake::default()
    };
    let doc = json_of(&go(
        &mut fake,
        Format::Json,
        &["transcript", "list", "--all"],
    ));
    assert_eq!(
        doc,
        json!({"transcripts": [
            {"harp": "amber-swift-owl", "captured": true, "bytes": 9, "path": "/h/t.jsonl"},
            {"harp": "brisk-copper-moth", "captured": false, "bytes": 0},
        ]})
    );
    assert!(fake.transcripts[0].all);
}

// session.feature: "A session with no captured transcript is still named": the
// text says "false".
#[test]
fn the_transcript_table_says_false_for_an_uncaptured_one() {
    let mut fake = Fake {
        rows: rows(),
        ..Fake::default()
    };
    let ran = go(&mut fake, Format::Text, &["transcript", "list"]);
    let lines: Vec<&str> = ran.out.lines().collect();
    for header in ["HARP", "CAPTURED", "BYTES", "PATH"] {
        assert!(lines[0].contains(header), "{header}: {}", lines[0]);
    }
    let moth = lines
        .iter()
        .find(|l| l.contains("brisk-copper-moth"))
        .unwrap();
    assert!(moth.contains("false"), "{moth}");
}

#[test]
fn the_bare_transcript_lists_and_names_the_harp_it_is_given() {
    let mut fake = Fake {
        rows: rows(),
        ..Fake::default()
    };
    go(&mut fake, Format::Json, &["transcript"]);
    go(
        &mut fake,
        Format::Json,
        &["transcript", "list", "amber-swift-owl"],
    );
    assert_eq!(fake.transcripts[0], TranscriptQuery::default());
    assert_eq!(fake.transcripts[1].harp.as_deref(), Some("amber-swift-owl"));
}

#[test]
fn an_empty_transcript_listing_is_the_placeholder() {
    let mut fake = Fake::default();
    assert_eq!(
        go(&mut fake, Format::Text, &["transcript", "list"]).out,
        "(no sessions)\n"
    );
}

// session.feature: "Show prints the whole transcript and exits, ready for a
// pipe": every line of stdout is a JSON object, and nothing else is written.
#[test]
fn transcript_show_prints_the_records_as_they_are_stored() {
    let line = r#"{"v":1,"harp":"amber-swift-owl","engine":"claude-code","seq":0,"ts":"2026-01-01T00:00:00Z","kind":"entry","entry":{"type":"user","content":"Seeded prompt for amber-swift-owl."}}"#;
    let mut fake = Fake {
        transcript: format!("{line}\n"),
        ..Fake::default()
    };
    let ran = go(
        &mut fake,
        Format::Json,
        &["transcript", "show", "amber-swift-owl"],
    );
    assert!(ran.result.is_ok());
    assert_eq!(ran.out, format!("{line}\n"));
    for l in ran.out.lines() {
        assert!(serde_json::from_str::<Value>(l).unwrap().is_object());
    }
}

#[test]
fn transcript_show_in_text_reads_as_a_conversation() {
    let user = r#"{"v":1,"harp":"h","engine":"claude-code","seq":0,"ts":"2026-01-01T00:00:00Z","kind":"entry","entry":{"type":"user","content":"What now?"}}"#;
    let assistant = r#"{"v":1,"harp":"h","engine":"claude-code","seq":1,"ts":"2026-01-01T00:00:01Z","kind":"entry","entry":{"type":"assistant","content":"This."}}"#;
    let session = r#"{"v":1,"harp":"h","engine":"claude-code","seq":2,"ts":"2026-01-01T00:00:02Z","kind":"session","session":{"session_id":"s"}}"#;
    let mut fake = Fake {
        transcript: format!("{user}\n{assistant}\n{session}\n"),
        ..Fake::default()
    };
    let ran = go(&mut fake, Format::Text, &["transcript", "show", "h"]);
    assert_eq!(ran.out, "user: What now?\nassistant: This.\n");
}

// ----------------------------------------------------------------------- shape

// session.feature: "The old rename verb is gone, not hidden" and "Watching
// moved under the transcript it watches": neither is a command here.
#[test]
fn the_retired_verbs_are_not_commands() {
    let command = Session::augment_args(Command::new("session")).no_binary_name(true);
    for retired in ["rename", "watch", "distill", "forget"] {
        assert!(
            command
                .clone()
                .try_get_matches_from([retired, "x"])
                .is_err(),
            "{retired}"
        );
    }
}

// ---------------------------------------------------------------------- import

fn report() -> ImportReport {
    ImportReport {
        imported: vec![Imported {
            harp: "amber-swift-owl".to_string(),
            project: "jolly-mute-magma".to_string(),
            id: "5f0c7a52-1111-5111-8111-000000000001".to_string(),
        }],
        already: vec!["brisk-copper-moth".to_string()],
        left: vec![Left {
            harp: "calm-dusty-wren".to_string(),
            why: Why::NoProject {
                project_dir: "/work/elsewhere".to_string(),
            },
        }],
        dead_letters: Vec::new(),
    }
}

#[test]
fn an_import_reports_what_it_imported_what_it_held_and_what_it_left() {
    let mut fake = Fake {
        report: Some(report()),
        ..Fake::default()
    };
    let doc = json_of(&go(&mut fake, Format::Json, &["import"]));
    assert_eq!(
        doc,
        json!({
            "imported": [{"harp": "amber-swift-owl", "project": "jolly-mute-magma", "id": "5f0c7a52-1111-5111-8111-000000000001"}],
            "already": ["brisk-copper-moth"],
            "left": [{"harp": "calm-dusty-wren", "code": "sessions-application-left-no-project", "args": {"project_dir": "/work/elsewhere"}}],
        })
    );
    assert_eq!(fake.imports, 1);
}

#[test]
fn an_import_in_text_counts_and_names_each_left_session() {
    let mut fake = Fake {
        report: Some(report()),
        ..Fake::default()
    };
    let ran = go(&mut fake, Format::Text, &["import"]);
    let lines: Vec<&str> = ran.out.lines().collect();
    assert_eq!(lines[0], "imported 1, already held 1, left 1");
    assert!(
        lines
            .iter()
            .any(|l| l.contains("calm-dusty-wren") && l.contains("left-no-project")),
        "{lines:?}"
    );
}

#[test]
fn an_import_of_nothing_says_so() {
    let mut fake = Fake::default();
    assert_eq!(
        go(&mut fake, Format::Text, &["import"]).out,
        "imported 0, already held 0, left 0\n"
    );
}

/// A writer that refuses everything.
struct Broken;

impl std::io::Write for Broken {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("broken pipe"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// An answer that cannot be written is the command's failure, not a success.
#[test]
fn an_answer_that_cannot_be_written_fails_the_command() {
    let mut fake = Fake::default();
    let session = parsed(&["list"]);
    let failure = run(&session, &mut fake, Format::Json, &mut Broken).unwrap_err();
    assert_eq!(failure.error.code(), "session-cli-output");
    assert_eq!(failure.exit, 1);
}

// A transcript without a last newline still ends its last line, once.
#[test]
fn a_transcript_is_printed_with_one_closing_newline() {
    let mut fake = Fake {
        transcript: "{\"v\":1}".to_string(),
        ..Fake::default()
    };
    assert_eq!(
        go(&mut fake, Format::Json, &["transcript", "show", "h"]).out,
        "{\"v\":1}\n"
    );
}

// ----------------------------------------------------------------------- sweep

fn notice_command(done: &crate::Done) -> String {
    assert_eq!(done.notices.len(), 1);
    assert_eq!(
        done.notices[0].code(),
        "sessions-application-removed-nothing"
    );
    done.notices[0]
        .args()
        .into_iter()
        .find(|(n, _)| *n == "command")
        .map(|(_, a)| format!("{a:?}"))
        .unwrap()
}

fn ended(harp: &str) -> View {
    let mut v = view(harp, State::Ended);
    v.record.ended_at = Some(ts(STARTED + 60));
    v
}

#[test]
fn a_sweep_reports_each_sessions_fate_with_its_reason_and_command_and_removes_nothing() {
    let mut kept = ended("held-one");
    kept.record.kept = true;
    let mut fake = Fake {
        views: vec![
            view("live-one", State::Live),
            kept,
            ended("plan-one"),
            view("unknown-one", State::Unknown),
        ],
        removal: Some(plan(false)),
        ..Fake::default()
    };
    let ran = go(
        &mut fake,
        Format::Text,
        &["sweep", "--purge-older-than", "1d", "--all-projects"],
    );
    let done = ran.result.unwrap();
    let blank = " ".repeat(28);
    let lines: Vec<&str> = ran.out.lines().collect();
    assert!(
        lines[0].starts_with("Session sweep: purge before 20"),
        "{}",
        ran.out
    );
    assert_eq!(lines[1], "");
    assert_eq!(
        lines[2..],
        [
            format!("  {:<28} {:<8} {:<8} {:>10}", "live-one", "skip", "skipped", ""),
            format!("  {blank} it is running"),
            format!("  {:<28} {:<8} {:<8} {:>10}", "held-one", "keep", "held", ""),
            format!(
                "  {blank} it is marked keep: its transcript is kept at any age"
            ),
            format!("  {blank} run: ctxloom session keep held-one --undo"),
            format!("  {:<28} {:<8} {:<8} {:>10}", "plan-one", "purge", "planned", "9 B"),
            format!("  {blank} run: ctxloom session purge plan-one --yes"),
            format!(
                "  {:<28} {:<8} {:<8} {:>10}",
                "unknown-one", "skip", "skipped", ""
            ),
            format!("  {blank} its liveness cannot be proven"),
            format!("  {blank} run: ctxloom session purge unknown-one --even-if-live"),
            String::new(),
            "0 done, 1 planned, 1 held, 0 left, 2 skipped, 0 failed; 0 session(s) had nothing to do."
                .to_string(),
        ]
    );
    assert_eq!(fake.list.len(), 1);
    assert!(fake.list[0].all);
    assert_eq!(fake.remove.len(), 1, "only the planning ask");
    assert!(!fake.remove[0].yes);
    assert!(notice_command(&done)
        .contains("ctxloom session sweep --all-projects --purge-older-than 1d --yes"));
}

#[test]
fn a_sweep_without_a_purge_age_says_purges_are_held() {
    let mut fake = Fake {
        views: vec![ended("plan-one")],
        removal: Some(plan(false)),
        ..Fake::default()
    };
    let ran = go(&mut fake, Format::Text, &["sweep"]);
    let done = ran.result.unwrap();
    assert!(
        ran.out
            .starts_with("Session sweep: purge before not set (purges are held).\n\n"),
        "{}",
        ran.out
    );
    assert!(fake.remove.is_empty());
    let command = notice_command(&done);
    assert!(command.contains("ctxloom session sweep --yes"), "{command}");
    assert!(!command.contains("--all-projects"), "{command}");
}

#[test]
fn a_sweep_report_in_json_is_the_report() {
    let mut fake = Fake {
        views: vec![ended("plan-one")],
        removal: Some(plan(false)),
        ..Fake::default()
    };
    let doc = json_of(&go(
        &mut fake,
        Format::Json,
        &["sweep", "--purge-older-than", "1d"],
    ));
    assert_eq!(doc["rows"][0]["harp"], "plan-one");
    assert_eq!(doc["rows"][0]["verdict"], "planned");
    assert_eq!(doc["counts"]["planned"], 1);
}

#[test]
fn a_sweep_with_yes_acts_and_leaves_no_notice() {
    let mut fake = Fake {
        views: vec![ended("plan-one")],
        removal: Some(plan(true)),
        ..Fake::default()
    };
    let ran = go(
        &mut fake,
        Format::Text,
        &["sweep", "--purge-older-than", "1d", "--yes"],
    );
    assert!(ran.result.unwrap().notices.is_empty());
    assert!(ran.out.contains("1 done"), "{}", ran.out);
    assert_eq!(fake.remove.len(), 2);
    assert!(fake.remove[1].yes);
}

#[test]
fn a_sweep_that_failed_while_acting_exits_nonzero_but_a_report_of_a_failure_does_not() {
    // The fake has nothing to remove: every purge fails.
    let mut fake = Fake {
        views: vec![ended("plan-one")],
        removal: None,
        ..Fake::default()
    };
    let ran = go(
        &mut fake,
        Format::Text,
        &["sweep", "--purge-older-than", "1d"],
    );
    assert!(ran.out.contains("1 failed"), "{}", ran.out);
    assert!(ran.result.is_ok(), "a report only reports");
    let ran = go(
        &mut fake,
        Format::Text,
        &["sweep", "--purge-older-than", "1d", "--yes"],
    );
    let failure = ran.result.unwrap_err();
    assert_eq!(failure.error.code(), "session-cli-sweep-failed");
    assert_eq!(failure.exit, 1);
}
