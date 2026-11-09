#!/usr/bin/env python3
"""Write the encoding fixture the Rust implementation is checked against.

Encodes the fixture corpus plus a set of adversarial strings with the shipped v2 artifact
and records the token ids, tokens, and decoded text for each. The Rust and Python-binding
test suites load this file and require identical output, so any behavioural drift between
the reference and the Rust implementation fails a test rather than going unnoticed.

Regenerate only when the v2 artifact or the pinned ``tokenizers`` version changes.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import tokenizers
from tokenizers import Tokenizer

from common import DEFAULT_TOKENIZER_V2, REPO_ROOT, add_repo_path_arg, resolve_under_repo

FIXTURE_CORPUS = Path(__file__).resolve().parent / "tests" / "fixtures" / "mini_corpus.jsonl"
DEFAULT_OUTPUT = REPO_ROOT / "tests" / "fixtures" / "parity_v2.jsonl"

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
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    tokenizer_path = resolve_under_repo(args.repo_root, args.tokenizer)
    output = resolve_under_repo(args.repo_root, args.output)
    tokenizer = Tokenizer.from_file(str(tokenizer_path))

    with FIXTURE_CORPUS.open(encoding="utf-8") as handle:
        corpus = [json.loads(line)["text"] for line in handle if line.strip()]

    output.parent.mkdir(parents=True, exist_ok=True)
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

    count = len(ADVERSARIAL) + len(corpus)
    print(f"Wrote {count} records to {output} (tokenizers {tokenizers.__version__})")


if __name__ == "__main__":
    main()
