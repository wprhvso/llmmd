//! Tests for `split_message_with_entities` and the public `process_llm_markdown_sync`.

mod common;

use _native::to_telegram::{MessageEntity, process_llm_markdown_sync, split_message_with_entities};

fn entity(kind: &str, offset: i64, length: i64) -> MessageEntity {
    MessageEntity {
        r#type: kind.to_string(),
        offset,
        length,
        url: None,
        language: None,
    }
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// Splitting may only drop whitespace-only chunks (Telegram refuses to send them);
/// every other character must survive, in order, without replacement characters.
fn assert_lossless(text: &str, chunks: &[(String, Vec<MessageEntity>)]) {
    let rejoined: String = chunks.iter().map(|(chunk, _)| chunk.as_str()).collect();
    assert!(
        !rejoined.contains('\u{FFFD}') || text.contains('\u{FFFD}'),
        "splitting introduced U+FFFD replacement characters"
    );

    let kept: String = rejoined.chars().filter(|c| !c.is_whitespace()).collect();
    let expected: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    assert_eq!(kept, expected, "splitting lost or reordered text");

    for (chunk, _) in chunks {
        assert!(
            !chunk.trim().is_empty(),
            "a whitespace-only chunk would be rejected by Telegram"
        );
    }
}

/// Every entity must still cover exactly the substring it covered before the split.
fn assert_entities_survive(text: &str, entities: &[MessageEntity], limit: usize) {
    let chunks = split_message_with_entities(text, entities, limit);
    assert_lossless(text, &chunks);

    let units: Vec<u16> = text.encode_utf16().collect();
    let mut base = 0_usize;
    let mut recovered: Vec<(String, String)> = Vec::new();

    for (chunk, chunk_entities) in &chunks {
        let chunk_units: Vec<u16> = chunk.encode_utf16().collect();
        for chunk_entity in chunk_entities {
            let offset = usize::try_from(chunk_entity.offset).expect("offset fits");
            let length = usize::try_from(chunk_entity.length).expect("length fits");
            assert!(
                offset.saturating_add(length) <= chunk_units.len(),
                "entity {chunk_entity:?} runs past its chunk"
            );
            let covered =
                String::from_utf16_lossy(chunk_units.get(offset..offset + length).unwrap_or(&[]));
            recovered.push((chunk_entity.r#type.clone(), covered));
        }
        base = base.saturating_add(chunk_units.len());
    }
    assert_eq!(base, units.len());

    // Concatenating the per-chunk pieces of one entity must rebuild the original span.
    for original in entities {
        let offset = usize::try_from(original.offset).expect("offset fits");
        let length = usize::try_from(original.length).expect("length fits");
        let expected = String::from_utf16_lossy(units.get(offset..offset + length).unwrap_or(&[]));
        let joined: String = recovered
            .iter()
            .filter(|(kind, _)| *kind == original.r#type)
            .map(|(_, piece)| piece.as_str())
            .collect();
        assert!(
            joined.contains(&expected) || expected.contains(&joined),
            "entity {original:?} did not survive: {joined:?} vs {expected:?}"
        );
    }
}

#[test]
fn empty_text_yields_no_chunks() {
    assert_eq!(split_message_with_entities("", &[], 10).len(), 0);
}

#[test]
fn short_text_stays_in_one_chunk() {
    let chunks = split_message_with_entities("hello", &[entity("bold", 0, 5)], 4096);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].0, "hello");
    assert_eq!(chunks[0].1.len(), 1);
}

#[test]
fn chunks_respect_the_limit() {
    let text = "x".repeat(10_000);
    let chunks = split_message_with_entities(&text, &[], 4096);
    assert!(chunks.len() >= 3);
    for (chunk, _) in &chunks {
        assert!(
            utf16_len(chunk) <= 4096,
            "chunk of {} units exceeds the limit",
            utf16_len(chunk)
        );
    }
    assert_lossless(&text, &chunks);
}

#[test]
fn splitting_prefers_a_newline_then_a_space() {
    let text = format!("{}\n{}", "a".repeat(50), "b".repeat(50));
    let chunks = split_message_with_entities(&text, &[], 60);
    assert_eq!(chunks[0].0, format!("{}\n", "a".repeat(50)));

    let text = format!("{} {}", "a".repeat(50), "b".repeat(50));
    let chunks = split_message_with_entities(&text, &[], 60);
    assert_eq!(chunks[0].0, format!("{} ", "a".repeat(50)));
}

#[test]
fn a_surrogate_pair_is_never_cut_in_half() {
    // Regression: the guard used to be skipped whenever the cut landed exactly on
    // the limit, which is precisely what happens for text without spaces.
    for limit in 2..40_usize {
        let text = "😀".repeat(40);
        let chunks = split_message_with_entities(&text, &[], limit);
        assert_lossless(&text, &chunks);
        for (chunk, _) in &chunks {
            assert!(
                !chunk.contains('\u{FFFD}'),
                "limit {limit} produced a broken chunk {chunk:?}"
            );
        }
    }
}

#[test]
fn surrogate_pairs_survive_next_to_ordinary_text() {
    let text = "aaa😀bbb😀ccc😀ddd";
    for limit in 1..text.encode_utf16().count() + 2 {
        let chunks = split_message_with_entities(text, &[], limit);
        assert_lossless(text, &chunks);
    }
}

#[test]
fn a_limit_smaller_than_the_first_character_still_makes_progress() {
    let chunks = split_message_with_entities("😀a", &[], 1);
    assert_lossless("😀a", &chunks);
    assert_eq!(chunks[0].0, "😀");
}

#[test]
fn entities_are_clipped_to_their_chunk() {
    let text = format!("{}{}", "a".repeat(100), "b".repeat(100));
    let entities = vec![entity("bold", 0, 200), entity("italic", 150, 20)];
    assert_entities_survive(&text, &entities, 100);

    let chunks = split_message_with_entities(&text, &entities, 100);
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].1, vec![entity("bold", 0, 100)]);
    assert_eq!(
        chunks[1].1,
        vec![entity("bold", 0, 100), entity("italic", 50, 20)]
    );
}

#[test]
fn an_entity_entirely_outside_a_chunk_is_dropped() {
    let text = "a".repeat(200);
    let chunks = split_message_with_entities(&text, &[entity("bold", 150, 10)], 100);
    assert_eq!(chunks[0].1, []);
    assert_eq!(chunks[1].1, vec![entity("bold", 50, 10)]);
}

#[test]
fn entity_urls_and_languages_are_copied_into_every_chunk() {
    let text = "a".repeat(200);
    let source = MessageEntity {
        r#type: "text_link".to_string(),
        offset: 90,
        length: 30,
        url: Some("https://example.com".to_string()),
        language: None,
    };
    let chunks = split_message_with_entities(&text, &[source], 100);
    for (_, chunk_entities) in &chunks {
        for chunk_entity in chunk_entities {
            assert_eq!(chunk_entity.url.as_deref(), Some("https://example.com"));
        }
    }
}

#[test]
fn zero_length_entities_are_dropped() {
    let chunks = split_message_with_entities("abc", &[entity("bold", 1, 0)], 10);
    assert_eq!(chunks[0].1, []);
}

#[test]
fn process_respects_the_photo_caption_limit() {
    let markdown = "word ".repeat(1000);

    let plain = process_llm_markdown_sync(&markdown, false);
    for (chunk, _) in &plain {
        assert!(utf16_len(chunk) <= 4096);
    }

    let with_photo = process_llm_markdown_sync(&markdown, true);
    for (chunk, _) in &with_photo {
        assert!(utf16_len(chunk) <= 1024);
    }
    assert!(with_photo.len() > plain.len());
}

#[test]
fn process_of_empty_markdown_is_empty() {
    assert_eq!(process_llm_markdown_sync("", false).len(), 0);
}

#[test]
fn process_keeps_entities_inside_their_chunk() {
    let markdown = format!("{}\n\n**bold tail**\n", "filler line\n".repeat(500));
    for chunks in [
        process_llm_markdown_sync(&markdown, false),
        process_llm_markdown_sync(&markdown, true),
    ] {
        for (chunk, entities) in &chunks {
            let units = utf16_len(chunk);
            for chunk_entity in entities {
                let offset = usize::try_from(chunk_entity.offset).expect("offset fits");
                let length = usize::try_from(chunk_entity.length).expect("length fits");
                assert!(
                    offset.saturating_add(length) <= units,
                    "entity {chunk_entity:?} escapes its {units}-unit chunk"
                );
            }
        }
    }
}

#[test]
fn whitespace_only_chunks_are_not_emitted() {
    // Telegram answers "message text is empty" for a message made only of newlines.
    let text = format!("{}\n\n\n", "a".repeat(100));
    let chunks = split_message_with_entities(&text, &[], 100);
    assert_lossless(&text, &chunks);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].0, "a".repeat(100));
}

#[test]
fn an_early_line_break_does_not_produce_a_tiny_chunk() {
    // The last newline in the window used to be taken however early it was, so a
    // leading "\n" produced a one-character message.
    let text = format!("\n{}", "a".repeat(300));
    let chunks = split_message_with_entities(&text, &[], 100);
    assert_lossless(&text, &chunks);
    // The last chunk is whatever is left over; every other one must be a full window.
    for (chunk, _) in chunks.split_last().expect("at least one chunk").1 {
        assert_eq!(
            utf16_len(chunk),
            100,
            "chunk {chunk:?} wastes most of the message"
        );
    }
}
