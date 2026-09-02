# Somali-Tokenizer Execution Plan

This is the implementation plan: what gets built, in what order, and what "done" means for
each step. [ROADMAP.md](ROADMAP.md) covers direction and releases; this document covers
execution.

Phases are ordered by dependency, not by calendar. No dates are given, because none are
known.

```text
P1 spec ─▶ P2 bootstrap ─▶ P3 config ─▶ P4 training ─▶ P5 encode/decode ─▶ P6 benchmark
                                                                              │
                     P10 release ◀─ P9 HF interop ◀─ P8 API/CLI ◀─ P7 parity ◀─┘
```

**Current phase: Phase 1.**

---

## Phase 1: Project specification and documentation

**Goal.** Specify the project well enough that another engineer can implement the Rust
tokenizer without guessing an architectural or research decision.

**Tasks**

- Define project scope and its boundary against SomNLP-Corpus and SomLTK
- Record the reference tokenizer specification and the rationale behind each parameter
- Record the reference benchmark baseline and the protocol that produced it
- Specify the training pipeline, corpus handling, and evaluation-split rule
- Specify reproducibility metadata and artifact-versioning rules
- Specify the intended Rust architecture and module boundaries
- Establish contribution, documentation, and repository-hygiene conventions
- Publish an example experiment configuration

**Deliverables.** `README.md`, `CONTRIBUTING.md`, `CHANGELOG.md`, `PLAN.md`, `ROADMAP.md`,
`LICENSE`, `.gitignore`, `docs/` (ARCHITECTURE, TOKENIZER_DESIGN, TRAINING, BENCHMARKING,
REPRODUCIBILITY, DEVELOPMENT, FINAL_REVIEW), `configs/bpe_48k.example.toml`.

**Acceptance criteria**

- No `src/` directory, no Rust source files, no tokenizer artifacts committed
- Tokenizer specification states algorithm, vocabulary size, normalizer, pre-tokenizer,
  decoder, post-processor, alphabet, and special-token policy
- Invariants are written down and testable as stated
- Benchmark metrics are defined precisely enough to reimplement, including how mean and
  corpus-level tokens per word differ
- Reference figures are consistent across every document that quotes them
- Every claim is marked measured or planned, and every measured claim names its source
- `docs/FINAL_REVIEW.md` reflects the true state of the phase

**Dependencies.** Read access to the Python reference implementation in SomNLP-Corpus
`tokenizer/` (`common.py`, `train.py`, `benchmark.py`, `prepare_corpus.py`, `export_hf.py`,
`tests/`, `PAPER.md`, `benchmark_results.json`, `tokenizer_stats.json`).

**Risks**

- *Specification drifts from the reference implementation.* Mitigation: quote parameters
  from the reference source, and re-verify against it at the start of Phase 2.
- *Over-specification freezes decisions that should stay open.* Mitigation: mark open
  questions explicitly rather than inventing an answer.

---

## Phase 2: Rust project bootstrap

**Goal.** A buildable, lintable, testable Rust project skeleton with no tokenizer logic in
it yet.

**Tasks**

- Create the Cargo project: library crate plus a thin binary target
- Pin edition, MSRV, `rustfmt.toml`, and `clippy.toml` consistently with Goobo Labs practice
- Add the minimum dependency set (see [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md))
- Define the error type and the crate's public error contract
- Set up CI: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`
- Re-verify the reference specification against the Python source and correct any drift in
  `docs/`, including copying the authoritative special-token list verbatim

**Deliverables.** `Cargo.toml`, `rustfmt.toml`, `clippy.toml`, `src/lib.rs`, `src/main.rs`,
`src/error.rs`, CI workflow, a passing empty test suite.

**Acceptance criteria**

- `cargo build`, `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` all pass
- The library exposes an error type; no `unwrap`, `expect`, or `panic!` on any library path
- The special-token list in `docs/TOKENIZER_DESIGN.md` is confirmed byte-for-byte against
  the reference implementation, or the document is corrected

**Dependencies.** Phase 1.

**Risks**

- *Dependency sprawl at the start.* Mitigation: add a dependency only when a phase needs it.
- *MSRV pinned too aggressively for the `tokenizers` crate.* Mitigation: check the crate's
  MSRV before pinning; record the decision.

---

## Phase 3: Tokenizer core configuration

**Goal.** A typed, validated configuration model that can express the reference tokenizer
exactly, and refuses to express something incoherent.

**Tasks**

- Define config types for the tokenizer, corpus, training, benchmark, and output sections
- Implement TOML deserialization with unknown-field rejection
- Validate: vocabulary size exceeds special tokens plus the 256-byte alphabet; minimum
  frequency at least 1; normalizer, pre-tokenizer, and decoder mutually consistent
- Make defaults equal the reference design so an empty config reproduces it
- Produce a canonical serialization of the effective configuration for run records
- Reconcile `configs/bpe_48k.example.toml` with the implemented schema

**Deliverables.** `src/config.rs`, config tests, an updated example config, schema
documentation.

**Acceptance criteria**

- The example config round-trips: parse, serialize, reparse to an equal value
- Every documented reference parameter is expressible
- Invalid configurations fail with an actionable message naming the offending field, and
  never panic
- The effective configuration serializes deterministically (stable key order) so it can be
  hashed into a run record

**Dependencies.** Phase 2.

**Risks**

- *Schema churn once training exists.* Mitigation: version the config schema from the start
  (`config_version`) and treat the example config as part of the schema's test surface.

---

## Phase 4: Corpus streaming and training pipeline

**Goal.** Train a ByteLevel BPE tokenizer from a corpus release without loading it into
memory.

**Tasks**

- Implement the corpus reader: streaming plain text for training, streaming JSONL for the
  evaluation holdout
- Implement corpus fingerprinting and the staleness guard
- Implement the deterministic evaluation-split rule and verify it against the reference
  (needed here regardless of where corpus preparation ends up living, so that holdout
  membership is checkable from a document id alone)
- Implement batched feeding into the trainer
- Implement the training run: build the tokenizer pipeline, train, save the artifact
- Implement vocabulary sweeps, including deriving smaller vocabularies by merge-list
  truncation
- Emit a run record for every training run

**Deliverables.** `src/corpus.rs`, `src/training.rs`, run-record output, tests on a small
committed fixture.

**Acceptance criteria**

- A training run on the fixture completes and produces a loadable artifact
- Peak resident memory is independent of corpus size (verified by training on inputs
  differing by an order of magnitude)
- The split rule assigns a given document id to the same split as the reference
  implementation, verified on a sample of real ids
- Training refuses to run against a corpus whose fingerprint does not match the recorded
  one, unless explicitly overridden
- A derived (truncated) vocabulary is byte-identical to a natively trained one at the same
  size, on the fixture
- Every run writes a complete run record as specified in
  [docs/REPRODUCIBILITY.md](docs/REPRODUCIBILITY.md)

**Dependencies.** Phase 3. A SomNLP-Corpus release for full-scale runs; the fixture
suffices for tests.

**Risks**

- *Streaming throughput becomes the bottleneck.* Mitigation: batch input; measure before
  optimizing.
- *Full-corpus runs are expensive to iterate on.* Mitigation: support a document limit for
  smoke tests, and prefer derivation over retraining for sweeps.
- *Corpus release changes mid-development.* Mitigation: pin the release in the config and
  record its fingerprint; treat a new release as a new experiment.

---

## Phase 5: Encoding and decoding

**Goal.** Encode and decode text with the invariants in
[docs/TOKENIZER_DESIGN.md](docs/TOKENIZER_DESIGN.md) enforced by tests.

**Tasks**

- Implement artifact loading and validation (reject an artifact whose pipeline does not
  match its declared configuration)
- Implement encode: text to ids, tokens, and offsets
- Implement decode, including the skip-special-tokens behaviour
- Implement batch encoding
- Write the invariant test suite, including the adversarial input set

**Deliverables.** `src/tokenizer.rs`, invariant tests, a documented public encode/decode
API.

**Acceptance criteria**

- `decode(encode(x)) == NFC(x)` for every fixture document and every adversarial input
  (whitespace runs, tabs, paragraph breaks, emoji, CJK, runic script, corpus sentinels,
  apostrophe clitics, empty and whitespace-only input)
- The 256-symbol byte alphabet is complete in the loaded vocabulary
- No unknown token exists in the vocabulary and none can be produced
- Every special token encodes to exactly one id and survives decoding when special tokens
  are kept
- No input causes a panic; malformed artifacts produce errors

**Dependencies.** Phase 4, or any artifact produced by the reference implementation.

**Risks**

- *Round-trip is asserted against raw input rather than NFC input.* Mitigation: the
  invariant is stated against `NFC(x)` and the test set includes a decomposed-input case.
- *Offsets are subtly wrong while ids are right.* Mitigation: test offsets explicitly, not
  only ids.

---

## Phase 6: Benchmarking

**Goal.** Reproduce the benchmark protocol in
[docs/BENCHMARKING.md](docs/BENCHMARKING.md), streaming, on a held-out split.

**Tasks**

- Implement the streaming scorer: per-document token and word counts, retaining ratios only
- Implement metrics: mean, median, P95, corpus-level tokens per word, bytes per token,
  total tokens
- Implement round-trip verification and unknown-token counting
- Implement the per-source breakdown
- Emit a benchmark report as JSON plus a human-readable summary
- Support scoring several tokenizers in one pass over the evaluation set

**Deliverables.** `src/benchmark.rs`, benchmark report schema, tests on the fixture.

**Acceptance criteria**

- Metric definitions match [docs/BENCHMARKING.md](docs/BENCHMARKING.md) exactly, including
  the percentile rule and the treatment of zero-word documents
- Peak memory is independent of evaluation-set size
- Every tokenizer in a run is scored on identical documents in one pass
- The report contains enough metadata to identify the tokenizer, the evaluation set, and
  the corpus release it came from

**Dependencies.** Phase 5.

**Risks**

- *Metric definitions diverge subtly from the reference (percentile method, rounding,
  zero-word handling).* Mitigation: specify them precisely, then diff against the
  reference in Phase 7.

---

## Phase 7: Python-reference parity validation

**Goal.** Establish, and document, how closely the Rust implementation reproduces the
Python reference, and explain every difference found.

**Tasks**

- Score the reference artifact (`somali-bpe-v2`, 48k) with the Rust benchmark and compare
  against the recorded reference numbers
- Compare encodings token-for-token on a fixed corpus sample using the same artifact
- Train a Rust tokenizer on the same corpus release and compare vocabulary and merge list
  against the reference artifact
- Classify every difference: identical, numerically negligible, or behavioural
- Document the outcome, including anything not reproduced and why

**Deliverables.** A parity report under `reports/`, referenced from
[docs/BENCHMARKING.md](docs/BENCHMARKING.md) and [CHANGELOG.md](CHANGELOG.md).

**Acceptance criteria**

- Loading the reference artifact in Rust and encoding a fixed sample yields token ids
  identical to the Python reference (this is a hard requirement: same artifact, same
  library, same result)
- Benchmark metrics computed by Rust on the reference artifact match the recorded reference
  figures within a stated tolerance, or the discrepancy is explained
- Any difference in a *newly trained* Rust artifact versus the reference is characterized
  and attributed (corpus release, library version, batching, tie-breaking, configuration)
- The report explicitly states what was **not** reproduced

**Dependencies.** Phases 5 and 6, the reference artifact, and the corpus release the
reference was trained on.

**Risks**

- *The corpus release used by the reference is no longer reproducible.* Mitigation: pin and
  fingerprint it early; if it is gone, retrain both sides on a current release and say so.
- *Parity is treated as a numeric target rather than a behavioural one.* Mitigation: the
  acceptance criterion for a retrained artifact is an explained difference, not an equal
  number.
- *Reference and Rust use different `tokenizers` versions.* Mitigation: record both
  versions; align them if a difference is traced to the library.

---

## Phase 8: CLI and library API

**Goal.** A stable, documented surface for both humans and dependent crates.

**Tasks**

- Design the CLI: subcommands for preparing, training, encoding, decoding, benchmarking,
  and inspecting an artifact
- Separate the library API from CLI concerns; no printing or process exit in the library
- Document every public item, with examples that compile
- Define the stability policy for the public API

**Deliverables.** `src/cli.rs`, a thin binary, rustdoc coverage on public items, CLI usage
documentation.

**Acceptance criteria**

- Every CLI operation is reachable through the library API without shelling out
- The library neither prints to stdout nor terminates the process
- Public items are documented; doc examples compile under `cargo test`
- Exit codes and error messages are specified and stable

**Dependencies.** Phases 4 through 6.

**Risks**

- *CLI convenience leaks into library semantics.* Mitigation: the dependency direction in
  [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) is one-way and reviewable.

---

## Phase 9: Hugging Face interoperability

**Goal.** A released artifact loads in the Hugging Face ecosystem without a bespoke loader.

**Tasks**

- Emit `tokenizer.json` in the standard schema
- Emit `tokenizer_config.json` and `special_tokens_map.json`
- Decide and document the exported special-token roles and the default maximum length
- Verify a round-trip through the exported directory against the native artifact
- Document how to load the export

**Deliverables.** An export command, an export layout specification, verification tests.

**Acceptance criteria**

- The exported directory loads with a standard fast-tokenizer loader
- Encoding through the export equals encoding through the native artifact on a fixed sample
- Special-token ids in the export match the artifact
- The export is refused if the artifact fails its round-trip check

**Dependencies.** Phases 5 and 8.

**Risks**

- *Ecosystem schema changes.* Mitigation: pin the schema version in the export and test
  against a known-good loader version.

---

## Phase 10: Release engineering

**Goal.** Publish immutable, reproducible tokenizer releases and a versioned Rust package.

**Tasks**

- Define the release layout: artifact, export directory, run record, benchmark report,
  checksums
- Define the artifact naming and versioning scheme, distinct from the software version
- Write the release checklist: tests, benchmark, invariants, checksums, changelog
- Decide the distribution channel for artifacts (they do not belong in Git history at scale)
- Tag the first software release

**Deliverables.** A release process document, a release checklist, the first tagged version.

**Acceptance criteria**

- A released artifact is accompanied by its run record, benchmark report, and checksum
- Re-running the documented command on the pinned corpus release reproduces the artifact,
  or the deviation is documented
- Released artifacts are never modified in place
- The changelog distinguishes software releases from artifact releases

**Dependencies.** Phases 7 through 9.

**Risks**

- *Artifact files bloat the repository.* Mitigation: decide the distribution channel before
  the first release, not after.
- *An artifact ships without its run record.* Mitigation: the release checklist gates on it.

---

## Cross-cutting requirements

These apply to every phase from Phase 2 onward:

- Correctness before speed; benchmark before optimizing
- Streaming over loading; memory use independent of corpus size
- Errors are explicit and typed; library paths do not panic
- Configuration is data, not code; experiments are described by config plus corpus release
- Every experiment produces a run record
- Documentation is updated in the same change as the behaviour it describes

## Open questions carried into Phase 2

Decisions deliberately not made in Phase 1, listed so they are not silently assumed:

1. Crate and binary names, and whether the library and CLI are separate crates.
2. Whether to depend on the `tokenizers` crate's trainer or only its model and pipeline
   types.
3. Exact minimum supported Rust version.
4. Distribution channel for release artifacts.
5. Whether the corpus preparation step (producing training text and the evaluation holdout
   from a corpus release) lives here or stays in SomNLP-Corpus.
6. Whether a `NOTICE` file is needed alongside the Apache 2.0 license.
