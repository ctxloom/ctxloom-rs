//! Companion loadouts in the shared assembly (virtuous-word, companions
//! slice 1), through the binary: a registered companion's fragments reach
//! `materialize`, `context assemble` and `run --dry-run`; `--no-companions` and
//! `CTXLOOM_NO_COMPANIONS` run none; a companion that fails never fails the
//! command. The companion is the stub, a compiled program the binary execs.

use super::companion_stub::{Misbehaving, Stub};
use super::library::Project;
use crate::tests::files;

const OWN: &str = "FRAGMENT-BODY-own";
const MARKER: &str = "STUB-COMPANION-GUIDANCE-MARKER";
const LATER: &str = "STUB-COMPANION-PREMISED-BODY";

const LOADOUT: &str = "run:\n  version: 1.0.0\n  fragments:\n    guidance:\n      content: STUB-COMPANION-GUIDANCE-MARKER\n    later:\n      premise: you are about to do the later thing\n      content: STUB-COMPANION-PREMISED-BODY\n";

/// A project whose profile `dev` carries one fragment of its own, and a stub
/// companion `acme` registered with `loadout`.
fn world(loadout: &str) -> (Project, Stub, file_system::Rooted) {
    let project = Project::initialized();
    project.bundle("demo");
    project.fragment("demo", "own");
    project.profile("dev", "demo");
    let dir = files::root();
    let stub = Stub::install(dir.root(), "acme", loadout);
    let args = stub.add_args("acme");
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let ran = project.run(&args);
    ran.succeeds();
    (project, stub, dir)
}

fn assembled(project: &Project, env: &[(&str, &str)], flags: &[&str]) -> super::Ran {
    let mut args: Vec<&str> = flags.to_vec();
    args.extend(["context", "assemble", "-p", "dev", "--format", "json"]);
    project.run_with(env, &args)
}

#[test]
fn a_registered_companions_fragment_reaches_context_assemble() {
    let (project, _stub, _dir) = world(LOADOUT);
    let ran = assembled(&project, &[], &[]);
    ran.succeeds();
    let doc = ran.json();
    let context = doc["context"].as_str().unwrap();
    assert!(
        context.contains(OWN) && context.contains(MARKER),
        "{context}"
    );
    assert!(
        context.find(OWN) < context.find(MARKER),
        "after the selected"
    );
    assert!(
        !context.contains(LATER),
        "a premise holds it back: {context}"
    );
    assert!(
        doc["fragments_loaded"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f == "ctxloom+companion:acme#fragments/guidance"),
        "{doc}"
    );
    assert_eq!(ran.stderr, "", "nothing to warn of");
}

#[test]
fn a_registered_companions_fragment_reaches_materialize_and_run() {
    let (project, _stub, _dir) = world(LOADOUT);
    project
        .run(&[
            "materialize",
            "dev",
            "--target",
            "out",
            "--backend",
            "claude-code",
        ])
        .succeeds();
    project.file_contains("out/CLAUDE.md", MARKER);
    project.file_contains("out/CLAUDE.md", OWN);
    project.file_lacks("out/CLAUDE.md", LATER);
    let run = project.run(&["run", "--dry-run", "-p", "dev", "--format", "json"]);
    run.succeeds();
    assert!(run.json()["context"].as_str().unwrap().contains(MARKER));
}

#[test]
fn a_premised_companion_fragment_is_offered_in_the_premise_index_and_delivered_when_named() {
    let (project, _stub, _dir) = world(LOADOUT);
    let premises = project.run(&["fragment", "premises", "--format", "json"]);
    premises.succeeds();
    premises
        .contains("ctxloom+companion:acme#fragments/later")
        .contains("you are about to do the later thing");
    let named = project.run(&[
        "context",
        "assemble",
        "-f",
        "ctxloom+companion:acme#fragments/later",
        "--format",
        "json",
    ]);
    named.succeeds();
    assert!(named.json()["context"].as_str().unwrap().contains(LATER));
}

#[test]
fn no_companions_runs_none_by_flag_or_by_environment() {
    let (project, stub, _dir) = world(LOADOUT);
    // The companion would hang now: were it run, the command would wait out
    // the probe's limit and warn. Neither happens.
    let _ = stub.misbehaving(Misbehaving::Hangs);
    let started = std::time::Instant::now();
    let by_flag = assembled(&project, &[], &["--no-companions"]);
    by_flag.succeeds();
    let by_env = assembled(&project, &[("CTXLOOM_NO_COMPANIONS", "1")], &[]);
    by_env.succeeds();
    for ran in [&by_flag, &by_env] {
        let context = ran.json()["context"].as_str().unwrap().to_string();
        assert!(
            context.contains(OWN) && !context.contains(MARKER),
            "{context}"
        );
        assert_eq!(ran.stderr, "", "nothing ran, nothing to warn of");
    }
    assert!(
        started.elapsed() < std::time::Duration::from_secs(3),
        "a companion was run: {:?}",
        started.elapsed()
    );
    // Without the switch it is run (and hangs, and is said).
    let off = assembled(&project, &[("CTXLOOM_NO_COMPANIONS", "0")], &[]);
    off.succeeds().contains("loadout probe failed");
    assert!(!off.json()["context"].as_str().unwrap().contains(MARKER));
}

#[test]
fn a_companion_that_exits_with_a_status_offers_no_loadout_quietly() {
    let (project, stub, _dir) = world(LOADOUT);
    let _ = stub.misbehaving(Misbehaving::Exits(3));
    let ran = assembled(&project, &[], &[]);
    ran.succeeds();
    assert!(ran.json()["context"].as_str().unwrap().contains(OWN));
    assert!(!ran.json()["context"].as_str().unwrap().contains(MARKER));
    assert_eq!(ran.stderr, "");
}

#[test]
fn a_companion_that_hangs_costs_its_limit_and_a_warning_never_the_command() {
    let (project, stub, _dir) = world(LOADOUT);
    let _ = stub.misbehaving(Misbehaving::Hangs);
    let ran = assembled(&project, &[], &[]);
    ran.succeeds().contains("loadout probe failed");
    assert!(ran.stderr.contains("acme"), "{}", ran.stderr);
    let context = ran.json()["context"].as_str().unwrap().to_string();
    assert!(
        context.contains(OWN) && !context.contains(MARKER),
        "{context}"
    );
}

#[test]
fn a_loadout_that_will_not_parse_is_a_warning_never_the_command() {
    // `companion add` asks only that something be printed; what it prints is
    // judged where it is used.
    for bad in ["not: a loadout\n", "run: [\n", "init:\n  toling: x\n"] {
        let (project, _stub, _dir) = world(bad);
        let ran = assembled(&project, &[], &[]);
        ran.succeeds().contains("unparseable loadout");
        assert!(
            ran.json()["context"].as_str().unwrap().contains(OWN),
            "{bad:?}"
        );
    }
}

#[test]
fn a_companion_whose_program_is_gone_is_a_warning_never_the_command() {
    let (project, _stub, _dir) = world(LOADOUT);
    let home = project.read_home(".ctxloom/config.yaml").unwrap();
    project.write_home(
        ".ctxloom/config.yaml",
        &format!("{home}  gone:\n    command: [/no/such/program]\n"),
    );
    let ran = assembled(&project, &[], &[]);
    ran.succeeds()
        .contains("registered but its binary is not on PATH");
    let context = ran.json()["context"].as_str().unwrap().to_string();
    assert!(context.contains(MARKER), "the others still contribute");
}

#[test]
fn companions_are_not_run_for_a_command_that_assembles_nothing() {
    let (project, stub, _dir) = world(LOADOUT);
    let _ = stub.misbehaving(Misbehaving::Hangs);
    let started = std::time::Instant::now();
    project.run(&["bundle", "list"]).succeeds();
    project.run(&["profile", "list"]).succeeds();
    assert!(started.elapsed() < std::time::Duration::from_secs(3));
}
