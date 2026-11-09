//! Artifact loading and the v2 contract.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use serde_json::Value;
use somali_tokenizer::contract::SPECIAL_TOKENS;
use somali_tokenizer::{Error, SomaliTokenizer};

fn v2_json() -> Value {
    serde_json::from_str(&std::fs::read_to_string(common::release("v2")).unwrap()).unwrap()
}

fn assert_contract_error(artifact: &Value, expected_field: &str) {
    match SomaliTokenizer::from_json(&artifact.to_string()) {
        Err(Error::Contract { field, .. }) => assert_eq!(field, expected_field),
        other => panic!("expected a contract error on {expected_field}, got {other:?}"),
    }
}

#[test]
fn bundled_artifact_is_the_release_artifact() {
    let bundled = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/somali-bpe-v2.json"),
    )
    .unwrap();
    let release = std::fs::read(common::release("v2")).unwrap();
    assert!(
        bundled == release,
        "assets/somali-bpe-v2.json differs from the v2 release"
    );
}

#[test]
fn release_v2_loads_from_file() {
    let tokenizer = SomaliTokenizer::from_file(common::release("v2")).unwrap();
    assert_eq!(tokenizer.vocab_size(), 48_000);
    for (id, token) in SPECIAL_TOKENS.iter().enumerate() {
        assert_eq!(tokenizer.token_to_id(token), Some(id as u32));
    }
}

#[test]
fn v1_is_rejected() {
    match SomaliTokenizer::from_file(common::release("v1")) {
        Err(Error::Contract { .. }) => {}
        other => panic!("v1 must fail the v2 contract, got {other:?}"),
    }
}

#[test]
fn prefix_space_on_the_pre_tokenizer_is_rejected() {
    let mut artifact = v2_json();
    artifact["pre_tokenizer"]["add_prefix_space"] = Value::Bool(true);
    assert_contract_error(&artifact, "pre_tokenizer.add_prefix_space");
}

#[test]
fn an_unknown_token_is_rejected() {
    let mut artifact = v2_json();
    artifact["model"]["unk_token"] = Value::from("<unk>");
    assert_contract_error(&artifact, "model.unk_token");
}

#[test]
fn a_missing_normalizer_is_rejected() {
    let mut artifact = v2_json();
    artifact["normalizer"] = Value::Null;
    assert_contract_error(&artifact, "normalizer.type");
}

#[test]
fn a_moved_special_token_is_rejected() {
    let mut artifact = v2_json();
    artifact["added_tokens"][2]["id"] = Value::from(40_000);
    assert_contract_error(&artifact, "added_tokens");
}

#[test]
fn a_missing_byte_symbol_is_rejected() {
    let mut artifact = v2_json();
    artifact["model"]["vocab"]
        .as_object_mut()
        .unwrap()
        .remove("Ġ");
    assert_contract_error(&artifact, "model.vocab");
}

#[test]
fn malformed_json_is_an_error_not_a_panic() {
    assert!(matches!(
        SomaliTokenizer::from_json("{\"model\":"),
        Err(Error::Json(_))
    ));
    assert!(matches!(
        SomaliTokenizer::from_json("[]"),
        Err(Error::Contract { .. })
    ));
}

#[test]
fn a_missing_file_is_an_io_error() {
    assert!(matches!(
        SomaliTokenizer::from_file("does/not/exist.json"),
        Err(Error::Io { .. })
    ));
}

#[test]
fn decoding_an_out_of_range_id_is_an_error() {
    let tokenizer = SomaliTokenizer::v2().unwrap();
    assert!(matches!(
        tokenizer.decode(&[1, 48_000], false),
        Err(Error::UnknownId { id: 48_000, .. })
    ));
}
