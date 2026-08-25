import random

import pytest
from helpers import FRAGMENTS


@pytest.fixture(scope="session")
def corpus() -> list[str]:
    rng = random.Random(20250819)
    return ["".join(rng.choices(FRAGMENTS, k=rng.randint(1, 60))) for _ in range(200)]
