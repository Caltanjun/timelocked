use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;
use timelocked::domains::timelocked_file::{
    encode_end_superblock_copy, encode_start_superblock_copy, parse_container,
    DEFAULT_LOCK_CHUNK_SIZE_BYTES,
};

#[test]
fn help_smoke() {
    let mut cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("lock"))
        .stdout(predicate::str::contains("unlock"))
        .stdout(predicate::str::contains("calibrate"))
        .stdout(predicate::str::contains("tui"));
}

#[test]
fn version_smoke() {
    let mut cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("timelocked"));
}

#[test]
fn calibrate_json_outputs_positive_rate() {
    let dir = tempdir().expect("tempdir");
    let mut cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    cmd.current_dir(dir.path()).args(["--json", "calibrate"]);

    let output = cmd.assert().success().get_output().stdout.clone();
    let parsed = output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty() && line.starts_with(b"{"))
        .map(|line| serde_json::from_slice::<Value>(line).expect("json line"))
        .find(|value| value["command"] == "calibrate")
        .expect("calibrate result");

    assert_eq!(parsed["command"], "calibrate");
    assert_eq!(parsed["hardwareProfile"], "current-machine");
    assert!(parsed["iterationsPerSecond"].as_u64().unwrap_or(0) > 0);
}

#[test]
fn lock_accepts_current_machine_profile() {
    let dir = tempdir().expect("tempdir");
    let input_path = dir.path().join("current-machine.txt");
    fs::write(&input_path, b"current machine profile").expect("write input");

    let mut lock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    lock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "--json",
            "lock",
            "--in",
            input_path.to_str().expect("input utf8"),
            "--target",
            "1s",
            "--hardware-profile",
            "current-machine",
        ]);

    let output = lock_cmd.assert().success().get_output().stdout.clone();
    let parsed = output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty() && line.starts_with(b"{"))
        .map(|line| serde_json::from_slice::<Value>(line).expect("json line"))
        .find(|value| value["command"] == "lock")
        .expect("lock result");

    assert_eq!(parsed["command"], "lock");
    assert_eq!(parsed["hardwareProfile"], "current-machine");
    assert!(parsed["iterations"].as_u64().unwrap_or(0) > 0);
}

#[test]
fn lock_inspect_verify_unlock_roundtrip_multichunk_file() {
    let dir = tempdir().expect("tempdir");
    let input_path = dir.path().join("note.txt");
    let output_path = dir.path().join("note.txt.timelocked");
    let out_dir = dir.path().join("out");
    fs::create_dir_all(&out_dir).expect("create output dir");

    let payload_len = DEFAULT_LOCK_CHUNK_SIZE_BYTES + 151_424;
    let mut payload = Vec::with_capacity(payload_len);
    for index in 0..payload_len {
        payload.push(b'a' + (index % 26) as u8);
    }
    fs::write(&input_path, &payload).expect("write input");

    let mut lock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    lock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "lock",
            "--in",
            input_path.to_str().expect("input path utf8"),
            "--iterations",
            "32",
        ]);
    lock_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("Timelocked file created"));

    assert!(output_path.exists());

    let mut inspect_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    inspect_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "inspect",
            "--in",
            output_path.to_str().expect("output path utf8"),
        ]);
    inspect_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("iterations"));

    let mut verify_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    verify_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "verify",
            "--in",
            output_path.to_str().expect("output path utf8"),
        ]);
    verify_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("Structural verification OK"))
        .stdout(predicate::str::contains(
            "Use unlock for full payload authentication and recovery.",
        ));

    let mut unlock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    unlock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "unlock",
            "--in",
            output_path.to_str().expect("output path utf8"),
            "--out-dir",
            out_dir.to_str().expect("out dir utf8"),
        ]);
    unlock_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("Unlock complete"));

    let unlocked_path = out_dir.join("note.txt");
    assert!(unlocked_path.exists());

    let original = fs::read(&input_path).expect("read original");
    let recovered = fs::read(&unlocked_path).expect("read recovered");
    assert_eq!(recovered, original);
    assert!(recovered.len() > DEFAULT_LOCK_CHUNK_SIZE_BYTES);
}

#[test]
fn lock_accepts_positional_file_input() {
    let dir = tempdir().expect("tempdir");
    let input_path = dir.path().join("story.txt");
    let output_path = dir.path().join("story.txt.timelocked");
    fs::write(&input_path, b"positional file input").expect("write input");

    let mut lock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    lock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "lock",
            input_path.to_str().expect("input path utf8"),
            "--iterations",
            "32",
        ]);
    lock_cmd.assert().success();

    assert!(output_path.exists());
}

#[test]
fn lock_unlock_roundtrip_text_input_from_first_arg() {
    let dir = tempdir().expect("tempdir");
    let text_input = "hello future self";
    let output_path = dir.path().join("message.timelocked");

    let mut lock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    lock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "lock",
            text_input,
            "--out",
            output_path.to_str().expect("output path utf8"),
            "--iterations",
            "32",
        ]);
    lock_cmd.assert().success();

    let mut unlock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    unlock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args(["unlock", output_path.to_str().expect("output path utf8")]);
    unlock_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("Recovered message:"))
        .stdout(predicate::str::contains(text_input));

    assert!(!dir.path().join("unlocked.bin").exists());
}

#[test]
fn unlock_text_message_rejects_out_override() {
    let dir = tempdir().expect("tempdir");
    let text_input = "display only message";
    let container_path = dir.path().join("message.timelocked");
    let forced_output = dir.path().join("forced-output.txt");

    let mut lock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    lock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "lock",
            text_input,
            "--out",
            container_path.to_str().expect("container path utf8"),
            "--iterations",
            "32",
        ]);
    lock_cmd.assert().success();

    let mut unlock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    unlock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "unlock",
            "--in",
            container_path.to_str().expect("container path utf8"),
            "--out",
            forced_output.to_str().expect("forced output utf8"),
        ]);
    unlock_cmd
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "invalid argument: output path options cannot be used when unlocking a text payload",
        ));

    assert!(!forced_output.exists());
}

#[test]
fn unlock_text_message_rejects_out_dir_override() {
    let dir = tempdir().expect("tempdir");
    let text_input = "display only message";
    let container_path = dir.path().join("message.timelocked");
    let forced_out_dir = dir.path().join("forced-out");

    let mut lock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    lock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "lock",
            text_input,
            "--out",
            container_path.to_str().expect("container path utf8"),
            "--iterations",
            "32",
        ]);
    lock_cmd.assert().success();

    let mut unlock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    unlock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "unlock",
            "--in",
            container_path.to_str().expect("container path utf8"),
            "--out-dir",
            forced_out_dir.to_str().expect("forced out dir utf8"),
        ]);
    unlock_cmd
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "invalid argument: output path options cannot be used when unlocking a text payload",
        ));

    assert!(!forced_out_dir.exists());
}

#[test]
fn text_input_requires_out() {
    let dir = tempdir().expect("tempdir");
    let mut cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    cmd.current_dir(dir.path())
        .args(["lock", "Hello future", "--iterations", "10"]);

    cmd.assert().failure().stderr(predicate::str::contains(
        "invalid argument: an explicit output path is required when locking stdin or text input",
    ));
}

#[test]
fn lock_appends_timelocked_extension_when_out_has_none() {
    let dir = tempdir().expect("tempdir");
    let explicit_out = dir.path().join("message");
    let expected_out = dir.path().join("message.timelocked");

    let mut lock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    lock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "lock",
            "Hello future",
            "--out",
            explicit_out.to_str().expect("output path utf8"),
            "--iterations",
            "10",
        ]);
    lock_cmd.assert().success();

    assert!(expected_out.exists());
    assert!(!explicit_out.exists());
}

#[test]
fn explicit_in_requires_existing_file() {
    let dir = tempdir().expect("tempdir");
    let output_path = dir.path().join("missing.timelocked");

    let mut cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    cmd.current_dir(dir.path()).args([
        "lock",
        "--in",
        "missing.txt",
        "--out",
        output_path.to_str().expect("output path utf8"),
        "--iterations",
        "10",
    ]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("input file does not exist"));
}

#[test]
fn verify_accepts_positional_input() {
    let dir = tempdir().expect("tempdir");
    let input_path = dir.path().join("payload.bin");
    let output_path = dir.path().join("payload.bin.timelocked");
    fs::write(&input_path, b"verify positional").expect("write input");

    let mut lock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    lock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "lock",
            "--in",
            input_path.to_str().expect("input utf8"),
            "--iterations",
            "24",
        ]);
    lock_cmd.assert().success();

    let mut verify_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    verify_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args(["verify", output_path.to_str().expect("output utf8")]);
    verify_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("Structural verification OK"));
}

#[test]
fn verify_stays_structural_when_wrapped_key_is_tampered() {
    let dir = tempdir().expect("tempdir");
    let input_path = dir.path().join("payload.bin");
    let output_path = dir.path().join("payload.bin.timelocked");
    fs::write(&input_path, b"verify stays structural").expect("write input");

    let mut lock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    lock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "lock",
            "--in",
            input_path.to_str().expect("input utf8"),
            "--iterations",
            "24",
        ]);
    lock_cmd.assert().success();

    let parsed = parse_container(&output_path).expect("parse artifact");
    let mut modified = parsed.superblock.clone();
    modified.timelock_material.wrapped_key[0] ^= 0xFF;
    replace_superblock_copies(&output_path, &parsed, &modified);

    let mut verify_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    verify_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args(["verify", "--in", output_path.to_str().expect("output utf8")]);
    verify_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("Structural verification OK"));

    let mut unlock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    unlock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args(["unlock", "--in", output_path.to_str().expect("output utf8")]);
    unlock_cmd
        .assert()
        .failure()
        .stderr(predicate::str::contains("payload authentication failed"));
}

#[test]
fn inspect_accepts_positional_input() {
    let dir = tempdir().expect("tempdir");
    let input_path = dir.path().join("inspect-me.txt");
    let output_path = dir.path().join("inspect-me.txt.timelocked");
    fs::write(&input_path, b"inspect positional").expect("write input");

    let mut lock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    lock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "lock",
            "--in",
            input_path.to_str().expect("input utf8"),
            "--iterations",
            "24",
        ]);
    lock_cmd.assert().success();

    let mut inspect_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    inspect_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args(["inspect", output_path.to_str().expect("output utf8")]);
    inspect_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("Delay params"));
}

#[test]
fn unlock_accepts_positional_input() {
    let dir = tempdir().expect("tempdir");
    let input_path = dir.path().join("unlock-me.txt");
    let output_path = dir.path().join("unlock-me.txt.timelocked");
    let out_dir = dir.path().join("out");
    fs::create_dir_all(&out_dir).expect("create output dir");
    fs::write(&input_path, b"unlock positional").expect("write input");

    let mut lock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    lock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "lock",
            "--in",
            input_path.to_str().expect("input utf8"),
            "--iterations",
            "24",
        ]);
    lock_cmd.assert().success();

    let mut unlock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    unlock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "unlock",
            output_path.to_str().expect("output utf8"),
            "--out-dir",
            out_dir.to_str().expect("out dir utf8"),
        ]);
    unlock_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("Unlock complete"));

    let recovered = fs::read(out_dir.join("unlock-me.txt")).expect("read recovered");
    assert_eq!(recovered, b"unlock positional");
}

#[test]
fn unlock_auto_suffixes_default_output_when_target_exists() {
    let dir = tempdir().expect("tempdir");
    let input_path = dir.path().join("letter.txt");
    let locked_path = dir.path().join("letter.txt.timelocked");
    fs::write(&input_path, b"letter content").expect("write input");

    let mut lock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    lock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "lock",
            "--in",
            input_path.to_str().expect("input utf8"),
            "--iterations",
            "24",
        ]);
    lock_cmd.assert().success();

    let mut unlock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    unlock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args(["unlock", "--in", locked_path.to_str().expect("locked utf8")]);
    unlock_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("letter.1.txt"));

    let recovered = fs::read(dir.path().join("letter.1.txt")).expect("read recovered");
    assert_eq!(recovered, b"letter content");
}

#[test]
fn unlock_auto_suffixes_explicit_out_when_target_exists() {
    let dir = tempdir().expect("tempdir");
    let input_path = dir.path().join("report.txt");
    let locked_path = dir.path().join("report.txt.timelocked");
    let explicit_out = dir.path().join("custom.txt");
    fs::write(&input_path, b"report content").expect("write input");
    fs::write(&explicit_out, b"existing output").expect("write existing output");

    let mut lock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    lock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "lock",
            "--in",
            input_path.to_str().expect("input utf8"),
            "--iterations",
            "24",
        ]);
    lock_cmd.assert().success();

    let mut unlock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    unlock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "unlock",
            "--in",
            locked_path.to_str().expect("locked utf8"),
            "--out",
            explicit_out.to_str().expect("explicit out utf8"),
        ]);
    unlock_cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("custom.1.txt"));

    let recovered = fs::read(dir.path().join("custom.1.txt")).expect("read recovered");
    assert_eq!(recovered, b"report content");
}

#[test]
fn stdin_lock_requires_out() {
    let dir = tempdir().expect("tempdir");
    let mut cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    cmd.current_dir(dir.path())
        .args(["lock", "--in", "-", "--iterations", "10"])
        .write_stdin("hello");

    cmd.assert().failure().stderr(predicate::str::contains(
        "invalid argument: an explicit output path is required when locking stdin or text input",
    ));
}

#[test]
fn verify_rejects_removed_deep_flag() {
    let dir = tempdir().expect("tempdir");
    let mut verify_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    verify_cmd
        .current_dir(dir.path())
        .args(["verify", "--in", "example.timelocked", "--deep"]);

    verify_cmd
        .assert()
        .failure()
        .stderr(predicate::str::contains("unexpected argument '--deep'"));
}

#[test]
fn unlock_fails_for_tampered_payload() {
    let dir = tempdir().expect("tempdir");
    let input_path = dir.path().join("payload.bin");
    let output_path = dir.path().join("payload.bin.timelocked");
    fs::write(&input_path, b"abcdef1234567890").expect("write input");

    let mut lock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    lock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args([
            "lock",
            "--in",
            input_path.to_str().expect("input utf8"),
            "--iterations",
            "24",
        ]);
    lock_cmd.assert().success();

    let parsed = parse_container(&output_path).expect("parse artifact");
    let shard_len = parsed.superblock.rs_shard_bytes as usize;
    let first_record_offset = parsed.payload_region_offset;
    let first_shard_offset = first_record_offset + 4;

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&output_path)
        .expect("open output");
    file.seek(SeekFrom::Start(first_shard_offset))
        .expect("seek shard");
    let mut shard = vec![0_u8; shard_len];
    file.read_exact(&mut shard).expect("read shard");
    shard[8 + 1 + 4 + 4 + 24] ^= 0xFF;
    let crc = crc32c::crc32c(&shard);
    file.seek(SeekFrom::Start(first_record_offset))
        .expect("seek record");
    file.write_all(&crc.to_le_bytes()).expect("write crc");
    file.write_all(&shard).expect("write shard");

    let mut unlock_cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("timelocked"));
    unlock_cmd
        .env("MODULUS_BITS", "256")
        .current_dir(dir.path())
        .args(["unlock", "--in", output_path.to_str().expect("output utf8")]);
    unlock_cmd
        .assert()
        .failure()
        .stderr(predicate::str::contains("payload authentication failed"));
}

fn replace_superblock_copies(
    path: &std::path::Path,
    parsed: &timelocked::domains::timelocked_file::ParsedContainer,
    superblock: &timelocked::domains::timelocked_file::SuperblockBody,
) {
    let start_copy = encode_start_superblock_copy(superblock).expect("encode start");
    let end_copy = encode_end_superblock_copy(superblock).expect("encode end");
    overwrite_bytes(path, parsed.start_superblock_offset, &start_copy);
    overwrite_bytes(path, parsed.end_superblock_offset, &end_copy);
}

fn overwrite_bytes(path: &std::path::Path, offset: u64, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .expect("open file");
    file.seek(SeekFrom::Start(offset)).expect("seek");
    file.write_all(bytes).expect("write");
}
