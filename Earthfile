VERSION 0.8

ARG --global RUST_IMAGE=rustlang/rust:nightly-bookworm
ARG --global PYTHON_VERSION=3.14

base:
    FROM $RUST_IMAGE
    WORKDIR /work
    RUN curl -LsSf https://astral.sh/uv/install.sh | sh
    ENV PATH=/root/.local/bin:$PATH
    RUN uv python install $PYTHON_VERSION
    COPY --dir .cargo .
    COPY Cargo.toml Cargo.lock rust-toolchain.toml pyproject.toml uv.lock .
    COPY README.md LICENSE .
    COPY clippy.toml rustfmt.toml ruff.toml pyrightconfig.json pytest.ini deny.toml .
    COPY --dir src python tests tests_python .
    RUN uv sync --locked

fmt:
    FROM +base
    RUN cargo fmt --check

clippy:
    FROM +base
    RUN cargo clippy --locked --all-targets --all-features -- -D warnings

test:
    FROM +base
    RUN cargo test --locked --all-features

ruff:
    FROM +base
    RUN uv run --no-project --with ruff ruff check .
    RUN uv run --no-project --with ruff ruff format --check --diff .

pytest:
    FROM +base
    RUN uv run maturin develop --release
    RUN uv run pytest tests_python

sdist:
    FROM +base
    RUN uv run maturin sdist --out dist
    SAVE ARTIFACT dist/* AS LOCAL dist/

wheel:
    FROM +base
    ARG TARGETPLATFORM
    RUN uv run maturin build --release --strip --out dist
    SAVE ARTIFACT dist/* AS LOCAL dist/

all:
    BUILD +fmt
    BUILD +clippy
    BUILD +test
    BUILD +ruff
    BUILD +pytest
    BUILD +sdist
    BUILD +wheel
