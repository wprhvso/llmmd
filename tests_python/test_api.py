import pytest
from helpers import utf16_len

from llmmd import (
    CAPTION_LIMIT,
    MESSAGE_LIMIT,
    MessageChunk,
    MessageEntity,
    process_markdown,
)

ENTITY_TYPES = {
    "bold",
    "italic",
    "underline",
    "strikethrough",
    "spoiler",
    "code",
    "pre",
    "text_link",
    "blockquote",
}


def test_the_markdown_argument_must_be_a_string() -> None:
    with pytest.raises(TypeError):
        process_markdown(None)  # pyright: ignore[reportArgumentType]


def test_calling_without_arguments_is_an_error() -> None:
    with pytest.raises(TypeError):
        process_markdown()  # pyright: ignore[reportCallIssue]


def test_the_photo_flag_can_be_passed_by_keyword() -> None:
    markdown = "word " * 1000
    assert process_markdown(markdown, with_photo=True) == process_markdown(
        markdown, True
    )


def test_with_photo_defaults_to_false() -> None:
    markdown = "word " * 1000
    assert process_markdown(markdown) == process_markdown(markdown, False)


def test_empty_markdown_yields_no_chunks() -> None:
    assert process_markdown("") == []


def test_a_document_of_only_whitespace_yields_no_chunks() -> None:
    assert process_markdown("   \n\n\t\n") == []


def test_repeated_calls_return_equal_results() -> None:
    markdown = "# T\n\n**b** *i* `c`\n"
    assert process_markdown(markdown) == process_markdown(markdown)


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


def test_code_block_carries_its_language() -> None:
    (chunk,) = process_markdown("```python\nprint(1)\n```\n")
    (pre,) = [e for e in chunk["entities"] if e["type"] == "pre"]
    assert pre["language"] == "python"


@pytest.mark.parametrize("with_photo", [False, True])
def test_random_documents_stay_within_the_limit(
    with_photo: bool, corpus: list[str]
) -> None:
    limit = CAPTION_LIMIT if with_photo else MESSAGE_LIMIT
    for markdown in corpus:
        for chunk in process_markdown(markdown, with_photo):
            size = utf16_len(chunk["text"])
            assert 0 < size <= limit
            assert chunk["text"].strip()
            for entity in chunk["entities"]:
                assert entity["offset"] >= 0
                assert entity["length"] > 0
                assert entity["offset"] + entity["length"] <= size


def test_random_documents_keep_every_visible_character(corpus: list[str]) -> None:
    for markdown in corpus:
        joined = "".join(chunk["text"] for chunk in process_markdown(markdown))
        for word in ("слово", "word", "code", "quote", "item"):
            if word in markdown:
                assert word in joined


def test_random_document_entities_are_well_formed(corpus: list[str]) -> None:
    for markdown in corpus:
        for chunk in process_markdown(markdown):
            for entity in chunk["entities"]:
                assert entity["type"] in ENTITY_TYPES
                if entity["type"] == "text_link":
                    assert entity["url"]
                else:
                    assert entity["url"] is None
                if entity["language"] is not None:
                    assert entity["type"] == "pre"


def test_surrogate_pairs_are_never_cut_in_half() -> None:
    markdown = "😀" * 3000
    for chunk in process_markdown(markdown):
        assert chunk["text"].encode("utf-16", "strict")
        assert "�" not in chunk["text"]


def test_a_very_long_line_without_spaces_is_still_split() -> None:
    markdown = "a" * 10_000
    chunks = process_markdown(markdown)
    assert len(chunks) >= 3
    assert "".join(chunk["text"] for chunk in chunks) == markdown


def test_a_huge_document_is_processed() -> None:
    markdown = "**bold** and `code` and [l](https://e.com)\n\n" * 2000
    chunks = process_markdown(markdown)
    assert len(chunks) > 1
    for chunk in chunks:
        assert utf16_len(chunk["text"]) <= MESSAGE_LIMIT


def test_windows_line_endings_are_normalised() -> None:
    markdown = "# T\n\n**b**\n\n- a\n- b\n"
    assert process_markdown(markdown.replace("\n", "\r\n")) == process_markdown(
        markdown
    )
    for chunk in process_markdown(markdown.replace("\n", "\r\n")):
        assert "\r" not in chunk["text"]
