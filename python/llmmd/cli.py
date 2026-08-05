import argparse
import json
import sys
from pathlib import Path

from llmmd import process_markdown


def main() -> int:
    parser = argparse.ArgumentParser(prog="llmmd")
    parser.add_argument("path", type=Path, nargs="?")
    parser.add_argument("--with-photo", action="store_true")
    args = parser.parse_args()

    markdown = args.path.read_text(encoding="utf-8") if args.path else sys.stdin.read()
    json.dump(
        process_markdown(markdown, args.with_photo),
        sys.stdout,
        ensure_ascii=False,
        indent=2,
    )
    sys.stdout.write("\n")
    return 0
