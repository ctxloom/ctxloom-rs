//! The Go fixtures the tests read, embedded (the core does no I/O, tests
//! included): `internal/adapters/content/testdata/tree` at 860336c4e.

use crate::catalog::{Catalog, Entry};
use crate::document::parse_document;
use crate::tree::Files;

angzarr_presentation::english_for!(&[crate::EN_US.1]);

macro_rules! tree {
    ($($path:literal),* $(,)?) => {
        vec![$(($path, include_bytes!(concat!("../testdata/tree/", $path)).as_slice())),*]
    };
}

/// Go's `code-quality` bundle tree with the envelope Go's golden test gives
/// it (`version: 1.0.0`, no `schema_version`: generation 0).
pub(crate) fn code_quality() -> Files {
    let mut files: Files = tree![
        "code-quality/fragments/solid.distilled.md",
        "code-quality/fragments/solid.md",
        "code-quality/fragments/tricky.md",
        "code-quality/hooks/pre_tool/audit.yaml",
        "code-quality/hooks/pre_tool/guard.yaml",
        "code-quality/hooks/session_start/greet.yaml",
        "code-quality/mcp/.postgres.meta.yaml",
        "code-quality/mcp/postgres.yaml",
        "code-quality/profiles/strict.yaml",
        "code-quality/prompts/review.md",
        "code-quality/skills/.code-reviewer.meta.yaml",
        "code-quality/skills/code-reviewer/SKILL.md",
        "code-quality/skills/code-reviewer/references/checklist.md",
        "code-quality/skills/code-reviewer/scripts/run.sh",
    ]
    .into_iter()
    .map(|(path, bytes)| {
        (
            path.trim_start_matches("code-quality/").to_string(),
            bytes.to_vec(),
        )
    })
    .collect();
    files.insert(
        "bundle.yaml".to_string(),
        b"version: 1.0.0\ndescription: golden corpus code-quality\n".to_vec(),
    );
    files
}

/// A tree from `(path, text)` pairs.
pub(crate) fn files(pairs: &[(&str, &str)]) -> Files {
    pairs
        .iter()
        .map(|(path, text)| (path.to_string(), text.as_bytes().to_vec()))
        .collect()
}

/// The unit corpus of Go's composite tests: two bundles from their inline
/// documents. "alpha" ships fragments (one premised, one using a variable),
/// two commands and an MCP server; "beta" one tagged fragment and one command.
pub(crate) const ALPHA: &str = "version: 1.0.0\ndescription: alpha corpus\nfragments:\n  rules:\n    tags: [house]\n    content: \"Rules for {{project}}.\"\n  style:\n    content: \"Prefer small functions.\"\n  maybe:\n    premise: \"the agent is about to touch a signed bundle\"\n    content: \"Do not edit a signed bundle in place.\"\ncommands:\n  review:\n    description: Review the diff\n    content: \"Review the staged diff.\"\n    llm:\n      claude-code:\n        description: Review (claude)\n        argument_hint: \"[path]\"\n        allowed_tools: [Read]\n        model: sonnet\n  release:\n    description: Cut a release\n    content: \"Prepare the release notes.\"\n    llm:\n      claude-code:\n        enabled: false\nmcp:\n  db:\n    command: mcp-db\n";
pub(crate) const BETA: &str = "version: 1.0.0\ndescription: beta corpus\nfragments:\n  tagged:\n    tags: [house]\n    content: \"Tagged body.\"\ncommands:\n  ship:\n    content: \"Ship it.\"\n";

pub(crate) fn catalog_of(docs: &[(&str, &str)]) -> Catalog {
    Catalog::new(
        docs.iter()
            .map(|(name, text)| Entry {
                name: name.to_string(),
                manifest_path: format!("/app/.ctxloom/content/bundles/v2/{name}/bundle.yaml"),
                bundle: parse_document(text).unwrap_or_else(|e| panic!("{name}: {}", e.english())),
            })
            .collect(),
    )
}

pub(crate) fn corpus() -> Catalog {
    catalog_of(&[("alpha", ALPHA), ("beta", BETA)])
}
