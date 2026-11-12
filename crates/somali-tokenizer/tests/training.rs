//! Training must reproduce the Python reference exactly, and derivation must equal training.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use serde_json::Value;
use somali_tokenizer::training::{self, TrainConfig, TrainingSource};
use somali_tokenizer::Error;

fn mini_release() -> TrainingSource {
    TrainingSource::Jsonl {
        path: common::fixture("mini_release.jsonl"),
        holdout: true,
        limit: None,
    }
}

fn config(vocab_size: usize) -> TrainConfig {
    TrainConfig {
        vocab_size,
        ..TrainConfig::default()
    }
}

fn model_parts(artifact: &Value) -> (Value, Value, Value) {
    (
        artifact["model"]["vocab"].clone(),
        artifact["model"]["merges"].clone(),
        artifact["added_tokens"].clone(),
    )
}

#[test]
fn training_matches_the_python_reference() {
    let (tokenizer, record) = training::train_from(&config(2_000), &mini_release()).unwrap();
    let trained: Value = serde_json::from_str(&tokenizer.to_json(false).unwrap()).unwrap();
    let expected: Value = serde_json::from_str(
        &std::fs::read_to_string(common::fixture("parity_train_2000.json")).unwrap(),
    )
    .unwrap();

    assert_eq!(model_parts(&trained), model_parts(&expected));
    assert_eq!(trained["pre_tokenizer"], expected["pre_tokenizer"]);
    assert_eq!(trained["decoder"], expected["decoder"]);
    assert_eq!(trained["post_processor"], expected["post_processor"]);
    assert_eq!(trained["normalizer"], expected["normalizer"]);
    assert_eq!(record.vocab_size, 2_000);
    // The fixture holds 210 training documents plus 3 the holdout rule must exclude.
    assert_eq!(record.documents, 210, "holdout not applied");
    assert!(tokenizer.token_to_id("Ġxqzzv").is_none());
}

#[test]
fn training_is_deterministic() {
    let first = training::train_from(&config(1_000), &mini_release())
        .unwrap()
        .0;
    let second = training::train_from(&config(1_000), &mini_release())
        .unwrap()
        .0;
    assert_eq!(
        first.to_json(false).unwrap(),
        second.to_json(false).unwrap()
    );
}

#[test]
fn derivation_equals_training_at_the_smaller_size() {
    let large: Value = serde_json::from_str(
        &std::fs::read_to_string(common::fixture("parity_train_2000.json")).unwrap(),
    )
    .unwrap();
    let derived = training::derive(&large, 1_000).unwrap();
    let trained = training::train_from(&config(1_000), &mini_release())
        .unwrap()
        .0;
    let trained: Value = serde_json::from_str(&trained.to_json(false).unwrap()).unwrap();
    assert_eq!(model_parts(&derived), model_parts(&trained));
}

#[test]
fn derivation_rejects_impossible_sizes() {
    let large: Value = serde_json::from_str(
        &std::fs::read_to_string(common::fixture("parity_train_2000.json")).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        training::derive(&large, 3_000),
        Err(Error::Contract { .. })
    ));
    assert!(matches!(
        training::derive(&large, 272),
        Err(Error::Contract { .. })
    ));
}

#[test]
fn a_vocabulary_without_room_for_merges_is_rejected() {
    let result = training::train_from(&config(272), &mini_release());
    assert!(matches!(result, Err(Error::Contract { .. })));
}

#[test]
fn a_trained_tokenizer_roundtrips_and_has_no_unknown_token() {
    let tokenizer = training::train_from(&config(1_000), &mini_release())
        .unwrap()
        .0;
    for text in ["Soomaaliya waa dal.", "emoji 🙂 中文 ᚠᚢᚦ", "  a\tb\n\nc  "] {
        let ids = tokenizer.encode(text).unwrap().ids;
        assert_eq!(tokenizer.decode(&ids, false).unwrap(), text);
    }
    assert!(tokenizer.token_to_id("<unk>").is_none());
}

#[test]
fn the_pinned_tokenizers_version_is_the_one_built() {
    let lock = std::fs::read_to_string(common::repo_root().join("Cargo.lock")).unwrap();
    let pinned = format!(
        "name = \"tokenizers\"\nversion = \"{}\"",
        training::TOKENIZERS_VERSION
    );
    assert!(lock.contains(&pinned), "Cargo.lock does not pin {pinned}");
}
