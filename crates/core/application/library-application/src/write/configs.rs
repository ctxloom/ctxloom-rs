//! `config create` and `config edit` (Go's `cli/config.go`,
//! `operations.InitializeProject`). They work on the `.ctxloom` under the
//! working directory and do not load the configuration first: a config that
//! does not load is the very thing `config edit` is for.

use ctxloom_ports::Put;
use library::write::envelope::new_envelope;
use library::write::profile::{profile_path, PROJECT_BUNDLE};
use library::write::scaffold::{self, DEFAULT_REMOTES, SEED_NAME, SEED_PROFILE};

use crate::agents::ENGINES;
use crate::error::AppError;
use crate::project::{Library, Outcome};
use crate::write::editor::editor_from_env;
use crate::write::views::ConfigCreated;
use crate::write::{WriteError, WritePorts};

impl Library {
    fn write_side(&self) -> Result<&WritePorts, AppError> {
        self.ports
            .as_ref()
            .ok_or_else(|| WriteError::NotWired {}.into())
    }

    /// `config create [--engine E]`: the config, the default remotes registry
    /// and the seeded default profile; it refuses an existing config.
    pub fn config_create(&self, engine: &str) -> Outcome<ConfigCreated> {
        Outcome {
            result: self.created(engine),
            notices: Vec::new(),
        }
    }

    fn created(&self, engine: &str) -> Result<ConfigCreated, AppError> {
        let scaffold = &self.write_side()?.scaffold;
        let path = format!("{}/config.yaml", scaffold.app_dir());
        if scaffold.exists("config.yaml").map_err(AppError::Cause)? {
            return Err(WriteError::ConfigExists { path }.into());
        }
        let engine = if engine.is_empty() {
            "claude-code"
        } else {
            engine
        };
        if !ENGINES.contains(&engine) {
            return Err(WriteError::ConfigEngine {
                engine: engine.to_string(),
                valid: ENGINES.join(", "),
            }
            .into());
        }
        let bundle = format!("content/bundles/v2/{PROJECT_BUNDLE}");
        let mut files = vec![
            Put::file("config.yaml", scaffold::initial_config(engine)),
            Put::file("remotes.yaml", DEFAULT_REMOTES),
        ];
        let manifest = format!("{bundle}/bundle.yaml");
        if !scaffold.exists(&manifest).map_err(AppError::Cause)? {
            files.push(Put::file(manifest, new_envelope("1.0.0", "")));
        }
        let seed = format!("{bundle}/{}", profile_path(SEED_NAME));
        if !scaffold.exists(&seed).map_err(AppError::Cause)? {
            files.push(Put::file(seed, SEED_PROFILE));
        }
        scaffold.write(&files).map_err(AppError::Cause)?;
        Ok(ConfigCreated { path })
    }

    /// `config edit`: the project's config handed to the editor of the
    /// environment (`$VISUAL`, `$EDITOR`); there is none to edit before
    /// `config create`.
    pub fn config_edit(&self) -> Outcome<()> {
        Outcome {
            result: self.edited(),
            notices: Vec::new(),
        }
    }

    fn edited(&self) -> Result<(), AppError> {
        let ports = self.write_side()?;
        let path = format!("{}/config.yaml", ports.scaffold.app_dir());
        if !ports
            .scaffold
            .exists("config.yaml")
            .map_err(AppError::Cause)?
        {
            return Err(WriteError::ConfigMissing { path }.into());
        }
        ports
            .editor
            .open(&editor_from_env(&self.environ), &path)
            .map_err(AppError::Cause)
    }
}

#[cfg(test)]
#[path = "configs.test.rs"]
mod tests;
