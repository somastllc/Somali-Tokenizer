from __future__ import annotations

import json
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[3]
FIXTURES = REPO_ROOT / "tests" / "fixtures"
RELEASE_V2 = REPO_ROOT / "artifacts" / "releases" / "v2"


@pytest.fixture(scope="session")
def parity_records() -> list[dict]:
    with (FIXTURES / "parity_v2.jsonl").open(encoding="utf-8") as handle:
        return [json.loads(line) for line in handle if line.strip()]
