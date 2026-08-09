import json
import subprocess
import sys
from pathlib import Path

import pytest
from llmmd import MessageChunk, MessageEntity, process_markdown

CAPTION_LIMIT = 1024
MESSAGE_LIMIT = 4096


def utf16_len(text: str) -> int:
    return len(text.encode("utf-16-le")) // 2


def test_empty_markdown_yields_no_chunks() -> None:
    assert process_markdown("") == []


def test_chunk_shape_matches_the_type_stubs() -> None:
    chunks = process_markdown("**bold** and [a](https://e.com)")
    assert len(chunks) == 1

    chunk = chunks[0]
    assert set(chunk) == set(MessageChunk.__annotations__)
    assert chunk["text"] == "bold and a"

    for entity in chunk["entities"]:
        assert set(entity) == set(MessageEntity.__annotations__)
        assert isinstance(entity["type"], str)
        assert isinstance(entity["offset"], int)
        assert isinstance(entity["length"], int)
        assert entity["url"] is None or isinstance(entity["url"], str)
        assert entity["language"] is None or isinstance(entity["language"], str)


def test_entities_address_utf16_offsets() -> None:
    (chunk,) = process_markdown("ж😀 **b**")
    units = chunk["text"].encode("utf-16-le")
    (bold,) = [e for e in chunk["entities"] if e["type"] == "bold"]
    start = bold["offset"] * 2
    end = start + bold["length"] * 2
    assert units[start:end].decode("utf-16-le") == "b"


def test_entities_stay_inside_their_chunk() -> None:
    markdown = ("word " * 3000) + "\n\n**tail**\n"
    for with_photo, limit in ((False, MESSAGE_LIMIT), (True, CAPTION_LIMIT)):
        chunks = process_markdown(markdown, with_photo)
        assert len(chunks) > 1
        for chunk in chunks:
            size = utf16_len(chunk["text"])
            assert size <= limit
            for entity in chunk["entities"]:
                assert entity["offset"] >= 0
                assert entity["length"] > 0
                assert entity["offset"] + entity["length"] <= size


def test_with_photo_defaults_to_false() -> None:
    markdown = "word " * 1000
    assert process_markdown(markdown) == process_markdown(markdown, False)


def test_code_block_carries_its_language() -> None:
    (chunk,) = process_markdown("```python\nprint(1)\n```\n")
    (pre,) = [e for e in chunk["entities"] if e["type"] == "pre"]
    assert pre["language"] == "python"


@pytest.mark.parametrize("source", ["stdin", "file"])
def test_cli_emits_json(source: str, tmp_path: Path) -> None:
    markdown = "# T\n\n**b**\n"
    if source == "file":
        path = tmp_path / "doc.md"
        path.write_text(markdown, encoding="utf-8")
        argv = [sys.executable, "-m", "llmmd", str(path)]
        stdin = b""
    else:
        argv = [sys.executable, "-m", "llmmd"]
        stdin = markdown.encode()

    result = subprocess.run(argv, input=stdin, capture_output=True, check=True)
    payload = json.loads(result.stdout)
    assert payload == process_markdown(markdown)


def test_cli_accepts_the_photo_flag() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "llmmd", "--with-photo"],
        input=("word " * 3000).encode(),
        capture_output=True,
        check=True,
    )
    payload = json.loads(result.stdout)
    assert all(utf16_len(chunk["text"]) <= CAPTION_LIMIT for chunk in payload)
