use super::*;

const GITHUB: &str = "https://github.com/acme/hub";

fn registry(forges: &str) -> Registry {
    let text = if forges.is_empty() {
        "schema_version: 1\n".to_string()
    } else {
        format!("schema_version: 1\nforges:\n{forges}")
    };
    Registry::decode(&text, "remotes.yaml").expect("a registry")
}

const CORP: &str = "  corp:\n    type: github\n    base_url: https://ghe.corp.test\n    api_url: https://ghe.corp.test/api/v3\n    token_env: CORP_TOKEN\n";

#[test]
fn test_a_kind_is_named_as_forges_name_it() {
    assert_eq!(Kind::Github.name(), "github");
    assert_eq!(Kind::Git.name(), "git");
}

// forge.go resolveForge, step 3: github.com is GitHub, other hosts plain git.
#[test]
fn test_github_com_is_github_with_the_public_api_and_the_ambient_token() {
    for url in [
        GITHUB,
        "https://www.github.com/acme/hub",
        "http://github.com/acme/hub",
        "https://GitHub.com/acme/hub",
    ] {
        let resolved = resolve(url, "", &registry(""));
        assert_eq!(resolved.kind, Kind::Github, "{url}");
        assert!(resolved.is_public(), "{url}");
        assert_eq!(resolved.api(), "https://api.github.com", "{url}");
        assert_eq!(resolved.token_vars(), ["GH_TOKEN", "GITHUB_TOKEN"], "{url}");
    }
}

#[test]
fn test_any_other_host_is_plain_git_with_no_token() {
    for url in [
        "https://gitlab.com/acme/hub",
        "file:///srv/hub.git",
        "https://ghe.corp.test/acme/hub",
    ] {
        let resolved = resolve(url, "", &registry(""));
        assert_eq!(resolved.kind, Kind::Git, "{url}");
        assert_eq!(resolved.token_vars(), Vec::<&str>::new(), "{url}");
    }
}

// forge.go resolveForge, step 1: an explicit label wins over the host.
#[test]
fn test_a_forge_label_wins_over_the_host() {
    assert_eq!(
        resolve("file:///srv/hub.git", "github", &registry("")).kind,
        Kind::Github
    );
    assert_eq!(resolve(GITHUB, "git", &registry("")).kind, Kind::Git);
}

// An explicit label this registry does not know falls through to the host.
#[test]
fn test_a_label_the_registry_does_not_have_falls_through_to_the_host() {
    assert_eq!(resolve(GITHUB, "nowhere", &registry("")).kind, Kind::Github);
    assert_eq!(
        resolve("file:///srv/hub.git", "nowhere", &registry("")).kind,
        Kind::Git
    );
}

#[test]
fn test_a_configured_forge_carries_its_endpoint_and_names_its_token() {
    let resolved = resolve("https://ghe.corp.test/acme/hub", "corp", &registry(CORP));
    assert_eq!(
        resolved,
        Resolved {
            kind: Kind::Github,
            base_url: "https://ghe.corp.test".to_string(),
            api_url: "https://ghe.corp.test/api/v3".to_string(),
            token_env: "CORP_TOKEN".to_string(),
        }
    );
    assert!(!resolved.is_public());
    assert_eq!(resolved.api(), "https://ghe.corp.test/api/v3");
}

// Plan over Go (forge.go Token): a credential of github.com is not spent at
// another host, so an Enterprise endpoint takes its token from its own
// token_env alone.
#[test]
fn test_an_enterprise_endpoint_never_falls_back_to_the_ambient_github_token() {
    let resolved = resolve("https://ghe.corp.test/acme/hub", "corp", &registry(CORP));
    assert_eq!(resolved.token_vars(), ["CORP_TOKEN"]);
    let bare = "  corp:\n    type: github\n    api_url: https://ghe.corp.test/api/v3\n";
    let resolved = resolve("https://ghe.corp.test/acme/hub", "corp", &registry(bare));
    assert_eq!(resolved.token_vars(), Vec::<&str>::new());
}

// forge.go resolveForge, step 2: no label, but the host (and port) is a
// configured forge's server.
#[test]
fn test_a_remote_with_no_label_is_bound_by_its_host_and_port() {
    let by_host = resolve("https://ghe.corp.test/acme/hub", "", &registry(CORP));
    assert_eq!(by_host.kind, Kind::Github);
    assert_eq!(by_host.token_env, "CORP_TOKEN");
    let www = resolve("https://www.ghe.corp.test/acme/hub", "", &registry(CORP));
    assert_eq!(www.token_env, "CORP_TOKEN");
    let other_port = resolve("https://ghe.corp.test:8443/acme/hub", "", &registry(CORP));
    assert_eq!(other_port.kind, Kind::Git);
    assert_eq!(other_port.token_env, "");
    let default_port = resolve("https://ghe.corp.test:443/acme/hub", "", &registry(CORP));
    assert_eq!(default_port.token_env, "CORP_TOKEN");
}

#[test]
fn test_two_forges_on_one_server_are_taken_in_label_order() {
    let forges = "  zeta:\n    type: git\n    base_url: https://ghe.corp.test\n  alpha:\n    type: github\n    base_url: https://ghe.corp.test\n    token_env: ALPHA\n";
    let resolved = resolve("https://ghe.corp.test/acme/hub", "", &registry(forges));
    assert_eq!(resolved.token_env, "ALPHA");
}

// forge.go MergeForges: a configured forge replaces the built-in of its label.
#[test]
fn test_a_configured_github_forge_replaces_the_built_in_one() {
    let forges = "  github:\n    type: github\n    token_env: MY_TOKEN\n";
    let resolved = resolve(GITHUB, "", &registry(forges));
    assert_eq!(
        resolved.token_vars(),
        ["MY_TOKEN", "GH_TOKEN", "GITHUB_TOKEN"]
    );
    let by_label = resolve("file:///srv/hub.git", "github", &registry(forges));
    assert_eq!(by_label.token_env, "MY_TOKEN");
}

#[test]
fn test_the_public_endpoint_written_out_is_still_the_public_one() {
    let forges = "  github:\n    type: github\n    api_url: https://api.github.com/\n";
    let resolved = resolve(GITHUB, "", &registry(forges));
    assert!(resolved.is_public());
    assert_eq!(resolved.token_vars(), ["GH_TOKEN", "GITHUB_TOKEN"]);
}

#[test]
fn test_a_configured_forge_that_names_no_type_is_plain_git() {
    let forges = "  corp:\n    base_url: https://ghe.corp.test\n";
    assert_eq!(
        resolve("https://ghe.corp.test/acme/hub", "", &registry(forges)).kind,
        Kind::Git
    );
}

// A forge that is plain git has no API and so no token.
#[test]
fn test_plain_git_names_no_token_whatever_it_is_configured_with() {
    let forges = "  corp:\n    type: git\n    token_env: CORP_TOKEN\n";
    let resolved = resolve("https://ghe.corp.test/acme/hub", "corp", &registry(forges));
    assert_eq!(resolved.kind, Kind::Git);
    assert_eq!(resolved.token_vars(), Vec::<&str>::new());
}

#[test]
fn test_the_registry_lists_every_forge_by_label() {
    let forges = registry(CORP).forges();
    let labels: Vec<_> = forges.iter().map(|f| f.label.as_str()).collect();
    assert_eq!(labels, ["corp", "git", "github"]);
    let corp = &forges[0];
    assert_eq!(
        (corp.kind.as_str(), corp.token_env.as_str()),
        ("github", "CORP_TOKEN")
    );
    assert_eq!(forges[1].kind, "git");
    assert_eq!(forges[2].kind, "github");
}

#[test]
fn test_github_com_without_a_configured_address_gets_its_own_as_the_base() {
    assert_eq!(
        resolve(GITHUB, "", &registry("")).base_url,
        "https://github.com"
    );
    // Another host on plain git has no base to give.
    assert_eq!(
        resolve("https://gitlab.com/acme/hub", "", &registry("")).base_url,
        ""
    );
    // A configured base is kept.
    let forges = "  github:\n    type: github\n    base_url: https://mirror.test\n";
    assert_eq!(
        resolve(GITHUB, "", &registry(forges)).base_url,
        "https://mirror.test"
    );
}

#[test]
fn test_an_http_server_s_own_port_is_80_and_an_address_in_brackets_has_no_port() {
    let http = "  corp:\n    type: github\n    base_url: http://ghe.corp.test\n    token_env: T\n";
    for url in ["http://ghe.corp.test/a/b", "http://ghe.corp.test:80/a/b"] {
        assert_eq!(resolve(url, "", &registry(http)).token_env, "T", "{url}");
    }
    assert_eq!(
        resolve("http://ghe.corp.test:443/a/b", "", &registry(http)).token_env,
        ""
    );
    let v6 = "  corp:\n    type: github\n    base_url: https://[::1]\n    token_env: T\n";
    assert_eq!(
        resolve("https://[::1]/a/b", "", &registry(v6)).token_env,
        "T"
    );
}
