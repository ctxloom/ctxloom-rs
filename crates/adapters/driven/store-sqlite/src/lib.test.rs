//! The SQLite store held to annals' store contract, the same checks
//! InMemoryStore passes, so the kernel behaves alike over either; then what
//! is particular to a store on a file.

use std::sync::atomic::{AtomicUsize, Ordering};

use annals::next::contract::{cover, page};
use annals::next::v1::Snapshot;
use annals::next::{AsyncEventStore, AsyncStore, AsyncUnitOfWork};

use super::{diagnostic_samples, SqliteError, SqliteStore};

annals::async_store_contract!(#[tokio::test], SqliteStore::open_in_memory().await.unwrap());

/// A path for a database file of this test run's own.
fn database() -> std::path::PathBuf {
    static RUN: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "store-sqlite-test-{}-{}.db",
        std::process::id(),
        RUN.fetch_add(1, Ordering::Relaxed)
    ))
}

#[tokio::test]
async fn test_what_was_committed_survives_reopening_the_file() {
    let path = database();
    let stream = cover("tasks", 1);
    let snapshot = Snapshot {
        sequence: 1,
        state: None,
        rules_version: "v1".to_string(),
    };
    {
        let mut store = SqliteStore::open(&path).await.unwrap();
        let mut unit = store.begin().await;
        unit.append(&stream, 0, &[page(0, 1), page(1, 2)])
            .await
            .unwrap();
        unit.commit().await.unwrap();
        store.write_snapshot(&stream, &snapshot).await.unwrap();
    }
    let mut store = SqliteStore::open(&path).await.unwrap();
    assert_eq!(
        store.read_from(&stream, 0).await.unwrap().unwrap().pages,
        [page(0, 1), page(1, 2)]
    );
    assert_eq!(store.read_snapshot(&stream).await, Ok(Some(snapshot)));
    assert_eq!(store.covers("tasks").await, Ok(vec![stream]));
    drop(store);
    std::fs::remove_file(&path).unwrap();
}

#[tokio::test]
async fn test_a_dropped_unit_leaves_the_file_as_it_was() {
    let path = database();
    let stream = cover("tasks", 1);
    {
        let mut store = SqliteStore::open(&path).await.unwrap();
        let mut unit = store.begin().await;
        unit.append(&stream, 0, &[page(0, 1)]).await.unwrap();
    }
    let mut store = SqliteStore::open(&path).await.unwrap();
    assert_eq!(store.read_from(&stream, 0).await, Ok(None));
    drop(store);
    std::fs::remove_file(&path).unwrap();
}

#[tokio::test]
async fn test_reopening_a_migrated_file_migrates_nothing_twice() {
    let path = database();
    drop(SqliteStore::open(&path).await.unwrap());
    let mut store = SqliteStore::open(&path).await.unwrap();
    assert_eq!(store.covers("tasks").await, Ok(vec![]));
    drop(store);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn test_an_error_renders_its_english_fallback_with_its_detail() {
    assert_eq!(
        SqliteError::Unavailable {
            detail: "disk I/O error".to_string()
        }
        .to_string(),
        "SQLite failed: disk I/O error"
    );
    assert_eq!(
        SqliteError::Undecodable {
            detail: "buffer underflow".to_string()
        }
        .to_string(),
        "a stored page does not decode: buffer underflow"
    );
}

#[test]
fn test_every_error_has_a_catalog_message_with_exactly_its_args() {
    use fluent_syntax::ast::{Entry, Expression, InlineExpression, PatternElement};

    let ftl = include_str!("../locales/en-US/store-sqlite.ftl");
    let resource = fluent_syntax::parser::parse(ftl).expect("the catalog parses");
    let samples = diagnostic_samples();
    // Both ways: every catalog message has a sample, and every sample a
    // message, so a missing or empty sample list fails too.
    let mut catalog: Vec<&str> = resource
        .body
        .iter()
        .filter_map(|e| match e {
            Entry::Message(m) => Some(m.id.name),
            _ => None,
        })
        .collect();
    catalog.sort_unstable();
    let mut sampled: Vec<&str> = samples.iter().map(|s| s.code()).collect();
    sampled.sort_unstable();
    assert_eq!(catalog, sampled, "catalog messages and samples");
    for sample in samples {
        let message = resource
            .body
            .iter()
            .find_map(|e| match e {
                Entry::Message(m) if m.id.name == sample.code() => Some(m),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no message for {}", sample.code()));
        let mut used: Vec<&str> = message
            .value
            .iter()
            .flat_map(|p| &p.elements)
            .filter_map(|el| match el {
                PatternElement::Placeable {
                    expression: Expression::Inline(InlineExpression::VariableReference { id }),
                } => Some(id.name),
                _ => None,
            })
            .collect();
        used.sort_unstable();
        let mut args: Vec<&str> = sample.args().iter().map(|(n, _)| *n).collect();
        args.sort_unstable();
        assert_eq!(used, args, "{}", sample.code());
    }
}
