//! A unified diff of two texts, line by line (what Go's `difflib` prints for
//! `materialize --diff`): a `---`/`+++` header, then hunks with three lines of
//! context. The lines are compared by value; the edit script is a longest
//! common subsequence, so a line moved shows as removed and added.

/// Lines of unchanged text around each change.
const CONTEXT: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tag {
    Equal,
    Delete,
    Insert,
}

/// The unified diff turning `from_text` into `to_text`, named `from_name` and
/// `to_name` in its header; empty when the two are the same text.
pub(crate) fn unified(from_text: &str, to_text: &str, from_name: &str, to_name: &str) -> String {
    let from: Vec<&str> = from_text.lines().collect();
    let to: Vec<&str> = to_text.lines().collect();
    let ops = edit_script(&from, &to);
    let groups = hunks(&ops);
    if groups.is_empty() {
        return String::new();
    }
    let mut out = format!("--- {from_name}\n+++ {to_name}\n");
    // The lines each op has consumed before it, on each side.
    let mut seen = vec![(0usize, 0usize)];
    for (tag, _) in &ops {
        let (a, b) = *seen.last().unwrap_or(&(0, 0));
        seen.push(match tag {
            Tag::Equal => (a + 1, b + 1),
            Tag::Delete => (a + 1, b),
            Tag::Insert => (a, b + 1),
        });
    }
    for (lo, hi) in groups {
        let (a0, b0) = seen[lo];
        let (a1, b1) = seen[hi];
        out.push_str(&format!(
            "@@ -{} +{} @@\n",
            range(a0, a1 - a0),
            range(b0, b1 - b0)
        ));
        for (tag, line) in &ops[lo..hi] {
            let mark = match tag {
                Tag::Equal => ' ',
                Tag::Delete => '-',
                Tag::Insert => '+',
            };
            out.push(mark);
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// A hunk side's range as unified diffs spell it: the 1-based first line and
/// the length, with the length left out when it is one and the line before
/// when it is none.
fn range(start: usize, len: usize) -> String {
    match len {
        1 => (start + 1).to_string(),
        0 => format!("{start},0"),
        _ => format!("{},{len}", start + 1),
    }
}

/// The lines of `from` and `to` in order, each kept, dropped or added.
fn edit_script<'a>(from: &[&'a str], to: &[&'a str]) -> Vec<(Tag, &'a str)> {
    // table[i][j]: the common subsequence length of from[i..] and to[j..].
    let mut table = vec![vec![0usize; to.len() + 1]; from.len() + 1];
    for i in (0..from.len()).rev() {
        for j in (0..to.len()).rev() {
            table[i][j] = if from[i] == to[j] {
                table[i + 1][j + 1] + 1
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut ops = Vec::new();
    while i < from.len() || j < to.len() {
        if i < from.len() && j < to.len() && from[i] == to[j] {
            ops.push((Tag::Equal, from[i]));
            i += 1;
            j += 1;
        } else if i < from.len() && (j == to.len() || table[i + 1][j] == table[i][j]) {
            ops.push((Tag::Delete, from[i]));
            i += 1;
        } else {
            ops.push((Tag::Insert, to[j]));
            j += 1;
        }
    }
    ops
}

/// The op ranges to print: each change with its context, runs of changes
/// closer than two contexts apart sharing one hunk.
fn hunks(ops: &[(Tag, &str)]) -> Vec<(usize, usize)> {
    let mut groups: Vec<(usize, usize)> = Vec::new();
    for (at, (tag, _)) in ops.iter().enumerate() {
        if *tag == Tag::Equal {
            continue;
        }
        let lo = at.saturating_sub(CONTEXT);
        let hi = (at + 1 + CONTEXT).min(ops.len());
        match groups.last_mut() {
            Some(last) if lo <= last.1 => last.1 = hi,
            _ => groups.push((lo, hi)),
        }
    }
    groups
}

#[cfg(test)]
#[path = "diff.test.rs"]
mod tests;
