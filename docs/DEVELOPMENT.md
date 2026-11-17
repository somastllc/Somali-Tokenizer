# Development

Conventions for the Rust implementation.

**Status.** In force. The workspace is `crates/somali-tokenizer` (library and CLI) and
`bindings/python` (the Python extension). Edition 2021, `max_width = 100`, MSRV 1.85 for
the default features; the `http` feature (Hub downloads, used by the Python package) needs
1.88.

Python package, from `bindings/python/`:

```bash
uv venv && . .venv/bin/activate
uv pip install maturin pytest transformers
maturin develop          # builds the extension into the virtual environment
cargo build              # the CLI parity test needs target/debug/somali-tokenizer
pytest
```

---

## 1. Toolchain

| Tool | Expectation |
|------|-------------|
| Rust | Stable channel. Minimum supported version pinned in Phase 2, consistent with the `tokenizers` crate's own minimum |
| `cargo` | Build, test, and dependency management |
| `rustfmt` | Required. Configuration in `rustfmt.toml` |
| `clippy` | Required, warnings denied in CI |

Goobo Labs practice elsewhere is edition 2021, `max_width = 100`, and a pinned clippy MSRV.
Following it keeps contributors moving between repositories without surprises. Confirm and
record the exact values in Phase 2 rather than assuming them here.

## 2. Commands

Once the project exists:

```bash
cargo build                     # debug build
cargo build --release           # optimized build; use this for any timing measurement
cargo test                      # unit and integration tests
cargo clippy -- -D warnings     # lint, warnings are errors
cargo fmt                       # format
cargo fmt --check               # verify formatting; this is what CI runs
```

CI runs `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test`. A pull request
that fails any of them is not ready for review.

Never quote a performance number from a debug build.

---

## 3. Dependencies

**Policy: a dependency needs a stated purpose and no smaller alternative.** Each one is
code the project ships, a supply-chain surface, and a compatibility constraint. Adding one
is a reviewable decision, and the pull request says why.

Likely dependencies, with the phase that justifies each. None is committed until the phase
that needs it:

| Crate | Purpose | Needed by |
|-------|---------|-----------|
| `tokenizers` | BPE model, ByteLevel pipeline, trainer, artifact format | Phase 4 |
| `serde` | Deriving serialization for config, run records, reports | Phase 3 |
| `serde_json` | Artifacts, evaluation JSONL, reports | Phase 3 |
| `toml` | Configuration files | Phase 3 |
| `clap` | CLI argument parsing | Phase 8 |
| `thiserror` | Typed library errors | Phase 2 |
| `anyhow` | Error context at the CLI boundary only | Phase 2 |
| `unicode-normalization` | NFC outside the tokenizer pipeline, if needed for verification | Phase 5 |
| `sha2` / `blake2` | Artifact checksums; the split-rule hash | Phase 4 |
| `indicatif` | Progress reporting on long runs | Phase 8 |
| `rayon` | Data parallelism, **only where measured to matter** | Deferred |

Notes on the ones with conditions:

- **`thiserror` in the library, `anyhow` at the CLI boundary.** Library callers need to
  match on error variants; the CLI only needs to print. Using `anyhow` in library paths
  makes errors unmatchable for consumers.
- **`unicode-normalization` may be unnecessary.** The tokenizer's own normalizer handles
  NFC. A separate dependency is justified only where normalization is needed outside the
  tokenizer, for instance computing `NFC(x)` to assert the round-trip invariant.
- **`rayon` is deferred deliberately.** Parallelism must not change results
  ([ARCHITECTURE.md](ARCHITECTURE.md) §5), and the underlying crate already parallelizes
  training internally. Add it only after profiling shows a bottleneck it fixes, and only
  where determinism is preserved.
- **Development dependencies** for property testing or benchmarking harnesses are held to
  the same standard, though the bar for a dev-only dependency is lower.

Prefer the standard library. Prefer one dependency that does the job over three that
each do part of it.

---

## 4. Coding principles

### Explicit errors, no panics in library paths

No `unwrap`, `expect`, `panic!`, `todo!`, or array indexing that can go out of bounds on
any library path. This includes handling of malformed artifacts, malformed JSONL, truncated
files, and adversarial text. A library returns errors; the CLI decides what to print and
which exit code to use.

`expect` is acceptable in tests and in code proving a genuine invariant, where the message
states why the case is unreachable.

### Stream, do not load

The corpus does not fit in memory and never will. Every stage reads incrementally. Memory
use must be independent of corpus size, verified by measurement. Retain scalars, not text.

### Predictable configuration

Configuration is data. Defaults equal the reference design. Unknown fields are rejected
rather than ignored. Otherwise a typo in a config field silently changes the experiment.
Validation errors name the offending field and say what was expected.

### Clear library / CLI separation

The library prints nothing and never exits the process. The CLI contains no tokenizer
logic. Every CLI capability is reachable through the library API.

### Test the invariants

The invariants in [TOKENIZER_DESIGN.md](TOKENIZER_DESIGN.md) are executable requirements,
not documentation. Each has a test, and the adversarial input set is part of the suite.

### Benchmark before optimizing

No optimization without a measurement showing it matters. Timing numbers come from release
builds. State what was measured, on what input, on what hardware.

### Correctness before speed

The reference implementation's v1 tokenizer was fast and produced unusable output for a
generative model. Correctness first: round-trip, byte coverage, stable ids. Then speed.

### Comment the why

Code shows what it does. Comments carry the reason a non-obvious choice was made: a
surprising default, a workaround, a constraint from the artifact format. Do not narrate the
obvious.

---

## 5. Testing

Details of what must be tested are in [../CONTRIBUTING.md](../CONTRIBUTING.md). The
mechanics:

- **Fast and hermetic by default.** `cargo test` must not need the multi-gigabyte corpus or
  network access. Train a tiny tokenizer on a small committed fixture; the reference suite
  does this and completes in under a second.
- **Fixtures stay small.** Kilobytes, spanning the corpus sources so real merges are
  exercised.
- **Tests needing a real corpus release** are feature-gated or ignored by default, and
  documented as such.
- **Invariant tests are not optional** and are written alongside the code they constrain,
  not afterwards.

---

## 6. Documentation

- Public items carry rustdoc. Examples compile under `cargo test`.
- Behaviour changes update the relevant document in the same change.
- [TOKENIZER_DESIGN.md](TOKENIZER_DESIGN.md) owns the specification;
  [BENCHMARKING.md](BENCHMARKING.md) owns metric definitions. Link to them instead of
  restating them.
- Mark measured versus planned. Do not describe unimplemented behaviour in the present
  tense.

---

## 7. Working with the Python reference

The reference implementation lives in `python-reference/`, imported from
[Somali NLP Corpus](https://github.com/somastllc/somali-nlp-corpus) with its history. It is
the behavioural target.

- **Change it only to keep it runnable** (paths, pinned versions). Its behaviour is the
  specification the Rust code is tested against.
- `python-reference/make_parity_fixture.py` regenerates `tests/fixtures/`. Run it only when
  an artifact, a reference script, or the pinned `tokenizers` version changes, and commit
  the fixtures with the change that required them.
- When this project's documentation and the reference disagree, the reference is
  authoritative about what *is*, and this repository's documents are authoritative about
  what *should be*. Reconcile explicitly; do not let both stand.
