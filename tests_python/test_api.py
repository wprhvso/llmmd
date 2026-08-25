import random

import pytest

from llmmd import CAPTION_LIMIT, MESSAGE_LIMIT, process_markdown

FRAGMENTS = [
    "слово ",
    "word ",
    "**b** ",
    "*i* ",
    "~~s~~ ",
    "||sp|| ",
    "`c` ",
    "[l](https://e.com) ",
    "![i](https://e.com/i.png) ",
    "$x$ ",
    "\n",
    "\n\n",
    "# h\n",
    "- item\n",
    "1. item\n",
    "> quote\n",
    "| a | b |\n|---|---|\n| 1 | 2 |\n",
    "```py\ncode\n```\n",
    "😀",
    "<u>u</u> ",
]


def utf16_len(text: str) -> int:
    return len(text.encode("utf-16-le")) // 2


def documents(count: int, seed: int) -> list[str]:
    rng = random.Random(seed)
    return ["".join(rng.choices(FRAGMENTS, k=rng.randint(1, 60))) for _ in range(count)]


def test_the_markdown_argument_must_be_a_string() -> None:
    with pytest.raises(TypeError):
        process_markdown(None)  # pyright: ignore[reportArgumentType]


def test_the_photo_flag_can_be_passed_by_keyword() -> None:
    markdown = "word " * 1000
    assert process_markdown(markdown, with_photo=True) == process_markdown(
        markdown, True
    )


def test_calling_without_arguments_is_an_error() -> None:
    with pytest.raises(TypeError):
        process_markdown()  # pyright: ignore[reportCallIssue]


def test_a_document_of_only_whitespace_yields_no_chunks() -> None:
    assert process_markdown("   \n\n\t\n") == []


def test_repeated_calls_return_equal_results() -> None:
    markdown = "# T\n\n**b** *i* `c`\n"
    assert process_markdown(markdown) == process_markdown(markdown)


@pytest.mark.parametrize("with_photo", [False, True])
def test_random_documents_stay_within_the_limit(with_photo: bool) -> None:
    limit = CAPTION_LIMIT if with_photo else MESSAGE_LIMIT
    for markdown in documents(200, seed=20250819):
        for chunk in process_markdown(markdown, with_photo):
            size = utf16_len(chunk["text"])
            assert 0 < size <= limit
            assert chunk["text"].strip()
            for entity in chunk["entities"]:
                assert entity["offset"] >= 0
                assert entity["length"] > 0
                assert entity["offset"] + entity["length"] <= size


def test_random_documents_keep_every_visible_character() -> None:
    for markdown in documents(200, seed=7):
        joined = "".join(chunk["text"] for chunk in process_markdown(markdown))
        for word in ("слово", "word", "code", "quote", "item"):
            if word in markdown:
                assert word in joined


def test_entity_types_are_the_ones_telegram_accepts() -> None:
    allowed = {
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
    for markdown in documents(200, seed=99):
        for chunk in process_markdown(markdown):
            for entity in chunk["entities"]:
                assert entity["type"] in allowed


def test_a_link_entity_always_carries_a_url() -> None:
    for markdown in documents(200, seed=1234):
        for chunk in process_markdown(markdown):
            for entity in chunk["entities"]:
                if entity["type"] == "text_link":
                    assert entity["url"]
                else:
                    assert entity["url"] is None


def test_a_language_is_only_attached_to_a_code_block() -> None:
    for markdown in documents(200, seed=4321):
        for chunk in process_markdown(markdown):
            for entity in chunk["entities"]:
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
