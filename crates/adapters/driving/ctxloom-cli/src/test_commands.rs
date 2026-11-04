//! The test-only commands (`ctxloom test ...`), behind `test-support`.

use std::io::Write;

use angzarr_presentation::{render, write_json, Catalog, Format, Render};
use serde::Serialize;

use crate::commands::TestVerb;
use crate::error::{diagnostic_samples, CliError};
use crate::run::Services;

#[derive(Serialize)]
struct Row {
    name: &'static str,
    count: u64,
}

/// A list of rows: the sample document.
struct Sample(Vec<Row>);

impl Render for Sample {
    fn json(&self) -> impl Serialize + '_ {
        &self.0
    }

    fn text(&self, out: &mut dyn Write) -> std::io::Result<()> {
        if self.0.is_empty() {
            return writeln!(out, "(none)");
        }
        for row in &self.0 {
            writeln!(out, "{}  {}", row.name, row.count)?;
        }
        Ok(())
    }
}

fn output(written: std::io::Result<()>) -> Result<(), CliError> {
    written.map_err(|e| CliError::Output {
        detail: e.to_string(),
    })
}

pub(crate) fn run(
    verb: TestVerb,
    format: Format,
    catalog: &Catalog,
    services: &mut impl Services,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    match verb {
        TestVerb::Refuse => Err(CliError::TestRefused),
        TestVerb::Findings => Err(CliError::TestFindings { count: 1 }),
        TestVerb::Sample { empty } => {
            let rows = if empty {
                Vec::new()
            } else {
                vec![
                    Row {
                        name: "alpha",
                        count: 1,
                    },
                    Row {
                        name: "beta",
                        count: 2,
                    },
                ]
            };
            output(render(out, format, &Sample(rows)))
        }
        TestVerb::Diagnostics => {
            let mut all = diagnostic_samples();
            all.extend(services.diagnostic_samples());
            let envelopes: Vec<_> = all.iter().map(|d| catalog.envelope(&**d)).collect();
            output(write_json(out, &envelopes))
        }
        TestVerb::SessionId => output(writeln!(out, "{}", services.session_name())),
        TestVerb::RunId {
            session,
            incarnation,
        } => {
            let id = services
                .run_id(&session, incarnation)
                .map_err(CliError::Service)?;
            output(writeln!(out, "{id}"))
        }
        TestVerb::OpenStore { project } => {
            let path = services.open_store(&project).map_err(CliError::Service)?;
            output(writeln!(out, "{path}"))
        }
    }
}
