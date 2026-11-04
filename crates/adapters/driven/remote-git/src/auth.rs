//! The token of a private repository, delivered to git out of band.
//!
//! As Go's `authEnv` does: an `Authorization` header scoped to the clone's
//! host, set through git's environment configuration (`GIT_CONFIG_COUNT`,
//! `GIT_CONFIG_KEY_n`, `GIT_CONFIG_VALUE_n`). Never `git -c` (argv is readable
//! by every user of the machine), never the URL (git echoes it in errors and
//! stores it in the clone's config). The environment is still readable by
//! the same user's processes and by what git spawns; that is the guarantee,
//! no more.
//!
//! Which variables hold the token is the deps read model's answer
//! (`forge::Resolved::token_vars`), handed in; this reads the first one set.
//! Only an `https` address is sent a token: not a cleartext one, not ssh,
//! whose auth is the user's own.

use std::fmt;
use std::sync::Arc;

use base64::Engine;

/// The process's environment as a lookup.
pub type Env = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// The variables a clone address's token may be in, the first set winning.
pub type TokenVars = Arc<dyn Fn(&str) -> Vec<String> + Send + Sync>;

/// How the token for an address is found.
#[derive(Clone)]
pub(crate) struct Auth {
    env: Env,
    vars: TokenVars,
}

impl Auth {
    pub(crate) fn new(env: Env, vars: TokenVars) -> Auth {
        Auth { env, vars }
    }

    /// No variable named, so no credential for any address.
    pub(crate) fn none() -> Auth {
        Auth::new(Arc::new(|_| None), Arc::new(|_| Vec::new()))
    }

    /// What `url` is sent: nothing, where it takes no token or none is set.
    pub(crate) fn credential(&self, url: &str) -> Credential {
        let Some(scope) = https_authority(url) else {
            return Credential::none();
        };
        let token = (self.vars)(url)
            .iter()
            .find_map(|var| (self.env)(var).filter(|token| !token.is_empty()));
        let Some(token) = token else {
            return Credential::none();
        };
        let basic =
            base64::engine::general_purpose::STANDARD.encode(format!("x-access-token:{token}"));
        Credential {
            pairs: vec![
                ("GIT_CONFIG_COUNT".to_string(), "1".to_string()),
                (
                    "GIT_CONFIG_KEY_0".to_string(),
                    format!("http.https://{scope}/.extraheader"),
                ),
                (
                    "GIT_CONFIG_VALUE_0".to_string(),
                    format!("AUTHORIZATION: basic {basic}"),
                ),
            ],
            secrets: vec![token, basic],
        }
    }
}

/// The host (and port, where there is one) of an `https` address, lowercase
/// and without userinfo; none for any other address.
fn https_authority(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// Environment pairs for one git run, and the secrets they carry.
pub(crate) struct Credential {
    pairs: Vec<(String, String)>,
    secrets: Vec<String>,
}

impl Credential {
    pub(crate) fn none() -> Credential {
        Credential {
            pairs: Vec::new(),
            secrets: Vec::new(),
        }
    }

    pub(crate) fn pairs(&self) -> &[(String, String)] {
        &self.pairs
    }

    /// `text` with every secret blotted out: git's words are shown to the
    /// user, and must not repeat what it was given.
    pub(crate) fn redact(&self, text: &str) -> String {
        self.secrets
            .iter()
            .fold(text.to_string(), |text, secret| text.replace(secret, "***"))
    }
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Credential({} variables, redacted)", self.pairs.len())
    }
}

#[cfg(test)]
#[path = "auth.test.rs"]
mod tests;
