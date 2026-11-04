use super::*;

#[test]
fn test_a_changed_line_is_shown_with_its_context() {
    assert_eq!(
        unified_diff("p", "a\nb\nc\n", "a\nB\nc\n"),
        "--- a/p\n+++ b/p\n@@ -1,3 +1,3 @@\n a\n-b\n+B\n c\n"
    );
}

#[test]
fn test_the_same_text_has_no_diff() {
    assert_eq!(unified_diff("p", "a\nb\n", "a\nb\n"), "");
    assert_eq!(unified_diff("p", "", ""), "");
}

// A first pin: the whole file is added.
#[test]
fn test_a_file_that_is_added_is_all_plus_lines() {
    assert_eq!(
        unified_diff("s.sh", "", "#!/bin/sh\necho SCRIPT-ONE\n"),
        "--- a/s.sh\n+++ b/s.sh\n@@ -0,0 +1,2 @@\n+#!/bin/sh\n+echo SCRIPT-ONE\n"
    );
}

#[test]
fn test_a_file_that_is_removed_is_all_minus_lines() {
    assert_eq!(
        unified_diff("s.sh", "x\ny\n", ""),
        "--- a/s.sh\n+++ b/s.sh\n@@ -1,2 +0,0 @@\n-x\n-y\n"
    );
}

#[test]
fn test_context_is_three_lines_and_far_apart_changes_are_two_hunks() {
    let before: String = (1..=20).map(|n| format!("l{n}\n")).collect();
    let after = before.replace("l2\n", "L2\n").replace("l18\n", "L18\n");
    let diff = unified_diff("p", &before, &after);
    assert_eq!(
        diff,
        "--- a/p\n+++ b/p\n@@ -1,5 +1,5 @@\n l1\n-l2\n+L2\n l3\n l4\n l5\n@@ -15,6 +15,6 @@\n l15\n l16\n l17\n-l18\n+L18\n l19\n l20\n"
    );
}

#[test]
fn test_changes_within_context_of_each_other_share_a_hunk() {
    let before: String = (1..=10).map(|n| format!("l{n}\n")).collect();
    let after = before.replace("l3\n", "L3\n").replace("l7\n", "L7\n");
    let diff = unified_diff("p", &before, &after);
    assert_eq!(diff.matches("@@ -").count(), 1, "{diff}");
    assert!(
        diff.contains("-l3\n+L3\n l4\n l5\n l6\n-l7\n+L7\n"),
        "{diff}"
    );
}

#[test]
fn test_an_inserted_line_and_a_missing_final_newline() {
    assert_eq!(
        unified_diff("p", "a\nc\n", "a\nb\nc\n"),
        "--- a/p\n+++ b/p\n@@ -1,2 +1,3 @@\n a\n+b\n c\n"
    );
    assert_eq!(
        unified_diff("p", "a\nb", "a\nb\n"),
        "",
        "a missing final newline is not a change of lines"
    );
}

fn numbered(prefix: &str, count: usize) -> Vec<String> {
    (0..count).map(|n| format!("{prefix}{n}")).collect()
}

fn text(lines: &[String]) -> String {
    lines.iter().map(|l| format!("{l}\n")).collect()
}

// `diff -U3` of the same two texts (headers aside).
#[test]
fn test_changes_after_a_common_start_keep_their_line_numbers() {
    let before = "p1\np2\np3\np4\np5\nkeep1\nold1\nkeep2\nkeep3\nold2\nkeep4\ns1\ns2\ns3\ns4\ns5\n";
    let after = "p1\np2\np3\np4\np5\nkeep1\nnew1\nnew1b\nkeep2\nkeep3\nkeep4\ns1\ns2\ns3\ns4\ns5\n";
    assert_eq!(
        unified_diff("p", before, after),
        "--- a/p\n+++ b/p\n@@ -4,10 +4,10 @@\n p4\n p5\n keep1\n-old1\n+new1\n+new1b\n keep2\n keep3\n-old2\n keep4\n s1\n s2\n"
    );
}

#[test]
fn test_lines_in_common_between_changes_are_kept_as_context() {
    let before = "h1\nh2\nh3\nh4\na\nx\nb\ny\nc\nz\nt1\nt2\nt3\nt4\n";
    let after = "h1\nh2\nh3\nh4\na\nb\nq\nc\nz\nw\nt1\nt2\nt3\nt4\n";
    assert_eq!(
        unified_diff("p", before, after),
        "--- a/p\n+++ b/p\n@@ -3,11 +3,11 @@\n h3\n h4\n a\n-x\n b\n-y\n+q\n c\n z\n+w\n t1\n t2\n t3\n"
    );
}

#[test]
fn test_a_one_line_range_is_written_without_its_length() {
    assert_eq!(
        unified_diff("p", "a\n", "b\n"),
        "--- a/p\n+++ b/p\n@@ -1 +1 @@\n-a\n+b\n"
    );
}

#[test]
fn test_lines_added_or_removed_before_a_common_end_are_numbered_on_each_side() {
    assert_eq!(
        unified_diff("p", "a\nb\nc\nd\ne\n", "a\nb\nX\nY\nc\nd\ne\n"),
        "--- a/p\n+++ b/p\n@@ -1,5 +1,7 @@\n a\n b\n+X\n+Y\n c\n d\n e\n"
    );
    assert_eq!(
        unified_diff("p", "a\nb\nX\nY\nc\nd\ne\n", "a\nb\nc\nd\ne\n"),
        "--- a/p\n+++ b/p\n@@ -1,7 +1,5 @@\n a\n b\n-X\n-Y\n c\n d\n e\n"
    );
}

// Beyond four million line pairs the middle is replaced whole rather than
// worked out; up to it, the common line between is found.
#[test]
fn test_the_work_limit_replaces_the_middle_whole_only_beyond_it() {
    let around = |count: usize| {
        let mut before = numbered("old", count);
        let mut after = numbered("new", count);
        before[count / 2] = "common".to_string();
        after[count / 2] = "common".to_string();
        (text(&before), text(&after))
    };
    let (before, after) = around(2000);
    let found = unified_diff("p", &before, &after);
    assert!(found.contains("\n common\n"), "2000 x 2000 is worked out");
    let (before, after) = around(2001);
    let replaced = unified_diff("p", &before, &after);
    assert!(!replaced.contains("\n common\n"), "2001 x 2001 is replaced");
    assert!(
        replaced.contains("\n-common\n"),
        "the common line is removed"
    );
    assert!(replaced.contains("\n+common\n"), "and added");
}

/// The length of a longest common subsequence, worked out the slow way.
fn lcs_len(a: &[&str], b: &[&str]) -> usize {
    match (a.split_first(), b.split_first()) {
        (Some((x, a_rest)), Some((y, b_rest))) => {
            if x == y {
                1 + lcs_len(a_rest, b_rest)
            } else {
                lcs_len(a_rest, b).max(lcs_len(a, b_rest))
            }
        }
        _ => 0,
    }
}

// An edit script keeps the longest common subsequence and rebuilds both texts.
#[test]
fn test_an_edit_script_keeps_a_longest_common_subsequence_and_rebuilds_both_texts() {
    let alphabet = ["a", "b", "c", "d"];
    let mut seed = 12345_u64;
    let mut next = move |bound: u64| {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) % bound
    };
    for _ in 0..300 {
        let (n, m) = (next(9) as usize, next(9) as usize);
        let a: Vec<&str> = (0..n).map(|_| alphabet[next(4) as usize]).collect();
        let b: Vec<&str> = (0..m).map(|_| alphabet[next(4) as usize]).collect();
        let ops = edit_script(&a, &b);
        let old: Vec<&str> = ops
            .iter()
            .filter_map(|op| match op {
                Op::Equal(l) | Op::Del(l) => Some(*l),
                Op::Add(_) => None,
            })
            .collect();
        let new: Vec<&str> = ops
            .iter()
            .filter_map(|op| match op {
                Op::Equal(l) | Op::Add(l) => Some(*l),
                Op::Del(_) => None,
            })
            .collect();
        assert_eq!((old, new), (a.clone(), b.clone()));
        let kept = ops.iter().filter(|op| matches!(op, Op::Equal(_))).count();
        assert_eq!(kept, lcs_len(&a, &b), "{a:?} -> {b:?}");
    }
}
