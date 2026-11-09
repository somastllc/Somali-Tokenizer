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
