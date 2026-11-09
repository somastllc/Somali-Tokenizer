# somali-tokenizer

Somali ByteLevel BPE tokenizer for Rust. Loads, validates, and runs the `somali-bpe`
artifacts: a 48,000-token ByteLevel BPE trained on the
[Somali NLP Corpus](https://github.com/somastllc/somali-nlp-corpus).

Built on the Hugging Face [`tokenizers`](https://crates.io/crates/tokenizers) crate, so
artifacts stay in the standard `tokenizer.json` format and the same file loads in
`transformers` from [`somast/somali-bpe-v2`](https://huggingface.co/somast/somali-bpe-v2).

## Library

```rust
use somali_tokenizer::SomaliTokenizer;

let tokenizer = SomaliTokenizer::v2()?; // bundled release, no network
let encoding = tokenizer.encode("Soomaaliya waa dal ku yaal Geeska Afrika.")?;
println!("{:?}", encoding.tokens);
let text = tokenizer.decode(&encoding.ids, false)?;
```

Other constructors:

- `SomaliTokenizer::from_file(path)` loads any `tokenizer.json`.
- `SomaliTokenizer::from_pretrained("somast/somali-bpe-v2", None)` downloads from the
  Hugging Face Hub (feature `http`; reads `HF_TOKEN` for private repositories).

Every constructor validates the artifact against the v2 contract: model type, no unknown
token, NFC normalizer, ByteLevel pre-tokenizer with `add_prefix_space = false`, ByteLevel
decoder and post-processor, the 16 special tokens at ids 0 to 15, and all 256 byte symbols.
An artifact that fails is rejected with `Error::Contract` naming the field.

## Command line

```bash
cargo install somali-tokenizer
somali-tokenizer encode "Soomaaliya waa dal."            # 5446 406 785 29
somali-tokenizer encode --format tokens "Soomaaliya waa dal."
somali-tokenizer decode 5446 406 785 29
echo "Nabad iyo caano" | somali-tokenizer encode --format json
somali-tokenizer inspect
```

`--tokenizer PATH` uses another artifact; with the `http` feature, `--hub REPO` loads one
from the Hub.

## Guarantees

- `decode(encode(x)) == NFC(x)` for any input: the round-trip is lossless.
- No unknown token can be produced: all 256 bytes are in the vocabulary.
- Token ids equal those of the Python `tokenizers` library on the same artifact; the parity
  fixture in `tests/fixtures/parity_v2.jsonl` enforces it.

## License

Apache-2.0.
