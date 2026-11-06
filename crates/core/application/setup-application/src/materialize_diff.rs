//! `materialize --diff FILE`: the context a materialize would write for the
//! profiles, compared with a file already delivered (Go's
//! `runMaterializeDiff`). A read: no command is issued and nothing is written.

use library_application::{Flags, PackageRequest};
use serde::Serialize;
use surface_ports::Kind;

use crate::diff::unified;
use crate::materialize::SurfaceSpec;
use crate::owner_voice::with_own_fragment;
use crate::{Done, Setup, SetupError};

/// One comparison: the profiles' context against `file`.
#[derive(Clone, Debug, Default)]
pub struct DiffRequest {
    /// The profile set; none is the default agent's.
    pub profiles: Vec<String>,
    /// The engines named; the comparison is of exactly one.
    pub engines: Vec<String>,
    /// The kinds named; when any, context must be among them.
    pub surfaces: Vec<SurfaceSpec>,
    /// `--release` was also given, which a comparison cannot honour.
    pub release: bool,
    /// The delivered file, absolute or relative to the working directory.
    pub file: String,
    pub flags: Flags,
}

/// What a comparison found.
#[derive(Clone, Debug, Default, Serialize)]
pub struct MaterializeDiff {
    pub profiles: Vec<String>,
    pub compared_to: String,
    pub identical: bool,
    /// The unified diff, from the file to the context; empty when identical.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub diff: String,
    /// How the profiles are named in words (`a+b`, or the default agent).
    #[serde(skip)]
    pub label: String,
}

impl Setup {
    /// The context a materialize of the request's profiles would write, set
    /// against the file at `request.file`. A trailing newline is not a
    /// difference: the section writer ends the file with one the assembled
    /// text may lack.
    pub fn materialize_diff(
        &self,
        request: &DiffRequest,
    ) -> Result<Done<MaterializeDiff>, SetupError> {
        let bad = |detail: &str| SetupError::Request {
            detail: detail.to_string(),
        };
        if request.release {
            return Err(bad(
                "--release takes no profiles and no --diff: it removes what an earlier materialize wrote",
            ));
        }
        if request.engines.len() > 1 {
            return Err(bad(
                "--diff compares one engine's context; name at most one --backend",
            ));
        }
        if !request.surfaces.is_empty() && !request.surfaces.iter().any(|s| s.kind == Kind::Context)
        {
            return Err(bad(
                "--diff compares the context; a --surface list without context leaves nothing to compare",
            ));
        }
        let outcome = self.library.package(
            &request.flags,
            &PackageRequest {
                profiles: request.profiles.clone(),
            },
        );
        let notices = outcome.notices;
        let package = outcome.result.map_err(SetupError::cause)?;
        let engines = self.engines_of(
            &crate::MaterializeRequest {
                engines: request.engines.clone(),
                ..crate::MaterializeRequest::default()
            },
            &package.configured_engines,
        )?;
        if engines.len() != 1 {
            return Err(SetupError::Request {
                detail: format!(
                    "--diff compares one engine's context; {} are configured ({}): name one with --backend",
                    engines.len(),
                    engines.join(", ")
                ),
            });
        }
        if !package.fatal.is_empty() {
            let findings: Vec<String> = package
                .fatal
                .iter()
                .map(crate::materialize::findings_text)
                .collect();
            return Err(SetupError::Refused {
                findings: findings.join("; "),
            });
        }
        let path = self.absolute(&request.file);
        let theirs = match self.store.read_absolute(&path) {
            Ok(Some(bytes)) => String::from_utf8_lossy(&bytes).into_owned(),
            Ok(None) => {
                return Err(SetupError::DiffFileMissing {
                    path: request.file.clone(),
                })
            }
            Err(e) => {
                return Err(SetupError::DiffFileUnreadable {
                    path: request.file.clone(),
                    detail: e.to_string(),
                })
            }
        };
        let label = if request.profiles.is_empty() {
            "the default agent".to_string()
        } else {
            request.profiles.join("+")
        };
        let mine = lines_ended(&with_own_fragment(&package.context, package.use_distilled));
        let delivered = lines_ended(&theirs);
        let diff = if mine == delivered {
            String::new()
        } else {
            unified(
                &delivered,
                &mine,
                &request.file,
                &format!("{label} (materialized here)"),
            )
        };
        Ok(Done {
            value: MaterializeDiff {
                profiles: request.profiles.clone(),
                compared_to: request.file.clone(),
                identical: diff.is_empty(),
                diff,
                label,
            },
            notices,
        })
    }
}

/// `text` with its trailing newlines replaced by exactly one.
fn lines_ended(text: &str) -> String {
    format!("{}\n", text.trim_end_matches('\n'))
}

#[cfg(test)]
#[path = "materialize_diff.test.rs"]
mod tests;
