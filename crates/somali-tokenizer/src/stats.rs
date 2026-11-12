//! Full-corpus token statistics: tokenize every document and report measured counts.
//!
//! Memory does not depend on corpus size: documents are encoded in batches and only
//! counters are kept. Percentiles come from a fixed-width ratio histogram, accurate to the
//! bin width (0.005 tokens per word). Output matches `python-reference/tokenize_corpus.py`.

use serde_json::{json, Map, Value};
use tokenizers::Tokenizer;

use crate::corpus::{count_words, nfc, CorpusDoc};
use crate::error::{Error, Result};
use crate::metrics::{ratio, round_to, BySource};

/// Width of one tokens-per-word histogram bin.
pub const RATIO_BIN_WIDTH: f64 = 0.005;
/// Number of histogram bins; ratios at or above 20 land in the last one.
pub const RATIO_BINS: usize = 4000;
/// Upper edges of the document-length buckets, in tokens.
pub const LENGTH_EDGES: [u64; 7] = [128, 256, 512, 1024, 2048, 4096, 8192];
/// Documents encoded per batch.
pub const BATCH_SIZE: usize = 10_000;
/// Every this-many-th document is checked for an exact round-trip.
pub const ROUNDTRIP_STRIDE: usize = 500;

fn length_bucket(n_tokens: u64) -> usize {
    LENGTH_EDGES
        .iter()
        .position(|&edge| n_tokens < edge)
        .unwrap_or(LENGTH_EDGES.len())
}

fn length_bucket_names() -> Vec<String> {
    let mut names = Vec::with_capacity(LENGTH_EDGES.len() + 1);
    let mut lower = 0;
    for edge in LENGTH_EDGES {
        names.push(format!("{lower}-{edge}"));
        lower = edge;
    }
    names.push(format!("{lower}+"));
    names
}

#[derive(Debug, Default, Clone, Copy)]
struct SourceCounts {
    documents: u64,
    words: u64,
    characters: u64,
    bytes: u64,
    tokens: u64,
}

/// Counters for one tokenizer over the corpus.
#[derive(Debug)]
struct Tally {
    label: String,
    documents: u64,
    words: u64,
    characters: u64,
    bytes: u64,
    tokens: u64,
    roundtrip_checked: u64,
    roundtrip_failures: u64,
    ratio_hist: Vec<u64>,
    length_hist: [u64; LENGTH_EDGES.len() + 1],
    by_source: BySource<SourceCounts>,
}

impl Tally {
    fn new(label: &str) -> Self {
        Self {
            label: label.to_owned(),
            documents: 0,
            words: 0,
            characters: 0,
            bytes: 0,
            tokens: 0,
            roundtrip_checked: 0,
            roundtrip_failures: 0,
            ratio_hist: vec![0; RATIO_BINS],
            length_hist: [0; LENGTH_EDGES.len() + 1],
            by_source: BySource::default(),
        }
    }

    fn add(&mut self, source: &str, text: &str, n_tokens: usize) {
        let words = count_words(text) as u64;
        let characters = text.chars().count() as u64;
        let bytes = text.len() as u64;
        let tokens = n_tokens as u64;
        self.documents += 1;
        self.words += words;
        self.characters += characters;
        self.bytes += bytes;
        self.tokens += tokens;
        self.length_hist[length_bucket(tokens)] += 1;
        let counts = self.by_source.entry(source);
        counts.documents += 1;
        counts.words += words;
        counts.characters += characters;
        counts.bytes += bytes;
        counts.tokens += tokens;
        if words > 0 {
            let bin = ((tokens as f64 / words as f64) / RATIO_BIN_WIDTH) as usize;
            self.ratio_hist[bin.min(RATIO_BINS - 1)] += 1;
        }
    }

    fn percentile(&self, q: f64) -> f64 {
        let total: u64 = self.ratio_hist.iter().sum();
        if total == 0 {
            return 0.0;
        }
        let target = q * total as f64;
        let mut seen = 0u64;
        for (index, &count) in self.ratio_hist.iter().enumerate() {
            seen += count;
            if seen as f64 >= target {
                return round_to((index as f64 + 0.5) * RATIO_BIN_WIDTH, 4);
            }
        }
        round_to(RATIO_BINS as f64 * RATIO_BIN_WIDTH, 4)
    }

    fn mean_ratio(&self) -> f64 {
        let total: u64 = self.ratio_hist.iter().sum();
        if total == 0 {
            return 0.0;
        }
        let weighted: f64 = self
            .ratio_hist
            .iter()
            .enumerate()
            .filter(|(_, &count)| count > 0)
            .map(|(index, &count)| count as f64 * (index as f64 + 0.5) * RATIO_BIN_WIDTH)
            .sum();
        round_to(weighted / total as f64, 4)
    }

    fn summary(&self) -> Value {
        let by_source: Map<String, Value> = self
            .by_source
            .sorted_by_desc(|counts| counts.tokens)
            .into_iter()
            .map(|(source, c)| {
                let share = if self.tokens == 0 {
                    0.0
                } else {
                    round_to(c.tokens as f64 / self.tokens as f64, 6)
                };
                let entry = json!({
                    "documents": c.documents,
                    "words": c.words,
                    "characters": c.characters,
                    "bytes": c.bytes,
                    "tokens": c.tokens,
                    "token_share": share,
                    "tokens_per_word": ratio(c.tokens, c.words, 4),
                    "bytes_per_token": ratio(c.bytes, c.tokens, 4),
                });
                (source.to_owned(), entry)
            })
            .collect();
        let lengths: Map<String, Value> = length_bucket_names()
            .into_iter()
            .zip(self.length_hist)
            .map(|(name, count)| (name, json!(count)))
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
            "words": self.words,
            "characters": self.characters,
            "bytes": self.bytes,
            "total_tokens": self.tokens,
            "corpus_tokens_per_word": ratio(self.tokens, self.words, 4),
            "mean_tokens_per_word": self.mean_ratio(),
            "median_tokens_per_word": self.percentile(0.50),
            "p95_tokens_per_word": self.percentile(0.95),
            "p99_tokens_per_word": self.percentile(0.99),
            "bytes_per_token": ratio(self.bytes, self.tokens, 4),
            "characters_per_token": ratio(self.characters, self.tokens, 4),
            "roundtrip_checked": self.roundtrip_checked,
            "roundtrip_failures": self.roundtrip_failures,
            "roundtrip_fidelity": fidelity,
            "document_length_histogram": lengths,
            "by_source": by_source,
        })
    }
}

/// Statistics options.
#[derive(Debug, Clone)]
pub struct StatsOptions {
    /// Check `decode(encode(x)) == NFC(x)` on every [`ROUNDTRIP_STRIDE`]-th document.
    pub check_roundtrip: bool,
}

impl Default for StatsOptions {
    fn default() -> Self {
        Self {
            check_roundtrip: true,
        }
    }
}

/// Tokenize every document with each tokenizer and return per-tokenizer summaries as JSON,
/// keyed by label.
///
/// # Errors
///
/// Any error the document stream yields, or [`Error::Tokenizer`] if encoding fails.
pub fn run<I>(tokenizers: &[(String, Tokenizer)], docs: I, options: &StatsOptions) -> Result<Value>
where
    I: IntoIterator<Item = Result<CorpusDoc>>,
{
    let mut tallies: Vec<Tally> = tokenizers
        .iter()
        .map(|(label, _)| Tally::new(label))
        .collect();
    let mut batch: Vec<CorpusDoc> = Vec::with_capacity(BATCH_SIZE);
    let mut seen = 0usize;
    for doc in docs {
        batch.push(doc?);
        if batch.len() >= BATCH_SIZE {
            flush(tokenizers, &mut tallies, &batch, seen, options)?;
            seen += batch.len();
            batch.clear();
        }
    }
    flush(tokenizers, &mut tallies, &batch, seen, options)?;

    Ok(Value::Object(
        tallies
            .iter()
            .map(|tally| (tally.label.clone(), tally.summary()))
            .collect(),
    ))
}

fn flush(
    tokenizers: &[(String, Tokenizer)],
    tallies: &mut [Tally],
    batch: &[CorpusDoc],
    offset: usize,
    options: &StatsOptions,
) -> Result<()> {
    if batch.is_empty() {
        return Ok(());
    }
    let texts: Vec<&str> = batch.iter().map(|doc| doc.text.as_str()).collect();
    for ((_, tokenizer), tally) in tokenizers.iter().zip(tallies.iter_mut()) {
        let encodings = tokenizer
            .encode_batch_fast(texts.clone(), true)
            .map_err(Error::tokenizer)?;
        for (index, (doc, encoding)) in batch.iter().zip(&encodings).enumerate() {
            tally.add(&doc.source, &doc.text, encoding.get_ids().len());
            if options.check_roundtrip && (offset + index) % ROUNDTRIP_STRIDE == 0 {
                let decoded = tokenizer
                    .decode(encoding.get_ids(), false)
                    .map_err(Error::tokenizer)?;
                tally.roundtrip_checked += 1;
                if decoded != nfc(&doc.text) {
                    tally.roundtrip_failures += 1;
                }
            }
        }
    }
    Ok(())
}
