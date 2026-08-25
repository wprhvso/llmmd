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
