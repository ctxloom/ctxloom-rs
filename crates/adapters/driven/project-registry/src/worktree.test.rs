//! Linked worktrees made by git itself.

use std::fs;

use super::primary_checkout;
use crate::tests::git;

#[test]
fn test_a_linked_worktree_names_its_primary_checkout() {
    let repo = git::Repo::new();
    let linked = repo.worktree("linked");
    assert_eq!(primary_checkout(&linked).unwrap(), Some(repo.root()));
}

#[test]
fn test_a_worktree_inside_a_subdirectory_names_its_primary_checkout() {
    let repo = git::Repo::new();
    let elsewhere = tempfile::tempdir().unwrap();
    let linked = repo.worktree_at(&elsewhere.path().join("deep/linked"));
    assert_eq!(primary_checkout(&linked).unwrap(), Some(repo.root()));
}

#[test]
fn test_the_primary_checkout_has_none() {
    let repo = git::Repo::new();
    assert_eq!(primary_checkout(&repo.root()).unwrap(), None);
}

#[test]
fn test_a_bare_repositorys_worktree_has_none() {
    let repo = git::Repo::bare();
    let linked = repo.worktree("linked");
    assert_eq!(primary_checkout(&linked).unwrap(), None);
}

#[test]
fn test_a_git_file_with_no_commondir_has_none() {
    // A submodule's `.git` names a git directory under its superproject's
    // `.git/modules`, with no commondir.
    let dir = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();
    let modules = root.join("super/.git/modules/sub");
    fs::create_dir_all(&modules).unwrap();
    fs::create_dir(root.join("sub")).unwrap();
    fs::write(root.join("sub/.git"), "gitdir: ../super/.git/modules/sub\n").unwrap();
    assert_eq!(primary_checkout(&root.join("sub")).unwrap(), None);
}

#[test]
fn test_a_git_file_that_is_not_a_gitdir_line_has_none() {
    let dir = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();
    fs::create_dir_all(root.join("real/.git/worktrees/x")).unwrap();
    fs::write(root.join("real/.git/worktrees/x/commondir"), "../..\n").unwrap();
    fs::create_dir(root.join("x")).unwrap();
    fs::write(root.join("x/.git"), "../real/.git/worktrees/x\n").unwrap();
    assert_eq!(primary_checkout(&root.join("x")).unwrap(), None);
    fs::write(root.join("x/.git"), "gitdir: ../real/.git/worktrees/x\n").unwrap();
    assert_eq!(
        primary_checkout(&root.join("x")).unwrap(),
        Some(root.join("real")),
        "the same with its prefix"
    );
}

#[test]
fn test_a_worktree_whose_git_directory_is_gone_is_an_error() {
    let repo = git::Repo::new();
    let linked = repo.worktree("linked");
    fs::remove_dir_all(repo.root()).unwrap();
    let error = primary_checkout(&linked).unwrap_err();
    assert!(
        matches!(&error, crate::RegistryError::StaleWorktree { root, .. } if *root == linked.display().to_string()),
        "{error:?}"
    );
}

#[test]
fn test_an_unreadable_git_file_is_an_error() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();
    fs::write(root.join(".git"), "gitdir: x\n").unwrap();
    fs::set_permissions(root.join(".git"), fs::Permissions::from_mode(0o000)).unwrap();
    let error = primary_checkout(&root);
    fs::set_permissions(root.join(".git"), fs::Permissions::from_mode(0o600)).unwrap();
    assert!(
        matches!(error, Err(crate::RegistryError::Io { .. })),
        "{error:?}"
    );
}
