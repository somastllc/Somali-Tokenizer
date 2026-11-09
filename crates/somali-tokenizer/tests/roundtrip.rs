//! The two properties v2 exists for: lossless round-trip and no unknown token.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use somali_tokenizer::SomaliTokenizer;
use unicode_normalization::UnicodeNormalization;

const ADVERSARIAL: &[&str] = &[
    "Soomaaliya waa dal ku yaal Geeska Afrika, oo leh taariikh hodan ah.",
    "  leading and trailing spaces  ",
    "tabs\there\tand\tthere",
    "line one\nline two\n\nparagraph break",
    "emoji 🙂 and 中文 and runes ᚠᚢᚦ",
    "Cafe\u{301} decomposed accent",
    "\u{0} null byte and \u{7f} delete",
    "",
    "   ",
    "\n",
];

fn assert_roundtrips(tokenizer: &SomaliTokenizer, text: &str) {
    let encoding = tokenizer.encode(text).unwrap();
    let decoded = tokenizer.decode(&encoding.ids, false).unwrap();
    let expected: String = text.nfc().collect();
    assert_eq!(decoded, expected, "round-trip failed for {text:?}");
}

#[test]
fn adversarial_strings_roundtrip() {
    let tokenizer = SomaliTokenizer::v2().unwrap();
    for text in ADVERSARIAL {
        assert_roundtrips(&tokenizer, text);
    }
}

#[test]
fn every_fixture_document_roundtrips() {
    let tokenizer = SomaliTokenizer::v2().unwrap();
    let path = common::repo_root().join("python-reference/tests/fixtures/mini_corpus.jsonl");
    for record in common::read_jsonl(&path) {
        assert_roundtrips(&tokenizer, record["text"].as_str().unwrap());
    }
}

#[test]
fn every_byte_value_roundtrips() {
    let tokenizer = SomaliTokenizer::v2().unwrap();
    for codepoint in (0u32..0x800).filter_map(char::from_u32) {
        assert_roundtrips(&tokenizer, &format!("a{codepoint}b"));
    }
}

#[test]
fn unseen_script_produces_known_tokens_only() {
    let tokenizer = SomaliTokenizer::v2().unwrap();
    let encoding = tokenizer.encode("ᚠᚢᚦ ꦗꦮ ⴰⵣⵓⵍ").unwrap();
    assert!(!encoding.is_empty());
    assert!(encoding
        .ids
        .iter()
        .all(|&id| (id as usize) < tokenizer.vocab_size()));
    assert!(tokenizer.token_to_id("<unk>").is_none());
}

#[test]
fn skip_special_tokens_drops_them() {
    let tokenizer = SomaliTokenizer::v2().unwrap();
    let encoding = tokenizer.encode("<|endoftext|>Nabad<|im_end|>").unwrap();
    assert_eq!(encoding.ids.first(), Some(&0));
    assert_eq!(tokenizer.decode(&encoding.ids, true).unwrap(), "Nabad");
}
