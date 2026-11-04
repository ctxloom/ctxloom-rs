use super::*;

#[test]
fn test_the_loadout_is_the_structure_and_none_of_the_owners_text() {
    assert_eq!(
        DOCUMENT,
        "run:\n  version: 1.0.0\n  tags:\n    - core\n    - mcp\n    - isolation\n  author: ctxloom\n  mcp:\n    ctxloom:\n      served_by: session-endpoint\n"
    );
}

#[test]
fn test_the_loadout_run_bundle_is_a_bundle() {
    let doc = angzarr_yaml::parse(DOCUMENT).unwrap();
    let run = doc.get("run").expect("run:");
    let text = angzarr_yaml::write(run, usize::MAX);
    let bundle = crate::document::parse_document(&text).unwrap();
    assert_eq!(bundle.version, "1.0.0");
    assert_eq!(bundle.author, "ctxloom");
    assert_eq!(bundle.mcp["ctxloom"].served_by, "session-endpoint");
    assert!(bundle.mcp["ctxloom"].command.is_empty());
    assert!(bundle.fragments.is_empty());
}
