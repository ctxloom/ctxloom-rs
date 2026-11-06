//! The helper every "with input" step writes a payload through. A command
//! that never reads its stdin (`auth set` is no command, so clap refuses it
//! before a byte is read) can have exited before the write: the pipe's read
//! end is then closed and the write is EPIPE. Rust ignores SIGPIPE, so that
//! is an error value, and a step that `expect`ed the write turned a command
//! that was right to refuse into a panic, whenever the exit won the race.
//! Here the exit is forced to win, by waiting for it before the write.

use std::process::{Command, Stdio};

use super::{binary, feed};

#[test]
fn test_feed_tolerates_a_child_that_exited_without_reading() {
    let mut child = Command::new(binary())
        .args(["version"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("ctxloom runs");
    // `wait` closes the child's stdin, so the write end is taken first.
    let stdin = child.stdin.take().expect("stdin is piped");
    // The child has exited, never having read: the interleaving, forced.
    child.wait().expect("ctxloom finishes");
    let consumed = feed(stdin, "sk-ant-fixture\n");
    assert!(!consumed, "a child that exited cannot have consumed it");
}

#[test]
fn test_feed_reports_a_payload_the_child_read() {
    let mut child = Command::new(binary())
        .args(["hook", "hud"])
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("ctxloom runs");
    let stdin = child.stdin.take().expect("stdin is piped");
    assert!(feed(stdin, "{}"), "a reader takes the payload");
    child.wait().expect("ctxloom finishes");
}
