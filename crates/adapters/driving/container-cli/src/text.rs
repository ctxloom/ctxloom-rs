//! The text renderings.

use std::io::{self, Write};

use isolation_application::{CheckReport, ToolingReport};

/// Go's `renderContainerCheck`.
pub(crate) fn check(report: &CheckReport, out: &mut dyn Write) -> io::Result<()> {
    let _ = (report, out);
    todo!("S11 green")
}

/// Go's `renderTooling`.
pub(crate) fn tooling(report: &ToolingReport, out: &mut dyn Write) -> io::Result<()> {
    let _ = (report, out);
    todo!("S11 green")
}
