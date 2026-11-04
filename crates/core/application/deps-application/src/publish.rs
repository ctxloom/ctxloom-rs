//! `bundle push` and `bundle move`: publish an authored bundle to a registered
//! remote, or hand it to another project. Registering a remote is the consent
//! to publish to it: a push asks for no second blessing.
//!
//! A move relocates the bundle, and the source goes only once the destination
//! holds it: every failure before that leaves the source as it was.

use std::collections::BTreeMap;

use deps_ports::{CopyOutcome, Endpoint, PullRequest};

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
        if let Some((empty, _)) = files.iter().find(|(_, data)| data.is_empty()) {
            return Err(AppError::EmptyFile {
                path: empty.clone(),
            });
        }
        let target = format!("{BUNDLES_IN_REPOSITORY}/{}", request.bundle);
        let (title, body) = split_message(&request.message);
        let title = if title.is_empty() {
            format!("Update bundle: {}", request.bundle)
        } else {
            title
        };
        let message = commit_message(&title, &body);
        let size: usize = files.values().map(Vec::len).sum();
        let in_repository: BTreeMap<String, Vec<u8>> = files
            .into_iter()
            .map(|(rel, data)| (format!("{target}/{rel}"), data))
            .collect();
        let published = |detail| AppError::Publish {
            remote: remote.name.clone(),
            detail,
        };
        let (status, commit, pr_url) = if request.pr {
            let forge = deps::forge::resolve(&remote.url, &remote.forge, &registry);
            if forge.kind != deps::forge::Kind::Github {
                return Err(AppError::PullRequestNeedsForge {
                    remote: remote.name,
                    forge: forge.kind.name().to_string(),
                });
            }
            let (owner, repo) = deps::address::owner_repo(&remote.url).ok_or_else(|| {
                published(format!(
                    "{} names no owner and repository for the forge's API",
                    remote.url
                ))
            })?;
            let (fitted, overflow) = fit_title(&title);
            let opened = self
                .forge
                .open_pull_request(
                    &Endpoint {
                        api_url: forge.api(),
                        token_vars: forge.token_vars(),
                    },
                    &PullRequest {
                        owner,
                        repo,
                        branch: format!("ctxloom/bundle/{}", request.bundle),
                        message,
                        title: fitted,
                        body: pull_request_body(&body, &overflow, &request.bundle, &target),
                        files: in_repository,
                    },
                )
                .map_err(published)?;
            ("pr-created", opened.commit, opened.url)
        } else {
            let commit = self
                .remotes
                .publish(&remote.url, &in_repository, &message)
                .map_err(published)?;
            ("pushed", commit, String::new())
        };
        Ok(Pushed {
            status,
            path,
            remote: remote.name,
            target_path: target,
            title,
            create_pr: request.pr,
            size_bytes: size as u64,
            commit_sha: commit,
            pr_url,
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

/// The longest a pull request's title is left (Go's `maxPRTitleRunes`).
const TITLE_LIMIT: usize = 72;

/// A message's title and body: the first line is the title, the rest the body,
/// both trimmed (Go's `SplitTitleBody`).
fn split_message(message: &str) -> (String, String) {
    let message = message.trim();
    match message.split_once('\n') {
        Some((title, body)) => (title.trim().to_string(), body.trim().to_string()),
        None => (message.to_string(), String::new()),
    }
}

/// A commit's message: the title, a blank line, the body.
fn commit_message(title: &str, body: &str) -> String {
    if body.is_empty() {
        title.to_string()
    } else {
        format!("{title}\n\n{body}")
    }
}

/// `title` as a pull request's, cut at a word within the limit; and, where it
/// was cut, the whole of it for the body (Go's `fitPRTitle`).
fn fit_title(title: &str) -> (String, String) {
    let title = title.trim();
    if title.chars().count() <= TITLE_LIMIT {
        return (title.to_string(), String::new());
    }
    let window: String = title.chars().take(TITLE_LIMIT).collect();
    // The title is trimmed, so a space is never its first character.
    let cut = match window.rfind(' ') {
        Some(at) => &window[..at],
        None => &window,
    };
    (format!("{cut}\u{2026}"), title.to_string())
}

/// A pull request's description: the full title where it was cut, the body,
/// and where it came from, under rules (Go's `buildPRBody`).
fn pull_request_body(body: &str, overflow: &str, bundle: &str, target: &str) -> String {
    let mut sections = Vec::new();
    if !overflow.is_empty() {
        sections.push(format!("**Full title:** {overflow}"));
    }
    if !body.is_empty() {
        sections.push(body.to_string());
    }
    sections.push(format!(
        "Publishing bundle `{bundle}` from local repository.\n\nPath: `{target}`"
    ));
    sections.join("\n\n---\n\n")
}

#[cfg(test)]
#[path = "publish.test.rs"]
mod tests;
