//! The project refresh a pull ends with: the project's hooks, MCP servers,
//! settings, commands and skills re-delivered into its engine files.
//!
//! That directory is shared by every session and person using the checkout,
//! so the refresh asks the consent `ctxloom materialize` asks before writing
//! there: `--yes`. Without it the pull still succeeds, writes no engine file,
//! and names what the refresh would have changed. The delivery itself is a
//! port ([`ProjectRefresh`]), so that this crate does not depend on the
//! surfaces it is delivered through: the program's composition wires one.

use angzarr_diagnostic::Diagnostic;

use crate::views::PullReport;
use crate::{AppError, Deps, Notice, Outcome};

/// The delivery of the project's surfaces into its engine files.
pub trait ProjectRefresh {
    /// Deliver them (`write`), or preview the delivery against the project as
    /// it stands and write nothing. A preview names only what would change.
    fn refresh(&self, write: bool) -> Result<Refresh, Box<dyn Diagnostic>>;
}

/// What a refresh did, or (a preview) would do, per engine.
#[derive(Debug, Default)]
pub struct Refresh {
    /// The directory it delivers into.
    pub target: String,
    pub engines: Vec<RefreshEngine>,
}

/// One engine's part of a refresh.
#[derive(Debug, Default)]
pub struct RefreshEngine {
    pub engine: String,
    /// The kinds whose files it writes.
    pub wrote: Vec<String>,
    /// The kinds it releases.
    pub released: Vec<String>,
    /// The kinds it would be refused.
    pub refused: Vec<RefusedKind>,
}

/// A kind of surface the refresh would be refused, and why.
#[derive(Debug)]
pub struct RefusedKind {
    pub kind: String,
    pub cause: Box<dyn Diagnostic>,
}

impl Refresh {
    /// Whether it would change a kind's files for any engine, or be refused.
    fn plans_anything(&self) -> bool {
        self.engines
            .iter()
            .any(|e| !e.wrote.is_empty() || !e.released.is_empty() || !e.refused.is_empty())
    }
}

/// A pull and the refresh that followed it.
#[derive(Debug)]
pub struct Pulled {
    pub report: PullReport,
    /// The refresh a pull without consent previewed and did not write; none
    /// when the project is already current.
    pub skipped: Option<Refresh>,
    /// A consented refresh that was refused or failed.
    pub failure: Option<AppError>,
}

impl Deps {
    /// `deps pull`, then the project refresh: written with `yes`, previewed
    /// without it. The refresh follows a pull that had remote references to
    /// install; one that failed to refresh does not fail the pull.
    pub fn pull_and_refresh(
        &self,
        refresher: &dyn ProjectRefresh,
        force: bool,
        lock: bool,
        yes: bool,
    ) -> Outcome<Pulled> {
        let Outcome {
            result,
            mut notices,
        } = self.pull(force, lock);
        let report = match result {
            Ok(report) => report,
            Err(e) => {
                return Outcome {
                    result: Err(e),
                    notices,
                }
            }
        };
        let (skipped, failure) = if report.total == 0 {
            (None, None)
        } else {
            match refresher.refresh(yes) {
                Ok(_) if yes => (None, None),
                Ok(preview) => (Some(preview).filter(Refresh::plans_anything), None),
                Err(cause) if yes => (
                    None,
                    Some(AppError::RefreshAborted {
                        detail: cause.to_string(),
                    }),
                ),
                Err(cause) => {
                    notices.push(Notice::text(format!(
                        "could not preview the project refresh: {cause}"
                    )));
                    (None, None)
                }
            }
        };
        Outcome {
            result: Ok(Pulled {
                report,
                skipped,
                failure,
            }),
            notices,
        }
    }
}

#[cfg(test)]
#[path = "refresh.test.rs"]
mod tests;
