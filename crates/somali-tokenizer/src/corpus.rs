//! Streaming corpus readers, document cleaning, fingerprinting, and the evaluation split.
//!
//! Every function here matches the Python reference (`python-reference/common.py`)
//! character for character: the same cleaning, the same notion of whitespace, and the same
//! hash rule for the held-out split. Nothing here can return the whole corpus.

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use blake2::digest::{Update, VariableOutput};
use blake2::Blake2bVar;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokenizers::normalizers::NFC;
use tokenizers::tokenizer::normalizer::NormalizedString;
use tokenizers::tokenizer::Normalizer;

use crate::error::{Error, Result};

/// Documents held out for evaluation: those whose id hashes into
/// `EVAL_SPLIT_BUCKETS` of `EVAL_SPLIT_MODULUS` buckets (0.8%).
pub const EVAL_SPLIT_MODULUS: u64 = 1000;
/// See [`EVAL_SPLIT_MODULUS`].
pub const EVAL_SPLIT_BUCKETS: u64 = 8;

/// Bytes hashed from each end of a corpus file when fingerprinting it.
const FINGERPRINT_EDGE_BYTES: u64 = 64 * 1024;

/// Whitespace exactly as Python's `str.isspace` defines it: Unicode `White_Space` plus the
/// four information separators U+001C to U+001F. `str.split()` and `str.strip()` use this.
#[must_use]
pub fn is_python_whitespace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// Number of whitespace-separated words, as Python's `len(text.split())`.
#[must_use]
pub fn count_words(text: &str) -> usize {
    text.split(is_python_whitespace)
        .filter(|word| !word.is_empty())
        .count()
}

/// Unicode NFC normalization, done by the same component the tokenizer uses.
#[must_use]
pub fn nfc(text: &str) -> String {
    let mut normalized = NormalizedString::from(text);
    match NFC.normalize(&mut normalized) {
        Ok(()) => normalized.get().to_owned(),
        // NFC never fails; fall back to the input rather than panic.
        Err(_) => text.to_owned(),
    }
}

/// Light tokenizer-oriented cleaning: trim, NFC, collapse three or more consecutive
/// newlines into a paragraph break, trim again. Returns an empty string for blank input.
#[must_use]
pub fn clean_document(text: &str) -> String {
    let text = nfc(text.trim_matches(is_python_whitespace));
    if text.is_empty() {
        return text;
    }
    let mut out = String::with_capacity(text.len());
    let mut newlines = 0usize;
    for c in text.chars() {
        if c == '\n' {
            newlines += 1;
            continue;
        }
        if newlines > 0 {
            out.push_str(if newlines >= 3 {
                "\n\n"
            } else {
                &"\n\n"[..newlines]
            });
            newlines = 0;
        }
        out.push(c);
    }
    out.trim_matches(is_python_whitespace).to_owned()
}

/// Whether a record id falls in the held-out evaluation split:
/// `blake2b(id, digest_size=8)` read big-endian, modulo 1000, below 8.
#[must_use]
pub fn is_eval_doc(doc_id: &str) -> bool {
    let mut digest = [0u8; 8];
    // An 8-byte output is always valid for BLAKE2b, so neither call can fail.
    let Ok(mut hasher) = Blake2bVar::new(digest.len()) else {
        return false;
    };
    hasher.update(doc_id.as_bytes());
    if hasher.finalize_variable(&mut digest).is_err() {
        return false;
    }
    u64::from_be_bytes(digest) % EVAL_SPLIT_MODULUS < EVAL_SPLIT_BUCKETS
}

/// Split cleaned document text into the lines the trainer sees. Matches reading the
/// prepared training file in Python text mode, which ends lines at `\n`, `\r\n`, or `\r`.
pub fn training_lines(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n')
        .flat_map(|line| line.split('\r'))
        .filter(|line| !line.is_empty())
}

/// Which documents a reader yields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Split {
    /// Everything not held out: the training split.
    Train,
    /// Only the held-out evaluation split.
    Eval,
    /// Every document.
    All,
}

impl Split {
    fn keeps(self, doc_id: &str) -> bool {
        match self {
            Self::Train => !is_eval_doc(doc_id),
            Self::Eval => is_eval_doc(doc_id),
            Self::All => true,
        }
    }
}

/// One cleaned document and the provenance needed for splitting and per-source stats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusDoc {
    /// Record id, or `source:line` when the record has none.
    pub id: String,
    /// `provenance.source`, else a top-level `source`, else `unknown`.
    pub source: String,
    /// Cleaned text; never empty.
    pub text: String,
}

/// Streams [`CorpusDoc`]s from JSONL, one record per line.
///
/// Accepts release records (`id`, `text`, `provenance.source`) and evaluation records
/// (`id`, `source`, `text`). Malformed lines and records without a string `text` are skipped
/// and counted; read [`JsonlDocs::skipped`] when the stream ends. I/O errors are yielded.
pub struct JsonlDocs<R> {
    lines: std::io::Lines<R>,
    path: PathBuf,
    line_no: usize,
    split: Split,
    skipped: usize,
}

impl JsonlDocs<BufReader<File>> {
    /// Open a JSONL file.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the file cannot be opened.
    pub fn open(path: impl AsRef<Path>, split: Split) -> Result<Self> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|err| Error::io(path, err))?;
        Ok(Self::new(BufReader::new(file), path, split))
    }
}

impl<R: BufRead> JsonlDocs<R> {
    /// Read from any buffered reader; `path` is used only in error messages.
    pub fn new(reader: R, path: impl Into<PathBuf>, split: Split) -> Self {
        Self {
            lines: reader.lines(),
            path: path.into(),
            line_no: 0,
            split,
            skipped: 0,
        }
    }

    /// Lines skipped so far because they were malformed or had no string `text`.
    #[must_use]
    pub fn skipped(&self) -> usize {
        self.skipped
    }

    fn parse(&mut self, line: &str) -> Option<CorpusDoc> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            self.skipped += 1;
            return None;
        };
        let Some(raw) = record.get("text").and_then(Value::as_str) else {
            self.skipped += 1;
            return None;
        };
        let source = record
            .get("provenance")
            .and_then(|p| p.get("source"))
            .or_else(|| record.get("source"))
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned();
        let id = match record.get("id").and_then(Value::as_str) {
            Some(id) => id.to_owned(),
            None => format!("{source}:{}", self.line_no),
        };
        if !self.split.keeps(&id) {
            return None;
        }
        let text = clean_document(raw);
        if text.is_empty() {
            return None;
        }
        Some(CorpusDoc { id, source, text })
    }
}

impl<R: BufRead> Iterator for JsonlDocs<R> {
    type Item = Result<CorpusDoc>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let line = match self.lines.next()? {
                Ok(line) => line,
                Err(err) => return Some(Err(Error::io(&self.path, err))),
            };
            self.line_no += 1;
            if let Some(doc) = self.parse(&line) {
                return Some(Ok(doc));
            }
        }
    }
}

/// Cheap identity for a large corpus file: size, modification time, and a SHA-256 of the
/// first and last 64 KiB. Detects a regenerated or truncated corpus without reading it all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Fingerprint {
    /// The path as given.
    pub path: String,
    /// File size.
    pub size_bytes: u64,
    /// Modification time, nanoseconds since the Unix epoch.
    pub mtime_ns: u128,
    /// SHA-256 over the head and tail, hex-encoded.
    pub edge_sha256: String,
}

/// Fingerprint a corpus file.
///
/// # Errors
///
/// [`Error::Io`] if the file cannot be read.
pub fn fingerprint(path: impl AsRef<Path>) -> Result<Fingerprint> {
    let path = path.as_ref();
    let io = |err| Error::io(path, err);
    let mut file = File::open(path).map_err(io)?;
    let metadata = file.metadata().map_err(io)?;
    let size = metadata.len();
    let mtime_ns = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos());

    let mut hasher = Sha256::new();
    let mut buffer = Vec::new();
    (&mut file)
        .take(FINGERPRINT_EDGE_BYTES)
        .read_to_end(&mut buffer)
        .map_err(io)?;
    Digest::update(&mut hasher, &buffer);
    if size > FINGERPRINT_EDGE_BYTES {
        buffer.clear();
        file.seek(SeekFrom::Start(size - FINGERPRINT_EDGE_BYTES))
            .map_err(io)?;
        file.read_to_end(&mut buffer).map_err(io)?;
        Digest::update(&mut hasher, &buffer);
    }
    let edge_sha256 = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();

    Ok(Fingerprint {
        path: path.display().to_string(),
        size_bytes: size,
        mtime_ns,
        edge_sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleaning_collapses_runs_of_newlines() {
        assert_eq!(clean_document("  a\n\n\n\nb\nc\n\nd  "), "a\n\nb\nc\n\nd");
        assert_eq!(clean_document(" \u{1f}\t "), "");
    }

    #[test]
    fn cleaning_applies_nfc() {
        assert_eq!(clean_document("Cafe\u{301}"), "Caf\u{e9}");
    }

    #[test]
    fn words_split_on_python_whitespace() {
        assert_eq!(count_words("a  b\tc\u{1c}d\u{a0}e"), 5);
        assert_eq!(count_words("   "), 0);
    }

    #[test]
    fn training_lines_split_like_python_text_mode() {
        let lines: Vec<&str> = training_lines("a\r\nb\rc\n\nd").collect();
        assert_eq!(lines, ["a", "b", "c", "d"]);
    }

    #[test]
    fn eval_split_is_about_eight_per_thousand() {
        let held_out = (0..100_000)
            .filter(|i| is_eval_doc(&format!("doc:{i}")))
            .count();
        assert!((600..1000).contains(&held_out), "{held_out}");
    }
}
