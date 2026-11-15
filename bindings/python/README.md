# somali-tokenizer

Somali ByteLevel BPE tokenizer for Python, implemented in Rust. It runs the released
`somali-bpe-v2` tokenizer: 48,000 tokens, trained on the
[Somali NLP Corpus](https://github.com/somastllc/somali-nlp-corpus), and published on the
Hugging Face Hub as [`somast/somali-bpe-v2`](https://huggingface.co/somast/somali-bpe-v2).

```bash
pip install somali-tokenizer
```

```python
from somali_tokenizer import SomaliTokenizer

tokenizer = SomaliTokenizer.v2()  # bundled, works offline
# tokenizer = SomaliTokenizer.from_pretrained()        # or from the Hub (somast/somali-bpe-v2)
# tokenizer = SomaliTokenizer.from_file("tokenizer.json")

encoding = tokenizer.encode("Soomaaliya waa dal ku yaal Geeska Afrika.")
encoding.ids  # [5446, 406, 785, 305, 1627, 4235, 1454, 29]
encoding.tokens  # ['Soomaaliya', 'Ġwaa', 'Ġdal', 'Ġku', 'Ġyaal', 'ĠGeeska', 'ĠAfrika', '.']
tokenizer.decode(encoding.ids)  # the original text, exactly

tokenizer.encode_batch(["Nabad", "Galab wanaagsan"])  # parallel, releases the GIL
```

The ids are the same as those from `transformers`:

```python
from transformers import AutoTokenizer

hf = AutoTokenizer.from_pretrained("somast/somali-bpe-v2")
assert hf.encode(text) == SomaliTokenizer.v2().encode(text).ids
```

## Guarantees

- Lossless: `decode(encode(x)) == unicodedata.normalize("NFC", x)` for any string.
- No unknown token: all 256 bytes are in the vocabulary, so any script encodes.
- Validated on load: an artifact with a different pipeline or special tokens raises
  `ContractError` instead of producing wrong ids.

Special tokens occupy ids 0 to 15: `<|endoftext|>`, `<|pad|>`, `<|im_start|>`,
`<|im_end|>`, and `<|reserved_0|>` to `<|reserved_11|>`.

## License

Apache-2.0.
