//! Output must match the Python reference exactly on the parity fixture, which
//! `python-reference/make_parity_fixture.py` generates with the same artifact.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use somali_tokenizer::SomaliTokenizer;

#[test]
fn encoding_matches_the_python_reference() {
    let tokenizer = SomaliTokenizer::v2().unwrap();
    let records = common::read_jsonl(&common::fixture("parity_v2.jsonl"));
    assert!(records.len() > 200, "parity fixture is unexpectedly small");

    for record in &records {
        let text = record["text"].as_str().unwrap();
        let expected_ids: Vec<u32> = serde_json::from_value(record["ids"].clone()).unwrap();
        let expected_tokens: Vec<String> =
            serde_json::from_value(record["tokens"].clone()).unwrap();

        let encoding = tokenizer.encode(text).unwrap();
        assert_eq!(encoding.ids, expected_ids, "ids differ for {text:?}");
        assert_eq!(
            encoding.tokens, expected_tokens,
            "tokens differ for {text:?}"
        );

        let decoded = tokenizer.decode(&encoding.ids, false).unwrap();
        assert_eq!(
            decoded,
            record["decoded"].as_str().unwrap(),
            "decode differs for {text:?}"
        );
    }
}

#[test]
fn batch_encoding_matches_single_encoding() {
    let tokenizer = SomaliTokenizer::v2().unwrap();
    let records = common::read_jsonl(&common::fixture("parity_v2.jsonl"));
    let texts: Vec<&str> = records
        .iter()
        .map(|r| r["text"].as_str().unwrap())
        .collect();

    let batch = tokenizer.encode_batch(&texts).unwrap();
    for (text, encoding) in texts.iter().zip(&batch) {
        assert_eq!(encoding, &tokenizer.encode(text).unwrap());
    }
}
