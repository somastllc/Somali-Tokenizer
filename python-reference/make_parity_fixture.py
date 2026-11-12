#!/usr/bin/env python3
"""Write the fixtures the Rust implementation is checked against.

All outputs go to ``tests/fixtures/`` at the repository root:

* ``parity_v2.jsonl`` -- token ids, tokens, and decoded text for the fixture corpus plus
  adversarial strings, encoded with the shipped v2 artifact.
* ``mini_release.jsonl`` -- the fixture corpus reshaped as release records
  (``id``, ``text``, ``provenance.source``), plus three records the holdout rule selects;
  the input to the checks below.
* ``parity_train_2000.json`` -- a 2,000-token tokenizer trained by the production path
  (prepare_corpus.py holdout and line splitting, then train.py's builder).
* ``parity_benchmark.json`` -- benchmark.py's report on the fixture corpus (evaluation
  records with a top-level ``source``) for v1 and v2.
* ``parity_stats.json`` -- tokenize_corpus.py's report on the fixture corpus for v1 and v2.
* ``parity_eval_split.json`` -- which of a fixed set of ids the holdout rule selects.

The Rust and Python-binding test suites require identical results, so behavioural drift
between the reference and the Rust implementation fails a test rather than going unnoticed.
Regenerate only when an artifact, a reference script, or the pinned ``tokenizers`` version
changes.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import tokenizers
from tokenizers import Tokenizer

from common import (
    DEFAULT_TOKENIZER,
    DEFAULT_TOKENIZER_V2,
    REPO_ROOT,
    add_repo_path_arg,
    is_eval_doc,
    iter_corpus_lines,
    iter_jsonl_docs,
    resolve_under_repo,
)
from train import build_tokenizer

FIXTURE_CORPUS = Path(__file__).resolve().parent / "tests" / "fixtures" / "mini_corpus.jsonl"
FIXTURES = REPO_ROOT / "tests" / "fixtures"
SCRIPTS = Path(__file__).resolve().parent
TRAIN_VOCAB_SIZE = 2_000
EVAL_SPLIT_PROBES = 20_000
HELD_OUT_RECORDS = 3

# Inputs that stress whitespace handling, the byte alphabet, and special-token matching.
ADVERSARIAL = [
    "Soomaaliya waa dal ku yaal Geeska Afrika, oo leh taariikh hodan ah.",
    "  leading and trailing spaces  ",
    "tabs\there\tand\tthere",
    "double  spaces   and    more",
    "line one\nline two\n\nparagraph break",
    "emoji 🙂 and 中文 and runes ᚠᚢᚦ",
    "⟨url⟩ iyo ⟨email⟩ sentinels",
    "reer binu Israa 'iil",
    "Af-Soomaali iyo Garaad Gacmeed",
    "digits 1234567890 and punctuation !?;:.,",
    "Café decomposed accent",
    "<|endoftext|>Qoraal cusub<|im_start|>user",
    "",
    "   ",
    "\n",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    add_repo_path_arg(parser)
    parser.add_argument("--tokenizer", type=Path, default=DEFAULT_TOKENIZER_V2)
    parser.add_argument("--output-dir", type=Path, default=FIXTURES)
    return parser.parse_args()


def write_encodings(tokenizer: Tokenizer, corpus: list[str], output: Path) -> None:
    with output.open("w", encoding="utf-8") as handle:
        for text in ADVERSARIAL + corpus:
            encoding = tokenizer.encode(text)
            record = {
                "text": text,
                "ids": encoding.ids,
                "tokens": encoding.tokens,
                "decoded": tokenizer.decode(encoding.ids, skip_special_tokens=False),
            }
            handle.write(json.dumps(record, ensure_ascii=False) + "\n")


def write_release_corpus(output: Path) -> None:
    with (
        FIXTURE_CORPUS.open(encoding="utf-8") as source,
        output.open("w", encoding="utf-8") as handle,
    ):
        for line in source:
            if not line.strip():
                continue
            record = json.loads(line)
            reshaped = {
                "id": record["id"],
                "text": record["text"],
                "provenance": {"source": record["source"]},
            }
            handle.write(json.dumps(reshaped, ensure_ascii=False) + "\n")
        for record in held_out_records():
            handle.write(json.dumps(record, ensure_ascii=False) + "\n")


def held_out_records() -> list[dict[str, object]]:
    """Records the holdout rule selects, with text that would add merges if trained on.

    None of the fixture corpus ids fall in the 0.8% split, so without these the training
    parity check could not tell whether the holdout was applied.
    """
    ids = (f"heldout:{index}" for index in range(10_000))
    chosen = [doc_id for doc_id in ids if is_eval_doc(doc_id)][:HELD_OUT_RECORDS]
    text = "Xqzzv xqzzv xqzzvaal xqzzvaal xqzzvoow. " * 20
    return [
        {"id": doc_id, "text": text.strip(), "provenance": {"source": "heldout"}}
        for doc_id in chosen
    ]


def write_trained(release: Path, output: Path) -> None:
    """Mirror prepare_corpus.py (holdout, one line per paragraph) then train.py."""
    with tempfile.TemporaryDirectory() as tmp:
        text_path = Path(tmp) / "train.txt"
        with text_path.open("w", encoding="utf-8", newline="\n") as handle:
            for doc in iter_jsonl_docs(release):
                if not is_eval_doc(doc.doc_id):
                    handle.write(doc.text + "\n")
        tokenizer, trainer = build_tokenizer(TRAIN_VOCAB_SIZE, min_frequency=2)
        trainer.show_progress = False
        tokenizer.train_from_iterator(iter_corpus_lines(text_path), trainer=trainer)
    tokenizer.save(str(output))


def run_script(*args: str) -> None:
    subprocess.run([sys.executable, *args], check=True, cwd=SCRIPTS, capture_output=True)


def write_eval_split(output: Path) -> None:
    ids = [f"doc:{index}" for index in range(EVAL_SPLIT_PROBES)]
    payload = {"probes": EVAL_SPLIT_PROBES, "held_out": [i for i in ids if is_eval_doc(i)]}
    output.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def strip_machine_fields(path: Path) -> None:
    """Drop absolute paths, timings, and fingerprints so the fixture is portable."""
    report = json.loads(path.read_text(encoding="utf-8"))
    for key in ("eval_split", "corpus", "corpus_fingerprint", "elapsed_seconds", "jobs"):
        report.pop(key, None)
    path.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def main() -> None:
    args = parse_args()
    tokenizer_path = resolve_under_repo(args.repo_root, args.tokenizer)
    out = resolve_under_repo(args.repo_root, args.output_dir)
    out.mkdir(parents=True, exist_ok=True)
    tokenizer = Tokenizer.from_file(str(tokenizer_path))

    with FIXTURE_CORPUS.open(encoding="utf-8") as handle:
        corpus = [json.loads(line)["text"] for line in handle if line.strip()]

    write_encodings(tokenizer, corpus, out / "parity_v2.jsonl")
    release = out / "mini_release.jsonl"
    write_release_corpus(release)
    write_trained(release, out / "parity_train_2000.json")
    write_eval_split(out / "parity_eval_split.json")

    specs = ["--tokenizer", f"v1={DEFAULT_TOKENIZER}", "--tokenizer", f"v2={tokenizer_path}"]
    run_script(
        "benchmark.py", "--eval", str(FIXTURE_CORPUS), *specs, "--no-baseline",
        "--output", str(out / "parity_benchmark.json"),
    )  # fmt: skip
    run_script(
        "tokenize_corpus.py", "--corpus", str(release), *specs, "--jobs", "1",
        "--output", str(out / "parity_stats.json"),
    )  # fmt: skip

    strip_machine_fields(out / "parity_benchmark.json")
    strip_machine_fields(out / "parity_stats.json")

    print(f"Wrote parity fixtures to {out} (tokenizers {tokenizers.__version__})")


if __name__ == "__main__":
    main()
