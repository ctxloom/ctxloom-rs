//! `remote list|create|edit|remove|default|show` and `remote discover`.

use deps::address::canonical_ref;
use deps::registry::{Edit, Remote};

use crate::cache::ByAddress;
use crate::views::{
    BrowseItem, Browsed, Created, DefaultSet, EditedRemote, RemoteListing, RemoteRow, RemoteState,
    RemoveResult,
};
use crate::{AppError, Deps, Notice, Outcome};

/// What `remote create` is asked.
#[derive(Clone, Debug, Default)]
pub struct CreateRequest {
    pub name: String,
    pub url: String,
    /// The forge label to bind it to; empty resolves by the URL's host.
    pub forge: String,
}

fn done<T>(result: Result<T, AppError>, notices: Vec<Notice>) -> Outcome<T> {
    Outcome { result, notices }
}

impl Deps {
    /// `remote list`.
    pub fn remote_list(&self) -> Outcome<RemoteListing> {
        done(self.remote_listing(), Vec::new())
    }

    fn remote_listing(&self) -> Result<RemoteListing, AppError> {
        let registry = self.registry()?;
        let remotes: Vec<RemoteRow> = registry
            .remotes()
            .into_iter()
            .map(|r| RemoteRow {
                name: r.name,
                url: r.url,
            })
            .collect();
        Ok(RemoteListing {
            count: remotes.len() as u64,
            remotes,
            default: registry.default_name(),
        })
    }

    /// `remote create`: register, then clone, so a wrong URL or missing access
    /// shows up now. A clone that fails is a warning, not a refusal.
    pub fn remote_create(&self, request: &CreateRequest) -> Outcome<Created> {
        let mut notices = Vec::new();
        let result = self.create(request, &mut notices);
        done(result, notices)
    }

    fn create(
        &self,
        request: &CreateRequest,
        notices: &mut Vec<Notice>,
    ) -> Result<Created, AppError> {
        if request.name.is_empty() {
            return Err(AppError::Required {
                what: "name".to_string(),
            });
        }
        if request.url.is_empty() {
            return Err(AppError::Required {
                what: "url".to_string(),
            });
        }
        let url = self.resolve_path(&request.url)?;
        let mut registry = self.registry()?;
        let remote = registry
            .add(&request.name, &url, &request.forge)
            .map_err(AppError::cause)?;
        self.save_registry(&registry)?;
        let warning = match ByAddress(self.remotes.as_ref()).ensure(&remote.url) {
            Ok(()) => String::new(),
            Err(detail) => format!("{}: clone failed: {detail}", remote.name),
        };
        if !warning.is_empty() {
            notices.push(Notice::text(warning.clone()));
        }
        Ok(Created {
            status: "added",
            name: remote.name,
            url: remote.url,
            warning,
        })
    }

    /// A repository given as a path on this machine, as the file URL of the
    /// repository there; any other address as it is.
    fn resolve_path(&self, raw: &str) -> Result<String, AppError> {
        if !deps::address::is_local_path(raw.trim()) {
            return Ok(raw.to_string());
        }
        self.remotes.local_repository(raw.trim()).map_err(|detail| {
            AppError::cause(deps::DepsError::NoRepository {
                url: raw.to_string(),
                detail,
            })
        })
    }

    /// `remote edit`.
    pub fn remote_edit(&self, name: &str, edit: &Edit) -> Outcome<EditedRemote> {
        done(self.edit(name, edit), Vec::new())
    }

    fn edit(&self, name: &str, edit: &Edit) -> Result<EditedRemote, AppError> {
        let mut edit = edit.clone();
        if let Some(url) = &edit.url {
            edit.url = Some(self.resolve_path(url)?);
        }
        let mut registry = self.registry()?;
        let edited = registry.edit(name, &edit).map_err(AppError::cause)?;
        if !edited.changed.is_empty() {
            self.save_registry(&registry)?;
        }
        Ok(EditedRemote {
            status: "edited",
            name: edited.after.name.clone(),
            before: state(&edited.before),
            after: state(&edited.after),
            changed: edited.changed.iter().map(|f| f.to_string()).collect(),
            default_pointer_updated: edited.default_followed,
        })
    }

    /// `remote remove`: what it would remove, or, with `apply`, remove it. The
    /// content already pulled from it is untouched.
    pub fn remote_remove(&self, name: &str, apply: bool) -> Outcome<RemoveResult> {
        done(self.remove(name, apply), Vec::new())
    }

    fn remove(&self, name: &str, apply: bool) -> Result<RemoveResult, AppError> {
        let mut registry = self.registry()?;
        if !apply {
            if registry.get(name).is_none() {
                return Err(AppError::NoRemoteNamed {
                    name: name.to_string(),
                });
            }
            return Ok(RemoveResult::Preview {
                status: "preview",
                applied: false,
                target: format!("remote {name:?}"),
                apply: format!("ctxloom remote remove {name} --yes"),
            });
        }
        registry.remove(name).map_err(AppError::cause)?;
        self.save_registry(&registry)?;
        Ok(RemoveResult::Removed {
            status: "removed",
            name: name.to_string(),
        })
    }

    /// `remote default`: make `name` the default; none clears it.
    pub fn remote_default(&self, name: Option<&str>) -> Outcome<DefaultSet> {
        done(self.set_default(name), Vec::new())
    }

    fn set_default(&self, name: Option<&str>) -> Result<DefaultSet, AppError> {
        let mut registry = self.registry()?;
        registry.set_default(name).map_err(AppError::cause)?;
        self.save_registry(&registry)?;
        Ok(match name {
            Some(name) => DefaultSet {
                status: "set",
                name: name.to_string(),
            },
            None => DefaultSet {
                status: "cleared",
                name: String::new(),
            },
        })
    }

    /// `remote show`: the bundles a remote publishes, read as the last fetch
    /// left them; nothing is installed.
    pub fn remote_show(&self, name: &str) -> Outcome<Browsed> {
        done(self.show(name), Vec::new())
    }

    fn show(&self, name: &str) -> Result<Browsed, AppError> {
        let remote = self.registry()?.get(name).ok_or_else(|| AppError::Browse {
            remote: name.to_string(),
            detail: format!("remote not found: {name}"),
        })?;
        let browse = |detail: String| AppError::Browse {
            remote: name.to_string(),
            detail,
        };
        ByAddress(self.remotes.as_ref())
            .ensure(&remote.url)
            .map_err(browse)?;
        let names = ByAddress(self.remotes.as_ref())
            .list_bundles(&remote.url)
            .map_err(browse)?;
        let items: Vec<BrowseItem> = names
            .into_iter()
            .map(|bundle| BrowseItem {
                pull_ref: canonical_ref(&remote.url, &bundle).unwrap_or_default(),
                path: bundle.clone(),
                name: bundle,
                kind: "bundle",
                is_dir: true,
            })
            .collect();
        Ok(Browsed {
            remote: remote.name,
            url: remote.url,
            count: items.len() as u64,
            items,
            warnings: Vec::new(),
        })
    }
}

fn state(remote: &Remote) -> RemoteState {
    RemoteState {
        name: remote.name.clone(),
        url: remote.url.clone(),
        forge: remote.forge.clone(),
    }
}

#[cfg(test)]
#[path = "registry.test.rs"]
mod tests;
