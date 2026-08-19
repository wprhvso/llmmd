import argparse
import json
import sys
from pathlib import Path

from llmmd import process_markdown


def _read(parser: argparse.ArgumentParser, path: Path | None) -> str:
    source = str(path) if path else "входные данные"
    try:
        text = path.read_text(encoding="utf-8") if path else sys.stdin.read()
    except OSError as error:
        message = error.strerror or str(error)
        parser.error(f"не удалось прочитать {source}: {message}")
    except UnicodeDecodeError:
        parser.error(f"{source}: ожидается текст в UTF-8")
    else:
        return text


def main() -> int:
    parser = argparse.ArgumentParser(prog="llmmd")
    parser.add_argument("path", type=Path, nargs="?")
    parser.add_argument("--with-photo", action="store_true")
    args = parser.parse_args()

    markdown = _read(parser, args.path)
    json.dump(
        process_markdown(markdown, args.with_photo),
        sys.stdout,
        ensure_ascii=False,
        indent=2,
    )
    sys.stdout.write("\n")
    return 0
