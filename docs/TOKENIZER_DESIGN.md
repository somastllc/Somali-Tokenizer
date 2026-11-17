# Tokenizer Design

This document owns the tokenizer specification. Other documents link here rather than
restating it.

**Status.** The design below describes **Somali ByteLevel BPE v2**, the Python reference
tokenizer, now in `python-reference/` (imported from
[Somali NLP Corpus](https://github.com/somastllc/somali-nlp-corpus)). Every parameter here was
read from the reference implementation and its shipped artifact; none was inferred. The
Rust crate enforces this design on load (`crates/somali-tokenizer/src/contract.rs`) and
trains with it (`training.rs`).

---

## 1. Specification

| Component | Value |
|-----------|-------|
| Algorithm | Byte Pair Encoding (BPE) |
| Target vocabulary | 48,000 (inclusive of special tokens and the byte alphabet) |
| Minimum merge frequency | 2 |
| Normalizer | Unicode NFC |
| Pre-tokenizer | ByteLevel |
| Decoder | ByteLevel |
| Post-processor | ByteLevel |
| Initial alphabet | All 256 byte symbols |
| Unknown token | None |
| BPE dropout | None |
| Continuing-subword prefix | None |
| End-of-word suffix | None |
| Byte fallback | Not used (unnecessary; the alphabet is already complete) |

Component parameters, as serialized in the reference artifact:

| Component | Parameters |
|-----------|------------|
| Pre-tokenizer `ByteLevel` | `add_prefix_space = false`, `use_regex = true`, `trim_offsets = true` |
| Decoder `ByteLevel` | library defaults (`add_prefix_space = true`, `use_regex = true`, `trim_offsets = true`) |
| Post-processor `ByteLevel` | `trim_offsets = true`, remaining fields at library defaults |

The asymmetry is intentional and load-bearing. `add_prefix_space = false` **on the
pre-tokenizer** is what keeps the round-trip byte-exact: no synthetic leading space is
introduced at encode time. The decoder and post-processor were constructed with library
defaults, which serialize `add_prefix_space = true`; that is the reference behaviour and
must be reproduced as-is rather than "corrected" for symmetry. A Rust implementation that
sets these three fields to the same value is not reproducing the reference.

### Vocabulary composition

For a target vocabulary of size `V`:

```text
non-merge entries = 256 (byte alphabet) + 16 (special tokens) = 272
merge rules       = V - 272
```

Measured on the shipped 48k artifact: 48,000 vocabulary entries, 47,728 merge rules,
16 added tokens. This identity is a useful sanity check on any artifact claiming to
implement this design.

### Special tokens

Sixteen tokens, occupying ids 0 through 15 in this exact order:

| Id | Token | Role |
|---:|-------|------|
| 0 | `<\|endoftext\|>` | Document boundary; also exported as BOS, EOS, and the nominal UNK |
| 1 | `<\|pad\|>` | Padding |
| 2 | `<\|im_start\|>` | Chat-template turn start |
| 3 | `<\|im_end\|>` | Chat-template turn end |
| 4-15 | `<\|reserved_0\|>` … `<\|reserved_11\|>` | Reserved slots for future control tokens |

This list was copied from the reference implementation (`tokenizer/common.py`,
`SPECIAL_TOKENS_V2`) and verified against the shipped artifact's `added_tokens`. It is
authoritative as written. Phase 2 must re-verify it against the reference before the first
Rust artifact is trained, and correct this table if the reference has moved.

Three properties matter and are tested:

1. **Order is identity.** Special tokens occupy the lowest ids. Reordering them changes the
   artifact and breaks every model trained against it.
2. **Reserved slots are not padding for its own sake.** They exist so chat-template control
   tokens can be added later without resizing an embedding matrix or shifting merge ids.
3. **The nominal UNK is a compatibility affordance, not a fallback.** The Hugging Face
   export declares `<|endoftext|>` as `unk_token` because downstream loaders expect the
   field to be populated. The tokenizer itself has no unknown token and cannot emit one.

The set is decoder-LM oriented. The earlier v1 tokenizer used a BERT-style set
(`<unk>`, `[CLS]`, `[SEP]`, `<pad>`, `[MASK]`); that set is not carried forward and new
work must not mix the two.

### Hugging Face export

The reference export writes `tokenizer.json`, `tokenizer_config.json`, and
`special_tokens_map.json`, with:

| Field | Value |
|-------|-------|
| `tokenizer_class` | `PreTrainedTokenizerFast` |
| `bos_token` / `eos_token` / `unk_token` | `<\|endoftext\|>` |
| `pad_token` | `<\|pad\|>` |
| `model_max_length` | 8192 |
| `clean_up_tokenization_spaces` | `false` |

`clean_up_tokenization_spaces = false` is required: the cleanup pass rewrites whitespace
around punctuation and would break the round-trip guarantee.

---

## 2. Design rationale

### Why ByteLevel?

Because it makes losslessness structural rather than best-effort.

A ByteLevel pre-tokenizer maps text to a 256-symbol byte alphabet before any merging.
Every possible input is therefore already representable, so:

- there is no out-of-alphabet character and no need for an unknown token;
- decoding is the exact inverse of encoding, because no information was discarded at the
  pre-tokenization step;
- unseen scripts, emoji, control characters, and corrupt-looking byte sequences all encode
  and decode without loss, if inefficiently.

The alternative the project actually tried was a whitespace pre-tokenizer (v1). It failed
on both counts: its decoder expected an end-of-word suffix its trainer never emitted, so
decoding concatenated tokens and lost every space (`"Soomaaliya waa dal"` decoded as
`"Soomaaliyawaadal"`), and it emitted `<unk>` on four held-out documents. For a
classification encoder that damage is invisible. For a generative model it is
disqualifying. ByteLevel removes the entire class of failure.

### Why BPE?

- It is the algorithm the reference implementation uses, so choosing it minimizes the
  number of dimensions in which the first Rust release can differ.
- Its merges are greedy and order-independent of the target size, which makes vocabulary
  sweeps cheap and interpretable: candidate sizes differ only in where a single shared
  merge list is cut, not in which merges were learned.
- It is well supported, widely understood, and directly compatible with decoder-LM tooling.

BPE was inherited, not benchmarked against alternatives on Somali. A controlled comparison
against Unigram is listed as future research in [../ROADMAP.md](../ROADMAP.md), not as a
settled question.

### Why NFC?

Unicode allows the same visible text to be encoded several ways (for example `á` as one
code point, or as `a` followed by a combining acute accent). Without normalization those
forms are different byte sequences, so they receive different merges and different token
ids for identical text, wasting vocabulary and making counts inconsistent.

NFC is chosen specifically because the upstream corpus is already NFC-normalized, so the
tokenizer's normalizer agrees with the corpus convention and is close to a no-op on
in-distribution text.

The consequence is stated openly in the invariants below: the round-trip guarantee is
exact **with respect to NFC**, not with respect to arbitrary byte sequences. Decomposed
input comes back composed. The normalizer is part of the tokenizer by design.

### Why 48,000?

Measured, on the held-out evaluation set of 49,424 documents / 4,933,796 words. Because
BPE merges are greedy, these four candidates differ only in where the shared merge list was
cut, so the comparison isolates vocabulary size exactly:

| Vocabulary | Mean tokens/word | Median | P95 | Corpus-level | Bytes/token |
|-----------:|-----------------:|-------:|----:|-------------:|------------:|
| 16,384 | 1.5210 | 1.4438 | 2.1429 | 1.5460 | 4.2792 |
| 32,000 | 1.4058 | 1.3333 | 2.0000 | 1.4302 | 4.6258 |
| **48,000** | **1.3528** | **1.2857** | **1.8571** | **1.3763** | **4.8069** |
| 65,536 | 1.3195 | 1.2558 | 1.8000 | 1.3423 | 4.9287 |

Returns diminish steadily:

| Step | Reduction in mean tokens/word | Additional vocabulary |
|------|------------------------------:|----------------------:|
| 16,384 → 32,000 | 7.6% | +95% |
| 32,000 → 48,000 | 3.8% | +50% |
| 48,000 → 65,536 | 2.5% | +36% |

### Why not 65,536, which fragments less?

Because sequence length is not the only cost. A larger vocabulary buys shorter sequences
and pays for them in parameters:

```text
larger vocabulary
   ├─▶ fewer tokens per document          (cheaper attention, more content per context window)
   └─▶ larger embedding and output matrices
          └─▶ more parameters, more memory, more compute in the output projection
```

The embedding and output projection each scale linearly with vocabulary size. Moving from
48,000 to 65,536 is 36% more vocabulary rows on both, in exchange for 2.5% off the mean
tokens-per-word. For a small or mid-sized Somali model, where the embedding is a
significant share of total parameters, that trade is not obviously good.

48,000 was selected as the practical trade-off point, not as an optimum. This is a
judgement about deployment economics, and it is corpus- and model-dependent: 65,536 remains
a reasonable choice where context length dominates the parameter budget, and the measured
sweep above is retained so the decision can be revisited with real numbers rather than
re-derived.

---

## 3. Mean versus corpus-level tokens per word

Two different averages appear throughout this project. They are not interchangeable, and
quoting one where the other is meant is a real reporting error.

For a document `d`, let `T(d)` be its token count and `W(d)` its whitespace-delimited word
count, and define the per-document ratio `R(d) = T(d) / W(d)`.

**Mean tokens per word**: the unweighted mean of per-document ratios, over the `N`
documents with `W(d) > 0`:

```text
mean_TPW = (1/N) * Σ R(d)
```

**Corpus-level tokens per word**: total tokens divided by total words:

```text
corpus_TPW = Σ T(d) / Σ W(d)
```

The second is a *word-weighted* mean of the same ratios:

```text
corpus_TPW = Σ ( W(d) / Σ W ) * R(d)
```

So `mean_TPW` treats every document as one observation, and `corpus_TPW` treats every word
as one observation. They coincide only when `R(d)` is uncorrelated with `W(d)`.

On the reference evaluation set they do not coincide: 1.3528 against 1.3763. The gap is
explained by composition. Short parallel sentences from `nllb` are 72.7% of the evaluation
documents but only 11.1% of its words, and they fragment relatively little (1.3159). Long
web documents from `mc4` are 9.6% of documents but 36.9% of words, and fragment more
(1.4683). The unweighted mean is dominated by the short sentences; the word-weighted figure
is dominated by the long web documents. Longer documents fragment slightly more, so
`corpus_TPW > mean_TPW`.

Which to use:

- **`mean_TPW`** to compare tokenizers on a fixed evaluation set. It is the primary
  reported metric and is what "1.3528" refers to throughout this repository.
- **`corpus_TPW`** to estimate real token volume for a corpus, for example projecting
  training cost. Multiplying a word count by `corpus_TPW` is correct; multiplying it by
  `mean_TPW` is not.

Both must be reported. Reporting only one invites the confusion this section exists to
prevent.

---

## 4. Invariants

These are the properties the implementation must hold and the test suite must enforce.
Changing any of them creates a new tokenizer version.

**I1. Lossless round-trip.**

```text
decode(encode(x)) == NFC(x)
```

for all input `x`, including whitespace runs, tabs, paragraph breaks, emoji, unseen
scripts, corpus sentinels such as `⟨url⟩` and `⟨email⟩`, apostrophe clitics, empty strings,
and whitespace-only strings. Round-trip equality is asserted against the NFC-normalized
input, not the raw input, because the normalizer is part of the tokenizer.

**I2. No data loss for arbitrary UTF-8 text.** The 256-byte alphabet is complete in the
vocabulary; no input can fall back to an unknown token, because no unknown token exists. An
implementation must verify alphabet completeness on load rather than assume it.

**I3. Special-token ids are stable for a release.** Every special token encodes to exactly
one id, survives decoding when special tokens are retained, and keeps that id for the life
of the artifact. Models trained against an artifact depend on these ids.

**I4. Released artifacts are immutable.** A published artifact is never edited in place:
not to add a token, not to fix a defect, not to retrain on a newer corpus. Defects are
fixed in a new version, and the old artifact stays available so its published figures stay
reproducible. (The defective v1 artifact is retained upstream for exactly this reason.)

**I5. Changing vocabulary or merge rules creates a new tokenizer version.** Also true for
changes to normalization, pre-tokenization, decoding, or the special-token set. A change
that leaves the artifact byte-identical (a Rust rewrite of the surrounding software, for
instance) does **not** create a new tokenizer version. See
[REPRODUCIBILITY.md](REPRODUCIBILITY.md) for how the two version lines are kept apart.

**I6. Default decoding drops control tokens.** `decode(ids)` omits special tokens;
`decode(ids, skip_special_tokens = false)` retains them. I1 is stated with respect to the
latter.

---

## 5. Somali-specific considerations

Recorded because they shape what the merge table learns, not because the tokenizer contains
any Somali-specific rule. It does not: the design is entirely corpus-driven.

- **Orthography.** Somali uses the Latin alphabet with digraphs (`dh`, `kh`, `sh`) and
  consonants (`x`, `q`, `c`) whose frequencies differ sharply from English. Merges learned
  on English text do not align with them.
- **Agglutination.** High-frequency particles (`waa`, `oo`, `ka`, `ku`, `ay`) and verbal
  morphology produce recurrent suffix chains that BPE merges into stable multi-character
  tokens when they are attested at scale.
- **Apostrophe clitics.** Forms such as `Israa 'iil` place apostrophes inside word spans.
  Under ByteLevel pre-tokenization, whether merges span or respect a clitic boundary is
  decided by corpus frequency, not by a hand-written rule.
- **Corpus sentinels.** Masked `⟨url⟩` and `⟨email⟩` spans introduced by corpus cleaning
  appear in roughly 16% of documents. Whether they become single tokens is a function of
  vocabulary size, not a design decision.
- **Register skew.** Religious and formulaic sources contribute a small fraction of corpus
  words and correspondingly few merges, which is why they remain the hardest sources to
  tokenize efficiently. See [BENCHMARKING.md](BENCHMARKING.md) for the per-source figures.

---

## 6. Known limitations of this design

1. **Web-crawl dominance.** The merge table is shaped by what contributes words, and web
   crawl contributes the overwhelming majority. The tokenizer is, in effect, a web-register
   Somali tokenizer.
2. **Latin-script assumption.** The corpus is Latin-script Somali. Arabic-script Somali is
   out of distribution: it encodes losslessly under ByteLevel, but inefficiently.
3. **Whitespace word definition.** The headline metric depends on splitting on whitespace,
   which handles clitics and attached punctuation inconsistently. Bytes-per-token is
   reported alongside as a definition-free compression measure.
4. **Efficiency is not model quality.** Nothing in this design or its measurements says
   anything about downstream accuracy. That is a separate experiment.
5. **Baseline comparisons isolate fragmentation.** BERT and XLM-R use different normalizers
   and pre-tokenizers; comparing token counts compares tokenizers, not models.
6. **Vocabulary size is corpus-dependent.** 48,000 is the trade-off point on one corpus
   release. A more balanced or larger corpus could move it, and the sweep would need
   rerunning rather than reasoning about.
7. **Corpus composition affects merges directly.** Word share, not document share, governs
   which merges are learned. A source that is 72% of documents but 11% of words has
   correspondingly little influence.
