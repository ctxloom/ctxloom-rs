//! A session as a listing shows it: its record joined with the derived facts
//! every renderer used to compute for itself: its title, its state now, and
//! what its summary says. Go: `operations.SessionView` and `cli.SessionRow`
//! (release/0.7).

use angzarr_embedded::Timestamp;

use crate::{stamp, SessionRecord};

/// What a session is doing now, from its liveness lock (never from its end
/// stamp, which says when the LAST run ended and so lies about now in both
/// directions): held is live, free is ended, anything the lock cannot prove
/// is unknown, never rounded to a guess.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Live,
    Ended,
    Unknown,
}

/// Every state, in the order `--state` lists them.
pub const STATES: [&str; 3] = ["live", "ended", "unknown"];

impl State {
    /// The state's word.
    pub fn as_str(self) -> &'static str {
        match self {
            State::Live => STATES[0],
            State::Ended => STATES[1],
            State::Unknown => STATES[2],
        }
    }

    /// The state a `--state` word names; `None` for any other word.
    pub fn parse(word: &str) -> Option<State> {
        [State::Live, State::Ended, State::Unknown]
            .into_iter()
            .find(|state| state.as_str() == word)
    }
}

/// One session as listed.
#[derive(Clone, PartialEq, Debug)]
pub struct View {
    pub record: SessionRecord,
    pub state: State,
    /// The first thing its user asked ([`crate::title_of`]); `None` when its
    /// transcript holds none.
    pub title: Option<String>,
    /// The canonical transcript on disk, when one was captured.
    pub captured: Option<Captured>,
}

/// A captured canonical transcript.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Captured {
    pub path: String,
    pub bytes: u64,
}

/// The placeholder a session with no title shows.
pub const NO_SUMMARY: &str = "(no summary)";

impl View {
    /// The row's summary: the title, or [`NO_SUMMARY`], with a badge for a
    /// purged session and for a kept one, so each rides the row in every
    /// format and a purged session never reads as one never purged.
    pub fn summary(&self) -> String {
        let mut summary = self.title.clone().unwrap_or_else(|| NO_SUMMARY.to_string());
        if self.record.purged_at.is_some() {
            summary.push_str("  \u{1f5d1} purged (transcript destroyed)");
        }
        if self.record.kept {
            summary.push_str("  \u{1f4cc} kept (transcript kept at any age)");
        }
        summary
    }

    /// When the session's last run ended, blank while it is live: a resumed
    /// session keeps its previous run's stamp, which is no end of the run in
    /// progress.
    pub fn end(&self) -> Option<Timestamp> {
        match self.state {
            State::Live => None,
            State::Ended | State::Unknown => self.record.ended_at,
        }
    }

    /// The lowercase text a query matches its words against: the harp, the
    /// title, and the start and end as the text row shows them.
    pub fn haystack(&self) -> String {
        let mut text = format!(
            "{} {} {}",
            self.record.harp,
            self.title.as_deref().unwrap_or_default(),
            stamp::text(self.record.started_at)
        );
        if let Some(end) = self.end() {
            text.push(' ');
            text.push_str(&stamp::text(end));
        }
        text.to_lowercase()
    }

    /// Whether every word is found, case-insensitively, in the haystack: an
    /// AND across words, vacuously true for none.
    pub fn matches(&self, words: &[String]) -> bool {
        let haystack = self.haystack();
        words
            .iter()
            .all(|word| haystack.contains(&word.to_lowercase()))
    }
}

/// `views` in `state`, in order; all of them for `None`.
pub fn only_state(views: Vec<View>, state: Option<State>) -> Vec<View> {
    match state {
        None => views,
        Some(state) => views.into_iter().filter(|v| v.state == state).collect(),
    }
}

#[cfg(test)]
#[path = "view.test.rs"]
mod tests;
