//! Acceptance: the companion contract, as ctxloom reads it from the real
//! binary. `version --format json` is fed to a port of the parsing Go
//! ctxloom does at boot, and `loadout --format yaml` is Go's document with
//! the reviewed changes applied.

use std::process::Command;

use serde::Deserialize;

use super::{binary, Ran};

/// Go ctxloom's `cliversion.Info` (`internal/shared/cliversion`): what its
/// boot probe and its lock reaper decode from `<bin> version --format
/// json`. Like Go's `json.Unmarshal` into it, a field left out decodes as
/// zero and one it does not know is ignored.
#[derive(Deserialize, Default, Debug)]
#[serde(default)]
struct Info {
    name: String,
    version: String,
    lock_protocol: i64,
}

/// Go's `safefs.LockProtocol`: the protocol a running binary must report,
/// at least, before ctxloom removes a lock file it may take.
const GO_LOCK_PROTOCOL: i64 = 2;

/// Go's `cliversion.Parse` over `cliversion.ParseInfo`: the whole output
/// decoded, and a missing version an error, never an empty version.
fn parse(raw: &[u8]) -> Result<Info, String> {
    let info: Info = serde_json::from_slice(raw)
        .map_err(|e| format!("parse version --format json output: {e}"))?;
    if info.version.is_empty() {
        return Err("version --format json output has no version field".to_string());
    }
    Ok(info)
}

/// Go's lock reaper's verdict on a running binary
/// (`internal/adapters/operations/lockreap.go`): none when it reports a
/// protocol at least Go's, else why it predates the re-check.
fn predates_the_recheck(info: &Info) -> Option<String> {
    match info.lock_protocol {
        0 => Some("reports no lock_protocol, so it predates the re-check".to_string()),
        n if n < GO_LOCK_PROTOCOL => Some(format!(
            "reports lock_protocol {n}, below {GO_LOCK_PROTOCOL}"
        )),
        _ => None,
    }
}

/// Go's probe, `cliversion.Output`: `<bin> version --format json`, its
/// stdout, failing on a non-zero exit. Run with nothing but a temporary
/// HOME and PATH.
fn probe(home: &std::path::Path) -> Ran {
    let output = Command::new(binary())
        .args(["version", "--format", "json"])
        .current_dir(home)
        .env_clear()
        .env("HOME", home)
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("taskloom runs");
    Ran::of(output)
}

#[test]
fn test_the_version_probe_parses_as_go_ctxloom_parses_it() {
    let home = tempfile::tempdir().unwrap();
    let ran = probe(home.path());
    assert!(ran.ok, "the probe failed: {}", ran.stderr);
    let info = parse(ran.stdout.as_bytes()).unwrap_or_else(|e| panic!("{e}: {:?}", ran.stdout));
    assert_eq!(info.name, "taskloom");
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(predates_the_recheck(&info), None, "{info:?}");
    assert_eq!(info.lock_protocol, GO_LOCK_PROTOCOL);
    assert_eq!(
        std::fs::read_dir(home.path()).unwrap().count(),
        0,
        "the probe writes nothing"
    );
}

#[test]
fn test_the_ported_parse_refuses_what_go_refuses() {
    assert!(parse(b"not json").is_err());
    assert!(parse(br#"{"name":"taskloom"}"#).is_err());
    let old = parse(br#"{"name":"taskloom","version":"v1","extra":1}"#).unwrap();
    assert!(predates_the_recheck(&old).is_some());
    let older = parse(br#"{"name":"taskloom","version":"v1","lock_protocol":1}"#).unwrap();
    assert!(predates_the_recheck(&older).is_some());
}

/// Go taskloom's loadout document, as `scripts/go-companion-fixtures.sh`
/// copied it from the Go source (`cmd/taskloom/loadout.yaml`), with the
/// reviewed factual changes applied.
#[path = "../../../../adapters/driving/tasks-cli/fixtures/reviewed.rs"]
mod reviewed;

#[test]
fn test_the_loadout_probe_prints_gos_document_reviewed() {
    let home = tempfile::tempdir().unwrap();
    let output = Command::new(binary())
        .args(["loadout", "--format", "yaml"])
        .current_dir(home.path())
        .env_clear()
        .env("HOME", home.path())
        .output()
        .expect("taskloom runs");
    let ran = Ran::of(output);
    assert!(ran.ok, "{}", ran.stderr);
    assert!(
        ran.stdout == reviewed::loadout(),
        "the loadout differs from Go's, reviewed"
    );
}

// Nothing served to a model carries a placeholder the human has yet to fill.
// A JSON text is read as its strings, so an escaped placeholder is seen too.
#[test]
#[ignore = "the loadout's two install placeholders (run.installation, mcp.taskloom.installation) await the human's install text; fill them, then un-ignore this"]
fn test_no_served_text_carries_a_placeholder() {
    let json = |text: &str| {
        serde_json::from_str::<serde_json::Value>(text)
            .unwrap()
            .to_string()
    };
    for (what, text) in [
        ("the MCP instructions", tasks_mcp::INSTRUCTIONS.to_string()),
        ("the tools", json(tasks_mcp::TOOLS)),
        ("the resources", json(tasks_mcp::RESOURCES)),
        ("the loadout", tasks_cli::LOADOUT.to_string()),
    ] {
        assert!(
            !text.contains("\u{ab}HUMAN:"),
            "a placeholder is served in {what}"
        );
    }
}
