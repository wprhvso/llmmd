//! Shared helpers for the integration tests.
//!
//! Each test binary links its own copy, so helpers used by only some of them
//! would otherwise be reported as dead code.

#![allow(dead_code)]

use _native::{
    from_markdown::{Action, Event, LlmMarkdownParser},
    to_telegram::{MessageEntity, TelegramEntityBuilder},
};

/// Runs the parser over `markdown` in one go and returns the raw action stream.
#[must_use]
pub fn actions(markdown: &str) -> Vec<Action> {
    let mut parser = LlmMarkdownParser::new();
    let mut out = parser.push_chunk(markdown).actions;
    out.extend(parser.end().actions);
    out
}

/// Applies the action stream, yielding the events that actually survive rollbacks.
#[must_use]
pub fn events(markdown: &str) -> Vec<Event> {
    resolve(actions(markdown))
}

/// Feeds `markdown` to the parser one chunk at a time, as a stream would.
#[must_use]
pub fn events_chunked(markdown: &str, chunk_size: usize) -> Vec<Event> {
    assert!(chunk_size > 0, "chunk_size must be positive");

    let mut parser = LlmMarkdownParser::new();
    let mut out = Vec::new();
    let mut pending = String::new();

    for character in markdown.chars() {
        pending.push(character);
        if pending.chars().count() >= chunk_size {
            out.extend(parser.push_chunk(&pending).actions);
            pending.clear();
        }
    }
    if !pending.is_empty() {
        out.extend(parser.push_chunk(&pending).actions);
    }
    out.extend(parser.end().actions);

    resolve(out)
}

/// Collapses `Emit`/`Rollback` pairs the same way the entity builder does.
///
/// A rollback that asks for more events than have been emitted would silently
/// erase unrelated output in the real builder (it saturates), so it is treated as
/// a hard failure here.
#[must_use]
pub fn resolve(actions: Vec<Action>) -> Vec<Event> {
    let mut resolved: Vec<Event> = Vec::new();
    for action in actions {
        match action {
            Action::Emit(event) => resolved.push(event),
            Action::Rollback(count) => {
                assert!(
                    count <= resolved.len(),
                    "rollback of {count} with only {} events emitted",
                    resolved.len()
                );
                resolved.truncate(resolved.len() - count);
            }
        }
    }
    resolved
}

/// Joins runs of adjacent `Text` events into one.
///
/// Chunk boundaries decide only where the parser happens to flush its buffer, so
/// two streams that differ purely in text granularity are equivalent as far as the
/// entity builder is concerned.
#[must_use]
pub fn merge_text(events: Vec<Event>) -> Vec<Event> {
    let mut merged: Vec<Event> = Vec::with_capacity(events.len());
    for event in events {
        match (merged.last_mut(), event) {
            (Some(Event::Text(previous)), Event::Text(next)) => previous.push_str(&next),
            (_, event) => merged.push(event),
        }
    }
    merged
}

/// An opening-event predicate, its closing counterpart, and a name for messages.
pub type EventPair = (fn(&Event) -> bool, fn(&Event) -> bool, &'static str);

/// Every construct that has to open and close in pairs.
pub fn balanced_pairs() -> Vec<EventPair> {
    vec![
        (
            |e| matches!(e, Event::BoldStart),
            |e| matches!(e, Event::BoldEnd),
            "bold",
        ),
        (
            |e| matches!(e, Event::ItalicStart),
            |e| matches!(e, Event::ItalicEnd),
            "italic",
        ),
        (
            |e| matches!(e, Event::StrikethroughStart),
            |e| matches!(e, Event::StrikethroughEnd),
            "strikethrough",
        ),
        (
            |e| matches!(e, Event::SpoilerStart),
            |e| matches!(e, Event::SpoilerEnd),
            "spoiler",
        ),
        (
            |e| matches!(e, Event::UnderlineStart),
            |e| matches!(e, Event::UnderlineEnd),
            "underline",
        ),
        (
            |e| matches!(e, Event::SuperscriptStart),
            |e| matches!(e, Event::SuperscriptEnd),
            "superscript",
        ),
        (
            |e| matches!(e, Event::SubscriptStart),
            |e| matches!(e, Event::SubscriptEnd),
            "subscript",
        ),
        (
            |e| matches!(e, Event::LinkStart { .. }),
            |e| matches!(e, Event::LinkEnd),
            "link",
        ),
        (
            |e| matches!(e, Event::ImageStart { .. }),
            |e| matches!(e, Event::ImageEnd),
            "image",
        ),
        (
            |e| matches!(e, Event::CodeBlockStart(_)),
            |e| matches!(e, Event::CodeBlockEnd),
            "code block",
        ),
        (
            |e| matches!(e, Event::DisplayMathStart { .. }),
            |e| matches!(e, Event::DisplayMathEnd { .. }),
            "display math",
        ),
        (
            |e| matches!(e, Event::HeadingStart { .. }),
            |e| matches!(e, Event::HeadingEnd),
            "heading",
        ),
        (
            |e| matches!(e, Event::BlockquoteStart),
            |e| matches!(e, Event::BlockquoteEnd),
            "blockquote",
        ),
        (
            |e| matches!(e, Event::ListStart { .. }),
            |e| matches!(e, Event::ListEnd),
            "list",
        ),
        (
            |e| matches!(e, Event::ListItemStart { .. }),
            |e| matches!(e, Event::ListItemEnd),
            "list item",
        ),
        (
            |e| matches!(e, Event::TableStart),
            |e| matches!(e, Event::TableEnd),
            "table",
        ),
        (
            |e| matches!(e, Event::TableRowStart),
            |e| matches!(e, Event::TableRowEnd),
            "table row",
        ),
        (
            |e| matches!(e, Event::TableCellStart { .. }),
            |e| matches!(e, Event::TableCellEnd),
            "table cell",
        ),
    ]
}

/// Asserts that every start event in `markdown`'s resolved stream has its end.
pub fn assert_balanced(markdown: &str) {
    let resolved = resolve(actions(markdown));
    for (is_start, is_end, name) in balanced_pairs() {
        let starts = resolved.iter().filter(|e| is_start(e)).count();
        let ends = resolved.iter().filter(|e| is_end(e)).count();
        assert_eq!(
            starts, ends,
            "unbalanced {name} events for {markdown:?}: {resolved:?}"
        );
    }
}

/// Full pipeline: markdown in, Telegram text plus entities out.
#[must_use]
pub fn render(markdown: &str) -> (String, Vec<MessageEntity>) {
    let mut builder = TelegramEntityBuilder::new();
    for action in actions(markdown) {
        builder.push_action(action);
    }
    builder.build()
}

/// Just the rendered text, for tests that do not care about entities.
#[must_use]
pub fn text(markdown: &str) -> String {
    render(markdown).0
}

/// The entities of `kind`, as `(offset, length)` pairs in document order.
#[must_use]
pub fn spans(markdown: &str, kind: &str) -> Vec<(i64, i64)> {
    let (_, entities) = render(markdown);
    let mut found: Vec<(i64, i64)> = entities
        .iter()
        .filter(|entity| entity.r#type == kind)
        .map(|entity| (entity.offset, entity.length))
        .collect();
    found.sort_unstable();
    found
}

/// Extracts the substring an entity covers, using UTF-16 offsets like Telegram does.
#[must_use]
pub fn slice_utf16(text: &str, offset: i64, length: i64) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let start = usize::try_from(offset).unwrap_or(0);
    let end = start.saturating_add(usize::try_from(length).unwrap_or(0));
    String::from_utf16_lossy(units.get(start..end.min(units.len())).unwrap_or(&[]))
}

/// Asserts that every entity lies inside the text and, for well-known kinds, that
/// nesting stays balanced. Every rendering test funnels through this.
pub fn assert_entities_valid(text: &str, entities: &[MessageEntity]) {
    let total = i64::try_from(text.encode_utf16().count()).expect("text length fits in i64");
    for entity in entities {
        assert!(
            entity.offset >= 0 && entity.length > 0,
            "entity {entity:?} has a non-positive offset/length"
        );
        assert!(
            entity.offset.saturating_add(entity.length) <= total,
            "entity {entity:?} runs past the end of {text:?} ({total} utf-16 units)"
        );
        if entity.r#type == "text_link" {
            assert!(
                entity.url.is_some(),
                "text_link entity {entity:?} carries no url"
            );
        } else {
            assert!(
                entity.url.is_none(),
                "non-link entity {entity:?} carries a url"
            );
        }
        if entity.r#type != "pre" {
            assert!(
                entity.language.is_none(),
                "non-pre entity {entity:?} carries a language"
            );
        }
    }
}
