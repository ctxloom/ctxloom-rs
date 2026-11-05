use super::*;

#[test]
fn test_a_bundle_slot_is_its_repository_s_place_then_the_bundle_s_path() {
    let remote = RemoteRef::parse("ctxloom+git://example.test/o/r//bundles/a/../b/./c")
        .expect("a remote reference");
    let slot = slot(&remote).expect("a slot");
    assert_eq!(slot.identity, remote.identity);
    assert_eq!(slot.repository.segments(), ["example.test", "o", "r"]);
    assert_eq!(slot.bundle.segments(), ["b", "c"]);
}

#[test]
fn test_a_slot_of_an_identity_that_is_no_remote_bundle_is_refused() {
    for identity in ["demo", "ctxloom+local:demo", ""] {
        let why = slot_of(identity).expect_err(identity);
        assert_eq!(why, format!("{identity} is not a remote bundle reference"));
    }
    assert!(slot_of("ctxloom+git://example.test/o/r//bundles/demo").is_ok());
}

#[test]
fn test_a_bundle_path_that_climbs_names_no_slot() {
    assert!(slot_of("ctxloom+git://example.test/o/r//bundles/../../x").is_err());
}
