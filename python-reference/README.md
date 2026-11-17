# Python reference implementation

The original Somali tokenizer pipeline, imported from
[Somali NLP Corpus](https://github.com/somastllc/somali-nlp-corpus) (`tokenizer/`) with its
Git history. It produced the released v1 and v2 artifacts and every full-corpus figure in
the documentation. It is kept as the behavioural specification for the Rust
implementation: the parity fixtures in `../tests/fixtures/` are generated from it.

| Script | Rust equivalent |
|--------|-----------------|
| `prepare_corpus.py` + `train.py` | `somali-tokenizer train` (one streaming pass) |
| `benchmark.py` | `somali-tokenizer benchmark` |
| `tokenize_corpus.py` | `somali-tokenizer stats` |
| `export_hf.py` | none yet; the released `tokenizer.json` already loads in `transformers` |
| `make_parity_fixture.py` | generates the fixtures the Rust and Python tests check |

```bash
cd python-reference
python -m venv .venv && . .venv/bin/activate
pip install -r requirements.txt
pytest
python make_parity_fixture.py     # only when an artifact or a script changes
```

Paths resolve against the repository root: artifacts in `../artifacts/releases/`, corpus in
`../data/final/final_so.jsonl`. `PAPER.md` is the technical note on the method and the
vocabulary-size experiments.
