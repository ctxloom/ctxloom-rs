//! The text renderings.

use std::io::{self, Write};

use doctor_checks::{Report, Status};
use reclaim_application::CleanReport;

/// A size as a person reads it.
pub(crate) fn human(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["KB", "MB", "GB", "TB", "PB", "EB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let (mut value, mut unit) = (bytes as f64 / 1024.0, 0);
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// Go's cache plan: what would go (or went), and what rebuilds each.
pub(crate) fn clean(report: &CleanReport, out: &mut dyn Write) -> io::Result<()> {
    if report.targets.iter().all(|t| !t.present) {
        return writeln!(
            out,
            "Nothing to clean: this project's cache is already absent."
        );
    }
    let verb = if report.applied {
        "removed"
    } else {
        "would remove"
    };
    writeln!(
        out,
        "ctxloom {verb} {} of regenerable cache:\n",
        human(report.bytes)
    )?;
    for t in report.targets.iter().filter(|t| t.present) {
        writeln!(
            out,
            "  {:<42} {:>10}   rebuild: {}",
            t.rel,
            human(t.bytes),
            t.rebuild
        )?;
    }
    writeln!(out)
}

fn word(status: Status) -> &'static str {
    match status {
        Status::Ok => "ok",
        Status::Warn => "warn",
        Status::Fail => "fail",
        Status::Info => "info",
    }
}

/// The text report: the warnings and failures (every check with `all`), then
/// one line counting them and naming the first fix.
pub(crate) fn doctor(report: &Report, all: bool, out: &mut dyn Write) -> io::Result<()> {
    writeln!(out, "ctxloom doctor")?;
    let (mut warnings, mut failures, mut first_fix) = (0, 0, String::new());
    for check in &report.checks {
        match check.status {
            Status::Fail => failures += 1,
            Status::Warn => warnings += 1,
            _ if all => {}
            _ => continue,
        }
        if first_fix.is_empty() && matches!(check.status, Status::Fail | Status::Warn) {
            first_fix = check.remedy.clone();
        }
        writeln!(
            out,
            "  [{}] {} {}: {}",
            word(check.status),
            check.marker,
            check.name,
            check.detail
        )?;
        if !check.remedy.is_empty() && matches!(check.status, Status::Fail | Status::Warn) {
            writeln!(out, "      fix: {}", check.remedy)?;
        }
    }
    let fix = if first_fix.is_empty() {
        "see each row above for its fix".to_string()
    } else {
        format!("first fix: {first_fix}")
    };
    match (failures, warnings) {
        (0, 0) => writeln!(
            out,
            "No warnings. `ctxloom doctor --all` lists every check."
        ),
        (0, w) => writeln!(out, "{w} warning(s); {fix}"),
        (f, w) => writeln!(out, "{f} failure(s), {w} warning(s); {fix}"),
    }
}
