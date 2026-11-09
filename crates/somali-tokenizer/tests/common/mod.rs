//! Shared helpers for the integration tests.

#![allow(dead_code, clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

/// Repository root (two levels above this crate).
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn fixture(name: &str) -> PathBuf {
    repo_root().join("tests/fixtures").join(name)
}

pub fn release(version: &str) -> PathBuf {
    repo_root()
        .join("artifacts/releases")
        .join(version)
        .join("tokenizer.json")
}

/// Lines of a JSONL file, parsed.
pub fn read_jsonl(path: &PathBuf) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
