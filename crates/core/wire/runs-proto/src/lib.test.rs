//! The wire types carry the names the kernel routes by, and the proto holds
//! the plan's one hard lint: nothing on any message is a cross-stream
//! position (S5, plan 6: "no watch carries or needs a position across
//! streams").

use super::v1;
use prost::Name;

// NEW-DESIGN: the type URLs are what the fact allowlist matches on.
#[test]
fn the_wire_messages_are_named_by_their_full_names() {
    assert_eq!(v1::RunEnqueued::type_url(), "/ctxloom.runs.v1.RunEnqueued");
    assert_eq!(v1::TurnStarted::type_url(), "/ctxloom.runs.v1.TurnStarted");
    assert_eq!(v1::FoundTree::type_url(), "/ctxloom.runs.v1.FoundTree");
}

// NEW-DESIGN (plan S5 red test "nothing on any message is a cross-stream
// position (a proto lint over runs-proto)"): no field of any message in the
// package is named like a store-wide or cross-stream position. The lint reads
// the proto source, so a new message is held to it at once.
#[test]
fn no_message_carries_a_cross_stream_position() {
    const SOURCES: [&str; 2] = [
        include_str!("../../../../../proto/ctxloom/runs/v1/runs.proto"),
        include_str!("../../../../../proto/ctxloom/runs/v1/trees.proto"),
    ];
    const BANNED: [&str; 5] = [
        "global_sequence",
        "commit_position",
        "store_position",
        "offset",
        "cursor",
    ];
    for source in SOURCES {
        for line in source.lines() {
            let code = line.split("//").next().unwrap_or_default();
            for banned in BANNED {
                assert!(!code.contains(banned), "`{banned}` in `{}`", line.trim());
            }
        }
    }
}
