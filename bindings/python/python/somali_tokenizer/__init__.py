"""Somali ByteLevel BPE tokenizer, implemented in Rust.

>>> from somali_tokenizer import SomaliTokenizer
>>> tokenizer = SomaliTokenizer.v2()
>>> encoding = tokenizer.encode("Soomaaliya waa dal.")
>>> encoding.ids
[5446, 406, 785, 29]
>>> tokenizer.decode(encoding.ids)
'Soomaaliya waa dal.'
"""

from ._somali_tokenizer import (
    HUB_REPO_V2,
    ContractError,
    Encoding,
    SomaliTokenizer,
    __version__,
)

__all__ = ["HUB_REPO_V2", "ContractError", "Encoding", "SomaliTokenizer", "__version__"]
