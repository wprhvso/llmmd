mod common;

use _native::to_telegram::{MessageEntity, process_llm_markdown_sync, split_message_with_entities};
use common::{assert_entities_valid, corpus_seeded, render, slice_utf16};

const LIMITS: &[usize] = &[1, 2, 3, 8, 64, 1024, 4096];

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

fn non_whitespace(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

fn overlaps_partially(first: &MessageEntity, second: &MessageEntity) -> bool {
    let first_end = first.offset.saturating_add(first.length);
    let second_end = second.offset.saturating_add(second.length);
    let disjoint = first_end <= second.offset || second_end <= first.offset;
    let nested = (first.offset <= second.offset && second_end <= first_end)
        || (second.offset <= first.offset && first_end <= second_end);
    !disjoint && !nested
}

#[test]
fn entities_never_partially_overlap() {
    for document in corpus_seeded(0x0BAD_F00D, 1500) {
        let (text, entities) = render(&document);
        for (index, first) in entities.iter().enumerate() {
            for second in entities.get(index.saturating_add(1)..).unwrap_or(&[]) {
                assert!(
                    !overlaps_partially(first, second),
                    "{first:?} and {second:?} overlap partially in {text:?} from {document:?}"
                );
            }
        }
    }
}

#[test]
fn splitting_keeps_every_non_whitespace_character() {
    for document in corpus_seeded(0x00D5_1DE0, 600) {
        let (text, entities) = render(&document);
        for limit in LIMITS {
            let chunks = split_message_with_entities(&text, &entities, *limit);
            let rejoined: String = chunks.iter().map(|(chunk, _)| chunk.as_str()).collect();
            assert_eq!(
                non_whitespace(&rejoined),
                non_whitespace(&text),
                "a limit of {limit} lost text from {document:?}"
            );
        }
    }
}

#[test]
fn every_chunk_respects_the_limit_and_carries_valid_entities() {
    for document in corpus_seeded(0x4242_4242, 600) {
        let (text, entities) = render(&document);
        for limit in LIMITS {
            for (chunk, chunk_entities) in split_message_with_entities(&text, &entities, *limit) {
                assert_entities_valid(&chunk, &chunk_entities);
                assert!(
                    utf16_len(&chunk) <= (*limit).max(2),
                    "a chunk of {} units exceeds the limit of {limit} for {document:?}",
                    utf16_len(&chunk)
                );
                assert!(
                    !chunk.trim().is_empty(),
                    "an empty chunk would be rejected by Telegram"
                );
            }
        }
    }
}

#[test]
fn a_split_entity_still_covers_text_it_covered_before() {
    for document in corpus_seeded(0x7E57_1234, 400) {
        let (text, entities) = render(&document);
        let originals: Vec<(String, String)> = entities
            .iter()
            .map(|entity| {
                (
                    entity.r#type.clone(),
                    non_whitespace(&slice_utf16(&text, entity.offset, entity.length)),
                )
            })
            .collect();

        for limit in [8_usize, 64, 1024] {
            for (chunk, chunk_entities) in split_message_with_entities(&text, &entities, limit) {
                for entity in &chunk_entities {
                    let piece = non_whitespace(&slice_utf16(&chunk, entity.offset, entity.length));
                    assert!(
                        originals
                            .iter()
                            .any(|(kind, whole)| *kind == entity.r#type && whole.contains(&piece)),
                        "{entity:?} covers {piece:?} which no original {:?} entity covered at limit {limit} for {document:?}",
                        entity.r#type
                    );
                }
            }
        }
    }
}

#[test]
fn the_public_entry_point_never_returns_an_unusable_chunk() {
    for document in corpus_seeded(0x9999_0001, 800) {
        for with_photo in [false, true] {
            let limit = if with_photo { 1024_usize } else { 4096 };
            for (chunk, entities) in process_llm_markdown_sync(&document, with_photo) {
                assert_ne!(chunk.trim(), "");
                assert!(utf16_len(&chunk) <= limit);
                assert_entities_valid(&chunk, &entities);
            }
        }
    }
}

#[test]
fn pathological_documents_finish_without_panicking() {
    let documents = [
        "[".repeat(20_000),
        "*".repeat(20_000),
        "`".repeat(20_000),
        "> ".repeat(20_000),
        "- ".repeat(20_000),
        "#".repeat(20_000),
        "|".repeat(20_000),
        "<u>".repeat(5_000),
        "![](".repeat(5_000),
        "$".repeat(20_000),
        "a\n".repeat(20_000),
        "***nested ".repeat(2_000),
        format!("{}{}", "| a |\n|---|\n", "| b |\n".repeat(5_000)),
        format!("{}text", "\\[".repeat(5_000)),
    ];

    for document in documents {
        let chunks = process_llm_markdown_sync(&document, false);
        for (chunk, entities) in &chunks {
            assert_entities_valid(chunk, entities);
        }
    }
}

#[test]
fn a_long_document_is_split_into_deliverable_messages() {
    let paragraph = "**bold** text with a [link](https://example.com) and `code`. ";
    let document = paragraph.repeat(400);

    let chunks = process_llm_markdown_sync(&document, false);
    assert!(chunks.len() > 1, "the document should not fit one message");

    for (chunk, entities) in &chunks {
        assert!(utf16_len(chunk) <= 4096);
        assert_entities_valid(chunk, entities);
        assert!(
            entities.iter().any(|entity| entity.r#type == "bold"),
            "every chunk of this document contains bold text"
        );
    }
}
