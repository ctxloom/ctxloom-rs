//! The little the ports hold besides traits.

use crate::GoOp;

#[test]
fn test_a_go_op_is_spelled_as_the_log_spells_it() {
    let ops = [
        GoOp::Add,
        GoOp::Status,
        GoOp::Text,
        GoOp::Remove,
        GoOp::Tag,
        GoOp::Untag,
    ];
    let spelled: Vec<&str> = ops.iter().map(|op| op.as_str()).collect();
    assert_eq!(spelled, ["add", "status", "text", "remove", "tag", "untag"]);
}
