use super::*;

// resources/default-remotes.yaml at ca14d6046
#[test]
fn test_the_default_remotes_are_gos_resource_byte_for_byte() {
    assert_eq!(
        DEFAULT_REMOTES,
        "# ctxloom Remote Sources\n# Pre-configured remotes for common ctxloom bundles and profiles.\n#\n# Registering a remote is the trust act: its content reaches the assistant\n# because the project added it.\n\nschema_version: 1\n\nremotes:\n  ctxloom-default:\n    url: https://github.com/ctxloom/ctxloom-default\n"
    );
}

// resources/profiles/default.yaml at ca14d6046
#[test]
fn test_the_seed_profile_is_gos_resource_byte_for_byte() {
    assert_eq!(
        SEED_PROFILE,
        "description: \"Local default coding profile scaffolded by `ctxloom init`: the project's primary implementer context, inheriting ctxloom-default's developer profile. Edit it freely; `ctxloom init prompt` re-enters the setup interview any time.\"\ntags:\n  - default\n# A LOCAL profile (owned and editable in this project's .ctxloom), not a bare\n# remote reference.\n#\n# The baseline-conduct parent was REMOVED with the `default` bundle it pointed\n# at. Conduct is no longer SEEDED into every project as a fixed parent; it is\n# offered by init's phase-3 survey as content carrying the `default` tag, which\n# the user includes or declines like anything else. A tag is a recommendation\n# the user answers; a seeded parent is a decision made on their behalf before\n# they arrive.\nparents:\n  - ctxloom+git://github.com/ctxloom/ctxloom-default//bundles/ai-developer@branch:main#profiles/developer\n"
    );
    assert!(crate::profile::Profile::decode(SEED_PROFILE).is_ok());
    assert_eq!(SEED_NAME, "default");
}

// captured from Go runs: config create --engine <e>
#[test]
fn test_the_claude_config_is_the_registrys_primary_and_fast_entries() {
    assert_eq!(
        initial_config("claude-code"),
        "agents:\n  default:\n    engine_home: session\n    llm: claude-code\n    profiles:\n      - default\n    runtime: host\nauth: token\nconfig:\n  use_distilled: true\ndefault_agent: default\nllm:\n  configs:\n    claude-code:\n      model: opus\n      type: claude-code\n    claude-fast:\n      model: haiku\n      type: claude-code\n  defaults:\n    fast: claude-fast\n    primary: claude-code\nschema_version: 9\n"
    );
}

#[test]
fn test_an_engine_without_roles_is_one_entry_serving_both() {
    for engine in [
        "mock",
        "opencode",
        "mock-launch",
        "mock-lossy",
        "mock-noskills",
    ] {
        assert_eq!(
            initial_config(engine),
            format!("agents:\n  default:\n    engine_home: session\n    llm: {engine}\n    profiles:\n      - default\n    runtime: host\nauth: token\nconfig:\n  use_distilled: true\ndefault_agent: default\nllm:\n  configs:\n    {engine}:\n      type: {engine}\n  defaults:\n    fast: {engine}\n    primary: {engine}\nschema_version: 9\n")
        );
    }
}

#[test]
fn test_what_it_scaffolds_is_a_config_this_build_reads() {
    for engine in ["claude-code", "mock"] {
        let doc = angzarr_yaml::parse(&initial_config(engine)).unwrap();
        assert_eq!(initial_sections(engine), doc);
        assert!(crate::config::schema::validate(&doc).is_empty(), "{engine}");
        assert_eq!(doc.get("schema_version").and_then(Value::as_str), Some("9"));
    }
}
