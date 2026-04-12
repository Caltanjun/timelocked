use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

#[test]
fn release_script_bumps_lockfile_and_pushes_tag() {
    if cfg!(windows) {
        return;
    }

    let remote_dir = TempDir::new().expect("create remote temp dir");
    let repo_dir = TempDir::new().expect("create repo temp dir");

    run(Command::new("git")
        .arg("init")
        .arg("--bare")
        .current_dir(remote_dir.path()));

    run(Command::new("git")
        .arg("init")
        .arg("--initial-branch=main")
        .current_dir(repo_dir.path()));
    run(Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_dir.path()));
    run(Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_dir.path()));

    write_fixture_manifest(repo_dir.path());
    fs::create_dir(repo_dir.path().join("src")).expect("create src dir");
    fs::write(
        repo_dir.path().join("src/main.rs"),
        "fn main() { println!(\"release fixture\"); }\n",
    )
    .expect("write main.rs");
    fs::write(repo_dir.path().join(".gitignore"), "/target\n").expect("write .gitignore");

    run(Command::new("cargo")
        .arg("generate-lockfile")
        .current_dir(repo_dir.path()));

    run(Command::new("git")
        .args(["remote", "add", "origin"])
        .arg(remote_dir.path())
        .current_dir(repo_dir.path()));
    run(Command::new("git")
        .args(["add", "."])
        .current_dir(repo_dir.path()));
    run(Command::new("git")
        .args(["commit", "-m", "chore: initial fixture"])
        .current_dir(repo_dir.path()));
    run(Command::new("git")
        .args(["push", "-u", "origin", "main"])
        .current_dir(repo_dir.path()));

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/release.sh");
    run(Command::new("bash")
        .arg(script)
        .arg("0.1.1")
        .current_dir(repo_dir.path()));

    let cargo_toml =
        fs::read_to_string(repo_dir.path().join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("version = \"0.1.1\""));

    let cargo_lock =
        fs::read_to_string(repo_dir.path().join("Cargo.lock")).expect("read Cargo.lock");
    assert!(cargo_lock.contains("name = \"release-fixture\""));
    assert!(cargo_lock.contains("version = \"0.1.1\""));

    let commit_subject = run(Command::new("git")
        .args(["log", "-1", "--pretty=%s"])
        .current_dir(repo_dir.path()));
    assert_eq!(commit_subject.trim(), "chore: prepare release 0.1.1");

    let local_tag = run(Command::new("git")
        .args(["tag", "--list", "v0.1.1"])
        .current_dir(repo_dir.path()));
    assert_eq!(local_tag.trim(), "v0.1.1");

    let remote_tag = run(Command::new("git")
        .args(["ls-remote", "--tags", "origin", "refs/tags/v0.1.1"])
        .current_dir(repo_dir.path()));
    assert!(remote_tag.contains("refs/tags/v0.1.1"));
}

fn write_fixture_manifest(repo_root: &Path) {
    fs::write(
        repo_root.join("Cargo.toml"),
        "[package]\nname = \"release-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
    )
    .expect("write Cargo.toml");
}

fn run(command: &mut Command) -> String {
    let output = command.output().expect("run command");

    if !output.status.success() {
        panic!(
            "command failed: status={}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    String::from_utf8(output.stdout).expect("stdout should be utf-8")
}
