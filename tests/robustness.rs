
mod common;

use _native::to_telegram::{MessageEntity, process_llm_markdown_sync};
use common::{actions, assert_entities_valid, balanced_pairs, render, resolve};

struct Rng(u64);

impl Rng {
    const fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    #[allow(clippy::missing_const_for_fn)]
    fn next(&mut self) -> u64 {

        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        let index = usize::try_from(self.next() % items.len() as u64).unwrap_or(0);
        &items[index]
    }
}

const FRAGMENTS: &[&str] = &[
    "a",
    "b",
    " ",
    "\n",
    "\n\n",
    "*",
    "**",
    "***",
    "_",
    "__",
    "~",
    "~~",
    "`",
    "``",
    "```",
    "$",
    "$$",
    "\\",
    "\\(",
    "\\)",
    "\\[",
    "\\]",
    "|",
    "||",
    "<",
    ">",
    "<u>",
    "</u>",
    "<sup>",
    "</sup>",
    "<sub>",
    "</sub>",
    "#",
    "##",
    "# ",
    "- ",
    "1. ",
    "1) ",
    "[",
    "]",
    "(",
    ")",
    "![",
    "](x)",
    "(https://e.com)",
    "> ",
    "---",
    "***",
    "[ ]",
    "[x]",
    "|---|",
    "| a |",
    "rust",
    "😀",
    "ж",
    "\t",
    "   ",
    ">>",
    "  - ",
    "    ",
    "1",
    "2",
    ".",
    "!",
    "-",
    "+",
    "=",
];

fn corpus(count: usize) -> Vec<String> {
    corpus_seeded(0x5EED_1234, count)
}

fn corpus_seeded(seed: u64, count: usize) -> Vec<String> {
    let mut rng = Rng::new(seed);
    (0..count)
        .map(|_| {
            let pieces = usize::try_from(rng.next() % 40).unwrap_or(0) + 1;
            (0..pieces).map(|_| *rng.pick(FRAGMENTS)).collect()
        })
        .collect()
}

#[test]
fn random_markup_soup_never_panics_and_keeps_entities_valid() {
    for document in corpus(4000) {

        let _ = resolve(actions(&document));

        let (text, entities) = render(&document);
        assert_entities_valid(&text, &entities);
    }
}

#[test]
fn random_markup_soup_survives_the_public_entry_point() {
    for document in corpus(1500) {
        for with_photo in [false, true] {
            let limit = if with_photo { 1024 } else { 4096 };
            let chunks = process_llm_markdown_sync(&document, with_photo);
            for (chunk, entities) in &chunks {
                assert_entities_valid(chunk, entities);
                assert!(
                    chunk.encode_utf16().count() <= limit.max(2),
                    "chunk exceeds the {limit}-unit limit for {document:?}"
                );
            }
        }
    }
}

#[test]
fn plain_prose_is_reproduced_byte_for_byte() {

    let documents = [
        "just some words",
        "line one\nline two\nline three\n",
        "числа 123 и слова",
        "emoji 😀 and 🎉 mixed in",
        "trailing spaces   \nand more\n",
        "tabs\tin\tthe\tmiddle\n",
    ];
    for document in documents {
        assert_eq!(render(document).0, document, "{document:?} was altered");
    }
}

#[test]
fn every_text_character_of_a_document_reaches_the_output() {

    let document = "\
# Heading one

Some **bold** and *italic* and `code` and $math$ text.

- alpha
- beta

> quoted words

| head | cell |
|------|------|
| val1 | val2 |

```python
snippet = 1
```

[anchor](https://example.com/path) and ![image](https://example.com/i.png)
";
    let rendered = render(document).0;
    let expected: String = document
        .chars()
        .filter(|character| character.is_alphanumeric())
        .collect();
    let actual: String = rendered
        .chars()
        .filter(|character| character.is_alphanumeric())
        .collect();

    for needle in [
        "Heading", "one", "bold", "italic", "code", "math", "alpha", "beta", "quoted", "head",
        "cell", "val1", "val2", "snippet", "anchor", "image",
    ] {
        assert!(
            rendered.contains(needle),
            "{needle:?} disappeared from the output"
        );
    }
    assert!(!expected.is_empty() && !actual.is_empty());
}

#[test]
fn rendering_is_deterministic() {
    for document in corpus(500) {
        let first = render(&document);
        let second = render(&document);
        assert_eq!(first.0, second.0);
        assert_eq!(first.1, second.1);
    }
}

#[test]
fn no_speculative_start_event_survives_without_its_end() {

    let pairs = balanced_pairs();

    for seed in 0..24_u64 {
        for document in corpus_seeded(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15), 400) {
            let events = resolve(actions(&document));
            for (is_start, is_end, name) in &pairs {
                let starts = events.iter().filter(|e| is_start(e)).count();
                let ends = events.iter().filter(|e| is_end(e)).count();
                assert_eq!(
                    starts, ends,
                    "unbalanced {name} events for {document:?}: {events:?}"
                );
            }
        }
    }
}

#[test]
fn entities_are_never_empty_or_out_of_range_for_real_documents() {
    let documents = [
        "**", "`", "[", "](", "![]()", "> ", "- ", "#", "$$", "|", "|---|", "\n\n\n", "```", "<u>",
        "<sup>",
    ];
    for document in documents {
        let (text, entities): (String, Vec<MessageEntity>) = render(document);
        assert_entities_valid(&text, &entities);
    }
}
