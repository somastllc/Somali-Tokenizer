# Phase 1 Completion Review

A checklist for closing Phase 1 (specification and documentation) and the record of its
actual state. Phase 1 is defined in [../PLAN.md](../PLAN.md).

Reviewed against the repository as it stands. An unchecked box below is a real gap, not a
formality.

---

## Repository

- [x] `README.md` accurately explains the project, its scope, and its current status
- [x] License exists (Apache 2.0, Goobo Labs, 2026)
- [x] Contribution guide exists, including the required tokenizer-impact statement
- [x] Roadmap exists and marks future research as exploratory
- [x] Execution plan exists with per-phase goals, tasks, deliverables, acceptance criteria,
      dependencies, and risks
- [x] Changelog exists and marks the project pre-implementation, with no fabricated releases
- [x] `.gitignore` is appropriate for the eventual Rust project and excludes corpus data,
      training artifacts, and benchmark output without excluding project configuration
- [x] Example experiment configuration exists with no machine-specific paths and no secrets

## Tokenizer specification

- [x] Algorithm specified (ByteLevel BPE)
- [x] Vocabulary size specified (48,000, inclusive of special tokens and byte alphabet)
- [x] Minimum merge frequency specified (2)
- [x] Normalizer specified (Unicode NFC)
- [x] Pre-tokenizer specified (ByteLevel, `add_prefix_space = false`, `use_regex = true`)
- [x] Decoder specified (ByteLevel)
- [x] Post-processor specified (ByteLevel, `trim_offsets = true`)
- [x] Initial alphabet specified (all 256 bytes)
- [x] Unknown-token policy specified, and why one is unnecessary
- [x] Special-token set documented in full, in order, with ids, copied from the reference
      implementation and verified against the shipped artifact
- [x] Vocabulary composition identity documented (`merges = vocab_size − 272`), verified
      against the shipped 48k artifact (48,000 vocabulary / 47,728 merges / 16 added tokens)
- [x] Hugging Face export fields documented
- [x] Benchmark baseline documented with its evaluation set and provenance
- [x] Invariants documented (I1-I6) and stated in testable form

## Research methodology

- [x] Train / evaluation separation documented, including the deterministic split rule
- [x] Leakage prevention requirements stated
- [x] Benchmark methodology documented precisely enough to reimplement: metric formulas,
      percentile convention, zero-word handling, rounding, and encoding conventions
- [x] Mean versus corpus-level tokens per word explained mathematically
- [x] Vocabulary sweep recorded, with the trade-off that motivated 48,000
- [x] Per-source reporting documented, with the reference figures
- [x] Comparison baselines recorded, with an explicit statement of what they do not show
- [x] Reproducibility metadata documented as a run-record field list
- [x] Artifact versioning separated from software versioning
- [x] Artifact immutability stated
- [x] Known limitations documented in `README.md`, `TOKENIZER_DESIGN.md`, and
      `BENCHMARKING.md`
- [x] Known reproducibility hazards documented

## Architecture

- [x] Rust architecture planned: layers, modules, dependency direction
- [x] Rationale for building on the `tokenizers` crate, with its costs and the conditions
      that would change it
- [x] Streaming, memory safety, determinism, and error-handling requirements stated
- [x] Separation of training from inference documented
- [x] Separation of benchmarking from tokenizer logic documented
- [x] Proposed source layout documented **without being created**
- [x] Corpus dependency separated: no corpus data in this repository, releases referenced
      by identity and fingerprint
- [x] SomLTK responsibility separated: general Somali NLP explicitly out of scope
- [x] No source implementation written

## Phase 1 non-goals, verified

- [x] No `src/` directory
- [x] No Rust source files
- [x] No Python implementation
- [x] No `Cargo.toml` or build target
- [x] No tokenizer artifacts committed
- [x] No corpus data committed
- [x] No benchmark executed in this repository
- [x] No claim that Rust parity has been achieved
- [x] No invented benchmark results: every figure traces to the reference implementation's
      recorded output
- [x] SomNLP-Corpus and SomLTK unmodified

Verified by inspection of the repository tree: 15 files, no `.rs`, no `.py`, no
`Cargo.toml`.

## Documentation quality

- [x] Measured and planned claims are distinguished throughout
- [x] Every quoted figure names its source (Python reference implementation, held-out
      evaluation set, eleven-source corpus release of 2026-08-31)
- [x] Reference figures are consistent across every document that quotes them
- [x] Ownership is clear: `TOKENIZER_DESIGN.md` owns the specification, `BENCHMARKING.md`
      owns metric definitions, and other documents link rather than restate
- [x] Internal relative links resolve
- [x] No exaggerated efficiency claims; the tokenizer-efficiency-is-not-model-quality
      caveat appears wherever a comparison is made

---

## Consistency check

Values confirmed identical wherever they appear:

| Quantity | Value |
|----------|-------|
| Algorithm | ByteLevel BPE |
| Vocabulary | 48,000 |
| Minimum merge frequency | 2 |
| Normalization | NFC |
| Mean tokens/word | 1.3528 |
| Median tokens/word | 1.2857 |
| P95 tokens/word | 1.8571 |
| Corpus-level tokens/word | 1.3763 |
| Bytes/token | 4.8069 |
| Round-trip fidelity | 100% (49,424 of 49,424) |
| Unknown-token documents | 0 |
| Evaluation set | 49,424 documents / 4,933,796 words |
| Baselines | XLM-RoBERTa-base 1.8233 · BERT-base-uncased 2.6291 |

---

## Deferred to Phase 2 and later

Open decisions, recorded so they are decided rather than assumed. The full list is in
[../PLAN.md](../PLAN.md).

1. Crate and binary names; whether the library and CLI are one crate or two.
2. Exact minimum supported Rust version, checked against the `tokenizers` crate.
3. Whether to use the `tokenizers` crate's trainer or only its model and pipeline types.
4. Whether corpus preparation lives here or stays in SomNLP-Corpus.
5. Resolution of the `mtime` component in the corpus fingerprint, which is machine-local
   ([TRAINING.md](TRAINING.md) §6).
6. Distribution channel for release artifacts.
7. Run-record and benchmark-report serialization formats.
8. Whether a `NOTICE` file accompanies the Apache 2.0 license.

Phase 2 must also **re-verify the special-token list and pipeline parameters against the
reference implementation** before training the first Rust artifact, and correct
[TOKENIZER_DESIGN.md](TOKENIZER_DESIGN.md) if the reference has changed. The values here
were read from the reference on 2026-09-02.

---

## Notes for whoever picks this up

Three things are easy to get wrong and are documented specifically because of it:

1. **The reference tokenizer was trained on the eleven-source SomNLP-Corpus build of
   2026-08-31, not the current thirteen-source build.** Parity work must pin that release
   or state plainly that it compares across releases. See [TRAINING.md](TRAINING.md) §1.
2. **The pre-tokenizer's `add_prefix_space = false` is not mirrored by the decoder and
   post-processor**, which carry library defaults of `true`. Reproduce the asymmetry; do
   not tidy it. See [TOKENIZER_DESIGN.md](TOKENIZER_DESIGN.md) §1.
3. **The training text is not one document per line.** Counting its lines as documents
   produces numbers that are not comparable with anything, and this has already happened
   once upstream. See [TRAINING.md](TRAINING.md) §3.

---

## Phase 1 Status

**READY FOR PHASE 2.**

Every required Phase 1 deliverable exists, the specification is internally consistent, the
non-goals were verified by inspecting the repository tree, and the decisions that were not
made are listed above rather than left implicit.

Phase 2 begins with the Rust project bootstrap, including re-verification of the reference
specification.
