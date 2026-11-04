//! The project refresh a pull previewed and did not write for want of
//! `--yes`: said in words, or carried in the pull's json as
//! `refresh_skipped`.

use std::io::{self, Write};

use angzarr_presentation::Catalog;
use deps_application::{PullReport, Refresh};
use serde::Serialize;

use cli_support::FixOf;

/// A pull as json: its report, and the refresh it skipped, when it did.
#[derive(Serialize)]
pub(crate) struct Pulled<'a> {
    #[serde(flatten)]
    pub report: &'a PullReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_skipped: Option<Skipped>,
}

/// The refresh a pull skipped.
#[derive(Serialize)]
pub(crate) struct Skipped {
    target: String,
    engines: Vec<Engine>,
}

#[derive(Serialize)]
struct Engine {
    engine: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    wrote: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    released: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    refused: Vec<Refused>,
}

#[derive(Serialize)]
struct Refused {
    kind: String,
    reason: String,
    fix: String,
}

/// `refresh`, its refusals said from the catalog.
pub(crate) fn view(refresh: &Refresh, catalog: &Catalog) -> Skipped {
    Skipped {
        target: refresh.target.clone(),
        engines: refresh
            .engines
            .iter()
            .map(|e| Engine {
                engine: e.engine.clone(),
                wrote: e.wrote.clone(),
                released: e.released.clone(),
                refused: e
                    .refused
                    .iter()
                    .map(|r| Refused {
                        kind: r.kind.clone(),
                        reason: catalog.message(r.cause.as_ref()),
                        fix: catalog
                            .render(&FixOf(r.cause.as_ref(), format!("{}-fix", r.cause.code())))
                            .unwrap_or_default(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// The note: the directory nothing was written into, per engine the kinds it
/// would have written or released, each kind it would have been refused with
/// the reason and its fix, and the two ways to apply it: `rerun`, the
/// command's own `--yes` form, and the `ctxloom materialize` limited by
/// `--surface` to the kinds the refresh touches. With only refusals there is
/// nothing to apply, so no apply line.
pub(crate) fn text(out: &mut dyn Write, skipped: &Skipped, rerun: &str) -> io::Result<()> {
    writeln!(
        out,
        "  Project refresh skipped: without --yes nothing was written into {}.",
        skipped.target
    )?;
    let mut kinds: Vec<&str> = Vec::new();
    for engine in &skipped.engines {
        let mut parts = Vec::new();
        if !engine.wrote.is_empty() {
            parts.push(format!("would write {}", engine.wrote.join(", ")));
        }
        if !engine.released.is_empty() {
            parts.push(format!("would release {}", engine.released.join(", ")));
        }
        if !parts.is_empty() {
            writeln!(out, "    {}: {}", engine.engine, parts.join("; "))?;
        }
        for refused in &engine.refused {
            writeln!(
                out,
                "    {}: {} refused: {}",
                engine.engine, refused.kind, refused.reason
            )?;
            writeln!(out, "      fix: {}", refused.fix)?;
        }
        for kind in engine.wrote.iter().chain(&engine.released) {
            if !kinds.contains(&kind.as_str()) {
                kinds.push(kind);
            }
        }
    }
    if kinds.is_empty() {
        return Ok(());
    }
    let surfaces: String = kinds.iter().map(|k| format!(" --surface {k}")).collect();
    writeln!(
        out,
        "    Apply it with '{rerun}', or 'ctxloom materialize --yes{surfaces}'."
    )
}

#[cfg(test)]
#[path = "refresh.test.rs"]
mod tests;
