"""The Python package must give the same ids as the reference and as transformers."""

from __future__ import annotations

import doctest
import json
import os
import pickle
import threading
import unicodedata

import pytest
import somali_tokenizer
from conftest import RELEASE_V2
from somali_tokenizer import HUB_REPO_V2, ContractError, Encoding, SomaliTokenizer, __version__


@pytest.fixture(scope="module")
def tokenizer() -> SomaliTokenizer:
    return SomaliTokenizer.v2()


def test_matches_the_python_reference(tokenizer, parity_records) -> None:
    for record in parity_records:
        encoding = tokenizer.encode(record["text"])
        assert encoding.ids == record["ids"], record["text"]
        assert encoding.tokens == record["tokens"], record["text"]
        assert tokenizer.decode(encoding.ids) == record["decoded"]


def test_batch_matches_single(tokenizer, parity_records) -> None:
    texts = [record["text"] for record in parity_records]
    batch = tokenizer.encode_batch(texts)
    assert [e.ids for e in batch] == [r["ids"] for r in parity_records]
    assert tokenizer.decode_batch([e.ids for e in batch]) == [r["decoded"] for r in parity_records]


def test_matches_transformers(tokenizer, parity_records) -> None:
    transformers = pytest.importorskip("transformers")
    hf = transformers.AutoTokenizer.from_pretrained(str(RELEASE_V2))
    for record in parity_records:
        assert hf.encode(record["text"]) == tokenizer.encode(record["text"]).ids


def test_roundtrip_is_lossless(tokenizer) -> None:
    for text in ["Soomaaliya waa dal.", "  a\tb\n\nc  ", "Café", "🙂 中文 ᚠᚢᚦ", ""]:
        ids = tokenizer.encode(text).ids
        assert tokenizer.decode(ids) == unicodedata.normalize("NFC", text)


def test_release_file_and_bundled_agree(tokenizer) -> None:
    from_file = SomaliTokenizer.from_file(RELEASE_V2 / "tokenizer.json")
    from_text = SomaliTokenizer.from_str((RELEASE_V2 / "tokenizer.json").read_text("utf-8"))
    text = "Waxaan u baahanahay nidaam ku hadla Af-Soomaali."
    assert from_file.encode(text) == from_text.encode(text) == tokenizer.encode(text)


def test_introspection(tokenizer) -> None:
    assert tokenizer.vocab_size == len(tokenizer) == 48_000
    first_four = ["<|endoftext|>", "<|pad|>", "<|im_start|>", "<|im_end|>"]
    assert tokenizer.special_tokens[:4] == first_four
    assert tokenizer.token_to_id("<|pad|>") == 1
    assert tokenizer.id_to_token(0) == "<|endoftext|>"
    assert tokenizer.token_to_id("<unk>") is None
    assert repr(tokenizer) == "SomaliTokenizer(vocab_size=48000)"
    assert HUB_REPO_V2 == "somast/somali-tokenizer"
    assert __version__


def test_encoding_object(tokenizer) -> None:
    encoding = tokenizer.encode("Nabad")
    assert isinstance(encoding, Encoding)
    assert len(encoding) == len(encoding.ids) == len(encoding.offsets)
    assert encoding.offsets[0][0] == 0
    assert "Encoding(ids=" in repr(encoding)


def test_skip_special_tokens(tokenizer) -> None:
    ids = tokenizer.encode("<|endoftext|>Nabad<|im_end|>").ids
    assert tokenizer.decode(ids, skip_special_tokens=True) == "Nabad"


def test_contract_violation_raises(tokenizer) -> None:
    artifact = json.loads(tokenizer.to_str())
    artifact["pre_tokenizer"]["add_prefix_space"] = True
    with pytest.raises(ContractError, match="add_prefix_space"):
        SomaliTokenizer.from_str(json.dumps(artifact))
    assert issubclass(ContractError, ValueError)


def test_errors_map_to_python_exceptions(tokenizer) -> None:
    with pytest.raises(OSError):
        SomaliTokenizer.from_file("does/not/exist.json")
    with pytest.raises(ValueError):
        SomaliTokenizer.from_str("{not json")
    with pytest.raises(ValueError, match="outside the vocabulary"):
        tokenizer.decode([48_000])
    with pytest.raises(OverflowError):
        tokenizer.decode([-1])


def test_concurrent_use_from_threads(tokenizer) -> None:
    expected = tokenizer.encode("Geeska Afrika").ids
    results: list[list[int]] = []

    def work() -> None:
        for _ in range(200):
            results.append(tokenizer.encode("Geeska Afrika").ids)

    threads = [threading.Thread(target=work) for _ in range(4)]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()
    assert results == [expected] * 800


def test_not_picklable_by_accident(tokenizer) -> None:
    with pytest.raises(TypeError):
        pickle.dumps(tokenizer)


@pytest.mark.skipif(not os.environ.get("SOMALI_TOKENIZER_NETWORK"), reason="needs network")
def test_from_pretrained_matches_bundled(tokenizer) -> None:
    hub = SomaliTokenizer.from_pretrained()
    text = "Soomaaliya waa dal ku yaal Geeska Afrika."
    assert hub.encode(text) == tokenizer.encode(text)


def test_module_docstring_example() -> None:
    assert doctest.testmod(somali_tokenizer).failed == 0


def _cli_binary():
    from conftest import REPO_ROOT

    for profile in ("release", "debug"):
        binary = REPO_ROOT / "target" / profile / "somali-tokenizer"
        for candidate in (binary, binary.with_suffix(".exe")):
            if candidate.is_file():
                return candidate
    return None


@pytest.mark.skipif(_cli_binary() is None, reason="build the CLI first: cargo build")
def test_matches_the_rust_cli(tokenizer, parity_records) -> None:
    import subprocess

    texts = [r["text"] for r in parity_records if "\n" not in r["text"] and "\r" not in r["text"]]
    result = subprocess.run(
        [str(_cli_binary()), "encode", "--format", "json"],
        input="\n".join(texts) + "\n",
        capture_output=True,
        text=True,
        encoding="utf-8",
        check=True,
    )
    cli_ids = [json.loads(line)["ids"] for line in result.stdout.splitlines()]
    assert cli_ids == [tokenizer.encode(text).ids for text in texts]
