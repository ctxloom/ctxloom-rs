//! Tag queries: tagma-core's engines over a listing's tasks, as Go's
//! `tagquery.go` runs tagma's Go port.
//!
//! A query is postfix, as Go's (`urgent/release/and`, `urgent/not`), or
//! infix (`urgent and not release`, `(a or b) c`): postfix is tried first,
//! and infix when postfix cannot read it. No string reads as both with two
//! meanings: postfix separates its tokens with `/`, which no infix atom
//! holds, and infix with spaces and parentheses, which no postfix atom
//! holds unquoted, so a string both read is a single atom. The operators
//! `and`, `or` and `not` are matched in any case.
//!
//! The index holds only the tasks the listing admits (its candidates), so
//! `not` complements within them, not within every task. Each candidate
//! carries an inert presence tag besides its own, so a task with no tag
//! still takes part (`urgent/not` holds it). A target the schema declares
//! `type=<name>` is ordered by the comparator registered under that name:
//! SemVer precedence for `semver`, the one taskloom registers.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use annals_diagnostic::{Arg, Diagnostic};
use tagma_core::Index;

use crate::schema::{TagSchema, SEMVER};
use crate::semver::Semver;
use crate::Tag;

/// The namespace of the presence tag every candidate carries. A task may
/// hold a tag in it too, which changes nothing: the presence tag is on
/// every candidate already.
const PRESENCE_NAMESPACE: &str = "taskloom.internal";

/// The item the schema's type declarations are indexed under: no harp id,
/// since a harp has no colon.
const TYPE_CONFIG_ITEM: &str = "taskloom.internal:type-config";

/// How to write a query, said with every malformed one (Go's
/// `tagQueryRemedy`, with infix).
pub const QUERY_GRAMMAR: &str = "queries are postfix, tags first and the operator after (urgent/release/and, urgent/not), and an and/or/not needs enough operands before it; or infix (urgent and not release)";

/// A malformed query.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct QueryError {
    pub query: String,
    /// Why postfix cannot read it.
    pub postfix: String,
    /// Why infix cannot read it.
    pub infix: String,
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the tag query {} is malformed (as postfix: {}; as infix: {}): {QUERY_GRAMMAR}",
            self.query, self.postfix, self.infix
        )
    }
}

impl Diagnostic for QueryError {
    fn code(&self) -> &'static str {
        "tasks-application-tag-query"
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        vec![
            ("query", self.query.as_str().into()),
            ("postfix", self.postfix.as_str().into()),
            ("infix", self.infix.as_str().into()),
            ("grammar", QUERY_GRAMMAR.into()),
        ]
    }
}

/// The ids of `candidates` (`(harp id, tags)`) that `query` selects under
/// `schema`, or why the query is malformed. An empty query is no query, and
/// selects every candidate.
pub fn select<'a>(
    query: &str,
    candidates: impl IntoIterator<Item = (&'a str, &'a [Tag])>,
    schema: &TagSchema,
) -> Result<BTreeSet<String>, QueryError> {
    let mut index = Index::new();
    index.register_type(SEMVER, Arc::new(Semver));
    let types: Vec<tagma_core::Tag> = schema
        .types()
        .map(|(target, name)| tagma_core::Tag {
            namespace: Some("tagma.type".to_string()),
            key: target.to_string(),
            value: Some(name.to_string()),
        })
        .collect();
    if !types.is_empty() {
        index.add_item(TYPE_CONFIG_ITEM, types);
    }
    let mut ids = BTreeSet::new();
    for (id, tags) in candidates {
        let mut tags: Vec<tagma_core::Tag> = tags.iter().map(Tag::to_core).collect();
        tags.push(presence());
        index.add_item(id, tags);
        ids.insert(id.to_string());
    }
    if query.trim().is_empty() {
        return Ok(ids);
    }
    let matched = match index.query_postfix(query) {
        Ok(matched) => matched,
        Err(postfix) => index.query(query).map_err(|infix| QueryError {
            query: query.to_string(),
            postfix,
            infix,
        })?,
    };
    Ok(matched.into_iter().filter(|id| ids.contains(id)).collect())
}

/// Refuse `query` if it is malformed, whatever it would be run over.
pub fn check(query: &str) -> Result<(), QueryError> {
    select(query, [], &TagSchema::default()).map(|_| ())
}

/// The tag every candidate carries, so that it takes part in the query.
fn presence() -> tagma_core::Tag {
    tagma_core::Tag {
        namespace: Some(PRESENCE_NAMESPACE.to_string()),
        key: "candidate".to_string(),
        value: None,
    }
}

#[cfg(test)]
#[path = "query.test.rs"]
mod tests;
