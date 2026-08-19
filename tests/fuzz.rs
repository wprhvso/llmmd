mod common;

use _native::to_telegram::{MessageEntity, process_llm_markdown_sync};
use common::{Rng, assert_entities_valid, render, render_chunked};

const SEEDS: &[&str] = &[
    "# Заголовок\n\nТекст с **жирным**, *курсивом* и `кодом`.\n",
    "1. первый\n2. второй\n   - вложенный\n   - ещё\n",
    "> цитата\n>> глубже\n> назад\n",
    "```rust\nfn main() {}\n```\ntail\n",
    "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n",
    "[ссылка](https://e.com/a?b=1) и ![картинка](https://e.com/i.png)\n",
    "$$\na^2 + b^2\n$$\n\nи инлайн $x$ с \\(y\\)\n",
    "x<sup>2</sup> H<sub>2</sub>O <u>подчёркнуто</u> ||спойлер||\n",
    "- [ ] todo\n- [x] done\n\n---\n\nконец\n",
    "😀 эмодзи и ж буквы\n",
];

const INSERTS: &[char] = &[
    '*', '_', '~', '`', '|', '$', '\\', '[', ']', '(', ')', '<', '>', '#', '-', '!', '\n', '\r',
    '\t', ' ', 'a', '😀',
];

fn mutate(seed: &str, rng: &mut Rng) -> String {
    let mut characters: Vec<char> = seed.chars().collect();
    let edits = rng.below(8).saturating_add(1);

    for _ in 0..edits {
        if characters.is_empty() {
            break;
        }
        let at = rng.below(characters.len());
        match rng.below(4) {
            0 => {
                characters.remove(at);
            }
            1 => {
                let inserted = *rng.pick(INSERTS);
                characters.insert(at, inserted);
            }
            2 =>
                if let Some(existing) = characters.get(at).copied() {
                    characters.insert(at, existing);
                },
            _ => characters.truncate(at),
        }
    }

    characters.into_iter().collect()
}

fn overlaps_partially(first: &MessageEntity, second: &MessageEntity) -> bool {
    let first_end = first.offset.saturating_add(first.length);
    let second_end = second.offset.saturating_add(second.length);
    let disjoint = first_end <= second.offset || second_end <= first.offset;
    let nested = (first.offset <= second.offset && second_end <= first_end)
        || (second.offset <= first.offset && first_end <= second_end);
    !disjoint && !nested
}

fn mutants(seed: u64, count: usize) -> Vec<String> {
    let mut rng = Rng::new(seed);
    (0..count)
        .map(|index| {
            let base = SEEDS.get(index % SEEDS.len()).copied().unwrap_or("");
            mutate(base, &mut rng)
        })
        .collect()
}

#[test]
fn mutated_documents_keep_their_entities_valid() {
    for document in mutants(0xF0F0_1111, 3000) {
        let (text, entities) = render(&document);
        assert_entities_valid(&text, &entities);

        for (index, first) in entities.iter().enumerate() {
            for second in entities.get(index.saturating_add(1)..).unwrap_or(&[]) {
                assert!(
                    !overlaps_partially(first, second),
                    "{first:?} and {second:?} overlap partially for {document:?}"
                );
            }
        }
    }
}

#[test]
fn mutated_documents_render_the_same_when_streamed() {
    for document in mutants(0xF0F0_2222, 1200) {
        let whole = render(&document);
        for size in [1_usize, 2, 5] {
            assert_eq!(
                render_chunked(&document, size),
                whole,
                "{document:?} rendered differently in {size}-char chunks"
            );
        }
    }
}

#[test]
fn mutated_documents_survive_the_public_entry_point() {
    for document in mutants(0xF0F0_3333, 1200) {
        for with_photo in [false, true] {
            let limit = if with_photo { 1024_usize } else { 4096 };
            for (chunk, entities) in process_llm_markdown_sync(&document, with_photo) {
                assert_entities_valid(&chunk, &entities);
                assert!(chunk.encode_utf16().count() <= limit);
                assert!(!chunk.contains('\r'), "a carriage return reached {chunk:?}");
            }
        }
    }
}

#[test]
fn a_truncated_document_is_a_prefix_of_the_whole_one() {
    for seed in SEEDS {
        let characters: Vec<char> = seed.chars().collect();
        for cut in 0..characters.len() {
            let prefix: String = characters.get(..cut).unwrap_or(&[]).iter().collect();
            let (text, entities) = render(&prefix);
            assert_entities_valid(&text, &entities);
        }
    }
}
