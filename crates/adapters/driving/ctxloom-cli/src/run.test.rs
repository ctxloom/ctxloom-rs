//! The command line as `run` answers it, over an extension of its own: help,
//! version, usage errors, the exit ladder and what an extension is handed.
//! Ported from Go's `exit_codes_test.go`, `version.feature` and
//! `cli_flag_gate_test.go` behaviours as D28 reshapes them.

use std::ffi::OsString;
use std::io::Write;

#[cfg(feature = "test-support")]
use angzarr_diagnostic::Diagnostic;

use angzarr_presentation::Format;
use clap::{Arg, Command};
use sessions_application::{
    AppError, BindRequest, Bound, ImportReport, ImportRequest, ImportService, KeepRequest, Kept,
    ListQuery, Removal, RemoveRequest, RenameRequest, Renamed, SearchQuery, SessionService,
    ShowQuery, TranscriptQuery, TranscriptRow, View,
};

use super::*;

/// What the fake extension is told to answer.
pub(crate) struct Fake {
    pub(crate) answer: Result<u8, u8>,
}

impl Extension for Fake {
    fn nouns(&self) -> Vec<Command> {
        vec![Command::new("demo")
            .subcommand(Command::new("list"))
            .arg(Arg::new("word").long("word"))]
    }

    fn run(
        &mut self,
        noun: &str,
        call: &Call<'_>,
        _catalog: &Catalog,
        out: &mut dyn Write,
        _err: &mut dyn Write,
    ) -> Result<u8, Failure> {
        let _ = writeln!(
            out,
            "{noun} explicit={:?} default={:?} terminal={} degraded={} companions={} sets={:?}",
            call.explicit_format,
            call.default_format,
            call.stdout_is_terminal,
            call.degraded,
            call.no_companions,
            call.config_set
        );
        self.answer.map_err(|exit| Failure {
            cause: Box::new(CliError::Format {
                value: "odd".to_string(),
            }),
            exit,
        })
    }
}

/// No sessions: a command line test of an extension never reaches them.
pub(crate) struct NoSessions;

impl SessionService for NoSessions {
    async fn list(&mut self, _: ListQuery) -> Result<Vec<View>, AppError> {
        unreachable!("no session command is run here")
    }

    async fn search(&mut self, _: SearchQuery) -> Result<Vec<View>, AppError> {
        unreachable!("no session command is run here")
    }

    async fn show(&mut self, _: ShowQuery) -> Result<View, AppError> {
        unreachable!("no session command is run here")
    }

    async fn keep(&mut self, _: KeepRequest) -> Result<Kept, AppError> {
        unreachable!("no session command is run here")
    }

    async fn remove(&mut self, _: RemoveRequest) -> Result<Removal, AppError> {
        unreachable!("no session command is run here")
    }

    async fn rename(&mut self, _: RenameRequest) -> Result<Renamed, AppError> {
        unreachable!("no session command is run here")
    }

    async fn transcripts(&mut self, _: TranscriptQuery) -> Result<Vec<TranscriptRow>, AppError> {
        unreachable!("no session command is run here")
    }

    async fn transcript(&mut self, _: &str) -> Result<String, AppError> {
        unreachable!("no session command is run here")
    }

    async fn bind(&mut self, _: BindRequest) -> Result<Bound, AppError> {
        unreachable!("no session command is run here")
    }
}

impl isolation_application::WorktreeUse for NoSessions {}

impl ImportService for NoSessions {
    async fn import(&mut self, _: ImportRequest) -> Result<ImportReport, AppError> {
        unreachable!("no session command is run here")
    }
}

pub(crate) struct Wire(pub(crate) Fake, pub(crate) NoSessions);

impl Services for Wire {
    type Sessions = NoSessions;

    fn sessions(&mut self) -> Result<&mut NoSessions, Box<dyn angzarr_diagnostic::Diagnostic>> {
        Ok(&mut self.1)
    }

    fn extension(&mut self) -> &mut dyn Extension {
        &mut self.0
    }

    #[cfg(feature = "test-support")]
    fn session_name(&mut self) -> String {
        String::new()
    }

    #[cfg(feature = "test-support")]
    fn run_id(&mut self, _: &str, _: u64) -> Result<String, Box<dyn Diagnostic>> {
        Ok(String::new())
    }

    #[cfg(feature = "test-support")]
    fn open_store(&mut self, _: &str) -> Result<String, Box<dyn Diagnostic>> {
        Ok(String::new())
    }

    #[cfg(feature = "test-support")]
    fn diagnostic_samples(&self) -> Vec<Box<dyn Diagnostic>> {
        Vec::new()
    }
}

struct Ran {
    code: u8,
    out: String,
    err: String,
}

fn go_in(terminal: bool, answer: Result<u8, u8>, line: &[&str]) -> Ran {
    let context = Context {
        version: "1.2.3".to_string(),
        lock_protocol: 4,
        stdout_is_terminal: terminal,
    };
    let catalog = Catalog::en_us(&[crate::EN_US]).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let args = std::iter::once("ctxloom")
        .chain(line.iter().copied())
        .map(OsString::from);
    let code = run(
        args,
        &context,
        &catalog,
        &mut Wire(Fake { answer }, NoSessions),
        &mut out,
        &mut err,
    );
    Ran {
        code,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

fn go(line: &[&str]) -> Ran {
    go_in(false, Ok(0), line)
}

// version.feature: every spelling of version agrees.
#[test]
fn test_every_spelling_of_version_agrees() {
    let json = go(&["version", "--format", "json"]);
    assert_eq!(json.code, 0);
    assert_eq!(
        json.out,
        "{\n  \"name\": \"ctxloom\",\n  \"version\": \"1.2.3\",\n  \"lock_protocol\": 4\n}\n"
    );
    assert_eq!(go(&["version", "--format", "text"]).out, "1.2.3\n");
    assert_eq!(go_in(true, Ok(0), &["version"]).out, "1.2.3\n");
    let flag = go(&["--version"]);
    assert_eq!(
        (flag.code, flag.out.as_str()),
        (0, "ctxloom version 1.2.3\n")
    );
}

// help is an answer, not an error; a bare command prints its help.
#[test]
fn test_help_is_an_answer() {
    for line in [&["-h"][..], &["--help"], &["version", "-h"], &[]] {
        let ran = go(line);
        assert_eq!(ran.code, 0, "{line:?}");
        assert!(ran.out.contains("Usage:"), "{line:?}: {}", ran.out);
        assert!(ran.err.is_empty());
    }
}

// exit_codes_test.go: a refusal is 2, with the envelope under json and
// words (and a fix) under text.
#[test]
fn test_a_usage_error_is_refused_with_its_envelope_or_words() {
    let json = go(&["--bogus", "--format", "json"]);
    assert_eq!(json.code, 2);
    assert!(json.out.is_empty());
    let envelope: serde_json::Value = serde_json::from_str(&json.err).unwrap();
    assert_eq!(envelope["code"], "ctxloom-cli-usage");
    assert!(envelope["message"].as_str().unwrap().contains("--bogus"));
    let text = go(&["--bogus", "--format", "text"]);
    assert_eq!(text.code, 2);
    assert!(text
        .err
        .starts_with("ctxloom: error: unexpected argument '--bogus'"));
    assert!(text
        .err
        .ends_with("\n  fix: run ctxloom -h, or the command with -h, for its usage\n"));
}

#[test]
fn test_an_unknown_verb_says_did_you_mean_and_lists_the_verbs() {
    let ran = go(&["versoin", "--format=text"]);
    assert_eq!(ran.code, 2);
    assert!(ran
        .err
        .starts_with("ctxloom: unknown command versoin for ctxloom: Did you mean one of "));
    // the verbs: an extension's noun, `version`, and the commands other
    // slices own (`session`; `hook` is hidden)
    for verb in ["demo", "version", "session"] {
        assert!(ran.err.contains(verb), "{verb} is listed: {}", ran.err);
    }
    assert!(ran
        .err
        .ends_with("  fix: run ctxloom -h for its commands\n"));
    let nested = go(&["demo", "lisst", "--format", "json"]);
    assert_eq!(nested.code, 2);
    let envelope: serde_json::Value = serde_json::from_str(&nested.err).unwrap();
    assert_eq!(envelope["code"], "ctxloom-cli-unknown-verb");
    assert_eq!(envelope["args"]["command"], "ctxloom demo");
    assert_eq!(envelope["args"]["verb"], "lisst");
    assert!(envelope["args"]["choices"]
        .as_str()
        .unwrap()
        .contains("list"));
}

// A format that is neither is refused, whatever it once was (D28).
#[test]
fn test_a_format_that_is_neither_json_nor_text_is_refused() {
    let ran = go(&["version", "--format", "yaml"]);
    assert_eq!(ran.code, 2);
    assert!(ran.out.is_empty());
    let envelope = go_in(true, Ok(0), &["version", "--format=yaml"]);
    assert!(envelope
        .err
        .starts_with("ctxloom: the format yaml is not one of text or json\n"));
    let json = go(&["version", "--format", "yaml", "--format", "json"]);
    assert_eq!(json.code, 0);
    let shown = go(&["demo", "--format", "yaml"]);
    assert_eq!(shown.code, 2);
    assert!(shown.out.is_empty());
}

// What an extension is handed: its noun, the format asked for and the
// terminal's, and the global flags.
#[test]
fn test_an_extension_is_handed_its_noun_and_the_global_flags() {
    let ran = go(&[
        "demo",
        "list",
        "--format",
        "text",
        "--degraded",
        "--no-companions",
        "--config-set",
        "a.b=c",
        "--config-set",
        "d=e",
    ]);
    assert_eq!(ran.code, 0);
    assert_eq!(
        ran.out,
        "demo explicit=Some(Text) default=Json terminal=false degraded=true companions=true sets=[\"a.b=c\", \"d=e\"]\n"
    );
    let plain = go_in(true, Ok(0), &["demo"]);
    assert_eq!(
        plain.out,
        "demo explicit=None default=Text terminal=true degraded=false companions=false sets=[]\n"
    );
    assert_eq!(Format::Text, Format::choose(false, None, true));
}

// exit_codes_test.go: an extension's exit code is the process's, its failure
// is reported in the format asked for.
#[test]
fn test_an_extensions_exit_code_and_failure_pass_through() {
    assert_eq!(go_in(false, Ok(3), &["demo"]).code, 3);
    assert_eq!(go_in(false, Ok(7), &["demo"]).code, 7);
    let failed = go_in(false, Err(1), &["demo"]);
    assert_eq!(failed.code, 1);
    let envelope: serde_json::Value = serde_json::from_str(&failed.err).unwrap();
    assert_eq!(envelope["code"], "ctxloom-cli-unknown-format");
    assert_eq!(envelope["args"]["value"], "odd");
    let refused = go_in(false, Err(2), &["demo", "--format", "text"]);
    assert_eq!(refused.code, 2);
    assert!(refused.err.starts_with(
        "ctxloom: the format odd is not one of text or json\n  fix: use --format json"
    ));
    assert_eq!(go_in(true, Err(3), &["demo"]).code, 3);
}

// The builtin commands are not the extension's.
#[test]
fn test_a_builtin_command_never_reaches_the_extension() {
    let ran = go_in(false, Err(9), &["version"]);
    assert_eq!(ran.code, 0);
    assert!(!ran.out.contains("explicit="));
}

// -v is accepted and changes nothing.
#[test]
fn test_verbose_changes_no_output() {
    assert_eq!(go(&["-v", "version", "--format", "text"]).out, "1.2.3\n");
}

// An answer that cannot be written is exit 1.
#[test]
fn test_an_unwritable_answer_is_a_failure() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("closed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let context = Context {
        version: "1".to_string(),
        lock_protocol: 1,
        stdout_is_terminal: false,
    };
    let catalog = Catalog::en_us(&[crate::EN_US]).unwrap();
    let mut err = Vec::new();
    let code = run(
        ["ctxloom", "version"].map(OsString::from),
        &context,
        &catalog,
        &mut Wire(Fake { answer: Ok(0) }, NoSessions),
        &mut Broken,
        &mut err,
    );
    assert_eq!(code, 1);
    let envelope: serde_json::Value = serde_json::from_slice(&err).unwrap();
    assert_eq!(envelope["code"], "ctxloom-cli-output");
    assert_eq!(envelope["args"]["ext_err_txt"], "closed");
}

// The seam with nothing behind it adds nothing and answers success.
#[test]
fn test_no_extension_adds_no_nouns_and_answers_success() {
    let mut none = NoExtension;
    assert!(none.nouns().is_empty());
    let matches = Command::new("x").get_matches_from(["x"]);
    let call = Call::new(&matches, None, Format::Json);
    let catalog = Catalog::en_us(&[crate::EN_US]).unwrap();
    let ran = none.run("x", &call, &catalog, &mut Vec::new(), &mut Vec::new());
    assert!(matches!(ran, Ok(0)));
    assert_eq!(call.format(), Format::Json);
    let asked = Call {
        explicit_format: Some(Format::Text),
        ..call
    };
    assert_eq!(asked.format(), Format::Text);
}

// D28: the exit ladder by error.
#[test]
fn test_each_error_has_its_exit_code() {
    assert_eq!(
        CliError::Usage {
            detail: String::new()
        }
        .exit_code(),
        2
    );
    assert_eq!(
        CliError::UnknownVerb {
            verb: String::new(),
            command: String::new(),
            choices: String::new()
        }
        .exit_code(),
        2
    );
    assert_eq!(
        CliError::Format {
            value: String::new()
        }
        .exit_code(),
        2
    );
    assert_eq!(
        CliError::Output {
            detail: String::new()
        }
        .exit_code(),
        1
    );
}
