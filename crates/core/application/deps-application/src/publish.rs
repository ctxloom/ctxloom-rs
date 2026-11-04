//! `bundle push` and `bundle move`: publish an authored bundle to a registered
//! remote, or hand it to another project. Registering a remote is the consent
//! to publish to it: a push asks for no second blessing.
//!
//! A move relocates the bundle, and the source goes only once the destination
//! holds it: every failure before that leaves the source as it was.

use std::collections::BTreeMap;

use deps_ports::CopyOutcome;

use crate::views::{Moved, Pushed};
use crate::{AppError, Deps, Outcome};

/// Where a bundle is in a repository.
const BUNDLES_IN_REPOSITORY: &str = ".ctxloom/content/bundles/v2";

/// What `bundle push` is asked.
#[derive(Clone, Debug, Default)]
pub struct PushRequest {
    pub bundle: String,
    pub remote: String,
    /// The commit message; empty is `Update bundle: <name>`.
    pub message: String,
    /// `--pr`, which needs a forge's API.
    pub pr: bool,
}

/// What `bundle move` is asked.
#[derive(Clone, Debug, Default)]
pub struct MoveRequest {
    pub name: String,
    /// A registered remote's name, or an existing directory.
    pub to: String,
    /// Overwrite a bundle of the same name at a directory.
    pub force: bool,
    pub message: String,
}

impl Deps {
    /// `bundle push`: every file of the authored tree to the remote's default
    /// branch as one commit.
    pub fn push(&self, request: &PushRequest) -> Outcome<Pushed> {
        Outcome {
            result: self.pushing(request),
            notices: Vec::new(),
        }
    }

    fn pushing(&self, request: &PushRequest) -> Result<Pushed, AppError> {
        if request.pr {
            return Err(AppError::PullRequestNeedsForge);
        }
        let registry = self.registry()?;
        let name = if request.remote.is_empty() {
            registry.default_name()
        } else {
            request.remote.clone()
        };
        if name.is_empty() {
            return Err(AppError::Required {
                what:
                    "a remote (none was given and no default is set: ctxloom remote default <name>)"
                        .to_string(),
            });
        }
        let remote = registry
            .get(&name)
            .ok_or_else(|| AppError::cause(deps::DepsError::RemoteNotFound { name }))?;
        let (path, files) = self.authored(&request.bundle)?;
        let target = format!("{BUNDLES_IN_REPOSITORY}/{}", request.bundle);
        let title = if request.message.is_empty() {
            format!("Update bundle: {}", request.bundle)
        } else {
            request.message.clone()
        };
        let size: usize = files.values().map(Vec::len).sum();
        let in_repository: BTreeMap<String, Vec<u8>> = files
            .into_iter()
            .map(|(rel, data)| (format!("{target}/{rel}"), data))
            .collect();
        let commit = self
            .remotes
            .publish(&remote.url, &in_repository, &title)
            .map_err(|detail| AppError::Publish {
                remote: remote.name.clone(),
                detail,
            })?;
        Ok(Pushed {
            status: "pushed",
            path,
            remote: remote.name,
            target_path: target,
            title,
            create_pr: false,
            size_bytes: size as u64,
            commit_sha: commit,
        })
    }

    /// The authored bundle: its manifest as shown, and its files.
    fn authored(&self, name: &str) -> Result<(String, BTreeMap<String, Vec<u8>>), AppError> {
        let file = |detail| AppError::File {
            what: format!("bundle {name}"),
            detail,
        };
        let path = self
            .files
            .local_bundle(name)
            .map_err(file)?
            .ok_or_else(|| AppError::BundleNotFound {
                name: name.to_string(),
            })?;
        let files = self.files.read_local_bundle(name).map_err(file)?;
        Ok((path, files))
    }

    /// `bundle move`.
    pub fn move_bundle(&self, request: &MoveRequest) -> Outcome<Moved> {
        Outcome {
            result: self.moving(request),
            notices: Vec::new(),
        }
    }

    fn moving(&self, request: &MoveRequest) -> Result<Moved, AppError> {
        let registry = self.registry()?;
        let (source, files) = self.authored(&request.name)?;
        let moved = if registry.get(&request.to).is_some() {
            let pushed = self.pushing(&PushRequest {
                bundle: request.name.clone(),
                remote: request.to.clone(),
                message: request.message.clone(),
                pr: false,
            })?;
            Moved {
                status: "moved",
                name: request.name.clone(),
                source,
                dest_kind: "remote",
                dest: pushed.target_path,
                remote: pushed.remote,
                commit_sha: pushed.commit_sha,
            }
        } else if self.files.is_directory(&request.to) {
            let landed = self
                .files
                .copy_bundle(&request.to, &request.name, &files, request.force)
                .map_err(|detail| AppError::File {
                    what: request.to.clone(),
                    detail,
                })?;
            let dest = match landed {
                CopyOutcome::Copied(dest) => dest,
                CopyOutcome::Exists(dest) => return Err(AppError::DestinationExists { dest }),
                CopyOutcome::Itself(dest) => return Err(AppError::DestinationItself { dest }),
            };
            Moved {
                status: "moved",
                name: request.name.clone(),
                source,
                dest_kind: "path",
                dest,
                remote: String::new(),
                commit_sha: String::new(),
            }
        } else {
            return Err(self.bad_destination(&request.to, &registry));
        };
        // The destination holds it; only now does the source go.
        self.files
            .remove_local_bundle(&request.name)
            .map_err(|detail| AppError::File {
                what: format!(
                    "{}: the bundle was written to {} but its source could not be removed, so it now exists in both places",
                    request.name, moved.dest
                ),
                detail,
            })?;
        Ok(moved)
    }

    fn bad_destination(&self, to: &str, registry: &deps::registry::Registry) -> AppError {
        let names: Vec<String> = registry.remotes().into_iter().map(|r| r.name).collect();
        let known = if names.is_empty() {
            " (no remotes configured)".to_string()
        } else {
            format!(" (configured remotes: {})", names.join(", "))
        };
        AppError::BadDestination {
            to: to.to_string(),
            known,
        }
    }
}

#[cfg(test)]
#[path = "publish.test.rs"]
mod tests;
