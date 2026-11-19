# Roadmap

Direction and releases. For the phase-by-phase execution detail, see [PLAN.md](PLAN.md).

```text
v0.1 parity ─▶ v0.2 measurement ─▶ v0.3 library API ─▶ v0.4 CLI ─▶ v0.5 interop ─▶ v1.0
 (built)         (built)             (built)            (built)      (partial)     (planned)
```

Nothing below is released yet. v0.1 to v0.4 are implemented and tested on fixtures; what
remains before tagging is a full-corpus parity report and the release engineering in
[PLAN.md](PLAN.md) Phase 10. For v0.5, artifacts load in `transformers` unchanged, but the
Rust export command is still open.

---

## v0.1: Reference-compatible Rust implementation

Reproduce the behaviour of the Python reference tokenizer in Rust.

- Rust project, configuration model, and error contract
- Streaming corpus reader and ByteLevel BPE training
- Encoding and decoding with the round-trip invariant enforced by tests
- Loads and correctly encodes with the existing `somali-bpe-v2` artifact

**Done when:** the Rust implementation loads the reference artifact and produces identical
token ids on a fixed sample, and a Rust-trained artifact on the same corpus release is
behaviourally equivalent, with any difference explained.

## v0.2: Benchmarking and reproducibility tools

Make measurement a first-class, repeatable operation.

- Benchmark engine implementing [docs/BENCHMARKING.md](docs/BENCHMARKING.md)
- Per-source reporting and vocabulary sweeps
- Run records, corpus fingerprinting, and artifact checksums
- Published parity report against the Python reference

**Done when:** a single documented command reproduces the full benchmark table from a
pinned corpus release, and every quoted number has a run record.

## v0.3: Stable library API

Make the crate dependable for other Somast projects.

- Public API frozen and documented, with compiling examples
- Stability policy stated; breaking changes require a major bump
- Clean separation of training, inference, and benchmarking surfaces

**Done when:** a downstream crate can train, encode, decode, and benchmark without
depending on internal types or shelling out to the CLI.

## v0.4: CLI tooling

Make it usable without writing Rust.

- Subcommands for prepare, train, encode, decode, benchmark, and inspect
- Configuration-driven experiments with sensible defaults
- Progress reporting for long runs; specified exit codes and error messages

**Done when:** every library capability is reachable from the CLI, and a new user can run
the documented pipeline end to end from a corpus release.

## v0.5: Model and export interoperability

Make artifacts usable in the wider ecosystem.

- Hugging Face-compatible export: `tokenizer.json`, `tokenizer_config.json`,
  `special_tokens_map.json`
- Verified equivalence between the export and the native artifact
- Documented integration path for downstream model training

**Done when:** an exported artifact loads with a standard fast-tokenizer loader and encodes
identically to the native artifact.

## v1.0: Stable Somali tokenizer release

- A named, immutable, fully documented tokenizer release with run record, benchmark report,
  and checksums
- Stable library API and CLI
- Reproducible from a pinned corpus release by a documented command
- Documented compatibility guarantees for models trained against the artifact

**Done when:** a model team can adopt the artifact, know exactly what it will and will not
change under, and reproduce it themselves.

---

## Future research

Exploratory. None of this is committed, scheduled, or promised; each item is a question the
project may investigate, not a feature on the way.

**Alternate vocabulary sizes.** 48,000 was chosen as a compression-versus-parameters
trade-off on one corpus release (see [docs/TOKENIZER_DESIGN.md](docs/TOKENIZER_DESIGN.md)).
A different corpus, or a deployment where context length dominates parameter budget, may
justify a different point. 65,536 has already been measured and remains a candidate.

**Unigram comparison.** BPE was inherited from the reference implementation, not selected
against alternatives on Somali. A controlled Unigram-versus-BPE comparison at matched
vocabulary size would settle whether the choice costs anything.

**Domain-balanced tokenizer training.** The current merge table is shaped overwhelmingly by
web crawl, because word share drives merge learning. Reweighting or resampling sources
during training would produce a tokenizer whose merges are less web-specific, at some cost
in efficiency on web text, which would need measuring rather than assuming.

**Morphology-aware experiments.** Somali is agglutinative. Whether morphological awareness
in pre-tokenization or vocabulary construction improves anything a language model cares
about is an open question, and one where a negative result would be worth publishing.

**Somali-English code-switching.** Real Somali text mixes English, especially online. How
the tokenizer handles code-switched spans is currently unmeasured; a dedicated evaluation
subset would be the first step.

**Tokenizer and model co-design.** The metrics here measure the tokenizer alone. Whether a
change in tokenization moves downstream model quality is a different experiment requiring
model training, and would be run with the consuming project rather than here.

**Arabic-script Somali.** Currently out of distribution. ByteLevel coverage means such text
encodes losslessly but inefficiently. Improving it requires corpus work upstream in
Somali NLP Corpus before anything can be done here.

---

## Out of scope

Permanently, not "not yet":

- General Somali NLP tooling: linguistic word and sentence segmentation, stemming, POS
  tagging, morphological analysis. That is SomLTK's responsibility.
- Corpus collection, cleaning, and deduplication. That is Somali NLP Corpus's responsibility.
- Model training, fine-tuning, or serving.
