//! Benchmark tokenizers on a held-out evaluation split.
//!
//! Every tokenizer is scored on the same documents in one streaming pass, through the same
//! public encode/decode calls a third party would use. Only per-document ratios are kept,
//! never document text. Metrics and the report layout match `python-reference/benchmark.py`.

use std::path::Path;

use serde_json::{json, Map, Value};
use tokenizers::Tokenizer;

use crate::corpus::{count_words, nfc, CorpusDoc};
use crate::error::{Error, Result};
use crate::metrics::{ratio, round_to, BySource};

/// Default number of documents checked for `decode(encode(x)) == NFC(x)`.
pub const DEFAULT_ROUNDTRIP_LIMIT: usize = 100_000;

/// Load any `tokenizer.json` without checking the v2 contract, so older or third-party
/// artifacts (v1, sweep candidates) can be compared. Do not use it to serve text.
///
/// # Errors
///
/// [`Error::Io`] or [`Error::Tokenizer`] if the file cannot be read or parsed.
pub fn load_artifact(path: impl AsRef<Path>) -> Result<Tokenizer> {
    let path = path.as_ref();
    let json = std::fs::read_to_string(path).map_err(|err| Error::io(path, err))?;
    json.parse().map_err(Error::tokenizer)
}

#[derive(Debug, Default, Clone, Copy)]
struct SourceCounts {
    tokens: u64,
    words: u64,
}

/// Running tallies for one tokenizer. Holds numbers only.
#[derive(Debug)]
struct Accumulator {
    label: String,
    ratios: Vec<f64>,
    total_tokens: u64,
    total_bytes: u64,
    total_words: u64,
    documents: u64,
    roundtrip_checked: u64,
    roundtrip_failures: u64,
    unk_documents: u64,
    by_source: BySource<SourceCounts>,
}

impl Accumulator {
    fn new(label: &str) -> Self {
        Self {
            label: label.to_owned(),
            ratios: Vec::new(),
            total_tokens: 0,
            total_bytes: 0,
            total_words: 0,
            documents: 0,
            roundtrip_checked: 0,
            roundtrip_failures: 0,
            unk_documents: 0,
            by_source: BySource::default(),
        }
    }

    fn add(&mut self, source: &str, text: &str, n_tokens: usize) {
        let words = count_words(text) as u64;
        let n_tokens = n_tokens as u64;
        self.documents += 1;
        self.total_tokens += n_tokens;
        self.total_bytes += text.len() as u64;
        self.total_words += words;
        if words > 0 {
            self.ratios.push(n_tokens as f64 / words as f64);
            let counts = self.by_source.entry(source);
            counts.tokens += n_tokens;
            counts.words += words;
        }
    }

    fn summary(&self) -> Value {
        let mut ordered = self.ratios.clone();
        ordered.sort_by(f64::total_cmp);
        let n = ordered.len();
        if n == 0 {
            return json!({"label": self.label, "documents": 0});
        }
        let mean = ordered.iter().sum::<f64>() / n as f64;
        let median = if n % 2 == 1 {
            ordered[n / 2]
        } else {
            (ordered[n / 2 - 1] + ordered[n / 2]) / 2.0
        };
        // Python's round(): ties to even.
        let p95_index = ((0.95 * (n - 1) as f64).round_ties_even() as usize).min(n - 1);

        let by_source: Map<String, Value> = self
            .by_source
            .sorted_by_desc(|counts| counts.words)
            .into_iter()
            .map(|(source, counts)| {
                (
                    source.to_owned(),
                    json!(ratio(counts.tokens, counts.words, 4)),
                )
            })
            .collect();
        let fidelity = (self.roundtrip_checked > 0).then(|| {
            round_to(
                1.0 - self.roundtrip_failures as f64 / self.roundtrip_checked as f64,
                6,
            )
        });

        json!({
            "label": self.label,
            "documents": self.documents,
            "mean_tokens_per_word": round_to(mean, 4),
            "median_tokens_per_word": round_to(median, 4),
            "p95_tokens_per_word": round_to(ordered[p95_index], 4),
            "corpus_tokens_per_word": ratio(self.total_tokens, self.total_words, 4),
            "bytes_per_token": ratio(self.total_bytes, self.total_tokens, 4),
            "total_tokens": self.total_tokens,
            "roundtrip_checked": self.roundtrip_checked,
            "roundtrip_failures": self.roundtrip_failures,
            "roundtrip_fidelity": fidelity,
            "unk_documents": self.unk_documents,
            "by_source_tokens_per_word": by_source,
        })
    }
}

/// Benchmark options.
#[derive(Debug, Clone)]
pub struct BenchmarkOptions {
    /// Verify the round-trip on the first this-many documents.
    pub roundtrip_limit: usize,
    /// Score at most this many documents.
    pub limit: Option<usize>,
}

impl Default for BenchmarkOptions {
    fn default() -> Self {
        Self {
            roundtrip_limit: DEFAULT_ROUNDTRIP_LIMIT,
            limit: None,
        }
    }
}

/// Score `tokenizers` (label, tokenizer) on `docs` and return the report as JSON:
/// `documents_scored`, `roundtrip_limit`, and one summary per label under `tokenizers`.
///
/// # Errors
///
/// Any error the document stream yields, [`Error::Tokenizer`] if encoding fails, or
/// [`Error::Contract`] if no documents were read.
pub fn run<I>(
    tokenizers: &[(String, Tokenizer)],
    docs: I,
    options: &BenchmarkOptions,
) -> Result<Value>
where
    I: IntoIterator<Item = Result<CorpusDoc>>,
{
    let mut accumulators: Vec<Accumulator> = tokenizers
        .iter()
        .map(|(label, _)| Accumulator::new(label))
        .collect();
    let unk_ids: Vec<Option<u32>> = tokenizers
        .iter()
        .map(|(_, tokenizer)| tokenizer.token_to_id("<unk>"))
        .collect();

    let mut seen = 0usize;
    for doc in docs {
        if options.limit.is_some_and(|limit| seen >= limit) {
            break;
        }
        let doc = doc?;
        seen += 1;
        let check_roundtrip = seen <= options.roundtrip_limit;
        let normalized = check_roundtrip.then(|| nfc(&doc.text));

        for (((_, tokenizer), accumulator), unk_id) in
            tokenizers.iter().zip(&mut accumulators).zip(&unk_ids)
        {
            let encoding = tokenizer
                .encode(doc.text.as_str(), true)
                .map_err(Error::tokenizer)?;
            let ids = encoding.get_ids();
            accumulator.add(&doc.source, &doc.text, ids.len());
            if unk_id.is_some_and(|unk| ids.contains(&unk)) {
                accumulator.unk_documents += 1;
            }
            if let Some(normalized) = &normalized {
                accumulator.roundtrip_checked += 1;
                let decoded = tokenizer.decode(ids, false).map_err(Error::tokenizer)?;
                if &decoded != normalized {
                    accumulator.roundtrip_failures += 1;
                }
            }
        }
    }
    if seen == 0 {
        return Err(Error::contract("eval", "no evaluation documents were read"));
    }

    let summaries: Map<String, Value> = accumulators
        .iter()
        .map(|accumulator| (accumulator.label.clone(), accumulator.summary()))
        .collect();
    Ok(json!({
        "documents_scored": seen,
        "roundtrip_limit": options.roundtrip_limit,
        "tokenizers": summaries,
    }))
}
