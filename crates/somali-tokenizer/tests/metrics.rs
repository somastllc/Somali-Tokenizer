//! Benchmark, corpus statistics, and the holdout rule against the Python reference.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use serde_json::Value;
use somali_tokenizer::benchmark::{self, BenchmarkOptions};
use somali_tokenizer::corpus::{is_eval_doc, JsonlDocs, Split};
use somali_tokenizer::stats::{self, StatsOptions};

fn reference_tokenizers() -> Vec<(String, tokenizers::Tokenizer)> {
    ["v1", "v2"]
        .iter()
        .map(|v| {
            let tokenizer = benchmark::load_artifact(common::release(v)).unwrap();
            ((*v).to_owned(), tokenizer)
        })
        .collect()
}

fn read_json(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(common::fixture(name)).unwrap()).unwrap()
}

/// Compare two JSON values; numbers may differ by `tolerance`, everything else exactly.
fn assert_close(actual: &Value, expected: &Value, tolerance: f64, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!((a - b).abs() <= tolerance, "{path}: {a} != {b}");
        }
        (Value::Object(a), Value::Object(b)) => {
            let a_keys: Vec<&String> = a.keys().collect();
            let b_keys: Vec<&String> = b.keys().collect();
            assert_eq!(a_keys, b_keys, "{path}: keys or key order differ");
            for (key, value) in b {
                assert_close(&a[key], value, tolerance, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn holdout_rule_matches_the_python_reference() {
    let expected = read_json("parity_eval_split.json");
    let probes = expected["probes"].as_u64().unwrap();
    let held_out: Vec<String> = (0..probes)
        .map(|i| format!("doc:{i}"))
        .filter(|id| is_eval_doc(id))
        .collect();
    let expected: Vec<String> = serde_json::from_value(expected["held_out"].clone()).unwrap();
    assert_eq!(held_out, expected);
}

#[test]
fn benchmark_matches_the_python_reference() {
    let path = common::repo_root().join("python-reference/tests/fixtures/mini_corpus.jsonl");
    let docs = JsonlDocs::open(path, Split::All).unwrap();
    let report =
        benchmark::run(&reference_tokenizers(), docs, &BenchmarkOptions::default()).unwrap();
    let mut expected = read_json("parity_benchmark.json");
    // Network baselines (BERT, XLM-R) are a Python-only extra; with --no-baseline it is {}.
    assert_eq!(
        expected.as_object_mut().unwrap().remove("baselines"),
        Some(Value::Object(Default::default()))
    );
    // Means are summed in a different order than Python's exact `statistics.mean`.
    assert_close(&report, &expected, 1e-4, "benchmark");
}

#[test]
fn stats_match_the_python_reference() {
    let docs = JsonlDocs::open(common::fixture("mini_release.jsonl"), Split::All).unwrap();
    let options = StatsOptions {
        check_roundtrip: true,
    };
    let mut actual = stats::run(&reference_tokenizers(), docs, &options).unwrap();
    let mut expected = read_json("parity_stats.json")["tokenizers"].clone();
    // Python samples the round-trip per worker shard, so how many documents get checked
    // depends on sharding; compare the outcome, not the count.
    for report in [&mut actual, &mut expected] {
        assert_eq!(report["v2"]["roundtrip_failures"], 0);
        assert!(
            report["v1"]["roundtrip_failures"].as_u64().unwrap() > 0,
            "v1 is lossy"
        );
        for summary in report.as_object_mut().unwrap().values_mut() {
            let summary = summary.as_object_mut().unwrap();
            for key in [
                "roundtrip_checked",
                "roundtrip_failures",
                "roundtrip_fidelity",
            ] {
                summary.remove(key);
            }
        }
    }
    assert_close(&actual, &expected, 0.0, "stats");
}

#[test]
fn benchmark_on_a_release_corpus_scores_only_the_holdout() {
    let docs = JsonlDocs::open(common::fixture("mini_release.jsonl"), Split::Eval).unwrap();
    let held_out = JsonlDocs::open(common::fixture("mini_release.jsonl"), Split::Eval)
        .unwrap()
        .count();
    let report =
        benchmark::run(&reference_tokenizers(), docs, &BenchmarkOptions::default()).unwrap();
    assert_eq!(report["documents_scored"], held_out);
    assert_eq!(held_out, 3);
}

#[test]
fn an_empty_evaluation_set_is_an_error() {
    let docs = JsonlDocs::new(std::io::Cursor::new(""), "empty.jsonl", Split::All);
    assert!(benchmark::run(&reference_tokenizers(), docs, &BenchmarkOptions::default()).is_err());
}

#[test]
fn malformed_lines_are_skipped_and_counted() {
    let input = "{\"text\": \"Nabad\"}\nnot json\n{\"id\": \"x\"}\n\n{\"text\": \"  \"}\n";
    let mut docs = JsonlDocs::new(std::io::Cursor::new(input), "in.jsonl", Split::All);
    let read: Vec<_> = docs.by_ref().map(Result::unwrap).collect();
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].source, "unknown");
    assert_eq!(read[0].id, "unknown:1");
    assert_eq!(docs.skipped(), 2);
}
