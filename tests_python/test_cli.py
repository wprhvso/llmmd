import json
import subprocess
import sys
from pathlib import Path

import pytest

from llmmd import process_markdown

MODULE = [sys.executable, "-m", "llmmd"]


def run(
    *args: str,
    stdin: bytes = b"",
) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        [*MODULE, *args], input=stdin, capture_output=True, check=False
    )


def test_help_exits_cleanly() -> None:
    result = run("--help")
    assert result.returncode == 0
    assert b"llmmd" in result.stdout


def test_empty_stdin_yields_an_empty_document() -> None:
    result = run()
    assert result.returncode == 0
    assert json.loads(result.stdout) == []


def test_output_is_pretty_printed_and_ends_with_a_newline() -> None:
    result = run(stdin=b"**bold**")
    assert result.returncode == 0
    assert result.stdout.endswith(b"\n")
    assert b"\n  " in result.stdout


def test_non_ascii_text_is_not_escaped() -> None:
    result = run(stdin="привет 😀".encode())
    assert result.returncode == 0
    assert "привет 😀" in result.stdout.decode()


def test_a_missing_file_is_reported_without_a_traceback(tmp_path: Path) -> None:
    result = run(str(tmp_path / "absent.md"))
    assert result.returncode == 2
    assert b"Traceback" not in result.stderr
    assert result.stderr


def test_a_directory_is_reported_without_a_traceback(tmp_path: Path) -> None:
    result = run(str(tmp_path))
    assert result.returncode == 2
    assert b"Traceback" not in result.stderr


def test_a_file_that_is_not_utf8_is_reported_without_a_traceback(
    tmp_path: Path,
) -> None:
    path = tmp_path / "broken.md"
    path.write_bytes(b"\xff\xfe\x00binary")

    result = run(str(path))
    assert result.returncode == 2
    assert b"Traceback" not in result.stderr


def test_an_unknown_flag_is_rejected() -> None:
    result = run("--nope")
    assert result.returncode == 2


@pytest.mark.parametrize("flag", [[], ["--with-photo"]])
def test_the_cli_matches_the_library(flag: list[str], tmp_path: Path) -> None:
    markdown = "# T\n\n**b** and [a](https://e.com)\n\n" + ("word " * 500)
    path = tmp_path / "doc.md"
    path.write_text(markdown, encoding="utf-8")

    result = run(*flag, str(path))
    assert result.returncode == 0
    assert json.loads(result.stdout) == process_markdown(markdown, bool(flag))


def test_a_file_and_stdin_produce_the_same_output(tmp_path: Path) -> None:
    markdown = "> quote\n\n```python\nprint(1)\n```\n"
    path = tmp_path / "doc.md"
    path.write_text(markdown, encoding="utf-8")

    from_file = run(str(path))
    from_stdin = run(stdin=markdown.encode())

    assert from_file.returncode == 0
    assert from_file.stdout == from_stdin.stdout


def test_the_installed_entry_point_works() -> None:
    script = Path(sys.executable).with_name("llmmd")
    if not script.exists():
        pytest.skip("консольный скрипт не установлен")

    result = subprocess.run(
        [str(script)],
        input=b"**bold**",
        capture_output=True,
        check=False,
    )
    assert result.returncode == 0
    assert json.loads(result.stdout) == process_markdown("**bold**")
