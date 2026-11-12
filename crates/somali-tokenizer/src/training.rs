//! Training a Somali ByteLevel BPE, and deriving smaller vocabularies from a larger run.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use serde_json::Value;
use tokenizers::decoders::byte_level::ByteLevel as ByteLevelDecoder;
use tokenizers::models::bpe::{BpeTrainer, BPE};
use tokenizers::models::TrainerWrapper;
use tokenizers::normalizers::NFC;
use tokenizers::pre_tokenizers::byte_level::ByteLevel;
use tokenizers::processors::byte_level::ByteLevel as ByteLevelProcessor;
use tokenizers::{AddedToken, Tokenizer};

use crate::contract::{BYTE_ALPHABET_SIZE, SPECIAL_TOKENS};
use crate::corpus::{fingerprint, training_lines, Fingerprint, JsonlDocs, Split};
use crate::error::{Error, Result};
use crate::tokenizer::SomaliTokenizer;

/// Default vocabulary size of the released tokenizer.
pub const DEFAULT_VOCAB_SIZE: usize = 48_000;
/// Default minimum pair frequency for a merge.
pub const DEFAULT_MIN_FREQUENCY: u64 = 2;

/// Number of vocabulary entries that are not merges: special tokens plus the byte alphabet.
#[must_use]
pub const fn reserved_entries() -> usize {
    SPECIAL_TOKENS.len() + BYTE_ALPHABET_SIZE
}

/// Trainer settings. The defaults reproduce the released v2 tokenizer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrainConfig {
    /// Total vocabulary size, including special tokens and the byte alphabet.
    pub vocab_size: usize,
    /// Minimum pair frequency for a merge.
    pub min_frequency: u64,
    /// Draw a progress bar on stderr (only with the `cli` feature).
    #[serde(skip)]
    pub show_progress: bool,
}

impl Default for TrainConfig {
    fn default() -> Self {
        Self {
            vocab_size: DEFAULT_VOCAB_SIZE,
            min_frequency: DEFAULT_MIN_FREQUENCY,
            show_progress: false,
        }
    }
}

impl TrainConfig {
    /// Check that the configuration can produce a valid tokenizer.
    ///
    /// # Errors
    ///
    /// [`Error::Contract`] if the vocabulary does not exceed the reserved entries.
    pub fn validate(&self) -> Result<()> {
        if self.vocab_size <= reserved_entries() {
            return Err(Error::contract(
                "vocab_size",
                format!(
                    "{} must exceed the {} special tokens plus the {BYTE_ALPHABET_SIZE}-byte \
                     alphabet",
                    self.vocab_size,
                    SPECIAL_TOKENS.len()
                ),
            ));
        }
        Ok(())
    }
}

/// An untrained tokenizer with the v2 pipeline: NFC, ByteLevel pre-tokenizer without a
/// prefix space, ByteLevel decoder and post-processor with library defaults.
#[must_use]
pub fn untrained_tokenizer() -> Tokenizer {
    let mut tokenizer = Tokenizer::new(BPE::default());
    tokenizer
        .with_normalizer(Some(NFC))
        .with_pre_tokenizer(Some(ByteLevel::new(false, true, true)))
        .with_decoder(Some(ByteLevelDecoder::default()))
        .with_post_processor(Some(ByteLevelProcessor::default().trim_offsets(true)));
    tokenizer
}

fn trainer(config: &TrainConfig) -> TrainerWrapper {
    let alphabet = ByteLevel::alphabet().into_iter().collect();
    let special_tokens = SPECIAL_TOKENS
        .iter()
        .map(|token| AddedToken::from(*token, true))
        .collect();
    let trainer = BpeTrainer::builder()
        .vocab_size(config.vocab_size)
        .min_frequency(config.min_frequency)
        .show_progress(config.show_progress)
        .special_tokens(special_tokens)
        .initial_alphabet(alphabet)
        .build();
    trainer.into()
}

/// Train on a stream of texts. Each item is one training line; see
/// [`crate::corpus::training_lines`] for how documents become lines.
///
/// The stream is consumed once and never collected, so memory does not grow with corpus
/// size (the trainer keeps word counts, not text).
///
/// # Errors
///
/// [`Error::Contract`] for an invalid configuration, [`Error::Tokenizer`] if training fails.
pub fn train<I, S>(config: &TrainConfig, texts: I) -> Result<SomaliTokenizer>
where
    I: Iterator<Item = S> + Send,
    S: AsRef<str> + Send,
{
    config.validate()?;
    let mut tokenizer = untrained_tokenizer();
    let mut trainer = trainer(config);
    tokenizer
        .train(&mut trainer, texts)
        .map_err(Error::tokenizer)?;
    SomaliTokenizer::from_tokenizer(tokenizer)
}

/// Produce a smaller tokenizer by truncating a larger one's merge list.
///
/// BPE learns merges greedily, so merge *k* does not depend on the target size: a larger
/// run's merges extend a smaller run's, and cutting the list reproduces the smaller run.
/// This avoids re-reading the corpus for every size in a sweep.
///
/// # Errors
///
/// [`Error::Contract`] if the artifact has no BPE vocabulary or merges, or the target is not
/// between the reserved entries and the source size.
pub fn derive(artifact: &Value, target_vocab: usize) -> Result<Value> {
    let mut derived = artifact.clone();
    let model = derived
        .get_mut("model")
        .ok_or_else(|| Error::contract("model", "missing"))?;
    let source_vocab = model
        .get("vocab")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::contract("model.vocab", "missing or not an object"))?
        .len();
    let merges = model
        .get("merges")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::contract("model.merges", "missing or not an array"))?
        .len();
    let reserved = source_vocab.saturating_sub(merges);

    if target_vocab > source_vocab {
        return Err(Error::contract(
            "vocab_size",
            format!("cannot derive {target_vocab} from a {source_vocab}-token tokenizer"),
        ));
    }
    if target_vocab <= reserved {
        return Err(Error::contract(
            "vocab_size",
            format!("{target_vocab} must exceed the {reserved} non-merge tokens"),
        ));
    }

    if let Some(vocab) = model.get_mut("vocab").and_then(Value::as_object_mut) {
        vocab.retain(|_, id| id.as_u64().is_some_and(|id| id < target_vocab as u64));
    }
    if let Some(merges) = model.get_mut("merges").and_then(Value::as_array_mut) {
        merges.truncate(target_vocab - reserved);
    }
    if let Some(added) = derived
        .get_mut("added_tokens")
        .and_then(Value::as_array_mut)
    {
        added.retain(|token| {
            token
                .get("id")
                .and_then(Value::as_u64)
                .is_some_and(|id| id < target_vocab as u64)
        });
    }
    Ok(derived)
}

/// Where training text comes from.
#[derive(Debug, Clone)]
pub enum TrainingSource {
    /// A release corpus in JSONL. Documents are cleaned, the evaluation split is held out
    /// (unless `holdout` is false), and each document is fed line by line.
    Jsonl {
        /// Path to the JSONL corpus.
        path: PathBuf,
        /// Hold out the evaluation split.
        holdout: bool,
        /// Read at most this many documents.
        limit: Option<usize>,
    },
    /// Plain text, one training line per line, as written by `prepare_corpus.py`.
    Text {
        /// Path to the text file.
        path: PathBuf,
        /// Read at most this many lines.
        limit: Option<usize>,
    },
}

/// What a training run read and produced; serialized next to the artifact as a run record.
#[derive(Debug, Clone, Serialize)]
pub struct RunRecord {
    /// Version of this crate.
    pub somali_tokenizer_version: &'static str,
    /// Version of the `tokenizers` crate that did the training.
    pub tokenizers_version: &'static str,
    /// Trainer settings.
    pub config: TrainConfig,
    /// Identity of the input file.
    pub corpus: Fingerprint,
    /// Documents used for training (JSONL input only).
    pub documents: usize,
    /// Lines fed to the trainer.
    pub lines: usize,
    /// Malformed records skipped (JSONL input only).
    pub skipped_records: usize,
    /// Which split was trained on.
    pub split: Split,
    /// Wall-clock training time.
    pub elapsed_seconds: f64,
    /// Final vocabulary size.
    pub vocab_size: usize,
}

/// Version of the `tokenizers` crate this build uses.
pub const TOKENIZERS_VERSION: &str = "0.21.4";

/// Train from a file and return the tokenizer with its run record.
///
/// # Errors
///
/// [`Error::Io`] if the input cannot be read, [`Error::Contract`] for an invalid
/// configuration, [`Error::Tokenizer`] if training fails.
pub fn train_from(
    config: &TrainConfig,
    source: &TrainingSource,
) -> Result<(SomaliTokenizer, RunRecord)> {
    let started = Instant::now();
    let documents = AtomicUsize::new(0);
    let lines = AtomicUsize::new(0);
    let failure: Mutex<Option<Error>> = Mutex::new(None);
    let record_failure = |err: Error| {
        if let Ok(mut slot) = failure.lock() {
            slot.get_or_insert(err);
        }
    };

    let (path, split, skipped, tokenizer) = match source {
        TrainingSource::Jsonl {
            path,
            holdout,
            limit,
        } => {
            let split = if *holdout { Split::Train } else { Split::All };
            let mut docs = JsonlDocs::open(path, split)?;
            let skipped = AtomicUsize::new(0);
            let stream = std::iter::from_fn(|| {
                if limit.is_some_and(|limit| documents.load(Ordering::Relaxed) >= limit) {
                    return None;
                }
                let next = docs.next();
                skipped.store(docs.skipped(), Ordering::Relaxed);
                match next? {
                    Ok(doc) => {
                        documents.fetch_add(1, Ordering::Relaxed);
                        Some(
                            training_lines(&doc.text)
                                .map(str::to_owned)
                                .collect::<Vec<_>>(),
                        )
                    }
                    Err(err) => {
                        record_failure(err);
                        None
                    }
                }
            })
            .flatten()
            .inspect(|_| {
                lines.fetch_add(1, Ordering::Relaxed);
            });
            let tokenizer = train(config, stream);
            (path, split, skipped.into_inner(), tokenizer)
        }
        TrainingSource::Text { path, limit } => {
            let file = File::open(path).map_err(|err| Error::io(path, err))?;
            let stream = BufReader::new(file)
                .lines()
                .map_while(|line| {
                    line.map_err(|err| record_failure(Error::io(path, err)))
                        .ok()
                })
                .take(limit.unwrap_or(usize::MAX))
                .flat_map(|line| training_lines(&line).map(str::to_owned).collect::<Vec<_>>())
                .inspect(|_| {
                    lines.fetch_add(1, Ordering::Relaxed);
                });
            (path, Split::All, 0, train(config, stream))
        }
    };

    if let Some(err) = failure.into_inner().ok().flatten() {
        return Err(err);
    }
    let tokenizer = tokenizer?;
    let record = RunRecord {
        somali_tokenizer_version: env!("CARGO_PKG_VERSION"),
        tokenizers_version: TOKENIZERS_VERSION,
        config: config.clone(),
        corpus: fingerprint(path)?,
        documents: documents.into_inner(),
        lines: lines.into_inner(),
        skipped_records: skipped,
        split,
        elapsed_seconds: (started.elapsed().as_secs_f64() * 10.0).round() / 10.0,
        vocab_size: tokenizer.vocab_size(),
    };
    Ok((tokenizer, record))
}
