//! The setup commands' words, exactly as they are printed.

use setup_application::{
    EngineOutcome, GitignoreReport, HookRow, HooksEvent, HooksList, InstallPlan, InstallReport,
    MaterializeResult, NotCarried, PullReport, UninstallReport,
};

use super::*;

fn words(f: impl FnOnce(&mut dyn Write) -> io::Result<()>) -> String {
    let mut out = Vec::new();
    f(&mut out).expect("written");
    String::from_utf8(out).expect("text")
}

const NO_ENGINE_FILE: &str = "No engine file was written: a `ctxloom run` session delivers ctxloom's hooks and MCP server into its own session home. An engine launched directly here gets neither.\n";

fn install_report(initialized: bool, status: &str, installed: u64, error: &str) -> InstallReport {
    InstallReport {
        app_dir: "/p/.ctxloom".into(),
        initialized,
        gitignore: "/p/.gitignore".into(),
        gitignore_status: "updated".into(),
        pull: PullReport {
            status: status.into(),
            installed,
            error: error.into(),
        },
    }
}

fn install_words(report: &InstallReport) -> String {
    words(|w| install(w, report))
}

#[test]
fn install_says_what_it_initialized_and_ends_with_the_engine_file_note() {
    assert_eq!(
        install_words(&install_report(true, "none", 0, "")),
        format!("Initialized ctxloom directory: /p/.ctxloom\nupdated: /p/.ctxloom/.gitignore\n{NO_ENGINE_FILE}")
    );
    assert_eq!(
        install_words(&install_report(false, "none", 0, "")),
        format!("updated: /p/.ctxloom/.gitignore\n{NO_ENGINE_FILE}")
    );
}

#[test]
fn install_reports_the_pull_by_its_status() {
    let skipped = install_words(&install_report(false, "skipped", 0, ""));
    assert!(skipped.contains(
        "Dependencies were not pulled (--no-pull); run 'ctxloom deps pull' to install them.\n"
    ));
    let pulled = install_words(&install_report(false, "pulled", 3, ""));
    assert!(pulled.contains("Pulled 3 dependencies\n"), "{pulled}");
    let nothing = install_words(&install_report(false, "pulled", 0, ""));
    assert!(!nothing.contains("Pulled"), "{nothing}");
    let one = install_words(&install_report(false, "pulled", 1, ""));
    assert!(one.contains("Pulled 1 dependencies\n"), "{one}");
    let failed = install_words(&install_report(false, "failed", 0, "offline"));
    assert!(failed.contains("The dependency pull failed (offline); 'ctxloom deps pull' retries.\n"));
    assert!(!failed.contains("Pulled"));
}

#[test]
fn the_plan_reuses_or_scaffolds() {
    let mut plan = InstallPlan {
        ctxloom_dir: "/p/.ctxloom".into(),
        ctxloom_dir_exists: true,
        engine: "mock".into(),
        gitignore: "/p/.gitignore".into(),
    };
    let tail = "  - update .gitignore: /p/.gitignore\n  - write no engine file: hooks, MCP and statusline reach an engine through a `ctxloom run` session's own home\n";
    assert_eq!(
        words(|w| self::plan(w, &plan)),
        format!("manage install would:\n  - reuse existing .ctxloom: /p/.ctxloom\n{tail}")
    );
    plan.ctxloom_dir_exists = false;
    assert_eq!(
        words(|w| self::plan(w, &plan)),
        format!("manage install would:\n  - scaffold .ctxloom: /p/.ctxloom (engine: mock)\n{tail}")
    );
}

#[test]
fn uninstall_names_the_backends_and_the_note() {
    let report = UninstallReport {
        status: "ok".into(),
        backends: vec!["claude-code".into()],
        errors: vec![],
        note: "the note".into(),
    };
    assert_eq!(
        words(|w| uninstall(w, &report)),
        "Removed ctxloom harness from: [\"claude-code\"]\nthe note\n"
    );
}

#[test]
fn gitignore_says_whether_it_wrote() {
    let mut report = GitignoreReport {
        status: "updated".into(),
        path: "/p/.gitignore".into(),
        nested_path: "/p/.ctxloom/.gitignore".into(),
        nested_written: true,
    };
    assert_eq!(
        words(|w| gitignore(w, &report)),
        "Wrote /p/.ctxloom/.gitignore (ctxloom's private-state rules; commit it)\n"
    );
    report.nested_written = false;
    assert_eq!(
        words(|w| gitignore(w, &report)),
        "No change: /p/.ctxloom/.gitignore already matches ctxloom's private-state rules\n"
    );
}

fn hook(command: &str, prompt: &str, source: &str) -> HookRow {
    HookRow {
        position: 2,
        declared: 2,
        command: command.into(),
        args: vec!["a".into()],
        prompt: prompt.into(),
        kind: String::new(),
        source_kind: "bundle".into(),
        source: source.into(),
    }
}

#[test]
fn a_hook_is_labelled_by_its_command_then_its_prompt_then_a_placeholder() {
    let list = HooksList {
        events: vec![HooksEvent {
            event: "pre_tool".into(),
            hooks: vec![
                hook("run", "ignored", "x:y"),
                hook("", "say hi", ""),
                hook("", "", ""),
            ],
        }],
    };
    assert_eq!(
        words(|w| hooks(w, &list)),
        "pre_tool\n  2. run a                    [bundle x:y]\n  2. say hi                   [bundle]\n  2. (hook)                   [bundle]\n"
    );
}

fn engine(
    wrote: &[&str],
    released: &[&str],
    not_carried: &[(&str, &str)],
    skipped: &[&str],
) -> EngineOutcome {
    let own = |v: &[&str]| v.iter().map(ToString::to_string).collect();
    EngineOutcome {
        engine: "claude-code".into(),
        wrote: own(wrote),
        released: own(released),
        not_carried: not_carried
            .iter()
            .map(|(s, r)| NotCarried {
                surface: s.to_string(),
                reason: r.to_string(),
            })
            .collect(),
        skipped: own(skipped),
        ..EngineOutcome::default()
    }
}

fn result(engines: Vec<EngineOutcome>) -> MaterializeResult {
    MaterializeResult {
        target: "/out".into(),
        engines,
        status: "ok".into(),
        ..MaterializeResult::default()
    }
}

#[test]
fn materialize_lists_only_what_each_engine_did() {
    let mut r = result(vec![
        engine(&["a.md", "b.md"], &[], &[], &[]),
        engine(&[], &["c.md"], &[("hooks", "no hooks")], &["unchanged"]),
        engine(&[], &[], &[], &[]),
    ]);
    r.warnings = vec!["careful".into()];
    r.errors = vec!["boom".into()];
    assert_eq!(
        words(|w| materialize(w, &r)),
        "Target: /out\ncareful\n\
         claude-code: wrote a.md, b.md\n\
         claude-code: released c.md\n\
         claude-code: NOT carried: hooks (no hooks)\n\
         claude-code: unchanged\n\
         error: boom\n\
         Status: ok\n"
    );
}
