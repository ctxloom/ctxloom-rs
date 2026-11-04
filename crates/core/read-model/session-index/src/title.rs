//! A session's title: the first thing its user asked, read from its
//! canonical transcript (`transcripts/transcript.jsonl`, docs/transcript.schema
//! .json in Go; D27).

/// The longest a title is, in bytes (Go's `FirstLineSummary`).
pub const TITLE_BYTES: usize = 80;

/// `s` trimmed to its first line and capped at [`TITLE_BYTES`] bytes, never
/// splitting a character: the one-line form a session's title takes.
pub fn first_line_summary(s: &str) -> String {
    let s = s.trim();
    let first = s.split('\n').next().unwrap_or_default().trim();
    truncate_bytes(first, TITLE_BYTES).to_string()
}

/// `s` cut to at most `max` bytes, backed off to the nearest character
/// boundary (Go's `textutil.TruncateBytes`).
fn truncate_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// The title of the session whose canonical transcript is `transcript`: its
/// first main-thread user prompt that says anything, cut to one line
/// ([`first_line_summary`]). `None` when the transcript holds no such prompt,
/// or cannot be read (a line that is not a record, or a record of a version
/// this reader does not know): a renderer chooses its own placeholder.
pub fn title_of(transcript: &str) -> Option<String> {
    let mut title = None;
    for line in transcript.lines().filter(|l| !l.trim().is_empty()) {
        let record: Record = serde_json::from_str(line).ok()?;
        if record.v != 1 {
            return None;
        }
        if title.is_some() || record.kind != "entry" {
            continue;
        }
        let Some(entry) = record.entry else { continue };
        if entry.kind == "user" && !entry.sidechain && !entry.content.trim().is_empty() {
            title = Some(first_line_summary(&entry.content));
        }
    }
    title
}

/// One line of the canonical transcript, as far as a title reads it.
#[derive(serde::Deserialize)]
struct Record {
    v: u32,
    kind: String,
    entry: Option<Entry>,
}

#[derive(serde::Deserialize)]
struct Entry {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    sidechain: bool,
}

#[cfg(test)]
#[path = "title.test.rs"]
mod tests;
