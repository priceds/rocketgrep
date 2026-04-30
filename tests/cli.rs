use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn fixed_string_search_finds_expected_line() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("main.rs"),
        "fn main() {\n    println!(\"needle\");\n}\n",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("rocketgrep").unwrap();
    cmd.arg("-F")
        .arg("needle")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("println!"));
}

#[test]
fn json_mode_emits_match_objects_with_scores() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("lib.rs"), "pub fn needle() {}\n").unwrap();

    let mut cmd = Command::cargo_bin("rocketgrep").unwrap();
    cmd.arg("--json")
        .arg("-F")
        .arg("needle")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""type":"match""#))
        .stdout(predicate::str::contains(r#""line_number":1"#))
        .stdout(predicate::str::contains(r#""edit_distance":0"#));
}

#[test]
fn gitignore_files_are_respected() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".gitignore"), "ignored.txt\n").unwrap();
    fs::write(dir.path().join("ignored.txt"), "needle\n").unwrap();
    fs::write(dir.path().join("kept.txt"), "needle\n").unwrap();

    let mut cmd = Command::cargo_bin("rocketgrep").unwrap();
    cmd.arg("-F")
        .arg("needle")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("kept.txt"))
        .stdout(predicate::str::contains("ignored.txt").not());
}

#[test]
fn approximate_search_finds_one_error() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("main.rs"), "let name = \"nexdle\";\n").unwrap();

    let mut cmd = Command::cargo_bin("rocketgrep").unwrap();
    cmd.arg("-k")
        .arg("1")
        .arg("--scores")
        .arg("needle")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("nexdle"))
        .stdout(predicate::str::contains("d=1"));
}

#[test]
fn index_command_builds_and_search_uses_index() {
    let dir = TempDir::new().unwrap();
    let index_path = dir.path().join("rocket.index.json");
    fs::write(dir.path().join("a.txt"), "needle\n").unwrap();
    fs::write(dir.path().join("b.txt"), "haystack\n").unwrap();

    Command::cargo_bin("rocketgrep")
        .unwrap()
        .arg("index")
        .arg("-o")
        .arg(&index_path)
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Indexed"));

    Command::cargo_bin("rocketgrep")
        .unwrap()
        .arg("-F")
        .arg("--index")
        .arg(&index_path)
        .arg("needle")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("a.txt"))
        .stdout(predicate::str::contains("b.txt").not());
}

#[test]
fn stale_index_does_not_hide_changed_files() {
    let dir = TempDir::new().unwrap();
    let index_path = dir.path().join("rocket.index.json");
    let changed_path = dir.path().join("changed.txt");
    fs::write(&changed_path, "nothing\n").unwrap();

    Command::cargo_bin("rocketgrep")
        .unwrap()
        .arg("index")
        .arg("-o")
        .arg(&index_path)
        .arg(dir.path())
        .assert()
        .success();

    fs::write(&changed_path, "needle appears after indexing\n").unwrap();

    Command::cargo_bin("rocketgrep")
        .unwrap()
        .arg("-F")
        .arg("--index")
        .arg(&index_path)
        .arg("needle")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("changed.txt"));
}
