//! End-to-end checks of the command-line binary.

#![allow(clippy::unwrap_used, clippy::panic)]

use std::io::Write;
use std::process::{Command, Stdio};

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_somali-tokenizer"))
}

fn run(args: &[&str], stdin: Option<&str>) -> (bool, String, String) {
    let mut child = cli()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    } else {
        drop(child.stdin.take());
    }
    let output = child.wait_with_output().unwrap();
    (
        output.status.success(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn encode_then_decode_roundtrips() {
    let (ok, ids, _) = run(&["encode", "Soomaaliya waa dal."], None);
    assert!(ok);
    let ids: Vec<&str> = ids.split_whitespace().collect();
    let mut args = vec!["decode"];
    args.extend(ids);
    let (ok, text, _) = run(&args, None);
    assert!(ok);
    assert_eq!(text, "Soomaaliya waa dal.\n");
}

#[test]
fn encode_reads_stdin_line_by_line() {
    let (ok, out, _) = run(
        &["encode", "--format", "json"],
        Some("Nabad\nGeeska Afrika\n"),
    );
    assert!(ok);
    let lines: Vec<serde_json::Value> = out
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[1]["tokens"].as_array().unwrap().len() >= 2);
}

#[test]
fn inspect_lists_special_tokens() {
    let (ok, out, _) = run(&["inspect"], None);
    assert!(ok);
    assert!(out.contains("vocab_size: 48000"));
    assert!(out.contains("<|im_start|>"));
}

#[test]
fn a_bad_id_fails_with_a_message() {
    let (ok, _, err) = run(&["decode", "99999999"], None);
    assert!(!ok);
    assert!(err.contains("outside the vocabulary"));
}

fn fixture(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
        .display()
        .to_string()
}

fn scratch_dir(name: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn train_writes_an_artifact_and_a_run_record() {
    let dir = scratch_dir("cli-train");
    let output = dir.join("tok.json");
    let output_arg = output.display().to_string();
    let corpus = fixture("mini_release.jsonl");
    let args = [
        "train",
        "--corpus",
        &corpus,
        "--vocab-size",
        "600",
        "--quiet",
        "--output",
        &output_arg,
    ];
    let (ok, _, err) = run(&args, None);
    assert!(ok, "{err}");
    let record: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("tok.run.json")).unwrap()).unwrap();
    assert_eq!(record["vocab_size"], 600);
    assert_eq!(record["split"], "train");

    let (ok, _, err) = run(&args, None);
    assert!(!ok, "a second run must not overwrite without --force");
    assert!(err.contains("--force"));
}

#[test]
fn train_sweep_derives_candidates() {
    let dir = scratch_dir("cli-sweep");
    let sweep_dir = dir.display().to_string();
    let source = fixture("parity_train_2000.json");
    let args = [
        "train",
        "--derive-from",
        &source,
        "--sweep",
        "500,1000",
        "--sweep-dir",
        &sweep_dir,
    ];
    let (ok, _, err) = run(&args, None);
    assert!(ok, "{err}");
    assert!(dir.join("somali-bpe-v2-500.json").is_file());
    assert!(dir.join("somali-bpe-v2-1000.json").is_file());
}

#[test]
fn train_refuses_to_write_into_releases() {
    let corpus = fixture("mini_release.jsonl");
    let args = [
        "train",
        "--corpus",
        &corpus,
        "--output",
        "artifacts/releases/v9/tokenizer.json",
    ];
    let (ok, _, err) = run(&args, None);
    assert!(!ok);
    assert!(err.contains("immutable"));
}

#[test]
fn benchmark_and_stats_write_reports() {
    let dir = scratch_dir("cli-reports");
    let bench = dir.join("bench.json").display().to_string();
    let corpus = fixture("mini_release.jsonl");
    let (ok, table, err) = run(&["benchmark", "--eval", &corpus, "--output", &bench], None);
    assert!(ok, "{err}");
    assert!(table.contains("v2-bytelevel"));
    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&bench).unwrap()).unwrap();
    assert_eq!(report["documents_scored"], 3);

    let stats = dir.join("stats.json").display().to_string();
    let (ok, _, err) = run(&["stats", "--corpus", &corpus, "--output", &stats], None);
    assert!(ok, "{err}");
    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&stats).unwrap()).unwrap();
    assert_eq!(report["tokenizers"]["v2"]["documents"], 213);
}
