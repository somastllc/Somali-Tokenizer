//! The somali-bpe v2 artifact contract.
//!
//! An artifact decides how text is parsed, so one from an unknown source is untrusted input.
//! [`validate`] checks a parsed `tokenizer.json` against the design in
//! `docs/TOKENIZER_DESIGN.md` before any encoding happens: a mismatched artifact fails on load
//! instead of producing subtly wrong ids.

use serde_json::Value;
use tokenizers::pre_tokenizers::byte_level::ByteLevel;

use crate::error::{Error, Result};

/// Special tokens of the v2 contract, in id order: they occupy ids `0..16`.
pub const SPECIAL_TOKENS: [&str; 16] = [
    "<|endoftext|>",
    "<|pad|>",
    "<|im_start|>",
    "<|im_end|>",
    "<|reserved_0|>",
    "<|reserved_1|>",
    "<|reserved_2|>",
    "<|reserved_3|>",
    "<|reserved_4|>",
    "<|reserved_5|>",
    "<|reserved_6|>",
    "<|reserved_7|>",
    "<|reserved_8|>",
    "<|reserved_9|>",
    "<|reserved_10|>",
    "<|reserved_11|>",
];

/// End-of-text token; also the beginning-of-sequence token in the Hugging Face export.
pub const EOS_TOKEN: &str = "<|endoftext|>";

/// Padding token.
pub const PAD_TOKEN: &str = "<|pad|>";

/// Size of the ByteLevel seed alphabet.
pub const BYTE_ALPHABET_SIZE: usize = 256;

/// Validate a parsed artifact against the v2 contract.
///
/// Checks the model type, the absence of an unknown token, every pipeline component, the
/// special tokens and their ids, and that all 256 byte symbols are in the vocabulary.
///
/// # Errors
///
/// Returns [`Error::Contract`] naming the first field that does not match.
pub fn validate(artifact: &Value) -> Result<()> {
    expect_str(artifact, &["model", "type"], "BPE")?;
    if !field(artifact, &["model", "unk_token"]).is_some_and(Value::is_null) {
        return Err(Error::contract(
            "model.unk_token",
            "must be null: ByteLevel BPE has no unknown token",
        ));
    }
    expect_str(artifact, &["normalizer", "type"], "NFC")?;
    expect_str(artifact, &["pre_tokenizer", "type"], "ByteLevel")?;
    expect_bool(artifact, &["pre_tokenizer", "add_prefix_space"], false)?;
    expect_bool(artifact, &["pre_tokenizer", "use_regex"], true)?;
    expect_str(artifact, &["decoder", "type"], "ByteLevel")?;
    expect_str(artifact, &["post_processor", "type"], "ByteLevel")?;
    validate_special_tokens(artifact)?;
    validate_byte_alphabet(artifact)
}

fn validate_special_tokens(artifact: &Value) -> Result<()> {
    let added = field(artifact, &["added_tokens"])
        .and_then(Value::as_array)
        .ok_or_else(|| Error::contract("added_tokens", "missing or not an array"))?;
    for (expected_id, expected) in SPECIAL_TOKENS.iter().enumerate() {
        let entry = added
            .iter()
            .find(|token| token.get("content").and_then(Value::as_str) == Some(*expected))
            .ok_or_else(|| Error::contract("added_tokens", format!("missing {expected}")))?;
        let id = entry.get("id").and_then(Value::as_u64);
        if id != Some(expected_id as u64) {
            return Err(Error::contract(
                "added_tokens",
                format!("{expected} must have id {expected_id}, found {id:?}"),
            ));
        }
        if entry.get("special").and_then(Value::as_bool) != Some(true) {
            return Err(Error::contract(
                "added_tokens",
                format!("{expected} must be marked special"),
            ));
        }
    }
    Ok(())
}

fn validate_byte_alphabet(artifact: &Value) -> Result<()> {
    let vocab = field(artifact, &["model", "vocab"])
        .and_then(Value::as_object)
        .ok_or_else(|| Error::contract("model.vocab", "missing or not an object"))?;
    let missing = ByteLevel::alphabet()
        .into_iter()
        .filter(|symbol| !vocab.contains_key(&symbol.to_string()))
        .count();
    if missing > 0 {
        return Err(Error::contract(
            "model.vocab",
            format!("{missing} of the {BYTE_ALPHABET_SIZE} byte symbols are missing"),
        ));
    }
    Ok(())
}

fn field<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(value, |node, key| node.get(key))
}

fn expect_str(artifact: &Value, path: &[&str], expected: &str) -> Result<()> {
    match field(artifact, path).and_then(Value::as_str) {
        Some(found) if found == expected => Ok(()),
        found => Err(Error::contract(
            path.join("."),
            format!("expected {expected:?}, found {found:?}"),
        )),
    }
}

fn expect_bool(artifact: &Value, path: &[&str], expected: bool) -> Result<()> {
    match field(artifact, path).and_then(Value::as_bool) {
        Some(found) if found == expected => Ok(()),
        found => Err(Error::contract(
            path.join("."),
            format!("expected {expected}, found {found:?}"),
        )),
    }
}
