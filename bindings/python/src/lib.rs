//! Python bindings: `somali_tokenizer._somali_tokenizer`.
//!
//! A thin layer over the Rust crate. Encoding and decoding release the GIL, so batches run
//! in parallel with other Python threads.

use std::path::PathBuf;

use pyo3::create_exception;
use pyo3::exceptions::{PyOSError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use somali_tokenizer::{Encoding as RsEncoding, Error, SomaliTokenizer as RsTokenizer};

create_exception!(
    _somali_tokenizer,
    ContractError,
    PyValueError,
    "The artifact does not satisfy the somali-bpe v2 contract."
);

fn to_py(err: Error) -> PyErr {
    match err {
        Error::Io { .. } => PyOSError::new_err(err.to_string()),
        Error::Contract { .. } => ContractError::new_err(err.to_string()),
        Error::Json(_) | Error::UnknownId { .. } => PyValueError::new_err(err.to_string()),
        _ => PyRuntimeError::new_err(err.to_string()),
    }
}

/// The result of encoding one text.
#[pyclass(module = "somali_tokenizer", name = "Encoding", frozen, eq)]
#[derive(PartialEq)]
struct Encoding(RsEncoding);

#[pymethods]
impl Encoding {
    /// Token ids.
    #[getter]
    fn ids(&self) -> Vec<u32> {
        self.0.ids.clone()
    }

    /// Token strings in the ByteLevel alphabet (a leading space shows as ``Ġ``).
    #[getter]
    fn tokens(&self) -> Vec<String> {
        self.0.tokens.clone()
    }

    /// ``(start, end)`` character offsets of each token in the normalized text.
    #[getter]
    fn offsets(&self) -> Vec<(usize, usize)> {
        self.0.offsets.clone()
    }

    fn __len__(&self) -> usize {
        self.0.len()
    }

    fn __repr__(&self) -> String {
        format!("Encoding(ids={:?}, tokens={:?})", self.0.ids, self.0.tokens)
    }
}

/// Somali ByteLevel BPE tokenizer.
///
/// Every constructor validates the artifact against the v2 contract and raises
/// ``ContractError`` if it does not match.
#[pyclass(module = "somali_tokenizer", name = "SomaliTokenizer", frozen)]
struct SomaliTokenizer(RsTokenizer);

#[pymethods]
impl SomaliTokenizer {
    /// The released v2 tokenizer bundled with the package. No file or network access.
    #[staticmethod]
    fn v2() -> PyResult<Self> {
        RsTokenizer::v2().map(Self).map_err(to_py)
    }

    /// Load a ``tokenizer.json`` file.
    #[staticmethod]
    fn from_file(path: PathBuf) -> PyResult<Self> {
        RsTokenizer::from_file(path).map(Self).map_err(to_py)
    }

    /// Load from the JSON text of a ``tokenizer.json``.
    #[staticmethod]
    fn from_str(json: &str) -> PyResult<Self> {
        RsTokenizer::from_json(json).map(Self).map_err(to_py)
    }

    /// Download from the Hugging Face Hub, or reuse the local cache. Reads ``HF_TOKEN``.
    #[staticmethod]
    #[pyo3(signature = (repo_id = somali_tokenizer::HUB_REPO_V2, revision = None))]
    fn from_pretrained(py: Python<'_>, repo_id: &str, revision: Option<&str>) -> PyResult<Self> {
        py.detach(|| RsTokenizer::from_pretrained(repo_id, revision))
            .map(Self)
            .map_err(to_py)
    }

    /// Encode one text. Special-token strings in the text become special tokens.
    fn encode(&self, py: Python<'_>, text: &str) -> PyResult<Encoding> {
        py.detach(|| self.0.encode(text))
            .map(Encoding)
            .map_err(to_py)
    }

    /// Encode many texts in parallel.
    fn encode_batch(&self, py: Python<'_>, texts: Vec<String>) -> PyResult<Vec<Encoding>> {
        py.detach(|| self.0.encode_batch(&texts))
            .map(|encodings| encodings.into_iter().map(Encoding).collect())
            .map_err(to_py)
    }

    /// Decode ids to text. With ``skip_special_tokens=False`` this returns the
    /// NFC-normalized input of ``encode`` exactly.
    #[pyo3(signature = (ids, skip_special_tokens = false))]
    fn decode(&self, py: Python<'_>, ids: Vec<u32>, skip_special_tokens: bool) -> PyResult<String> {
        py.detach(|| self.0.decode(&ids, skip_special_tokens))
            .map_err(to_py)
    }

    /// Decode many id sequences.
    #[pyo3(signature = (sequences, skip_special_tokens = false))]
    fn decode_batch(
        &self,
        py: Python<'_>,
        sequences: Vec<Vec<u32>>,
        skip_special_tokens: bool,
    ) -> PyResult<Vec<String>> {
        py.detach(|| self.0.decode_batch(&sequences, skip_special_tokens))
            .map_err(to_py)
    }

    /// Vocabulary size, including special tokens.
    #[getter]
    fn vocab_size(&self) -> usize {
        self.0.vocab_size()
    }

    /// The special tokens, in id order.
    #[getter]
    fn special_tokens(&self) -> Vec<&'static str> {
        self.0.special_tokens().to_vec()
    }

    /// Id of a token, or ``None``.
    fn token_to_id(&self, token: &str) -> Option<u32> {
        self.0.token_to_id(token)
    }

    /// Token for an id, or ``None``.
    fn id_to_token(&self, id: u32) -> Option<String> {
        self.0.id_to_token(id)
    }

    /// The artifact as ``tokenizer.json`` text.
    #[pyo3(signature = (pretty = false))]
    fn to_str(&self, pretty: bool) -> PyResult<String> {
        self.0.to_json(pretty).map_err(to_py)
    }

    fn __len__(&self) -> usize {
        self.0.vocab_size()
    }

    fn __repr__(&self) -> String {
        format!("SomaliTokenizer(vocab_size={})", self.0.vocab_size())
    }
}

#[pymodule]
fn _somali_tokenizer(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<SomaliTokenizer>()?;
    m.add_class::<Encoding>()?;
    m.add("ContractError", m.py().get_type::<ContractError>())?;
    m.add("HUB_REPO_V2", somali_tokenizer::HUB_REPO_V2)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
