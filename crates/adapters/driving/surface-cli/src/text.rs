//! The setup commands' answers as words (Go's `manage.go` renderers).

use std::io::{self, Write};

use setup_application::{
    CheckReport, GitignoreReport, HooksList, InstallPlan, InstallReport, MaterializeDiff,
    MaterializeResult, UninstallReport,
};

pub(crate) fn install(w: &mut dyn Write, report: &InstallReport) -> io::Result<()> {
    if report.initialized {
        writeln!(w, "Initialized ctxloom directory: {}", report.app_dir)?;
    }
    writeln!(
        w,
        "{}: {}/.gitignore",
        report.gitignore_status, report.app_dir
    )?;
    match report.pull.status.as_str() {
        "skipped" => writeln!(
            w,
            "Dependencies were not pulled (--no-pull); run 'ctxloom deps pull' to install them."
        )?,
        "pulled" if report.pull.installed > 0 => {
            writeln!(w, "Pulled {} dependencies", report.pull.installed)?;
        }
        "failed" => writeln!(
            w,
            "The dependency pull failed ({}); 'ctxloom deps pull' retries.",
            report.pull.error
        )?,
        _ => {}
    }
    writeln!(
        w,
        "No engine file was written: a `ctxloom run` session delivers ctxloom's hooks and MCP server into its own session home. An engine launched directly here gets neither."
    )
}

pub(crate) fn plan(w: &mut dyn Write, plan: &InstallPlan) -> io::Result<()> {
    writeln!(w, "manage install would:")?;
    if plan.ctxloom_dir_exists {
        writeln!(w, "  - reuse existing .ctxloom: {}", plan.ctxloom_dir)?;
    } else {
        writeln!(
            w,
            "  - scaffold .ctxloom: {} (engine: {})",
            plan.ctxloom_dir, plan.engine
        )?;
    }
    writeln!(w, "  - update .gitignore: {}", plan.gitignore)?;
    writeln!(w, "  - write no engine file: hooks, MCP and statusline reach an engine through a `ctxloom run` session's own home")
}

pub(crate) fn uninstall(w: &mut dyn Write, report: &UninstallReport) -> io::Result<()> {
    writeln!(w, "Removed ctxloom harness from: {:?}", report.backends)?;
    writeln!(w, "{}", report.note)
}

pub(crate) fn gitignore(w: &mut dyn Write, report: &GitignoreReport) -> io::Result<()> {
    if report.nested_written {
        writeln!(
            w,
            "Wrote {} (ctxloom's private-state rules; commit it)",
            report.nested_path
        )
    } else {
        writeln!(
            w,
            "No change: {} already matches ctxloom's private-state rules",
            report.nested_path
        )
    }
}

pub(crate) fn check(w: &mut dyn Write, report: &CheckReport) -> io::Result<()> {
    writeln!(w, "Project: {}", report.work_dir)?;
    writeln!(w, "Statusline (HUD): {}\n", report.manage_statusline)?;
    for b in &report.backends {
        if !b.settings_exists {
            writeln!(w, "  {}: not configured", b.backend)?;
            continue;
        }
        writeln!(
            w,
            "  {}: hooks={} statusline={} mcp={}",
            b.backend, b.hooks_present, b.status_line, b.mcp_present
        )?;
    }
    if !report.surfaces.is_empty() {
        writeln!(w, "\nMaterialized surfaces:")?;
        for s in &report.surfaces {
            writeln!(w, "  {} ({}): {}", s.route, s.backend, s.status)?;
        }
    }
    writeln!(w)
}

pub(crate) fn hooks(w: &mut dyn Write, list: &HooksList) -> io::Result<()> {
    for event in &list.events {
        writeln!(w, "{}", event.event)?;
        if event.hooks.is_empty() {
            writeln!(w, "  (none)")?;
            continue;
        }
        for h in &event.hooks {
            let label = if !h.command.is_empty() {
                std::iter::once(h.command.as_str())
                    .chain(h.args.iter().map(String::as_str))
                    .collect::<Vec<_>>()
                    .join(" ")
            } else if !h.prompt.is_empty() {
                h.prompt.clone()
            } else {
                "(hook)".to_string()
            };
            let origin = if h.source.is_empty() {
                h.source_kind.clone()
            } else {
                format!("{} {}", h.source_kind, h.source)
            };
            writeln!(w, "  {}. {:<24} [{}]", h.position, label, origin)?;
        }
    }
    Ok(())
}

pub(crate) fn materialize(w: &mut dyn Write, result: &MaterializeResult) -> io::Result<()> {
    writeln!(w, "Target: {}", result.target)?;
    for warning in &result.warnings {
        writeln!(w, "{warning}")?;
    }
    for engine in &result.engines {
        if !engine.wrote.is_empty() {
            writeln!(w, "{}: wrote {}", engine.engine, engine.wrote.join(", "))?;
        }
        if !engine.released.is_empty() {
            writeln!(
                w,
                "{}: released {}",
                engine.engine,
                engine.released.join(", ")
            )?;
        }
        for lost in &engine.not_carried {
            writeln!(
                w,
                "{}: NOT carried: {} ({})",
                engine.engine, lost.surface, lost.reason
            )?;
        }
        for skipped in &engine.skipped {
            writeln!(w, "{}: {skipped}", engine.engine)?;
        }
    }
    for error in &result.errors {
        writeln!(w, "error: {error}")?;
    }
    writeln!(w, "Status: {}", result.status)
}

/// `materialize --diff`: that the two match, or the diff naming what is on one
/// side and not the other.
pub(crate) fn materialize_diff(w: &mut dyn Write, found: &MaterializeDiff) -> io::Result<()> {
    if found.identical {
        return writeln!(
            w,
            "{}'s materialized context matches {}: no difference.",
            found.label, found.compared_to
        );
    }
    writeln!(
        w,
        "Comparing {}'s materialized context with {} \u{2014} content present on one side and absent on the other:",
        found.label, found.compared_to
    )?;
    write!(w, "{}", found.diff)
}

#[cfg(test)]
#[path = "text.test.rs"]
mod tests;
