# Changelog

All notable changes to Somali-Tokenizer are documented here.
Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning
follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Two version lines are tracked separately and must not be conflated:

- **Software version**: the Rust library and CLI. Semantic versioning applies. That is
  what the headings below refer to.
- **Tokenizer artifact version**: the identity of a released vocabulary and merge table
  (`somali-bpe-v2` and successors). Defined in
  [docs/REPRODUCIBILITY.md](docs/REPRODUCIBILITY.md). An artifact change is recorded in an
  `### Tokenizer artifacts` section, never by renumbering a software release.

## [Unreleased]

The project is pre-implementation. No release has been published, no Rust code exists in
this repository, and no tokenizer artifact has been trained here.

### Added

- Initial repository specification and Phase 1 documentation set
- Project overview, ecosystem boundaries, and honest-claims policy (`README.md`)
- Contribution guidelines, including the required tokenizer-impact statement for pull
  requests (`CONTRIBUTING.md`)
- Phase-by-phase execution plan with goals, tasks, deliverables, acceptance criteria,
  dependencies, and risks (`PLAN.md`)
- Release direction from v0.1 to v1.0, plus exploratory research directions (`ROADMAP.md`)
- Intended Rust software architecture, module boundaries, and dependency direction
  (`docs/ARCHITECTURE.md`)
- Tokenizer specification and design rationale for the ByteLevel BPE reference design,
  with documented invariants (`docs/TOKENIZER_DESIGN.md`)
- Training specification: corpus sourcing, deterministic evaluation split, streaming
  requirements, and metadata capture (`docs/TRAINING.md`)
- Benchmark methodology, metric definitions, and the recorded reference baseline
  (`docs/BENCHMARKING.md`)
- Reproducibility protocol, run-record schema, and artifact-versus-software versioning
  rules (`docs/REPRODUCIBILITY.md`)
- Rust development conventions, toolchain expectations, and dependency policy
  (`docs/DEVELOPMENT.md`)
- Phase 1 completion review checklist (`docs/FINAL_REVIEW.md`)
- Example experiment configuration for the 48k ByteLevel BPE design
  (`configs/bpe_48k.example.toml`)
- Apache 2.0 license and a `.gitignore` shaped for the eventual Rust project

### Notes

- The benchmark figures recorded in this repository were measured by the Python reference
  implementation in [SomNLP-Corpus](https://github.com/goobolabs/SomNLP-Corpus) under
  `tokenizer/`. They are recorded here as the target for Phase 7 parity validation. No
  measurement in this repository was produced by Rust code.
