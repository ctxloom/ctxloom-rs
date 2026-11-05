//! The text renderings.

use std::io::{self, Write};

use isolation_application::{CheckReport, ToolingReport};

/// Go's `renderContainerCheck`.
pub(crate) fn check(report: &CheckReport, out: &mut dyn Write) -> io::Result<()> {
    writeln!(out, "Container capability (backend: {})", report.backend)?;
    if report.in_container {
        writeln!(
            out,
            "  in a container:  yes ({})",
            report.markers.join(", ")
        )?;
    } else {
        writeln!(out, "  in a container:  no")?;
    }
    if report.reachable {
        writeln!(out, "  runtime:         {} (reachable)", report.runtime)?;
    } else {
        writeln!(out, "  runtime:         {}", report.runtime)?;
    }
    if !report.image.is_empty() {
        let presence = if report.image_present {
            "present"
        } else {
            "absent"
        };
        writeln!(out, "  agent image:     {} ({presence})", report.image)?;
    }
    writeln!(out, "  shared fs:       {}", report.shared_fs)?;
    for line in &report.guidance {
        writeln!(out, "  -> {line}")?;
    }
    Ok(())
}

/// Go's `renderTooling`.
pub(crate) fn tooling(report: &ToolingReport, out: &mut dyn Write) -> io::Result<()> {
    if report.declarations.is_empty() {
        writeln!(
            out,
            "No registered companion declares container tooling (a companion declares it as `tooling` in its loadout's init section)."
        )?;
        return writeln!(
            out,
            "An unregistered companion is never run, so it declares nothing - register one with `ctxloom companion add`."
        );
    }
    writeln!(out, "{}", report.instructions)?;
    for declaration in &report.declarations {
        write!(
            out,
            "\n### {}\n\n{}\n",
            declaration.source, declaration.content
        )?;
    }
    Ok(())
}
