//! Loading, validating, encoding, and decoding.

use std::path::Path;
use std::str::FromStr;

use serde_json::Value;
use tokenizers::Tokenizer;

use crate::contract::{self, SPECIAL_TOKENS};
use crate::error::{Error, Result};

/// The released v2 artifact, compiled into the crate so it works offline.
///
/// Byte-identical to `artifacts/releases/v2/tokenizer.json` and to `tokenizer.json` in the
/// Hugging Face repository `somast/somali-tokenizer`; a test enforces the first.
const BUNDLED_V2: &str = include_str!("../assets/somali-bpe-v2.json");

/// Hugging Face Hub repository holding the released v2 artifact.
pub const HUB_REPO_V2: &str = "somast/somali-tokenizer";

/// The output of encoding one text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoding {
    /// Token ids.
    pub ids: Vec<u32>,
    /// Token strings, in the ByteLevel alphabet (a leading space shows as `Ġ`).
    pub tokens: Vec<String>,
    /// Character offsets of each token in the normalized input.
    pub offsets: Vec<(usize, usize)>,
}

impl Encoding {
    /// Number of tokens.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Whether the encoding holds no tokens.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

impl From<tokenizers::Encoding> for Encoding {
    fn from(encoding: tokenizers::Encoding) -> Self {
        Self {
            ids: encoding.get_ids().to_vec(),
            tokens: encoding.get_tokens().to_vec(),
            offsets: encoding.get_offsets().to_vec(),
        }
    }
}

/// A validated Somali ByteLevel BPE tokenizer.
///
/// Every constructor checks the artifact against the v2 contract (see [`contract::validate`]),
/// so a value of this type always has the expected pipeline and special tokens.
#[derive(Debug, Clone)]
pub struct SomaliTokenizer {
    inner: Tokenizer,
    vocab_size: usize,
}

impl SomaliTokenizer {
    /// The released v2 tokenizer bundled with the crate. Needs no file or network access.
    ///
    /// # Errors
    ///
    /// Only if the bundled artifact is corrupt, which a test rules out.
    pub fn v2() -> Result<Self> {
        Self::from_json(BUNDLED_V2)
    }

    /// Load and validate a `tokenizer.json` file.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the file cannot be read, [`Error::Json`] if it is not valid JSON,
    /// [`Error::Contract`] if it breaks the v2 contract.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let json = std::fs::read_to_string(path).map_err(|err| Error::io(path, err))?;
        Self::from_json(&json)
    }

    /// Load and validate a tokenizer from the text of a `tokenizer.json`.
    ///
    /// # Errors
    ///
    /// [`Error::Json`] if the text is not valid JSON, [`Error::Contract`] if it breaks the
    /// v2 contract, [`Error::Tokenizer`] if the library rejects it.
    pub fn from_json(json: &str) -> Result<Self> {
        let artifact: Value = serde_json::from_str(json)?;
        contract::validate(&artifact)?;
        let inner = Tokenizer::from_str(json).map_err(Error::tokenizer)?;
        Ok(Self::wrap(inner))
    }

    /// Download (or reuse from the local cache) a tokenizer from the Hugging Face Hub and
    /// validate it. `revision` defaults to `main`.
    ///
    /// # Errors
    ///
    /// [`Error::Tokenizer`] if the download fails, [`Error::Contract`] if the artifact
    /// breaks the v2 contract.
    #[cfg(feature = "http")]
    pub fn from_pretrained(repo_id: &str, revision: Option<&str>) -> Result<Self> {
        let params = tokenizers::FromPretrainedParameters {
            revision: revision.unwrap_or("main").to_owned(),
            token: std::env::var("HF_TOKEN").ok(),
            ..Default::default()
        };
        let inner = Tokenizer::from_pretrained(repo_id, Some(params)).map_err(Error::tokenizer)?;
        Self::from_tokenizer(inner)
    }

    /// Validate an already-built `tokenizers::Tokenizer` and wrap it.
    ///
    /// # Errors
    ///
    /// [`Error::Contract`] if it breaks the v2 contract.
    pub fn from_tokenizer(inner: Tokenizer) -> Result<Self> {
        let json = inner.to_string(false).map_err(Error::tokenizer)?;
        contract::validate(&serde_json::from_str(&json)?)?;
        Ok(Self::wrap(inner))
    }

    fn wrap(inner: Tokenizer) -> Self {
        let vocab_size = inner.get_vocab_size(true);
        Self { inner, vocab_size }
    }

    /// Encode one text. Special-token strings in the text are matched as special tokens.
    ///
    /// # Errors
    ///
    /// [`Error::Tokenizer`] if the library fails, which the contract makes unexpected.
    pub fn encode(&self, text: &str) -> Result<Encoding> {
        self.inner
            .encode(text, true)
            .map(Encoding::from)
            .map_err(Error::tokenizer)
    }

    /// Encode many texts, in parallel where the library chooses to.
    ///
    /// # Errors
    ///
    /// [`Error::Tokenizer`] if the library fails on any text.
    pub fn encode_batch<S: AsRef<str>>(&self, texts: &[S]) -> Result<Vec<Encoding>> {
        let inputs: Vec<&str> = texts.iter().map(AsRef::as_ref).collect();
        self.inner
            .encode_batch(inputs, true)
            .map(|encodings| encodings.into_iter().map(Encoding::from).collect())
            .map_err(Error::tokenizer)
    }

    /// Decode ids back to text. With `skip_special_tokens` false, decoding an encoding
    /// returns the NFC-normalized input exactly.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] if any id is outside the vocabulary.
    pub fn decode(&self, ids: &[u32], skip_special_tokens: bool) -> Result<String> {
        self.check_ids(ids)?;
        self.inner
            .decode(ids, skip_special_tokens)
            .map_err(Error::tokenizer)
    }

    /// Decode many id sequences.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] if any id is outside the vocabulary.
    pub fn decode_batch(
        &self,
        sequences: &[Vec<u32>],
        skip_special_tokens: bool,
    ) -> Result<Vec<String>> {
        sequences
            .iter()
            .map(|ids| self.decode(ids, skip_special_tokens))
            .collect()
    }

    fn check_ids(&self, ids: &[u32]) -> Result<()> {
        match ids.iter().find(|&&id| id as usize >= self.vocab_size) {
            Some(&id) => Err(Error::UnknownId {
                id,
                vocab_size: self.vocab_size,
            }),
            None => Ok(()),
        }
    }

    /// Vocabulary size, including special tokens.
    #[must_use]
    pub fn vocab_size(&self) -> usize {
        self.vocab_size
    }

    /// Id of a token, if it is in the vocabulary.
    #[must_use]
    pub fn token_to_id(&self, token: &str) -> Option<u32> {
        self.inner.token_to_id(token)
    }

    /// Token for an id, if the id is in the vocabulary.
    #[must_use]
    pub fn id_to_token(&self, id: u32) -> Option<String> {
        self.inner.id_to_token(id)
    }

    /// The special tokens, in id order.
    #[must_use]
    pub fn special_tokens(&self) -> &'static [&'static str] {
        &SPECIAL_TOKENS
    }

    /// The artifact serialized as `tokenizer.json` text.
    ///
    /// # Errors
    ///
    /// [`Error::Tokenizer`] if serialization fails.
    pub fn to_json(&self, pretty: bool) -> Result<String> {
        self.inner.to_string(pretty).map_err(Error::tokenizer)
    }

    /// The underlying `tokenizers::Tokenizer`, for features this crate does not wrap.
    #[must_use]
    pub fn inner(&self) -> &Tokenizer {
        &self.inner
    }
}
