VERSION 0.8

# Сборка дистрибутивов llmmd.
#
# Линтов и настроек QA здесь больше нет: они переехали в два общих экшена —
# wprhvso/qa-rust (fmt, clippy, test, doc) и wprhvso/qa-python (ruff, format,
# pyright, pytest), которые и гоняет ci.yml. Локально те же проверки с теми же
# конфигами запускают `qa-rust` и `uvx qa-python`. Тесты остаются здесь, потому
# что их удобно прогонять вместе со сборкой колеса.

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
    COPY --dir src python tests tests_python .
    RUN uv sync --locked

test:
    FROM +base
    RUN cargo test --locked --workspace --all-features

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
    BUILD +test
    BUILD +pytest
    BUILD +sdist
    BUILD +wheel
