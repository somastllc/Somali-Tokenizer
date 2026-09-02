<h1 align="center">Somali-Tokenizer</h1>

<p align="center">
  <strong>A research-grade subword tokenizer for the Somali language, to be implemented in Rust.</strong><br>
  <em>Qalab kala-jaridda qoraalka Soomaaliga oo loogu talagalay moodallada luqadda.</em>
</p>

<p align="center">
  ByteLevel BPE · lossless round-trip · reproducible releases · Goobo Labs
</p>

---

> **Project status: Phase 1, specification and planning.**
> This repository currently contains documentation only. There is no Rust source code,
> no build target, and no tokenizer artifact here yet. Phase 1 exists so that the Rust
> implementation in Phase 2 can be written without re-deciding the architecture,
> the training procedure, or the evaluation protocol.
> All measurements quoted below were produced by the Python reference implementation in
> [SomNLP-Corpus](https://github.com/goobolabs/SomNLP-Corpus) under `tokenizer/`. No
> figure in this repository was measured by Rust code, and no parity claim is made.

## What this project is

Somali-Tokenizer will be a Rust library and command-line tool for **training, evaluating,
and shipping learned subword tokenizers for Somali**: the vocabulary and merge rules that
sit between raw Somali text and a language model.

It covers exactly one job: subword tokenization for language models and related NLP
systems. It is deliberately not a general Somali NLP toolkit.

## Why a Somali-specific tokenizer

A language model sees tokens, not characters. Its context window, its training cost, and
its per-request inference cost are all denominated in tokens. When a tokenizer was fitted
mostly to other languages, Somali text is split into more, smaller pieces:

```text
fixed context window
      │
      │ more tokens per Somali word
      ▼
less Somali text fits in the same window
      │
      ▼
more compute per unit of Somali content
```

On a held-out Somali evaluation set, the reference 48k Somali tokenizer needs about
**1.35** tokens per whitespace-delimited word, against **1.82** for XLM-RoBERTa-base and
**2.63** for BERT-base-uncased. On the same text, that is roughly 1.35x less fragmentation
than XLM-R and 1.94x less than BERT.

Two honest caveats, stated once here and repeated in the technical docs:

1. **Fewer tokens per word is not the same as a better model.** Tokenizer efficiency and
   downstream model quality are related but separately measured questions. Nothing in this
   repository measures downstream accuracy, and no claim about it is made.
2. **"Word" here means whitespace-delimited span.** That is the benchmark's operational
   definition, not a linguistic claim about Somali morphology. Bytes-per-token is reported
   alongside as a definition-free compression measure.

## Where this sits in the Goobo Labs Somali NLP ecosystem

```text
SomNLP-Corpus
      │
      │ training / evaluation text (versioned corpus releases)
      ▼
Somali-Tokenizer
      │
      │ tokenizer artifacts (vocabulary + merge rules + config)
      ▼
LLMs / NLP systems
```

Separately:

```text
SomLTK
  → general-purpose Somali language processing
    (word and sentence segmentation, stemming, POS tagging, and similar)
```

| Repository | Owns |
|------------|------|
| [SomNLP-Corpus](https://github.com/goobolabs/SomNLP-Corpus) | Corpus collection, cleaning, deduplication, releases |
| **Somali-Tokenizer** | Learned subword tokenization: training, encoding, decoding, benchmarking, release |
| SomLTK | General Somali NLP: linguistic word/sentence tools, morphology, tagging |

Rules of thumb for what belongs here: if it learns a vocabulary from a corpus, or encodes
and decodes text for a model, it belongs in Somali-Tokenizer. If it is a rule-based or
linguistic language tool, it belongs in SomLTK. If it produces or cleans text, it belongs
in SomNLP-Corpus.

## Reference tokenizer

The Rust implementation's first target is to reproduce the behaviour of the existing
Python reference tokenizer, **Somali ByteLevel BPE v2**:

| Property | Value |
|----------|-------|
| Algorithm | Byte Pair Encoding (BPE) |
| Vocabulary | 48,000 (including special tokens and the byte alphabet) |
| Minimum merge frequency | 2 |
| Normalizer | Unicode NFC |
| Pre-tokenizer | ByteLevel (`add_prefix_space = false`) |
| Decoder | ByteLevel |
| Post-processor | ByteLevel (`trim_offsets = true`) |
| Initial alphabet | All 256 bytes |
| Unknown token | None; byte coverage makes one unreachable |

The design intent, and the reason for each choice, is in
[docs/TOKENIZER_DESIGN.md](docs/TOKENIZER_DESIGN.md).

## Benchmark baseline

Measured by the Python reference implementation on a held-out evaluation set of
**49,424 documents / 4,933,796 words**, excluded from training:

| Metric | Somali ByteLevel BPE v2 (48k) |
|--------|------------------------------:|
| Mean tokens per word | 1.3528 |
| Median tokens per word | 1.2857 |
| P95 tokens per word | 1.8571 |
| Corpus-level tokens per word | 1.3763 |
| Bytes per token | 4.8069 |
| Round-trip fidelity | 100% |
| Documents emitting an unknown token | 0 |

Comparison tokenizers scored on the identical documents: XLM-RoBERTa-base 1.8233 mean
tokens/word, BERT-base-uncased 2.6291. These isolate tokenizer fragmentation only.

Mean tokens per word (1.3528) and corpus-level tokens per word (1.3763) differ because the
first averages per-document ratios and the second divides total tokens by total words; the
distinction is explained in [docs/BENCHMARKING.md](docs/BENCHMARKING.md).

The Rust implementation is expected to land close to these figures, but **exact numeric
reproduction is not the acceptance criterion**. What must hold is that the configuration is
the same, the invariants hold, and any measured difference is explained rather than
ignored. See [docs/REPRODUCIBILITY.md](docs/REPRODUCIBILITY.md).

## Planned capabilities

- Train ByteLevel BPE tokenizers on a versioned SomNLP-Corpus release, streaming, without
  loading the corpus into memory
- Encode and decode with an exact round-trip guarantee: `decode(encode(x)) == NFC(x)`
- Benchmark tokenizers on a held-out split: tokens per word, bytes per token, round-trip
  fidelity, unknown-token rate, per-source breakdown
- Run vocabulary-size sweeps and report the compression/parameter trade-off
- Emit a Hugging Face-compatible `tokenizer.json` plus the companion files
  `AutoTokenizer.from_pretrained` expects
- Ship as both a Rust library and a CLI
- Publish reproducible, immutable tokenizer releases with full run metadata

## Architecture at a glance

```text
CLI
 ↓
application layer (experiment orchestration, config, reporting)
 ↓
tokenizer · training · benchmarking
 ↓
Hugging Face `tokenizers` Rust crate
 ↓
filesystem / corpus
```

The plan is to build on the `tokenizers` Rust crate rather than reimplement BPE from
scratch, so that the first Rust release differs from the reference implementation in as few
dimensions as possible. The reasoning, and the conditions under which that would change,
are in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Documentation

| Document | Contents |
|----------|----------|
| [PLAN.md](PLAN.md) | Phase-by-phase execution plan with acceptance criteria |
| [ROADMAP.md](ROADMAP.md) | Release direction, v0.1 through v1.0, and future research |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Intended module structure and dependency direction |
| [docs/TOKENIZER_DESIGN.md](docs/TOKENIZER_DESIGN.md) | Tokenizer specification, design rationale, invariants |
| [docs/TRAINING.md](docs/TRAINING.md) | Corpus handling, splits, and the training procedure |
| [docs/BENCHMARKING.md](docs/BENCHMARKING.md) | Metric definitions and the evaluation protocol |
| [docs/REPRODUCIBILITY.md](docs/REPRODUCIBILITY.md) | Run metadata, versioning, and artifact identity |
| [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) | Rust conventions, toolchain, and dependency policy |
| [docs/FINAL_REVIEW.md](docs/FINAL_REVIEW.md) | Phase 1 completion checklist and status |
| [configs/bpe_48k.example.toml](configs/bpe_48k.example.toml) | Example experiment configuration |

## Development status

| Area | Status |
|------|--------|
| Specification and documentation | Phase 1, this repository |
| Rust project bootstrap | Planned (Phase 2) |
| Training pipeline | Planned (Phase 4) |
| Encoding / decoding API | Planned (Phase 5) |
| Benchmarking | Planned (Phase 6) |
| Reference parity validation | Planned (Phase 7) |
| CLI and library API | Planned (Phase 8) |
| Hugging Face export | Planned (Phase 9) |
| First tagged release | Planned (Phase 10) |

There is nothing to build yet. `cargo` commands will start working in Phase 2.

## Known limitations

Stated up front because they shape how the numbers above should be read:

1. Training text is dominated by web crawl.
2. Latin-script Somali dominates the corpus; Arabic-script Somali is out of distribution.
3. "Word" means whitespace-delimited span.
4. Tokenizer compression efficiency is not downstream model accuracy.
5. Comparisons against BERT and XLM-R isolate fragmentation, not model quality.
6. Corpus composition materially affects which merges are learned.
7. A future, more balanced corpus may move the best vocabulary size.

Each is expanded in [docs/BENCHMARKING.md](docs/BENCHMARKING.md) and
[docs/TOKENIZER_DESIGN.md](docs/TOKENIZER_DESIGN.md).

## Contributing

Phase 1 contributions are documentation and specification review: gaps, contradictions,
and decisions that are still ambiguous enough to make Phase 2 guesswork. See
[CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).

Corpus data is **not** covered by this license. SomNLP-Corpus carries per-record upstream
licenses and no single corpus-wide license; anyone training on it is responsible for the
terms of each source.
