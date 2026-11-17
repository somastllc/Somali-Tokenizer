# Reproducibility

What it takes for a result in this project to be reproducible, and how tokenizer artifacts
are versioned.

**Status.** Specification. Run records are produced by the training and benchmark pipelines
(Phases 4 and 6 of [../PLAN.md](../PLAN.md)), neither of which exists yet.

---

## 1. The rule

**A number without a run record is an anecdote.**

Any figure quoted in a document, issue, pull request, or release must be traceable to a
recorded run. This is not bureaucracy: a tokenizer's numbers depend on the corpus release,
the exact configuration, the library version, and the evaluation split, and every one of
those has changed at least once in this project's short history.

---

## 2. Run record

Every training run and every benchmark run writes a machine-readable record. Field names
below are the requirement; their exact serialization is a Phase 4 decision.

### Software identity

| Field | Notes |
|-------|-------|
| `project_version` | Version of this repository's software |
| `git_commit` | Full commit hash; mark explicitly if the tree was dirty |
| `rust_version` | Toolchain version used to build |
| `tokenizer_library` | Name and exact version of the tokenization library |
| `command` | The command line that produced the run, verbatim |
| `run_timestamp` | Start time, UTC, ISO 8601 |

### Tokenizer configuration

| Field | Notes |
|-------|-------|
| `algorithm` | `bpe` |
| `vocab_size` | Target vocabulary, inclusive of special tokens and byte alphabet |
| `min_frequency` | Minimum merge frequency |
| `normalization` | e.g. `nfc` |
| `pre_tokenizer` | Type and its parameters, including `add_prefix_space` and `use_regex` |
| `decoder` | Type and its parameters |
| `post_processor` | Type and its parameters, including `trim_offsets` |
| `initial_alphabet` | e.g. `bytelevel-256` |
| `special_tokens` | The full ordered list, not a count |
| `effective_config` | The complete resolved configuration, including unset defaults |

`effective_config` matters more than it looks: most reproduction failures come from a
default that differed, not from a value someone typed.

### Corpus

| Field | Notes |
|-------|-------|
| `corpus_release` | Release identifier of the Somali NLP Corpus build |
| `corpus_fingerprint` | Fingerprint as specified in [TRAINING.md](TRAINING.md) |
| `train_documents` / `train_words` | Training split size |
| `split_rule` | Rule and parameters, e.g. `blake2b_8(id) mod 1000 < 8` |
| `staleness_override` | Whether the freshness guard was bypassed |

### Evaluation

| Field | Notes |
|-------|-------|
| `eval_dataset` | Which holdout, from which corpus release |
| `eval_documents` / `eval_words` | Evaluation split size |
| `roundtrip_checked` | Number of documents verified |
| `benchmark_output` | The full metric set, per tokenizer scored |

### Environment and artifact

| Field | Notes |
|-------|-------|
| `machine` | OS, CPU model, core count, RAM |
| `elapsed_seconds` | Wall clock |
| `peak_memory_bytes` | If measured |
| `artifact_path` | Where the artifact was written |
| `artifact_sha256` | Checksum of the artifact file |
| `artifact_vocab_size` / `artifact_merge_count` | Read back from the artifact, not from config |

Reading vocabulary and merge counts back from the saved artifact rather than echoing the
configuration is deliberate: it catches the case where the trainer could not reach the
target size.

Hardware is recorded for cost context, not as a correctness input. The same configuration
on different hardware must produce the same artifact; if it does not, the run records are
what makes that discoverable.

---

## 3. Two version lines

This is the most important distinction in the document, and the easiest to get wrong.

```text
software implementation version          tokenizer artifact version
    (the Rust library and CLI)               (a released vocabulary
     e.g. 0.1.0, 0.2.0, 1.0.0                 and merge table)
                                              e.g. somali-bpe-v1, somali-bpe-v2
    semantic versioning                       immutable, model-facing
    changes freely                            changes rarely and deliberately
```

**They are independent.**

- A Rust rewrite that produces a byte-identical artifact is a **software** change only. It
  does **not** create a new tokenizer version. The tokenizer's identity is its vocabulary,
  merge rules, and pipeline configuration, not the language its trainer was written in.
- A retrained artifact on a new corpus is a **tokenizer** version change, even if not one
  line of software changed.
- Both can happen at once, and the changelog says so as two separate entries.

Tokenizer artifact identity is deliberately not tied to an implementation language,
timestamp, or repository. `somali-bpe-v2` names a specific tokenizer; whether it was
produced by Python or Rust is metadata about how, not about what.

### What forces a new tokenizer artifact version

Any change to:

- the vocabulary or the merge rules,
- normalization,
- pre-tokenization,
- decoding or post-processing,
- the special-token set, its order, or its ids.

Anything that alters the mapping from text to token ids, or from token ids back to text,
is a new version. See invariants I3 through I5 in
[TOKENIZER_DESIGN.md](TOKENIZER_DESIGN.md).

### What does not

- Reimplementing the software in another language, given an identical artifact
- Performance work, refactoring, CLI changes, new commands
- Documentation, packaging, or export tooling that leaves the artifact unchanged
- Adding a benchmark or a metric

---

## 4. Artifact identity and immutability

A released artifact is **immutable**. It is never edited in place: not to add a token, not
to fix a defect, not to retrain on a newer corpus. Models trained against it depend on
exact ids, and a silently modified artifact corrupts them in ways that surface far from the
cause.

Defects are fixed by releasing a new version. The defective artifact stays available and
unmodified so its published figures remain reproducible. This is not hypothetical: the
upstream v1 tokenizer has a decoder defect that makes it unusable for generation, and it is
retained unchanged for exactly this reason.

A tokenizer release consists of:

| Component | Purpose |
|-----------|---------|
| The artifact file | Vocabulary, merge rules, and pipeline configuration |
| Its SHA-256 checksum | Identity and integrity |
| The training run record | How it was produced |
| The benchmark report | What it measures on a named holdout |
| An interoperability export | Ecosystem-loadable form, if applicable |

An artifact without its run record is not a release. The release checklist gates on this
(Phase 10).

---

## 5. What "reproducible" means here

Two different standards, and conflating them causes arguments:

**Bit-level reproducibility.** The same command, on the same pinned corpus release, with
the same library version, produces a byte-identical artifact. This is the goal for a
release: BPE as specified is deterministic, with no sampling and no seed. Where it does not
hold, the cause must be identified (a library version difference, a tie-breaking change, a
corpus that was not actually pinned) rather than tolerated as noise.

**Behavioural reproducibility.** A different implementation, or a different corpus release,
produces a tokenizer that behaves equivalently: it satisfies the same invariants, its
metrics are close, and any difference is measured and explained. This is the standard for
Phase 7 parity between the Python reference and Rust.

**The reference figures are not a target to hit.** Mean 1.3528 is a recorded measurement of
a specific artifact on a specific holdout. A Rust implementation loading *that same
artifact* must reproduce those numbers, because the artifact and the library are the same;
that is a hard requirement. A Rust implementation *retraining* on the same corpus should
land near them, and any difference must be attributed, not massaged toward the published
figure.

---

## 6. Reproducing a published result

The minimum a third party needs:

1. The corpus release identifier and its fingerprint
2. The configuration file, or the recorded `effective_config`
3. The command line
4. The software version or commit
5. The tokenization library version and Rust toolchain version
6. The expected artifact checksum
7. The expected benchmark output

If a published result cannot supply all seven, that is a defect in the record, and saying
so is more useful than reproducing the number by coincidence.

---

## 7. Known reproducibility hazards

Recorded so they are checked rather than rediscovered:

1. **Corpus drift.** Somali NLP Corpus releases change. The eleven-source build the reference
   tokenizer was trained on is not the current build. Pin and fingerprint the release; do
   not assume "the corpus" means one thing over time.
2. **`mtime` in the fingerprint.** The reference fingerprint includes modification time,
   which makes it machine-local: a byte-identical copy on another machine fails the check.
   See [TRAINING.md](TRAINING.md) §6; the resolution is a Phase 4 decision.
3. **Library tie-breaking.** Merge selection among equally frequent pairs is a library
   implementation detail, stable per version but not part of the specification. Record the
   version.
4. **Unit confusion.** The training text is not one document per line. Scoring its lines
   produces numbers that are not comparable with document-level results, and this has
   already happened once upstream.
5. **Silent staleness.** Training on prepared text that predates the corpus release
   produces a valid artifact from the wrong data. The freshness guard exists because this
   occurred; overrides must be recorded.
6. **Metric ambiguity.** Mean versus corpus-level tokens per word, and the percentile
   convention, both change the reported number without changing the tokenizer. Both are
   pinned in [BENCHMARKING.md](BENCHMARKING.md).
7. **Default drift.** A configuration default that changes between versions silently
   changes results. This is why the full `effective_config` is recorded, not just the
   fields a user set.
