use std::collections::BTreeMap;
use std::sync::Arc;

use base64::Engine;

use super::*;

const TOKEN: &str = "ghp_sekrit0123456789"; // gitleaks:allow

fn auth(vars: &'static [&'static str], env: &'static [(&'static str, &'static str)]) -> Auth {
    let table: BTreeMap<_, _> = env.iter().copied().collect();
    Auth::new(
        Arc::new(move |name| table.get(name).map(|v| (*v).to_string())),
        Arc::new(move |_| vars.iter().map(|v| (*v).to_string()).collect()),
    )
}

fn basic(token: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(format!("x-access-token:{token}"))
}

fn pairs(credential: &Credential) -> Vec<(&str, &str)> {
    credential
        .pairs()
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect()
}

// repo_cache.go authEnv: the token rides as an extraheader scoped to the clone
// host, by GIT_CONFIG_COUNT/KEY/VALUE.
#[test]
fn test_https_url_gets_a_host_scoped_basic_header_through_the_environment() {
    let credential =
        auth(&["GH_TOKEN"], &[("GH_TOKEN", TOKEN)]).credential("https://github.com/o/r");
    let header = format!("AUTHORIZATION: basic {}", basic(TOKEN));
    assert_eq!(
        pairs(&credential),
        vec![
            ("GIT_CONFIG_COUNT", "1"),
            ("GIT_CONFIG_KEY_0", "http.https://github.com/.extraheader"),
            ("GIT_CONFIG_VALUE_0", header.as_str()),
        ]
    );
}

#[test]
fn test_first_variable_set_and_not_empty_wins() {
    let credential = auth(
        &["CORP", "GH_TOKEN", "GITHUB_TOKEN"],
        &[
            ("CORP", ""),
            ("GITHUB_TOKEN", "second"),
            ("GH_TOKEN", TOKEN),
        ],
    )
    .credential("https://github.com/o/r");
    assert!(pairs(&credential)
        .iter()
        .any(|(_, v)| v.ends_with(&basic(TOKEN))));
}

#[test]
fn test_no_variable_set_is_no_credential() {
    let credential = auth(&["GH_TOKEN"], &[]).credential("https://github.com/o/r");
    assert!(credential.pairs().is_empty());
}

#[test]
fn test_no_variables_named_is_no_credential() {
    let credential = auth(&[], &[("GH_TOKEN", TOKEN)]).credential("https://github.com/o/r");
    assert!(credential.pairs().is_empty());
}

#[test]
fn test_only_https_is_sent_a_token() {
    for url in [
        "http://github.com/o/r",
        "ssh://git@github.com/o/r",
        "git@github.com:o/r.git",
        "file:///srv/r",
        "/srv/r",
        "https:///nohost",
    ] {
        let credential = auth(&["T"], &[("T", TOKEN)]).credential(url);
        assert!(credential.pairs().is_empty(), "{url}");
    }
}

#[test]
fn test_scope_is_the_authority_without_userinfo_and_a_port_is_kept() {
    let credential =
        auth(&["T"], &[("T", TOKEN)]).credential("https://user:pw@Git.Corp:8443/o/r?x#y");
    assert_eq!(
        pairs(&credential)[1],
        (
            "GIT_CONFIG_KEY_0",
            "http.https://git.corp:8443/.extraheader"
        )
    );
}

#[test]
fn test_debug_and_redaction_never_show_the_token() {
    let credential = auth(&["T"], &[("T", TOKEN)]).credential("https://github.com/o/r");
    let shown = format!("{credential:?}");
    assert!(
        !shown.contains(TOKEN) && !shown.contains(&basic(TOKEN)),
        "{shown}"
    );
    let said = format!("fatal: {TOKEN} and AUTHORIZATION: basic {}", basic(TOKEN));
    let clean = credential.redact(&said);
    assert!(
        !clean.contains(TOKEN) && !clean.contains(&basic(TOKEN)),
        "{clean}"
    );
    assert!(clean.starts_with("fatal: "), "{clean}");
}

#[test]
fn test_redacting_with_no_credential_changes_nothing() {
    assert_eq!(Credential::none().redact("fatal: x"), "fatal: x");
}
