# Benchmarking

This document owns the metric definitions and the evaluation protocol. It is written to be
reimplementable: an engineer should be able to produce comparable numbers from this
specification alone.

**Status.** Implemented as `somali-tokenizer benchmark` and `somali-tokenizer stats`, which
match `python-reference/benchmark.py` and `tokenize_corpus.py` on the fixture corpus. Every
full-corpus figure below was measured by the Python reference; none has been re-measured
by the Rust implementation yet.

---

## 1. Primary metric: tokens per word

For a document `d`:

```text
R(d) = T(d) / W(d)
```

| Symbol | Definition |
|--------|------------|
| `T(d)` | Number of token ids produced by encoding `d`, with no special tokens added |
| `W(d)` | Number of whitespace-delimited spans in `d` |

`W(d)` splits on runs of Unicode whitespace and discards empty spans: the semantics of a
plain "split on whitespace" with no empty results. Punctuation is not separated; `dal,` is
one span.

**This is an operational definition, not a linguistic one.** A whitespace span is not a
Somali word: clitics, hyphenation, and attached punctuation are all handled by accident
rather than by analysis. The metric is a comparison instrument between tokenizers on
identical text, and it is only valid used that way. Bytes-per-token is reported alongside
precisely because it needs no definition of "word".

---

## 2. Reported metrics

For each tokenizer scored, over an evaluation set of `N` documents:

| Metric | Definition |
|--------|------------|
| Mean tokens/word | Unweighted mean of `R(d)` over documents with `W(d) > 0` |
| Median tokens/word | Median of the same `R(d)` values |
| P95 tokens/word | 95th percentile of the same values, nearest-rank (below) |
| Corpus-level tokens/word | `Σ T(d) / Σ W(d)` |
| Total tokens | `Σ T(d)` |
| Bytes per token | `Σ bytes_utf8(d) / Σ T(d)` |
| Round-trip fidelity | `1 − failures / checked` |
| Unknown-token documents | Documents whose encoding contains an unknown-token id |
| Per-source tokens/word | Per source: `Σ T(d) / Σ W(d)` over that source's documents |

Precise rules, so two implementations agree:

- **Zero-word documents.** Documents with `W(d) = 0` are excluded from the ratio
  distribution (mean, median, P95) and from per-source aggregates, but are still counted in
  the document count, total tokens, and total bytes. Bytes-per-token therefore includes
  them.
- **Percentile rule.** Nearest-rank on the ascending sorted ratio list of length `n`:
  index `= round(0.95 × (n − 1))`, clamped to `[0, n − 1]`. No interpolation. Other
  percentile conventions give different answers on the same data; this one is the
  specification.
- **Per-source figures are word-weighted, not means of ratios.** They are corpus-level
  ratios computed within each source. Do not compare them against the headline *mean*
  tokens/word without saying so; the two are different averages (see
  [TOKENIZER_DESIGN.md](TOKENIZER_DESIGN.md) §3).
- **Bytes** are UTF-8 bytes of the document text as scored, before normalization changes
  anything, counted once per document regardless of how many tokenizers are scored.
- **Rounding.** Report ratios to 4 decimal places and fidelity to 6. Round only for
  reporting, never during accumulation.
- **No special tokens are added** during encoding. `T(d)` counts content tokens only,
  including for baseline tokenizers, which would otherwise be penalized for their
  `[CLS]`/`[SEP]` conventions.

### Mean versus corpus-level

Both must be reported. They are different averages of the same ratios (unweighted per
document, versus weighted by words), and they differ whenever ratio correlates with
document length, as it does on this corpus. The full explanation, with the reference
numbers, is in [TOKENIZER_DESIGN.md](TOKENIZER_DESIGN.md) §3. In short: use the mean to
compare tokenizers, use the corpus-level figure to project token volume.

### Round-trip fidelity

For each checked document:

```text
pass  ⟺  decode(encode(d), skip_special_tokens = false) == NFC(d)
```

Comparison is against the NFC-normalized document, because the normalizer is part of the
tokenizer (invariant I1 in [TOKENIZER_DESIGN.md](TOKENIZER_DESIGN.md)).

Checking is capped at a configurable number of documents (100,000 in the reference run,
which covered the entire 49,424-document evaluation set, hence 100% coverage, not a
sample). Report both `checked` and `failures`; a fidelity figure without its denominator is
not interpretable.

### Unknown-token rate

Count documents whose encoding contains the tokenizer's unknown-token id. For a ByteLevel
tokenizer with a complete byte alphabet there is no such id and the count is zero **by
construction**. The metric is a regression guard proving the property still holds, not a
discovery. Report it anyway: it is how a misconfigured artifact announces itself.

For a tokenizer that has no unknown token, report `0`, not `n/a`.

---

## 3. Protocol

1. **Fix the evaluation set.** The held-out split of a named corpus release, produced by
   the deterministic split rule in [TRAINING.md](TRAINING.md). It must not have been seen
   by any tokenizer being scored, and any tokenizer for which that is not true must be
   flagged in the report.
2. **Score every tokenizer on identical documents, in one streaming pass.** Not separate
   runs, not different samples. One pass, same documents, same order.
3. **Retain only per-document scalars.** Never accumulate document text. Peak memory must
   be independent of evaluation-set size.
4. **Emit machine-readable output** alongside the human-readable summary, containing the
   full metric set for every tokenizer, plus the metadata identifying the evaluation set
   and the corpus release.
5. **Record the run** as specified in [REPRODUCIBILITY.md](REPRODUCIBILITY.md).

Qualitative inspection (encoding a small fixed set of Somali sentences and printing their
surface tokens, ratios, and round-trip verdict) is a useful complement for spotting
pathologies that aggregates hide. It is not a substitute for the quantitative pass, and its
output does not belong in a results table.

---

## 4. Reference results

Measured by the Python reference implementation on the held-out split of the eleven-source
Somali NLP Corpus release of 2026-08-31: **49,424 documents / 4,933,796 words**, excluded from
v2 training.

### Somali ByteLevel BPE v2 (48k): the reference baseline

| Metric | Value |
|--------|------:|
| Mean tokens/word | 1.3528 |
| Median tokens/word | 1.2857 |
| P95 tokens/word | 1.8571 |
| Corpus-level tokens/word | 1.3763 |
| Bytes/token | 4.8069 |
| Total tokens on the holdout | 6,790,331 |
| Round-trip fidelity | 1.000000 (49,424 of 49,424 checked) |
| Documents emitting an unknown token | 0 |

### Vocabulary sweep

| Tokenizer | Mean | Median | P95 | Corpus-level | Bytes/token | Holdout tokens | Round-trip | UNK docs |
|-----------|-----:|-------:|----:|-------------:|------------:|---------------:|-----------:|---------:|
| v2 16,384 | 1.5210 | 1.4438 | 2.1429 | 1.5460 | 4.2792 | 7,627,628 | 1.000 | 0 |
| v2 32,000 | 1.4058 | 1.3333 | 2.0000 | 1.4302 | 4.6258 | 7,056,243 | 1.000 | 0 |
| **v2 48,000** | **1.3528** | **1.2857** | **1.8571** | **1.3763** | **4.8069** | **6,790,331** | **1.000** | **0** |
| v2 65,536 | 1.3195 | 1.2558 | 1.8000 | 1.3423 | 4.9287 | 6,622,501 | 1.000 | 0 |

Bytes-per-token rises with vocabulary size while tokens-per-word falls, as expected: both
measure the same compression from different directions.

### Comparison baselines

Scored on the identical documents, with no special tokens added:

| Tokenizer | Mean | Median | P95 | Corpus-level | Bytes/token | Holdout tokens |
|-----------|-----:|-------:|----:|-------------:|------------:|---------------:|
| Somali ByteLevel BPE v2 (48k) | 1.3528 | 1.2857 | 1.8571 | 1.3763 | 4.8069 | 6,790,331 |
| XLM-RoBERTa-base | 1.8233 | 1.7692 | 2.4286 | 1.8019 | 3.6716 | 8,890,112 |
| BERT-base-uncased | 2.6291 | 2.6154 | 3.2667 | 2.6157 | 2.5292 | 12,905,309 |

On this text the Somali tokenizer fragments about **1.35x less than XLM-RoBERTa-base** and
**1.94x less than BERT-base-uncased**.

> **What this comparison does and does not show.** It shows that a Somali-trained
> vocabulary segments Somali text into fewer pieces than these multilingual and English
> tokenizers do. It does **not** show that a model using it will be more accurate. BERT and
> XLM-R use different normalizers and pre-tokenizers and were built for different
> objectives; the comparison isolates tokenizer fragmentation on Somali, nothing else.
> Downstream quality requires training models, which is out of scope for this repository.

Round-trip fidelity is not reported for the baselines: they were scored for fragmentation
only, and neither claims a lossless round-trip.

### Against general-purpose tokenizers (FLORES-200, Somali Wikipedia)

A separate comparison, run on 2026-11-18 against the tokenizers of widely used models, on
two public test sets that are not part of the corpus holdout. The full report, with charts
and per-word examples, is
[benchmarks/somast-somali-tokenizer-benchmark.pdf](benchmarks/somast-somali-tokenizer-benchmark.pdf).

**FLORES-200 Somali**, dev and devtest: 2,009 sentences, 45,375 words.

| Tokenizer | Vocabulary | Tokens | Tokens/word | Chars/token | Single-token words | Round-trip | vs this |
|---|---:|---:|---:|---:|---:|---:|---:|
| **somast-bpe-v2 (this)** | **48,000** | **62,338** | **1.374** | **4.71** | **75.2%** | **100.0%** | **1.00×** |
| nllb-200 | 256,204 | 82,483 | 1.818 | 3.56 | 50.8% | 92.4% | 1.32× |
| xlm-roberta | 250,002 | 82,896 | 1.827 | 3.55 | 50.9% | 99.4% | 1.33× |
| o200k_base (GPT-4o) | 200,019 | 89,593 | 1.975 | 3.28 | 45.3% | 100.0% | 1.44× |
| gemma2 | 256,000 | 101,671 | 2.241 | 2.89 | 35.2% | 100.0% | 1.63× |
| llama3-8b | 128,256 | 114,643 | 2.527 | 2.56 | 23.4% | 100.0% | 1.84× |
| cl100k_base (GPT-4) | 100,277 | 115,789 | 2.552 | 2.54 | 22.9% | 100.0% | 1.86× |
| qwen2.5 | 151,665 | 116,525 | 2.568 | 2.52 | 22.9% | 100.0% | 1.87× |

**Somali Wikipedia sample**: 2,000 random paragraphs, 70,696 words.

| Tokenizer | Vocabulary | Tokens | Tokens/word | Chars/token | Single-token words | Round-trip | vs this |
|---|---:|---:|---:|---:|---:|---:|---:|
| **somast-bpe-v2 (this)** | **48,000** | **106,367** | **1.505** | **4.37** | **73.6%** | **100.0%** | **1.00×** |
| xlm-roberta | 250,002 | 132,441 | 1.873 | 3.51 | 51.7% | 83.9% | 1.25× |
| nllb-200 | 256,204 | 133,948 | 1.895 | 3.47 | 49.8% | 79.5% | 1.26× |
| o200k_base (GPT-4o) | 200,019 | 145,655 | 2.060 | 3.19 | 44.6% | 100.0% | 1.37× |
| gemma2 | 256,000 | 165,923 | 2.347 | 2.80 | 34.0% | 100.0% | 1.56× |
| llama3-8b | 128,256 | 182,857 | 2.587 | 2.54 | 23.7% | 100.0% | 1.72× |
| cl100k_base (GPT-4) | 100,277 | 186,066 | 2.632 | 2.50 | 23.1% | 100.0% | 1.75× |
| qwen2.5 | 151,665 | 187,237 | 2.648 | 2.48 | 23.3% | 100.0% | 1.76× |

*Tokens/word* counts whitespace-separated words, with no special tokens (such as BOS)
added. *Single-token words* is the share of words encoded as one token. *Round-trip* is the
share of texts that decode back exactly; NLLB and XLM-R lose exactness through their text
normalization. *vs this* is the total-token ratio against the Somali tokenizer.

On the parallel English FLORES sentences, Somali costs 0.85 tokens per English token with
this tokenizer, against 1.71 with GPT-4o's `o200k_base` and 2.19 with GPT-4's
`cl100k_base`.

How common words split, in pieces per word:

| Word | This tokenizer | GPT-4o | GPT-4 | Llama 3 | XLM-R |
|------|---------------:|-------:|------:|--------:|------:|
| `bilaabaan` | 1 | 3 | 4 | 4 | 3 |
| `tobanaan` | 1 | 2 | 3 | 3 | 3 |
| `marmarsoonaya` | 3 | 4 | 5 | 5 | 4 |
| `Cimaaraadka` | 3 | 4 | 5 | 5 | 4 |

**Method.** FLORES-200 from Meta's original release
(`dl.fbaipublicfiles.com/nllb/flores200_dataset.tar.gz`); Wikipedia from
`wikimedia/wikipedia`, config `20231101.so`. GPT-4 and GPT-4o were run with `tiktoken`;
Llama 3 and Gemma 2 from ungated mirrors carrying the same tokenizers; NLLB with `som_Latn`
as the source language. The script that produced these figures is not in this repository;
the Somali tokenizer's example tokenizations in the report were re-checked against the
released artifact.

**Caveats.** The training corpus is a large Somali web crawl, so Wikipedia text is probably
in it and FLORES could be. The FLORES figure (1.374) is close to the corpus-holdout figure
(1.344 on the 1.0.0 holdout), so there is no sign of a large leak; Wikipedia is the less
independent of the two tests. Only two domains were tested; news, social media, and chat,
where spelling varies more, were not. As with every comparison here, fewer tokens is not a
better model: a tokenizer helps only when a model is trained on it.

### Historical note: v1

The earlier v1 tokenizer (32,000, whitespace pre-tokenizer, `BPEDecoder`) measured 1.3936
mean tokens/word on the same holdout, but with **0.000** round-trip fidelity, failing on
all 49,424 documents, and emitting `<unk>` on 4. Its decoder expected an end-of-word suffix
its trainer never emitted, so decoding concatenated tokens and dropped every space. v1 also
saw an earlier version of the holdout during training, so its efficiency numbers are, if
anything, flattering.

It is recorded here for one reason: it is why the round-trip invariant is a tested
invariant and not an assumption. A tokenizer can look competitive on the headline metric
while being unusable for generation.

---

## 5. Per-source reporting

Word-weighted tokens/word within each source, on the same holdout:

| Source | v2 48k | XLM-R base | Holdout documents | Holdout words |
|--------|-------:|-----------:|------------------:|--------------:|
| `mc4` | 1.4683 | 1.8741 | 4,725 | 1,820,155 |
| `hplt` | 1.3419 | 1.7577 | 4,594 | 1,608,262 |
| `nllb` | 1.3159 | 1.7951 | 35,967 | 549,914 |
| `madlad` | 1.3324 | 1.7661 | 1,086 | 515,619 |
| `cc100` | 1.2413 | 1.7084 | 2,390 | 405,356 |
| `xlsum` | 1.2841 | 1.7244 | 43 | 14,000 |
| `mt560` | 1.2292 | 1.8067 | 384 | 9,040 |
| `wikipedia` | 1.4836 | 1.8331 | 41 | 6,369 |
| `opus` | 1.3521 | 1.8012 | 89 | 2,752 |
| `quran` | 1.5267 | 2.0063 | 54 | 1,274 |
| `quran-tanzil` | 1.5460 | 2.0635 | 51 | 1,055 |

### Why domain variation matters

The spread within a single tokenizer (1.2292 on `mt560` against 1.5460 on `quran-tanzil`,
a 26% difference) is larger than the gap between adjacent vocabulary sizes in the sweep. A
headline average conceals that entirely.

Three practical consequences:

1. **A single average describes the corpus mix, not the language.** Change the mix and the
   headline moves without any tokenizer changing. Per-source reporting is what makes a
   comparison across corpus releases interpretable.
2. **Efficiency tracks training word share.** Religious and formulaic sources contribute a
   fraction of a percent of training words, so few merges are shaped by their orthography,
   and they remain the hardest to tokenize. This is a corpus composition fact, not a
   tokenizer defect, and it is not fixable inside this repository.
3. **Deployment domain matters.** A model serving religious or administrative Somali will
   see worse effective compression than the headline suggests. Users are entitled to that
   number, so report it.

Sources with very few holdout documents (`wikipedia`, `quran`, `quran-tanzil`, `opus`,
`xlsum` here) carry wide uncertainty. Report their counts alongside their ratios and do not
build conclusions on them.

Note that sources are per corpus release: the eleven above are those in the reference
release. Newer Somali NLP Corpus builds add further sources, and per-source tables from
different releases are not directly comparable.

---

## 6. Comparability rules

Violating any of these produces a number that looks like a result and is not one:

- **Same evaluation set.** Tokenizers compared in one table must be scored on identical
  documents from the same corpus release.
- **Same unit.** Documents, not lines and not paragraphs. This project has already
  published a line-level figure as if it were document-level; the resulting number was not
  comparable to anything.
- **Same metric.** Never compare a mean tokens/word against a corpus-level tokens/word.
- **Same encoding conventions.** No special tokens added, on every tokenizer.
- **State the corpus release** for every reported figure.
- **Report the full metric set**, not the subset that moved favourably.

---

## 7. Limitations

1. **Whitespace word definition.** The primary metric depends on an operational definition
   that treats clitics and attached punctuation inconsistently across sources. Bytes/token
   is the definition-free companion measure.
2. **Efficiency is not accuracy.** Nothing here measures downstream model quality.
3. **Baseline mismatch.** BERT and XLM-R differ in normalization and pre-tokenization; the
   comparison isolates fragmentation.
4. **Corpus-bound.** All figures describe one corpus release. The evaluation set inherits
   its web-crawl dominance and Latin-script coverage, so it under-represents edited,
   spoken, administrative, and Arabic-script Somali.
5. **Small per-source samples.** Several sources contribute tens of holdout documents.
6. **Baselines need network access** to fetch their tokenizers; a run without it reports
   the Somali tokenizers only, and that must be visible in the output rather than silently
   omitted.
