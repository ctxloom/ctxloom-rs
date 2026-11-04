use ctxloom_ports::{Change, Put};

use super::*;

fn plan() -> Plan {
    let mut files = Files::new();
    files.insert("bundle.yaml".to_string(), b"b".to_vec());
    files.insert("fragments/a.md".to_string(), b"A".to_vec());
    files.insert("skills/s/SKILL.md".to_string(), b"S".to_vec());
    files.insert("skills/s/run.sh".to_string(), b"R".to_vec());
    let exec = BTreeSet::from(["skills/s/run.sh".to_string()]);
    Plan::of(files, exec)
}

#[test]
fn test_a_new_plan_has_changed_nothing() {
    let p = plan();
    assert!(p.is_unchanged());
    assert!(p.into_changes().is_empty());
}

#[test]
fn test_a_write_is_in_the_tree_and_in_the_changes() {
    let mut p = plan();
    p.write("fragments/b.md", "B");
    assert_eq!(p.text("fragments/b.md").as_deref(), Some("B"));
    assert!(p.has("fragments/b.md") && !p.has("fragments/c.md"));
    assert!(!p.is_executable("fragments/b.md"));
    assert!(!p.is_unchanged());
    assert_eq!(
        p.into_changes(),
        [Change::Write(Put::file("fragments/b.md", "B"))]
    );
}

#[test]
fn test_a_write_that_restates_a_file_is_no_change() {
    let mut p = plan();
    p.write("fragments/a.md", "A");
    p.put("skills/s/run.sh", b"R".to_vec(), true);
    assert!(p.is_unchanged());
    // The same bytes with the exec bit changed is a change.
    p.put("skills/s/run.sh", b"R".to_vec(), false);
    assert!(!p.is_executable("skills/s/run.sh"));
    assert_eq!(p.into_changes().len(), 1);
    let mut q = plan();
    q.put("fragments/a.md", b"A".to_vec(), true);
    assert!(q.is_executable("fragments/a.md"));
    assert_eq!(q.into_changes().len(), 1);
}

#[test]
fn test_a_remove_says_whether_anything_was_there() {
    let mut p = plan();
    assert!(!p.remove("nope"));
    assert!(p.is_unchanged());
    assert!(p.remove("fragments/a.md"));
    assert!(!p.has("fragments/a.md"));
    assert!(!p.remove("fragments/a.md"));
    assert_eq!(
        p.into_changes(),
        [Change::Remove {
            path: "fragments/a.md".to_string()
        }]
    );
    let mut q = plan();
    assert!(q.remove("skills/s/run.sh"));
    assert!(!q.is_executable("skills/s/run.sh"));
}

#[test]
fn test_a_directory_goes_with_everything_in_it_and_only_it() {
    let mut p = plan();
    assert!(!p.remove_dir("skills/nope"));
    assert!(!p.remove_dir("skill"), "a name that only starts like it");
    assert!(p.is_unchanged());
    assert_eq!(
        p.below("skills/s"),
        ["skills/s/SKILL.md", "skills/s/run.sh"]
    );
    assert!(p.remove_dir("skills/s"));
    assert!(p.below("skills/s").is_empty());
    assert!(!p.is_executable("skills/s/run.sh"));
    assert!(p.has("fragments/a.md") && p.has("bundle.yaml"));
    assert_eq!(
        p.into_changes(),
        [Change::RemoveDir {
            path: "skills/s".to_string()
        }]
    );
}

#[test]
fn test_the_files_are_the_tree_as_it_will_be() {
    let mut p = plan();
    p.write("x", "1");
    p.remove("bundle.yaml");
    let paths: Vec<&str> = p.files().keys().map(String::as_str).collect();
    assert_eq!(
        paths,
        [
            "fragments/a.md",
            "skills/s/SKILL.md",
            "skills/s/run.sh",
            "x"
        ]
    );
}
