//! The companion contract ctxloom reads from taskloom: `version --format
//! json`, which its boot probe parses (`name`, `version`, and
//! `lock_protocol`, which it asks of every running binary before it
//! removes a lock file), and `loadout --format yaml`, the document of the
//! hooks, MCP server and context taskloom contributes to a session.

use std::io::{self, Write};
use std::str::FromStr;

use presentation::{Format, Render, UnknownFormat};
use serde::Serialize;

use crate::{output_failed, CliError};

/// The loadout document, Go taskloom's own (`cmd/taskloom/loadout.yaml`)
/// with the human's reviewed factual changes (`fixtures/reviewed.rs`): a
/// parity test holds it to the Go source with those applied. Printed as
/// it is, comments and all, as Go prints it.
pub const LOADOUT: &str = include_str!("loadout.yaml");

/// A `--format` value: one of presentation's formats, or `yaml`, which
/// only `loadout` speaks.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Asked {
    Format(Format),
    Yaml,
}

impl FromStr for Asked {
    type Err = UnknownFormat;

    fn from_str(value: &str) -> Result<Asked, UnknownFormat> {
        match value {
            "yaml" => Ok(Asked::Yaml),
            other => other.parse().map(Asked::Format),
        }
    }
}

/// What `version` reports: Go's `cobrafmt.VersionInfo`, its fields in its
/// order, `lock_protocol` left out when 0 as Go leaves it.
#[derive(Serialize)]
pub struct VersionJson<'a> {
    pub name: &'a str,
    pub version: &'a str,
    #[serde(skip_serializing_if = "is_zero")]
    pub lock_protocol: u32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

impl Render for VersionJson<'_> {
    fn json(&self) -> impl Serialize + '_ {
        self
    }

    /// The bare version, as Go prints it.
    fn text(&self, out: &mut dyn Write) -> io::Result<()> {
        writeln!(out, "{}", self.version)
    }
}

/// Print the loadout, unless another format than yaml was asked for (a
/// `--json`, or a `--format` other than yaml); then nothing is printed.
pub(crate) fn loadout(
    json: bool,
    asked: Option<Asked>,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    let refused = match (json, asked) {
        (true, _) => Some(Format::Json),
        (false, Some(Asked::Format(format))) => Some(format),
        (false, Some(Asked::Yaml) | None) => None,
    };
    if let Some(format) = refused {
        return Err(CliError::LoadoutFormat {
            format: format_name(format).to_string(),
        });
    }
    out.write_all(LOADOUT.as_bytes()).map_err(output_failed)
}

fn format_name(format: Format) -> &'static str {
    match format {
        Format::Text => "text",
        Format::Json => "json",
    }
}
