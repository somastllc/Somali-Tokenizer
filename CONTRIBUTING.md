# Contributing to Somali-Tokenizer

Thank you for helping build Somali language technology. This document describes how to
propose changes and what a reviewable contribution looks like.

The Rust library, CLI, and Python package exist and are tested; see
[PLAN.md](PLAN.md) for which phases are complete. Every rule below applies to them.

## Before you start

Read, in this order:

1. [README.md](README.md): what the project is and is not
2. [PLAN.md](PLAN.md): the current phase and its acceptance criteria
3. [docs/TOKENIZER_DESIGN.md](docs/TOKENIZER_DESIGN.md): the tokenizer specification
4. [docs/BENCHMARKING.md](docs/BENCHMARKING.md): how claims about a tokenizer are measured

Open an issue before starting anything non-trivial. "Non-trivial" includes any change to
the tokenizer specification, the benchmark protocol, the config schema, or the dependency
set.

## Scope

This repository owns learned subword tokenization: training a vocabulary, encoding,
decoding, benchmarking, and releasing tokenizer artifacts.

Out of scope, with the right home:

| Proposal | Belongs in |
|----------|-----------|
| New corpus source, cleaning rule, deduplication change | Somali NLP Corpus |
| Linguistic word or sentence segmentation, stemming, POS tagging, morphology | SomLTK |
| Model training, fine-tuning, evaluation of downstream accuracy | The consuming project |

A tokenizer change motivated by a downstream model result is welcome, but the evidence for
it belongs in the issue; this repository still measures tokenizers, not models.

## Issue workflow

Use a title prefix so issues sort usefully:

- `spec:` for a change to the tokenizer specification or invariants
- `impl:` for implementation work against an existing specification
- `bench:` for benchmark methodology or measurement
- `docs:` for documentation only
- `bug:` for something that behaves differently from its specification

An issue proposing a specification change should state: the current documented behaviour,
the proposed behaviour, what it would do to the benchmark numbers, and whether it forces a
new tokenizer artifact version.

## Pull requests

A reviewable pull request:

- Addresses one thing. Split unrelated changes.
- Explains **why**, not only what. The diff shows what.
- Updates documentation in the same change when behaviour or specification changes.
- Passes formatting, lint, and tests (once those exist; see below).
- States its benchmark impact explicitly (see the next section).
- Does not add dependencies without justifying them against the policy in
  [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

Draft PRs are welcome for early feedback. Say what you want reviewed.

## Tokenizer-impact statement (required)

Any pull request that touches tokenizer behaviour, training, or evaluation must include
this block in its description, with every line answered:

```text
Tokenizer impact
- Vocabulary:            changed / unchanged
- Normalization:         changed / unchanged
- Pre-tokenization:      changed / unchanged
- Decoding:              changed / unchanged
- Special tokens:        changed / unchanged
- Benchmark numbers:     changed / unchanged  (attach before/after if changed)
- Round-trip behaviour:  changed / unchanged
- Model compatibility:   compatible / breaking for existing checkpoints
- New artifact version required: yes / no
```

"Unchanged" is a claim, not a default. If you cannot demonstrate it, write "unknown" and
say what would settle it.

Any `changed` on vocabulary, normalization, pre-tokenization, decoding, or special tokens
means **a new tokenizer artifact version**, not an edit to an existing one. Released
artifacts are immutable; see [docs/REPRODUCIBILITY.md](docs/REPRODUCIBILITY.md).

## Backwards compatibility

Two independent compatibility surfaces, and they must not be conflated:

1. **Tokenizer artifacts.** A model trained against a tokenizer depends on exact token ids.
   Changing vocabulary, merge rules, or special-token ids silently breaks every checkpoint
   trained against it. Such changes always create a new artifact version.
2. **The Rust library and CLI.** Ordinary semantic versioning applies. Breaking API changes
   need a major-version bump and a note in [CHANGELOG.md](CHANGELOG.md).

A change to (2) that leaves (1) byte-identical is not a tokenizer version change. A change
to (1) is not necessarily a library API change. Say which you are making.

## Formatting and lint

Once the Rust project exists (Phase 2):

```bash
cargo fmt            # required; CI checks `cargo fmt --check`
cargo clippy -- -D warnings
cargo test
```

Documentation: Markdown, wrapped at a reasonable width, no trailing whitespace. Use
fenced code blocks with a language tag. Prefer tables over prose for parameter lists.

## Documentation expectations

- Every documented number needs a source: which tokenizer, which evaluation set, which
  run. A figure with no provenance will be asked for one.
- Distinguish **measured** from **planned**. If it has not been run, say so.
- Do not restate the same specification in several documents. Link to the one that owns it.
  [docs/TOKENIZER_DESIGN.md](docs/TOKENIZER_DESIGN.md) owns the tokenizer specification;
  [docs/BENCHMARKING.md](docs/BENCHMARKING.md) owns metric definitions.
- No invented benchmark results, no invented citations, no describing unimplemented
  features in the present tense.

## Testing expectations (Phase 5 onward)

When implementation code lands, tests are part of the change, not a follow-up:

- **Invariant tests** for the properties in
  [docs/TOKENIZER_DESIGN.md](docs/TOKENIZER_DESIGN.md), including `decode(encode(x)) ==
  NFC(x)` over an adversarial input set, complete byte-alphabet coverage, absence of an
  unknown token, and stable single-id encoding for every special token.
- **Unit tests** for configuration parsing, corpus reading, and metric computation.
- **Fast and hermetic by default.** The default test suite must not require the multi-GB
  corpus or network access. Train a tiny tokenizer on a small committed fixture instead.
- Tests that need a real corpus release must be feature-gated or ignored by default and
  documented as such.

Fix the implementation, not the test, unless the test encodes the wrong expectation,
in which case say so explicitly in the PR.

## Benchmark expectations

- Any claim that a change improves or preserves tokenization efficiency needs a benchmark
  run on the documented held-out evaluation set, with before and after numbers.
- Report the full metric set, not the one that moved favourably: mean, median, P95,
  corpus-level tokens per word, bytes per token, round-trip fidelity, unknown-token rate.
- Report which corpus release the evaluation set came from. Numbers from different corpus
  releases are not comparable and must not be presented in the same table without a note.
- Never compare a document-level measurement against a line-level one. This exact mistake
  invalidated an earlier published figure in the reference implementation.

## Reproducibility expectations

Any experiment whose result is quoted in an issue, PR, or document must have a run record
containing the fields listed in [docs/REPRODUCIBILITY.md](docs/REPRODUCIBILITY.md),
including corpus release and fingerprint, configuration, tool versions, and the artifact
checksum. A number without a run record is an anecdote.

## Data, artifacts, and repository hygiene

- **No corpus files in Git.** Not the training text, not the evaluation holdout, not
  samples of either, compressed or otherwise. Reference a Somali NLP Corpus release instead.
- **No generated tokenizer artifacts** unless the artifact is part of an approved release,
  in which case it is added deliberately, with its run record, under the release path.
- **No secrets, tokens, or machine-specific absolute paths** in configs, docs, or code.
- Small text fixtures for tests are fine and should stay small: kilobytes, not megabytes.

## Commit hygiene

Conventional-commit style, one logical change per commit:

```text
<type>: <short description>

<body: why the change, what it affects, anything a reviewer would otherwise have to ask>
```

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`.

- Present tense, imperative mood, no trailing period in the subject.
- Reference the issue when there is one.
- Do not mix a formatting sweep with a behavioural change.

## Security

Do not open a public issue for a security problem. Report it privately to the Somast
maintainers and allow time for a fix before disclosure.

Relevant classes of problem for this project: unbounded memory or disk consumption from
untrusted input, path traversal through configuration-supplied paths, panics reachable from
library calls on adversarial input, and deserialization of untrusted tokenizer artifacts.
A tokenizer artifact is executable configuration in the sense that it directs how input is
parsed, so treat one from an unknown source as untrusted.

## License

By contributing, you agree that your contributions are licensed under the Apache License,
Version 2.0. See [LICENSE](LICENSE).
