# Training

How a Somali tokenizer is trained, and what a training run must record. This document
specifies the procedure; [TOKENIZER_DESIGN.md](TOKENIZER_DESIGN.md) owns the tokenizer
parameters and [BENCHMARKING.md](BENCHMARKING.md) owns the metrics.

**Status.** Specification. The pipeline described here is Phase 4 of [../PLAN.md](../PLAN.md)
and does not exist in this repository yet. The procedure mirrors the Python reference
implementation in SomNLP-Corpus `tokenizer/`.

---

## 1. Input: a versioned corpus release

Training text comes from a **versioned SomNLP-Corpus release**. This repository never
bundles, vendors, or commits corpus data: not the training text, not the evaluation
holdout, not samples of either.

An experiment names its corpus release in configuration and records the release identity
plus a fingerprint in its run record. Two runs against different corpus releases are
different experiments and their numbers are not comparable, even when every tokenizer
parameter is identical.

### The reference corpus release

The reference tokenizer `somali-bpe-v2` (48k) was trained on the **eleven-source
SomNLP-Corpus build of 2026-08-31**:

| | Documents | Words |
|---|---:|---:|
| Corpus release | 6,154,594 | 600,085,996 |
| Training split | 6,105,170 | 595,152,200 |
| Evaluation holdout | 49,424 | 4,933,796 |

Sources: `mc4`, `hplt`, `nllb`, `madlad`, `cc100`, `xlsum`, `wikipedia`, `mt560`, `opus`,
`quran`, `quran-tanzil`.

**SomNLP-Corpus has since moved on.** Its current build (2026-09-02) has thirteen sources,
7,352,961 documents, and 665,985,672 words. The reference tokenizer was *not* trained on
that build, and its benchmark was *not* measured against it. Any parity work (Phase 7) must
pin the eleven-source release, or state plainly that it is comparing across corpus
releases. Silently mixing the two would produce a difference that looks like an
implementation bug and is not one.

---

## 2. Pipeline

```text
SomNLP-Corpus release
       │
       ▼
verify corpus release identity and fingerprint
       │
       ▼
prepare tokenizer training text
       │
       ▼
split training / evaluation (deterministic, id-hashed)
       │
       ▼
NFC normalization
       │
       ▼
stream corpus into the trainer
       │
       ▼
train ByteLevel BPE
       │
       ▼
save tokenizer artifact  ──▶  run record
       │
       ▼
benchmark on the held-out split
       │
       ▼
validate invariants (round-trip, byte alphabet, special tokens)
       │
       ▼
release artifact (immutable)
```

Each stage is specified below.

---

## 3. Corpus preparation

The preparation step reads a corpus release and writes two artifacts, both local and both
untracked:

**Training text.** Plain UTF-8 text. Paragraph structure is preserved, so a multi-paragraph
document occupies several lines.

> **This file is not one document per line.** In the reference release, 1,293,692 of
> 6,105,170 training documents (21.2%) contain at least one internal newline. Treating its
> lines as documents is a measurement error, and it is one that has already been made once
> in this project's history: an earlier published figure scored lines and was reported as a
> per-document number. Any code that counts documents must use the JSONL holdout, never the
> training text.

**Evaluation holdout.** JSONL, one record per line, with `id`, `source`, and `text`. JSONL
rather than plain text specifically so that documents containing newlines stay addressable
as single records, and so provenance survives for per-source reporting.

Preparation applies three transformations and nothing else:

1. **NFC normalization**, stabilizing composed characters so merge counts are consistent.
2. **Paragraph collapse**, reducing runs of three or more newlines to a single blank line.
3. **Empty-document filtering**, dropping records whose text is blank after trimming.

Corpus-level sentinels for masked URLs and emails are **preserved**, so the tokenizer can
learn representations for them rather than being surprised by them at inference time.

Preparation must not do cleaning, filtering, deduplication, or language identification.
Those belong upstream in SomNLP-Corpus. If preparation is removing content, the corpus
release is the wrong input.

**Open decision (Phase 2/4).** Whether preparation lives in this repository or stays in
SomNLP-Corpus is deliberately unresolved. It reads corpus internals, which argues for
upstream; it defines the evaluation split that this repository's metrics depend on, which
argues for here.

---

## 4. Train / evaluation separation

**The evaluation holdout must never be used to learn merges.** This is the one rule in this
document that cannot be traded away for convenience. A tokenizer benchmarked on text it was
trained on reports its memorization, not its compression.

The split is deterministic and index-free: a document is held out when

```text
blake2b_8(document_id) mod 1000 < 8
```

which holds out 0.8% of documents, 49,424 of 6,154,594 in the reference release.

Properties this buys:

- **No stored index.** The split is recomputable from a document id alone.
- **Stability across rebuilds.** A document whose id is unchanged stays in the same split
  when the corpus is rebuilt, so evaluation sets remain comparable across corpus releases
  to the extent that ids are stable.
- **Composition preservation.** Hashing ids rather than sampling by source keeps the
  holdout's source mix close to the training mix, within 0.3 percentage points in the
  reference release.
- **Auditability.** Leakage is checkable: assert that no evaluation id was written to the
  training text.

Leakage prevention requirements for the implementation:

- The split rule is applied at preparation time, in one pass, before any text reaches the
  trainer.
- The trainer is given the training text only. There must be no code path that feeds the
  holdout to a trainer.
- A run record states the split rule, its parameters, and both split sizes, so a reviewer
  can confirm what was held out.
- When the split rule changes, previously published benchmark numbers become incomparable
  and must be remeasured, not carried forward.

---

## 5. Streaming and memory

Corpus releases are multi-gigabyte. The pipeline streams:

- The training text is read line by line and handed to the trainer in batches (10,000 lines
  in the reference implementation). Batching exists because per-item hand-off across a
  language boundary was the throughput bottleneck; a Rust implementation may not need the
  same batch size, but must still avoid materializing the corpus.
- The evaluation holdout is streamed record by record. Only per-document scalars are
  retained, never the document text. An earlier implementation held the sampled documents
  in memory and would have needed an estimated 10-14 GB at full corpus size.

**Requirement.** Peak resident memory must be independent of corpus size, and this must be
verified rather than assumed: train on inputs differing by an order of magnitude and
compare peak memory.

The trainer's own working set is not independent of corpus size, since pair counts scale
with vocabulary and corpus diversity. That is expected. What must not happen is holding the
corpus itself.

---

## 6. Corpus fingerprinting and the staleness guard

Training against stale prepared text is a silent failure: it produces a valid artifact from
the wrong corpus. The reference implementation guards against it by recording a fingerprint
of the source corpus at preparation time and refusing to train when it no longer matches.

Reference fingerprint fields:

| Field | Meaning |
|-------|---------|
| `path` | Source corpus path |
| `size_bytes` | Exact file size |
| `mtime_ns` | Modification time, nanoseconds |
| `edge_sha256` | SHA-256 over the first and last 64 KiB |

Hashing the edges rather than the whole file keeps the check near-instant on a multi-GB
corpus while still catching a regenerated or truncated corpus.

**Requirements.**

- Preparation records a fingerprint; training verifies it and refuses to proceed on a
  mismatch.
- The refusal is overridable only by an explicit flag, and the override is recorded in the
  run record.
- The fingerprint appears in the run record of every training run.

**Known weakness, to be resolved in Phase 4.** `mtime_ns` makes the fingerprint
machine-local: copying the corpus to another machine changes it, and the check then fails
for a file that is byte-identical. Copying `mtime` preserves it, but that is a property of
how the file was moved, not of its content. Options are to treat `mtime_ns` as advisory and
match on `size_bytes` plus `edge_sha256`, or to add a full content hash as an opt-in. The
decision must be made and documented rather than inherited by accident.

---

## 7. Training run

With prepared text and a verified fingerprint:

1. Build the tokenizer pipeline exactly as specified in
   [TOKENIZER_DESIGN.md](TOKENIZER_DESIGN.md): BPE model, NFC normalizer, ByteLevel
   pre-tokenizer with `add_prefix_space = false`, ByteLevel decoder, ByteLevel
   post-processor.
2. Configure the trainer: target vocabulary size, minimum merge frequency 2, the
   256-symbol initial alphabet, and the special-token list in its specified order.
3. Reject a target vocabulary that does not exceed the 272 non-merge entries, with a clear
   error naming the floor.
4. Stream the training text in batches.
5. Save the artifact.
6. Write a run record (see [REPRODUCIBILITY.md](REPRODUCIBILITY.md)).

### Determinism and seeds

BPE training as specified is deterministic: merges are selected greedily by frequency, so
the same corpus, configuration, and library version produce the same merge list. There is
no sampling step and therefore no seed to set. BPE dropout is not used and must not be
enabled during training of a release artifact.

Two caveats worth stating rather than discovering:

- **Tie-breaking** among equally frequent pairs is a library implementation detail. It is
  stable for a given library version but is not part of the specification, and it is a
  plausible source of small differences between implementations or versions. Record the
  library version.
- **Parallelism** must not change results. If a parallel implementation makes merge
  selection order-dependent, that is a defect, not a performance trade-off.

Where a future stage does introduce randomness (corpus subsampling, for example), a seed
becomes mandatory and must be recorded in the run record.

---

## 8. Vocabulary sweeps

Because BPE merges are greedy, merge `k` does not depend on the target vocabulary size: a
larger run's merge list is a superset of a smaller run's, with the smaller list as a prefix.

Two consequences:

1. **Sweeps are cheap.** Train once at the largest candidate size, then derive smaller
   candidates by truncating the merge list and the vocabulary to the target size, dropping
   added tokens whose ids exceed it. This avoids re-reading the corpus once per candidate,
   which otherwise dominates sweep cost.
2. **Sweeps are interpretable.** Candidates differ only in where the shared merge list was
   cut, so a sweep isolates vocabulary size and nothing else.

**Requirement.** A derived tokenizer must be verified equal to a natively trained one at
the same size (identical vocabulary and identical encodings on a fixture), and that
equivalence must be a test, not an assumption. Derivation must refuse to grow a tokenizer.

The reference sweep covered 16,384 / 32,000 / 48,000 / 65,536; results are in
[TOKENIZER_DESIGN.md](TOKENIZER_DESIGN.md).

---

## 9. Metadata capture

Every training run writes a run record. The full field list is in
[REPRODUCIBILITY.md](REPRODUCIBILITY.md); the training-specific requirements are:

- Corpus release identity and fingerprint, plus training and evaluation document and word
  counts
- The complete effective configuration, including defaults that were not written by hand
- Tokenizer library name and version, and Rust toolchain version
- Whether the staleness guard was overridden
- Wall-clock duration, peak memory if measured, and hardware summary (CPU model, core
  count, available RAM)
- Resulting artifact path, vocabulary size, merge count, and checksum

Hardware metadata is recorded for cost and performance context. It is not a correctness
input: a run on different hardware must produce the same artifact, and if it does not, that
is a defect worth the run record proving.

---

## 10. Retraining policy

New corpus releases arrive. They may justify retraining: a larger, more balanced corpus
plausibly yields a better merge table, and the sweep may move.

The rules:

- Retraining produces a **new tokenizer artifact version**. It never replaces a released
  artifact in place. Models trained against the old artifact depend on its exact ids.
- A new artifact version ships with its own benchmark run on its own corpus release's
  holdout. Numbers from a previous release are not carried forward.
- Comparing a new artifact against an old one requires scoring both on the same evaluation
  set. A "improvement" measured across two different holdouts is not a measurement.
- The old artifact stays available and unmodified, so its published figures remain
  reproducible.

Retraining is justified by: a materially different corpus (new sources, large size change,
composition shift), a specification change, or a defect in the artifact. It is not
justified by a new software version alone; a Rust reimplementation that produces the same
artifact produces the same tokenizer version.
