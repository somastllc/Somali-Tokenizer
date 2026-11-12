//! Somali ByteLevel BPE tokenizer.
//!
//! Loads, validates, and runs the `somali-bpe` tokenizer artifacts: a 48k-vocabulary
//! ByteLevel BPE trained on the Somali NLP Corpus. Built on the Hugging Face `tokenizers`
//! crate, so artifacts stay in the standard `tokenizer.json` format and load unchanged in
//! `transformers`.
//!
//! ```
//! use somali_tokenizer::SomaliTokenizer;
//!
//! # fn main() -> somali_tokenizer::Result<()> {
//! let tokenizer = SomaliTokenizer::v2()?;
//! let encoding = tokenizer.encode("Soomaaliya waa dal ku yaal Geeska Afrika.")?;
//! assert_eq!(
//!     tokenizer.decode(&encoding.ids, false)?,
//!     "Soomaaliya waa dal ku yaal Geeska Afrika."
//! );
//! # Ok(())
//! # }
//! ```

pub mod benchmark;
pub mod contract;
pub mod corpus;
mod error;
mod metrics;
pub mod stats;
mod tokenizer;
pub mod training;

pub use error::{Error, Result};
pub use tokenizer::{Encoding, SomaliTokenizer, HUB_REPO_V2};
