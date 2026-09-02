# Architecture

The intended software architecture for the Rust implementation.

**Status.** Design document. No source code exists in this repository. The layout below is
a proposal to be created in Phase 2 of [../PLAN.md](../PLAN.md); it is written here so the
first implementation starts from a decided structure rather than an accreted one.

---

## 1. Design goals

1. **Separate training from inference.** Encoding text with an existing artifact must not
   depend on any training code path. Most consumers only ever encode.
2. **Separate benchmarking from tokenizer logic.** Measurement code must not be able to
   influence what it measures. Metrics live outside the tokenizer.
3. **Configuration-driven experiments.** An experiment is a configuration plus a corpus
   release. Neither should require editing code.
4. **Streaming everywhere.** No stage may require the corpus in memory.
5. **Deterministic where the algorithm allows.** Same corpus, same configuration, same
   library version, same artifact.
6. **Explicit errors, no panics in library paths.** A library returns errors; a CLI decides
   what to print and which exit code to use.
7. **Artifacts are versioned data, not build output.** The artifact format and its identity
   rules are part of the design, not an implementation detail.

---

## 2. Layering

```text
┌─────────────────────────────────────────────┐
│ CLI                                         │  argument parsing, printing, exit codes
├─────────────────────────────────────────────┤
│ Application layer                           │  experiment orchestration, run records,
│                                             │  report writing
├─────────────────────────────────────────────┤
│ Tokenizer · Training · Benchmarking         │  domain logic
├─────────────────────────────────────────────┤
│ Hugging Face `tokenizers` Rust crate        │  BPE model, ByteLevel pipeline, trainer
├─────────────────────────────────────────────┤
│ Filesystem / corpus                         │  streaming readers, artifact I/O
└─────────────────────────────────────────────┘
```

**Dependencies point downward only.** The tokenizer layer knows nothing about the CLI. The
benchmark engine depends on the tokenizer's public encode/decode surface and on nothing
inside it. Training depends on corpus and configuration, not on benchmarking.

Concretely, these must remain true and are reviewable in a diff:

- No printing to stdout or stderr below the CLI layer, and no process exit.
- The benchmark engine cannot reach tokenizer internals; it uses the same public API a
  third party would.
- Training code is not required to compile an encode-only consumer (feature-gated if
  necessary).

---

## 3. Modules

Logical modules, each with one responsibility.

| Module | Responsibility | Depends on |
|--------|----------------|-----------|
| `config` | Typed configuration, parsing, validation, canonical serialization | `error` |
| `error` | The crate's error type and error contract | nothing |
| `corpus` | Streaming readers for training text and evaluation JSONL; fingerprinting; the split rule | `error` |
| `tokenizer` | Artifact loading and validation, encode, decode, introspection | `config`, `error` |
| `training` | Trainer construction, the training run, vocabulary derivation for sweeps | `config`, `corpus`, `tokenizer`, `error` |
| `benchmark` | Metric accumulation and report generation over an evaluation stream | `corpus`, `tokenizer`, `error` |
| `serialize` (export) | Artifact writing and ecosystem-compatible export | `tokenizer`, `error` |
| `cli` | Subcommands, argument parsing, human-readable output | everything above |

### Configuration

Owns the schema in [../configs/bpe_48k.example.toml](../configs/bpe_48k.example.toml).
Parses TOML, rejects unknown fields, validates coherence (for example, that the target
vocabulary exceeds the 272 non-merge entries), and produces a canonical serialization for
run records. Defaults equal the reference design, so an empty configuration reproduces
[TOKENIZER_DESIGN.md](TOKENIZER_DESIGN.md).

### Corpus reader

Streams. Two shapes: plain-text training material read line by line and batched for the
trainer, and evaluation JSONL read record by record with `id`, `source`, and `text`. Also
owns corpus fingerprinting and the deterministic split rule, so that "which split is this
document in" has exactly one implementation.

Never returns the whole corpus. There is no API that could.

### Normalizer, pre-tokenizer, decoder

Not separate modules. These are pipeline components configured on the tokenizer and
supplied by the underlying crate. Listing them separately in an architecture would imply a
choice about reimplementing them, which is the opposite of the intent.

### Trainer

Builds a trainer from configuration, streams the corpus into it, saves the artifact, and
emits a run record. Also owns vocabulary derivation: producing a smaller tokenizer by
truncating a larger one's merge list, which is valid because BPE merges are greedy (see
[TRAINING.md](TRAINING.md) §8).

### Encoder and decoder

The inference surface, and the only part most consumers touch. Loads an artifact, validates
it against its declared configuration (byte alphabet complete, expected pipeline
components, special tokens present at expected ids), then encodes and decodes. Validation
on load is deliberate: a mismatched artifact should fail immediately, not produce subtly
wrong ids.

### Benchmark engine

Consumes an evaluation stream and one or more tokenizers, accumulates per-document scalars
only, and produces the metric set in [BENCHMARKING.md](BENCHMARKING.md). Scores every
tokenizer on identical documents in one pass. Holds no document text.

### Model serializer

Writes the native artifact and the ecosystem-compatible export (`tokenizer.json`,
`tokenizer_config.json`, `special_tokens_map.json`). Refuses to export an artifact that
fails its round-trip check. An unusable artifact should not be promoted into a form that
looks official.

### CLI

Thin. Parses arguments, calls the application layer, formats output, chooses exit codes.
Contains no tokenizer logic. Everything the CLI can do must be reachable through the
library API.

---

## 4. Why build on the `tokenizers` crate

The plan is to depend on the Hugging Face `tokenizers` Rust crate rather than reimplement
BPE, ByteLevel pre-tokenization, and the artifact format from first principles.

**Reasons.**

1. **It is the same implementation the reference uses.** The Python reference is bindings
   over this crate. Using it directly means the Rust implementation and the reference share
   a merge algorithm, a ByteLevel implementation, and an artifact format, so a difference
   between them is a difference in *our* code, not a difference in two BPE implementations.
   That makes Phase 7 parity a tractable question instead of an open-ended one.
2. **Artifact compatibility is free.** Reading and writing `tokenizer.json` in the standard
   schema is what makes released artifacts usable in the wider ecosystem. Reimplementing
   the format would mean maintaining bug-for-bug compatibility with a moving target.
3. **The interesting work is elsewhere.** This project's value is in Somali corpus
   selection, vocabulary decisions, evaluation methodology, and reproducible releases,
   not in a fifth implementation of a well-understood merge loop.
4. **Correctness.** A well-exercised implementation has already survived the edge cases a
   new one would rediscover: byte mapping, offset handling, decoder round-trips.

**Costs, stated honestly.** A significant dependency, whose behaviour is our behaviour;
tie-breaking and other unspecified details become version-dependent (hence recording the
version in every run record); and some pipeline defaults are inherited rather than chosen.
See the pre-tokenizer/decoder `add_prefix_space` asymmetry documented in
[TOKENIZER_DESIGN.md](TOKENIZER_DESIGN.md) §1.

**When this would change.** Research requiring an algorithm the crate does not provide:
morphology-aware merge constraints, a custom objective, corpus reweighting during merge
learning. Such a change would be a deliberate decision with its own design document, not a
drift. Even then, the artifact format should stay compatible.

---

## 5. Cross-cutting concerns

### Streaming and memory

Memory use must be independent of corpus and evaluation-set size, verified by measurement
rather than asserted. The trainer's own working set scales with vocabulary and corpus
diversity, which is expected; holding the corpus is not.

### Memory safety and robustness

Safe Rust throughout. `unsafe` requires a documented justification and review. No `unwrap`,
`expect`, or `panic!` on library paths, including on malformed artifacts, malformed JSONL,
truncated files, and adversarial text. Untrusted input includes tokenizer artifacts: an
artifact directs how text is parsed, so one from an unknown source is untrusted data.

### Determinism

BPE as specified has no sampling step and no seed. Parallelism must not change results; if
a parallel implementation makes merge selection order-dependent, that is a defect. Anything
that does introduce randomness must take an explicit, recorded seed.

### Configuration-driven experiments

An experiment is a configuration plus a corpus release. Adding an experiment must not
require code changes, and every run must record its full effective configuration.

### Artifact versioning

Artifacts are immutable, checksummed, and versioned independently of the software. See
[REPRODUCIBILITY.md](REPRODUCIBILITY.md) §3 and §4. The architectural consequence: nothing
writes into a released artifact path, and artifact identity is computed from content.

### Error handling

The library returns typed errors with enough context to act on: which field, which file,
which expectation failed. The CLI maps errors to messages and exit codes. Errors are never
swallowed, and a failed invariant check is an error rather than a warning.

---

## 6. Proposed source layout

**Not created in Phase 1.** Proposed for Phase 2:

```text
src/
├── lib.rs          # public API surface and re-exports
├── config.rs       # configuration types, parsing, validation
├── corpus.rs       # streaming readers, fingerprinting, split rule
├── tokenizer.rs    # artifact loading/validation, encode, decode
├── training.rs     # trainer construction, training runs, sweep derivation
├── benchmark.rs    # metric accumulation and reporting
├── error.rs        # error type and contract
└── cli.rs          # CLI argument parsing and command dispatch
```

Plus, outside `src/`:

```text
tests/              # integration tests and small fixtures
configs/            # example experiment configurations
docs/               # this documentation
```

Open questions for Phase 2 (recorded in [../PLAN.md](../PLAN.md)): crate and binary names;
whether library and CLI are one crate with a binary target or two crates; whether the
export path is its own module or part of `tokenizer`; whether corpus preparation lives here
or stays in SomNLP-Corpus.

---

## 7. Boundaries with other repositories

```text
SomNLP-Corpus ──▶ Somali-Tokenizer ──▶ models / NLP systems

SomLTK ──▶ general Somali language processing (independent)
```

- **Upstream (SomNLP-Corpus).** This project consumes corpus releases and never writes to
  them. It does no collection, cleaning, deduplication, or language identification. If a
  tokenizer problem traces to corpus quality, the fix belongs upstream.
- **Sibling (SomLTK).** Linguistic word and sentence segmentation, stemming, morphology,
  and tagging belong there. This project's only notion of "word" is whitespace splitting
  inside a benchmark metric, and that must not grow into linguistic analysis.
- **Downstream (models).** This project produces artifacts and measures them. It does not
  train, evaluate, or serve models. Downstream accuracy is a separate experiment run
  elsewhere.
