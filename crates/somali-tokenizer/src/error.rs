//! The crate's error type.
//!
//! Library code returns [`Error`]; it never prints and never exits. Callers can match on the
//! variant to decide what to do, and every variant carries enough context to act on.

use std::path::PathBuf;

/// Result alias used throughout the crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Everything that can go wrong while loading, validating, or using a tokenizer.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A file could not be read or written.
    #[error("{path}: {source}")]
    Io {
        /// The file involved.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// An artifact is not valid JSON, or not the JSON shape expected.
    #[error("invalid artifact JSON: {0}")]
    Json(#[from] serde_json::Error),

    /// The artifact parsed but breaks the somali-bpe contract.
    #[error("artifact contract violated at `{field}`: {message}")]
    Contract {
        /// Dotted path of the offending field, for example `pre_tokenizer.add_prefix_space`.
        field: String,
        /// What was expected and what was found.
        message: String,
    },

    /// The underlying `tokenizers` library reported an error.
    #[error("tokenizer error: {0}")]
    Tokenizer(String),

    /// A token id is outside the vocabulary.
    #[error("token id {id} is outside the vocabulary of {vocab_size}")]
    UnknownId {
        /// The rejected id.
        id: u32,
        /// The vocabulary size, including added tokens.
        vocab_size: usize,
    },
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub(crate) fn contract(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Contract {
            field: field.into(),
            message: message.into(),
        }
    }

    pub(crate) fn tokenizer(err: impl std::fmt::Display) -> Self {
        Self::Tokenizer(err.to_string())
    }
}
