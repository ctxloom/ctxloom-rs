//! The registered companions' loadouts joined to every assembly (virtuous-word,
//! companions slice 1): Go's `companionAsks` and `Prober.ProbeCompanionLoadouts`
//! over the `CompanionLoadouts` port's fake.

use ctxloom_ports::CompanionLoadout::{self, Absent, Loaded, NoLoadout, ProbeFailed};

use crate::fakes::{plain, Mem, Setup, ENVELOPE};
use crate::{AppError, AssembleRequest, Assembled, Flags, Library, Notice, Outcome};

const BUNDLE: &[(&str, &str)] = &[ENVELOPE, ("fragments/own.md", "OWN-BODY\n")];

const CONFIG_WITH: &str = "schema_version: 9\ncompanions:\n  reprise:\n    command: [reprise]\n";

const LOADOUT: &str = "run:\n  version: 1.0.0\n  fragments:\n    guide:\n      content: COMPANION-MARKER\n    later:\n      premise: about to do the later thing\n      content: LATER-BODY\n";

fn setup<'a>(
    layers: &'a [&'a str],
    companions: &'a [(&'a str, CompanionLoadout)],
    environ: &'a [(&'a str, &'a str)],
) -> (Library, std::sync::Arc<Mem>) {
    Setup {
        bundles: &[("own", BUNDLE)],
        layers,
        companions,
        environ,
        ..Setup::default()
    }
    .fixture()
}

fn assemble(library: &Library, flags: &Flags) -> Outcome<Assembled> {
    library.assemble(
        flags,
        &AssembleRequest {
            fragments: vec!["own".to_string()],
            ..AssembleRequest::default()
        },
    )
}

fn context(library: &Library, flags: &Flags) -> (String, Vec<Notice>) {
    let outcome = assemble(library, flags);
    (outcome.result.expect("assembled").context, outcome.notices)
}

fn only_reprise(loadout: CompanionLoadout) -> [(&'static str, CompanionLoadout); 1] {
    [("reprise", loadout)]
}

#[test]
fn a_registered_companions_fragment_joins_the_context_after_the_selected_ones() {
    let companions = only_reprise(Loaded(LOADOUT.to_string()));
    let (library, mem) = setup(&[CONFIG_WITH], &companions, &[]);
    let (context, notices) = context(&library, &plain());
    assert_eq!(context, "OWN-BODY\n\n---\n\nCOMPANION-MARKER");
    assert!(notices.is_empty(), "{notices:?}");
    assert_eq!(mem.asked_companions(), ["reprise"]);
}

#[test]
fn the_companion_fragment_is_reported_loaded_under_its_companion_reference() {
    let companions = only_reprise(Loaded(LOADOUT.to_string()));
    let (library, _) = setup(&[CONFIG_WITH], &companions, &[]);
    let assembled = assemble(&library, &plain()).result.unwrap();
    assert_eq!(
        assembled.fragments_loaded,
        [
            "ctxloom+local:own#fragments/own",
            "ctxloom+companion:reprise#fragments/guide"
        ]
    );
}

#[test]
fn a_companion_fragment_with_a_premise_is_held_back_and_indexed() {
    let companions = only_reprise(Loaded(LOADOUT.to_string()));
    let (library, _) = setup(&[CONFIG_WITH], &companions, &[]);
    let (context, _) = context(&library, &plain());
    assert!(!context.contains("LATER-BODY"), "{context}");
    let premises = library.premises(&plain()).result.unwrap();
    let rows: Vec<(String, String)> = premises
        .fragments
        .into_iter()
        .map(|r| (r.name, r.premise))
        .collect();
    assert_eq!(
        rows,
        [(
            "ctxloom+companion:reprise#fragments/later".to_string(),
            "about to do the later thing".to_string()
        )]
    );
}

#[test]
fn asking_for_a_premised_companion_fragment_by_name_delivers_it() {
    let companions = only_reprise(Loaded(LOADOUT.to_string()));
    let (library, _) = setup(&[CONFIG_WITH], &companions, &[]);
    let outcome = library.assemble(
        &plain(),
        &AssembleRequest {
            fragments: vec!["ctxloom+companion:reprise#fragments/later".to_string()],
            ..AssembleRequest::default()
        },
    );
    let assembled = outcome.result.unwrap();
    assert!(
        assembled.context.contains("LATER-BODY"),
        "{}",
        assembled.context
    );
    assert!(assembled.missing_fragments.is_empty());
}

#[test]
fn only_a_registered_companion_is_asked() {
    let companions = [("ltk", Loaded(LOADOUT.to_string()))];
    let (library, mem) = setup(&[CONFIG_WITH], &companions, &[]);
    let (context, _) = context(&library, &plain());
    assert_eq!(context, "OWN-BODY");
    assert_eq!(mem.asked_companions(), ["reprise"]);
}

#[test]
fn nothing_registered_asks_nothing() {
    let companions = only_reprise(Loaded(LOADOUT.to_string()));
    let (library, mem) = setup(&["schema_version: 9\n"], &companions, &[]);
    let (context, notices) = context(&library, &plain());
    assert_eq!(context, "OWN-BODY");
    assert!(notices.is_empty());
    assert!(mem.asked_companions().is_empty());
}

#[test]
fn a_library_composed_without_companions_asks_none() {
    let mut library = Setup {
        bundles: &[("own", BUNDLE)],
        layers: &[CONFIG_WITH],
        ..Setup::default()
    }
    .library();
    library.companions = None;
    let (context, notices) = context(&library, &plain());
    assert_eq!(context, "OWN-BODY");
    assert!(notices.is_empty());
}

#[test]
fn the_flag_skips_every_companion() {
    let companions = only_reprise(Loaded(LOADOUT.to_string()));
    let (library, mem) = setup(&[CONFIG_WITH], &companions, &[]);
    let flags = Flags {
        no_companions: true,
        ..plain()
    };
    let (context, notices) = context(&library, &flags);
    assert_eq!(context, "OWN-BODY");
    assert!(notices.is_empty());
    assert!(mem.asked_companions().is_empty());
}

#[test]
fn the_environment_switch_skips_every_companion() {
    for on in ["1", "true", "TRUE", " yes ", "on", "y", "t"] {
        let companions = only_reprise(Loaded(LOADOUT.to_string()));
        let (library, mem) = setup(
            &[CONFIG_WITH],
            &companions,
            &[("CTXLOOM_NO_COMPANIONS", on)],
        );
        let (context, _) = context(&library, &plain());
        assert_eq!(context, "OWN-BODY", "{on:?}");
        assert!(mem.asked_companions().is_empty(), "{on:?}");
    }
}

#[test]
fn a_switch_that_is_off_or_not_a_switch_leaves_companions_on() {
    for off in ["0", "", "false", "no", "off", "maybe"] {
        let companions = only_reprise(Loaded(LOADOUT.to_string()));
        let (library, mem) = setup(
            &[CONFIG_WITH],
            &companions,
            &[("CTXLOOM_NO_COMPANIONS", off)],
        );
        let (context, _) = context(&library, &plain());
        assert!(context.contains("COMPANION-MARKER"), "{off:?}");
        assert_eq!(mem.asked_companions(), ["reprise"], "{off:?}");
    }
}

#[test]
fn a_companion_not_on_the_path_is_a_warning_and_nothing_more() {
    let (library, _) = setup(&[CONFIG_WITH], &only_reprise(Absent), &[]);
    let (context, notices) = context(&library, &plain());
    assert_eq!(context, "OWN-BODY");
    assert!(
        matches!(notices.as_slice(), [Notice::CompanionNotOnPath { name }] if name == "reprise"),
        "{notices:?}"
    );
}

#[test]
fn a_companion_with_no_loadout_is_quiet() {
    let (library, _) = setup(&[CONFIG_WITH], &only_reprise(NoLoadout), &[]);
    let (context, notices) = context(&library, &plain());
    assert_eq!(context, "OWN-BODY");
    assert!(notices.is_empty(), "{notices:?}");
}

#[test]
fn a_companion_whose_probe_failed_is_a_warning_and_nothing_more() {
    let (library, _) = setup(
        &[CONFIG_WITH],
        &only_reprise(ProbeFailed("exit status: 3".to_string())),
        &[],
    );
    let (context, notices) = context(&library, &plain());
    assert_eq!(context, "OWN-BODY");
    assert!(
        matches!(
            notices.as_slice(),
            [Notice::LoadoutProbeFailed { name, detail }]
                if name == "reprise" && detail == "exit status: 3"
        ),
        "{notices:?}"
    );
}

#[test]
fn a_loadout_that_will_not_parse_is_a_warning_and_nothing_more() {
    for text in ["not: a loadout\n", "run: [\n", "", "init:\n  toling: x\n"] {
        let (library, _) = setup(&[CONFIG_WITH], &only_reprise(Loaded(text.to_string())), &[]);
        let (context, notices) = context(&library, &plain());
        assert_eq!(context, "OWN-BODY", "{text:?}");
        assert!(
            matches!(notices.as_slice(), [Notice::LoadoutUnparseable { name, .. }] if name == "reprise"),
            "{text:?}: {notices:?}"
        );
    }
}

#[test]
fn a_failing_companion_is_a_warning_under_strict_too() {
    // `plain()` is not degraded: a finding would refuse, a notice does not.
    for loadout in [
        Absent,
        ProbeFailed("x".to_string()),
        Loaded("bad: 1".to_string()),
    ] {
        let (library, _) = setup(&[CONFIG_WITH], &only_reprise(loadout), &[]);
        let outcome = assemble(&library, &plain());
        assert!(!plain().degraded);
        assert!(outcome.result.is_ok());
        assert_eq!(outcome.notices.len(), 1);
    }
}

#[test]
fn one_companion_failing_does_not_cost_another_its_fragments() {
    let layers = ["schema_version: 9\ncompanions:\n  acme: {command: [acme]}\n  reprise: {command: [reprise]}\n  ltk: {command: [ltk]}\n"];
    let companions = [
        ("acme", Absent),
        ("reprise", Loaded(LOADOUT.to_string())),
        (
            "ltk",
            Loaded(
                "run:\n  version: 1\n  fragments:\n    f:\n      content: LTK-BODY\n".to_string(),
            ),
        ),
    ];
    let (library, mem) = setup(&layers, &companions, &[]);
    let (context, notices) = context(&library, &plain());
    // Companion references in order: ltk before reprise whatever the registry's.
    assert_eq!(
        context,
        "OWN-BODY\n\n---\n\nLTK-BODY\n\n---\n\nCOMPANION-MARKER"
    );
    assert_eq!(notices.len(), 1);
    assert_eq!(mem.asked_companions(), ["acme", "ltk", "reprise"]);
}

#[test]
fn an_init_only_loadout_delivers_no_context_and_no_warning() {
    let companions = only_reprise(Loaded("init:\n  tooling: jq\n".to_string()));
    let (library, _) = setup(&[CONFIG_WITH], &companions, &[]);
    let (context, notices) = context(&library, &plain());
    assert_eq!(context, "OWN-BODY");
    assert!(notices.is_empty());
}

#[test]
fn a_command_that_assembles_twice_asks_each_companion_once_and_warns_once() {
    let (library, mem) = setup(&[CONFIG_WITH], &only_reprise(Absent), &[]);
    let outcome = library.run(&plain(), |p| {
        let mut findings = Vec::new();
        p.assemble_set(&[], &["own".to_string()], &[], &mut findings);
        p.assemble_set(&[], &["own".to_string()], &[], &mut findings);
        Ok::<(), AppError>(())
    });
    assert!(outcome.result.is_ok());
    assert_eq!(mem.asked_companions(), ["reprise"]);
    assert_eq!(outcome.notices.len(), 1);
}

#[test]
fn a_registered_name_that_is_no_plain_name_is_said_and_never_asked() {
    let layers = ["schema_version: 9\ncompanions:\n  -x: {command: [x]}\n  a/b: {command: [x]}\n"];
    let (library, mem) = setup(&layers, &[], &[]);
    let (context, notices) = context(&library, &plain());
    assert_eq!(context, "OWN-BODY");
    assert_eq!(notices.len(), 2, "{notices:?}");
    assert!(notices.iter().all(|n| matches!(
        n,
        Notice::LoadoutProbeFailed { detail, .. } if detail == "not a plain companion name"
    )));
    assert!(mem.asked_companions().is_empty());
}

#[test]
fn companions_are_assembled_even_when_nothing_else_is_selected() {
    let companions = only_reprise(Loaded(LOADOUT.to_string()));
    let (library, _) = setup(&[CONFIG_WITH], &companions, &[]);
    let outcome = library.assemble(&plain(), &AssembleRequest::default());
    let assembled = outcome.result.unwrap();
    assert_eq!(assembled.context, "COMPANION-MARKER");
    assert_eq!(
        assembled.fragments_loaded,
        ["ctxloom+companion:reprise#fragments/guide"]
    );
    // Without a companion, nothing selected is nothing assembled.
    let (bare, _) = setup(&[CONFIG_WITH], &[], &[]);
    let nothing = bare.assemble(&plain(), &AssembleRequest::default());
    assert_eq!(nothing.result.unwrap().context, "");
}
