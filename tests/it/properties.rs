use _native::{
    limits::{CAPTION_LIMIT, MESSAGE_LIMIT},
    to_telegram::{EntityKind, process_markdown, split_message_with_entities},
};

use crate::support::{
    assert_entities_valid,
    assert_no_partial_overlap,
    corpus_seeded,
    limit_for,
    render,
    slice_utf16,
    utf16_len,
};

const LIMITS: &[usize] = &[1, 2, 3, 8, 64, CAPTION_LIMIT, MESSAGE_LIMIT];

fn non_whitespace(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn entities_never_partially_overlap() {
    for document in corpus_seeded(0x0BAD_F00D, 1500) {
        let (_, entities) = render(&document);
        assert_no_partial_overlap(&entities, &document);
    }
}

#[test]
fn splitting_keeps_every_non_whitespace_character() {
    for document in corpus_seeded(0x00D5_1DE0, 600) {
        let (text, entities) = render(&document);
        for limit in LIMITS {
            let chunks = split_message_with_entities(&text, &entities, *limit);
            let rejoined: String = chunks.iter().map(|chunk| chunk.text.as_str()).collect();
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
            for chunk in split_message_with_entities(&text, &entities, *limit) {
                assert_entities_valid(&chunk.text, &chunk.entities);
                assert!(
                    utf16_len(&chunk.text) <= (*limit).max(2),
                    "a chunk of {} units exceeds the limit of {limit} for {document:?}",
                    utf16_len(&chunk.text)
                );
                assert!(
                    !chunk.text.trim().is_empty(),
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
        let originals: Vec<(EntityKind, String)> = entities
            .iter()
            .map(|entity| {
                (
                    entity.kind,
                    non_whitespace(&slice_utf16(&text, entity.offset, entity.length)),
                )
            })
            .collect();

        for limit in [8_usize, 64, 1024] {
            for chunk in split_message_with_entities(&text, &entities, limit) {
                for entity in &chunk.entities {
                    let piece =
                        non_whitespace(&slice_utf16(&chunk.text, entity.offset, entity.length));
                    assert!(
                        originals
                            .iter()
                            .any(|(kind, whole)| *kind == entity.kind && whole.contains(&piece)),
                        "{entity:?} covers {piece:?} which no original {:?} entity covered at limit {limit} for {document:?}",
                        entity.kind
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
            let limit = limit_for(with_photo);
            for chunk in process_markdown(&document, with_photo) {
                assert_ne!(chunk.text.trim(), "");
                assert!(utf16_len(&chunk.text) <= limit);
                assert_entities_valid(&chunk.text, &chunk.entities);
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
        let chunks = process_markdown(&document, false);
        for chunk in &chunks {
            assert_entities_valid(&chunk.text, &chunk.entities);
        }
    }
}

#[test]
fn a_long_document_is_split_into_deliverable_messages() {
    let paragraph = "**bold** text with a [link](https://example.com) and `code`. ";
    let document = paragraph.repeat(400);

    let chunks = process_markdown(&document, false);
    assert!(chunks.len() > 1, "the document should not fit one message");

    for chunk in &chunks {
        assert!(utf16_len(&chunk.text) <= MESSAGE_LIMIT);
        assert_entities_valid(&chunk.text, &chunk.entities);
        assert!(
            chunk
                .entities
                .iter()
                .any(|entity| entity.kind == EntityKind::Bold),
            "every chunk of this document contains bold text"
        );
    }
}
