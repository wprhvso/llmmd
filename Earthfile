VERSION 0.8

ARG --global RUST_IMAGE=rustlang/rust:nightly-bookworm
ARG --global PYTHON_VERSION=3.14

base:
    FROM $RUST_IMAGE
    WORKDIR /work
    RUN rustup component add rustfmt clippy
    RUN curl -LsSf https://astral.sh/uv/install.sh | sh
    ENV PATH=/root/.local/bin:$PATH
    RUN uv python install $PYTHON_VERSION
    COPY --dir .cargo .
    COPY Cargo.toml Cargo.lock rust-toolchain.toml .rustfmt.toml ruff.toml pyrightconfig.json pyproject.toml uv.lock .
    COPY --dir src python tests tests_python .
    RUN uv sync --locked

fmt:
    FROM +base
    RUN cargo fmt --check

clippy:
    FROM +base
    RUN cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic -W clippy::nursery -W clippy::cargo

ruff:
    FROM +base
    RUN uvx ruff check python tests_python
    RUN uvx ruff format --check python tests_python

pyright:
    FROM +base
    RUN uv run maturin develop --release
    RUN uvx pyright

test:
    FROM +base
    RUN cargo test --all-features

pytest:
    FROM +base
    RUN uv run maturin develop --release
    RUN uv run pytest tests_python

lint:
    BUILD +fmt
    BUILD +clippy
    BUILD +ruff
    BUILD +pyright

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
    BUILD +lint
    BUILD +test
    BUILD +pytest
    BUILD +sdist
    BUILD +wheel
