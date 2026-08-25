use _native::{
    from_markdown::{Action, Event, LlmMarkdownParser},
    limits::{CAPTION_LIMIT, MESSAGE_LIMIT},
    to_telegram::{EntityKind, MessageEntity, TelegramEntityBuilder},
};

#[must_use]
pub const fn limit_for(with_photo: bool) -> usize {
    if with_photo {
        CAPTION_LIMIT
    } else {
        MESSAGE_LIMIT
    }
}

#[must_use]
pub fn actions(markdown: &str) -> Vec<Action> {
    let mut parser = LlmMarkdownParser::new();
    let mut out = parser.push_chunk(markdown).actions;
    out.extend(parser.end().actions);
    out
}

#[must_use]
pub fn events(markdown: &str) -> Vec<Event> {
    resolve(actions(markdown))
}

#[must_use]
pub fn actions_chunked(markdown: &str, chunk_size: usize) -> Vec<Action> {
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

    out
}

#[must_use]
pub fn actions_at_boundaries(markdown: &str, boundaries: &[usize]) -> Vec<Action> {
    let characters: Vec<char> = markdown.chars().collect();
    let mut cuts: Vec<usize> = boundaries
        .iter()
        .map(|cut| (*cut).min(characters.len()))
        .collect();
    cuts.push(characters.len());
    cuts.sort_unstable();

    let mut parser = LlmMarkdownParser::new();
    let mut out = Vec::new();
    let mut start = 0_usize;

    for cut in cuts {
        if cut <= start {
            continue;
        }
        let piece: String = characters.get(start..cut).unwrap_or(&[]).iter().collect();
        out.extend(parser.push_chunk(&piece).actions);
        start = cut;
    }
    out.extend(parser.end().actions);

    out
}

#[must_use]
pub fn events_chunked(markdown: &str, chunk_size: usize) -> Vec<Event> {
    resolve(actions_chunked(markdown, chunk_size))
}

#[must_use]
pub fn render_actions(actions: Vec<Action>) -> (String, Vec<MessageEntity>) {
    let mut builder = TelegramEntityBuilder::new();
    for action in actions {
        builder.push_action(action);
    }
    let chunk = builder.build();
    (chunk.text, chunk.entities)
}

#[must_use]
pub fn render_chunked(markdown: &str, chunk_size: usize) -> (String, Vec<MessageEntity>) {
    render_actions(actions_chunked(markdown, chunk_size))
}

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

pub type EventPair = (fn(&Event) -> bool, fn(&Event) -> bool, &'static str);

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

#[must_use]
pub fn render(markdown: &str) -> (String, Vec<MessageEntity>) {
    render_actions(actions(markdown))
}

#[must_use]
pub fn text(markdown: &str) -> String {
    render(markdown).0
}

#[must_use]
pub fn spans(markdown: &str, kind: EntityKind) -> Vec<(usize, usize)> {
    let (_, entities) = render(markdown);
    let mut found: Vec<(usize, usize)> = entities
        .iter()
        .filter(|entity| entity.kind == kind)
        .map(|entity| (entity.offset, entity.length))
        .collect();
    found.sort_unstable();
    found
}

#[must_use]
pub fn slice_utf16(text: &str, offset: usize, length: usize) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let end = offset.saturating_add(length);
    String::from_utf16_lossy(units.get(offset..end.min(units.len())).unwrap_or(&[]))
}

pub fn assert_entities_valid(text: &str, entities: &[MessageEntity]) {
    let total = text.encode_utf16().count();
    for entity in entities {
        assert!(entity.length > 0, "entity {entity:?} is empty");
        assert!(
            entity.offset.saturating_add(entity.length) <= total,
            "entity {entity:?} runs past the end of {text:?} ({total} utf-16 units)"
        );
        if entity.kind == EntityKind::TextLink {
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
        if entity.kind != EntityKind::Pre {
            assert!(
                entity.language.is_none(),
                "non-pre entity {entity:?} carries a language"
            );
        }
    }
}

#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            return 0;
        }
        let drawn = self.next_u64();
        let bound_u64 = u64::try_from(bound).unwrap_or(u64::MAX);
        usize::try_from(drawn % bound_u64).unwrap_or(0)
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        let index = self.below(items.len());
        items.get(index).expect("picked index is inside the slice")
    }
}

pub const FRAGMENTS: &[&str] = &[
    "a",
    "b",
    " ",
    "\n",
    "\n\n",
    "\r\n",
    "\r",
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

#[must_use]
pub fn corpus_seeded(seed: u64, count: usize) -> Vec<String> {
    let mut rng = Rng::new(seed);
    (0..count)
        .map(|_| {
            let pieces = rng.below(40).saturating_add(1);
            (0..pieces).map(|_| *rng.pick(FRAGMENTS)).collect()
        })
        .collect()
}

#[must_use]
pub fn corpus(count: usize) -> Vec<String> {
    corpus_seeded(0x5EED_1234, count)
}

#[must_use]
pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

#[must_use]
pub fn text_of(markdown: &str) -> String {
    events(markdown)
        .iter()
        .filter_map(|event| match event {
            Event::Text(value) => Some(value.clone()),
            _ => None,
        })
        .collect()
}

#[must_use]
pub fn overlaps_partially(first: &MessageEntity, second: &MessageEntity) -> bool {
    let first_end = first.offset.saturating_add(first.length);
    let second_end = second.offset.saturating_add(second.length);
    let disjoint = first_end <= second.offset || second_end <= first.offset;
    let nested = (first.offset <= second.offset && second_end <= first_end)
        || (second.offset <= first.offset && first_end <= second_end);
    !disjoint && !nested
}

pub fn assert_no_partial_overlap(entities: &[MessageEntity], document: &str) {
    for (index, first) in entities.iter().enumerate() {
        for second in entities.get(index.saturating_add(1)..).unwrap_or(&[]) {
            assert!(
                !overlaps_partially(first, second),
                "{first:?} and {second:?} overlap partially for {document:?}"
            );
        }
    }
}

pub fn checked(markdown: &str) -> (String, Vec<MessageEntity>) {
    let (rendered, entities) = render(markdown);
    assert_entities_valid(&rendered, &entities);
    (rendered, entities)
}

pub fn assert_well_formed(markdown: &str) {
    assert_balanced(markdown);
    checked(markdown);
}
