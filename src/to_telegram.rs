use std::cmp::Reverse;

use comfy_table::{Table, presets::UTF8_FULL};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    from_markdown::{Action, Event, LlmMarkdownParser, TaskStatus},
    limits::{CAPTION_LIMIT, MAX_ENTITIES, MESSAGE_LIMIT},
};

const LIST_INDENT: &str = "  ";
const BULLET_TOP: &str = "•";
const BULLET_NESTED: &str = "◦";
const BULLET_DEEP: &str = "▪";
const CHECKBOX_TODO: &str = "☐ ";
const CHECKBOX_DONE: &str = "☑ ";
const THEMATIC_BREAK: &str = "──────────\n";
const HEADING_MARKER: &str = "#";
const SUPERSCRIPT_OPEN: &str = "<sup>";
const SUPERSCRIPT_CLOSE: &str = "</sup>";
const SUBSCRIPT_OPEN: &str = "<sub>";
const SUBSCRIPT_CLOSE: &str = "</sub>";

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum EntityKind {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Spoiler,
    Code,
    Pre,
    TextLink,
    Blockquote,
}

impl EntityKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bold => "bold",
            Self::Italic => "italic",
            Self::Underline => "underline",
            Self::Strikethrough => "strikethrough",
            Self::Spoiler => "spoiler",
            Self::Code => "code",
            Self::Pre => "pre",
            Self::TextLink => "text_link",
            Self::Blockquote => "blockquote",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MessageEntity {
    pub kind: EntityKind,
    pub offset: usize,
    pub length: usize,
    pub url: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MessageChunk {
    pub text: String,
    pub entities: Vec<MessageEntity>,
}

#[derive(Debug)]
struct ActiveEntity {
    kind: EntityKind,
    start_utf16_offset: usize,
    url: Option<String>,
    language: Option<String>,
}

impl ActiveEntity {
    fn close_at(self, end: usize) -> Option<MessageEntity> {
        let length = end.saturating_sub(self.start_utf16_offset);
        if length == 0 {
            return None;
        }
        Some(MessageEntity {
            kind: self.kind,
            offset: self.start_utf16_offset,
            length,
            url: self.url,
            language: self.language,
        })
    }
}

#[derive(Debug, Default)]
struct TableState {
    active: bool,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    current_row: Vec<String>,
    current_cell: String,
    is_header_row: bool,
}

#[derive(Debug)]
struct BuildState {
    text: String,
    entities: Vec<MessageEntity>,
    active_stack: Vec<ActiveEntity>,
    current_utf16_offset: usize,
}

impl BuildState {
    fn push_text(&mut self, s: &str) {
        self.text.push_str(s);
        self.current_utf16_offset = self
            .current_utf16_offset
            .saturating_add(s.encode_utf16().count());
    }

    fn open_entity(&mut self, kind: EntityKind, url: Option<String>, language: Option<String>) {
        self.active_stack.push(ActiveEntity {
            kind,
            start_utf16_offset: self.current_utf16_offset,
            url,
            language,
        });
    }

    fn pop_newline(&mut self) -> bool {
        if !self.text.ends_with('\n') {
            return false;
        }

        self.text.pop();
        self.current_utf16_offset = self.current_utf16_offset.saturating_sub(1);

        let limit = self.current_utf16_offset;
        self.entities.retain_mut(|entity| {
            if entity.offset.saturating_add(entity.length) > limit {
                entity.length = limit.saturating_sub(entity.offset);
            }
            entity.length > 0
        });
        for active in &mut self.active_stack {
            active.start_utf16_offset = active.start_utf16_offset.min(self.current_utf16_offset);
        }

        true
    }

    fn close_entity(&mut self, expected: EntityKind) {
        if let Some(index) = self.active_stack.iter().rposition(|e| e.kind == expected) {
            let active = self.active_stack.remove(index);
            self.entities
                .extend(active.close_at(self.current_utf16_offset));
        }
    }
}

#[derive(Debug)]
pub struct TelegramEntityBuilder {
    resolved_events: Vec<Event>,
}

const fn to_superscript(c: char) -> Option<char> {
    match c {
        '0' => Some('⁰'),
        '1' => Some('¹'),
        '2' => Some('²'),
        '3' => Some('³'),
        '4' => Some('⁴'),
        '5' => Some('⁵'),
        '6' => Some('⁶'),
        '7' => Some('⁷'),
        '8' => Some('⁸'),
        '9' => Some('⁹'),
        'a' => Some('ᵃ'),
        'b' => Some('ᵇ'),
        'c' => Some('ᶜ'),
        'd' => Some('ᵈ'),
        'e' => Some('ᵉ'),
        'f' => Some('ᶠ'),
        'g' => Some('ᵍ'),
        'h' => Some('ʰ'),
        'i' => Some('ⁱ'),
        'j' => Some('ʲ'),
        'k' => Some('ᵏ'),
        'l' => Some('ˡ'),
        'm' => Some('ᵐ'),
        'n' => Some('ⁿ'),
        'o' => Some('ᵒ'),
        'p' => Some('ᵖ'),
        'r' => Some('ʳ'),
        's' => Some('ˢ'),
        't' => Some('ᵗ'),
        'u' => Some('ᵘ'),
        'v' => Some('ᵛ'),
        'w' => Some('ʷ'),
        'x' => Some('ˣ'),
        'y' => Some('ʸ'),
        'z' => Some('ᶻ'),
        'A' => Some('ᴬ'),
        'B' => Some('ᴮ'),
        'D' => Some('ᴰ'),
        'E' => Some('ᴱ'),
        'G' => Some('ᴳ'),
        'H' => Some('ᴴ'),
        'I' => Some('ᴵ'),
        'J' => Some('ᴶ'),
        'K' => Some('ᴷ'),
        'L' => Some('ᴸ'),
        'M' => Some('ᴹ'),
        'N' => Some('ᴺ'),
        'O' => Some('ᴼ'),
        'P' => Some('ᴾ'),
        'R' => Some('ᴿ'),
        'T' => Some('ᵀ'),
        'U' => Some('ᵁ'),
        'V' => Some('ⱽ'),
        'W' => Some('ᵂ'),
        '+' => Some('⁺'),
        '-' => Some('⁻'),
        '=' => Some('⁼'),
        '(' => Some('⁽'),
        ')' => Some('⁾'),
        _ => None,
    }
}

const fn to_subscript(c: char) -> Option<char> {
    match c {
        '0' => Some('₀'),
        '1' => Some('₁'),
        '2' => Some('₂'),
        '3' => Some('₃'),
        '4' => Some('₄'),
        '5' => Some('₅'),
        '6' => Some('₆'),
        '7' => Some('₇'),
        '8' => Some('₈'),
        '9' => Some('₉'),
        'a' => Some('ₐ'),
        'e' => Some('ₑ'),
        'h' => Some('ₕ'),
        'i' => Some('ᵢ'),
        'j' => Some('ⱼ'),
        'k' => Some('ₖ'),
        'l' => Some('ₗ'),
        'm' => Some('ₘ'),
        'n' => Some('ₙ'),
        'o' => Some('ₒ'),
        'p' => Some('ₚ'),
        'r' => Some('ᵣ'),
        's' => Some('ₛ'),
        't' => Some('ₜ'),
        'u' => Some('ᵤ'),
        'v' => Some('ᵥ'),
        'x' => Some('ₓ'),
        '+' => Some('₊'),
        '-' => Some('₋'),
        '=' => Some('₌'),
        '(' => Some('₍'),
        ')' => Some('₎'),
        _ => None,
    }
}

fn is_block_mappable(events: &[Event], start_idx: usize, is_sup: bool) -> bool {
    let mut depth = 1_usize;
    for ev in events.get(start_idx..).unwrap_or(&[]) {
        match ev {
            Event::SuperscriptStart | Event::SubscriptStart => depth = depth.saturating_add(1),
            Event::SuperscriptEnd | Event::SubscriptEnd => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return true;
                }
            }
            Event::Text(s) | Event::InlineCode(s) | Event::InlineMath { content: s, .. } =>
                for c in s.chars() {
                    if is_sup && to_superscript(c).is_none() {
                        return false;
                    }
                    if !is_sup && to_subscript(c).is_none() {
                        return false;
                    }
                },
            _ => {}
        }
    }
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Script {
    Super,
    Sub,
}

#[derive(Debug, Default)]
struct ScriptStack(Vec<Option<Script>>);

impl ScriptStack {
    fn open(&mut self, events: &[Event], next: usize, script: Script) -> Option<&'static str> {
        let mappable = is_block_mappable(events, next, script == Script::Super);
        self.0.push(mappable.then_some(script));
        if mappable {
            return None;
        }
        Some(match script {
            Script::Super => SUPERSCRIPT_OPEN,
            Script::Sub => SUBSCRIPT_OPEN,
        })
    }

    fn close(&mut self, script: Script) -> Option<&'static str> {
        if self.0.pop() != Some(None) {
            return None;
        }
        Some(match script {
            Script::Super => SUPERSCRIPT_CLOSE,
            Script::Sub => SUBSCRIPT_CLOSE,
        })
    }

    fn apply(&self, text: &str) -> String {
        apply_script(&self.0, text)
    }
}

fn apply_script(scripts: &[Option<Script>], text: &str) -> String {
    match scripts.iter().rev().find_map(|script| *script) {
        Some(Script::Super) => text
            .chars()
            .map(|character| to_superscript(character).unwrap_or(character))
            .collect(),
        Some(Script::Sub) => text
            .chars()
            .map(|character| to_subscript(character).unwrap_or(character))
            .collect(),
        None => text.to_string(),
    }
}

fn label_is_empty(events: &[Event], start_idx: usize) -> bool {
    let mut depth = 1_usize;
    for ev in events.get(start_idx..).unwrap_or(&[]) {
        match ev {
            Event::ImageStart { .. } | Event::LinkStart { .. } => depth = depth.saturating_add(1),
            Event::ImageEnd | Event::LinkEnd => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return true;
                }
            }
            Event::Text(s) | Event::InlineCode(s) | Event::InlineMath { content: s, .. }
                if !s.trim().is_empty() =>
            {
                return false;
            }
            _ => {}
        }
    }
    true
}

impl Default for TelegramEntityBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TelegramEntityBuilder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            resolved_events: Vec::new(),
        }
    }

    pub fn push_action(&mut self, action: Action) {
        match action {
            Action::Emit(event) => self.resolved_events.push(event),
            Action::Rollback(count) => {
                let new_len = self.resolved_events.len().saturating_sub(count);
                self.resolved_events.truncate(new_len);
            }
        }
    }

    #[must_use]
    pub fn build(self) -> MessageChunk {
        let mut state = BuildState {
            text: String::new(),
            entities: Vec::new(),
            active_stack: Vec::new(),
            current_utf16_offset: 0,
        };

        let mut list_stack: Vec<Option<u64>> = Vec::new();
        let mut table_state = TableState::default();

        let mut scripts = ScriptStack::default();

        let mut link_depth = 0_usize;
        let mut quote_depth = 0_usize;

        for (i, event) in self.resolved_events.iter().enumerate() {
            if table_state.active {
                match event {
                    Event::TableEnd => {
                        let mut table = Table::new();
                        table.load_preset(UTF8_FULL);

                        if !table_state.headers.is_empty() {
                            table.set_header(&table_state.headers);
                        }
                        for row in &table_state.rows {
                            table.add_row(row);
                        }

                        let rendered = table.to_string();
                        if !rendered.is_empty() {
                            if !state.text.is_empty() && !state.text.ends_with('\n') {
                                state.push_text("\n");
                            }
                            for (j, line) in rendered.lines().enumerate() {
                                if j > 0 {
                                    state.push_text("\n");
                                }
                                state.open_entity(EntityKind::Code, None, None);
                                state.push_text(line);
                                state.close_entity(EntityKind::Code);
                            }
                            state.push_text("\n");
                        }

                        let trailing = std::mem::take(&mut table_state.current_cell);
                        if !trailing.is_empty() {
                            state.push_text(&trailing);
                        }
                        table_state = TableState::default();
                    }
                    Event::TableRowStart => {
                        table_state.current_row.clear();
                        table_state.is_header_row = false;
                    }
                    Event::TableRowEnd => {
                        let row = std::mem::take(&mut table_state.current_row);
                        if table_state.is_header_row {
                            table_state.headers = row;
                        } else if !row.is_empty() {
                            table_state.rows.push(row);
                        }
                    }
                    Event::TableCellStart { is_header } => {
                        table_state.current_cell.clear();
                        if *is_header {
                            table_state.is_header_row = true;
                        }
                    }
                    Event::TableCellEnd => {
                        let cell_text = std::mem::take(&mut table_state.current_cell);
                        table_state.current_row.push(cell_text.trim().to_string());
                    }
                    Event::SuperscriptStart | Event::SubscriptStart => {
                        let script = if matches!(event, Event::SuperscriptStart) {
                            Script::Super
                        } else {
                            Script::Sub
                        };
                        if let Some(tag) =
                            scripts.open(&self.resolved_events, i.saturating_add(1), script)
                        {
                            table_state.current_cell.push_str(tag);
                        }
                    }
                    Event::SuperscriptEnd | Event::SubscriptEnd => {
                        let script = if matches!(event, Event::SuperscriptEnd) {
                            Script::Super
                        } else {
                            Script::Sub
                        };
                        if let Some(tag) = scripts.close(script) {
                            table_state.current_cell.push_str(tag);
                        }
                    }
                    Event::ImageStart { url } | Event::LinkStart { url } => {
                        if label_is_empty(&self.resolved_events, i.saturating_add(1)) {
                            table_state.current_cell.push_str(url);
                        }
                    }
                    Event::Text(string_value)
                    | Event::InlineCode(string_value)
                    | Event::InlineMath {
                        content: string_value,
                        ..
                    } => table_state
                        .current_cell
                        .push_str(&scripts.apply(string_value)),
                    _ => {}
                }
                continue;
            }

            match event {
                Event::TableStart => {
                    table_state.active = true;
                }
                Event::SuperscriptStart | Event::SubscriptStart => {
                    let script = if matches!(event, Event::SuperscriptStart) {
                        Script::Super
                    } else {
                        Script::Sub
                    };
                    if let Some(tag) =
                        scripts.open(&self.resolved_events, i.saturating_add(1), script)
                    {
                        state.push_text(tag);
                    }
                }
                Event::SuperscriptEnd | Event::SubscriptEnd => {
                    let script = if matches!(event, Event::SuperscriptEnd) {
                        Script::Super
                    } else {
                        Script::Sub
                    };
                    if let Some(tag) = scripts.close(script) {
                        state.push_text(tag);
                    }
                }
                Event::Text(string_value) => state.push_text(&scripts.apply(string_value)),
                Event::BoldStart => state.open_entity(EntityKind::Bold, None, None),
                Event::BoldEnd => state.close_entity(EntityKind::Bold),
                Event::ItalicStart => state.open_entity(EntityKind::Italic, None, None),
                Event::ItalicEnd => state.close_entity(EntityKind::Italic),
                Event::StrikethroughStart =>
                    state.open_entity(EntityKind::Strikethrough, None, None),
                Event::StrikethroughEnd => state.close_entity(EntityKind::Strikethrough),
                Event::UnderlineStart => state.open_entity(EntityKind::Underline, None, None),
                Event::UnderlineEnd => state.close_entity(EntityKind::Underline),
                Event::SpoilerStart => state.open_entity(EntityKind::Spoiler, None, None),
                Event::SpoilerEnd => state.close_entity(EntityKind::Spoiler),
                Event::BlockquoteStart => {
                    if quote_depth == 0 {
                        state.open_entity(EntityKind::Blockquote, None, None);
                    }
                    quote_depth = quote_depth.saturating_add(1);
                }
                Event::BlockquoteEnd => {
                    quote_depth = quote_depth.saturating_sub(1);
                    if quote_depth == 0 {
                        state.close_entity(EntityKind::Blockquote);
                    }
                }

                Event::LinkStart { url } | Event::ImageStart { url } => {
                    if link_depth == 0 {
                        state.open_entity(EntityKind::TextLink, Some(url.clone()), None);
                    }
                    link_depth = link_depth.saturating_add(1);
                    if label_is_empty(&self.resolved_events, i.saturating_add(1)) {
                        state.push_text(url);
                    }
                }
                Event::LinkEnd | Event::ImageEnd => {
                    link_depth = link_depth.saturating_sub(1);
                    if link_depth == 0 {
                        state.close_entity(EntityKind::TextLink);
                    }
                }

                Event::CodeBlockStart(lang) => {
                    let language = if lang.trim().is_empty() {
                        None
                    } else {
                        Some(lang.trim().to_string())
                    };
                    state.open_entity(EntityKind::Pre, None, language);
                }
                Event::CodeBlockEnd | Event::DisplayMathEnd { .. } => {
                    state.pop_newline();
                    state.close_entity(EntityKind::Pre);
                }
                Event::InlineCode(code) | Event::InlineMath { content: code, .. } => {
                    state.open_entity(EntityKind::Code, None, None);
                    state.push_text(&scripts.apply(code));
                    state.close_entity(EntityKind::Code);
                }

                Event::HeadingStart { level } => {
                    state.push_text(&format!("{} ", HEADING_MARKER.repeat(usize::from(*level))));
                    state.open_entity(EntityKind::Bold, None, None);
                }
                Event::HeadingEnd => {
                    state.close_entity(EntityKind::Bold);
                }

                Event::ListStart { ordered, start } => {
                    list_stack.push(ordered.then_some(*start));
                }
                Event::ListItemStart { task_status } => {
                    let depth = list_stack.len().saturating_sub(1);
                    let indent = LIST_INDENT.repeat(depth);
                    state.push_text(&indent);

                    let is_ordered = matches!(list_stack.last(), Some(Some(_)));

                    if is_ordered {
                        let mut num_str = String::new();
                        let len = list_stack.len();
                        for (j, level) in list_stack.iter().enumerate() {
                            if let Some(number) = level {
                                if !num_str.is_empty() {
                                    num_str.push('.');
                                }

                                let shown = if j == len.saturating_sub(1) {
                                    *number
                                } else {
                                    number.saturating_sub(1)
                                };
                                num_str.push_str(&shown.to_string());
                            }
                        }
                        num_str.push('.');

                        if let Some(Some(last)) = list_stack.last_mut() {
                            *last = last.saturating_add(1);
                        }

                        match task_status {
                            TaskStatus::None => state.push_text(&format!("{num_str} ")),
                            TaskStatus::Todo =>
                                state.push_text(&format!("{num_str} {CHECKBOX_TODO}")),
                            TaskStatus::Done =>
                                state.push_text(&format!("{num_str} {CHECKBOX_DONE}")),
                        }
                    } else if !list_stack.is_empty() {
                        let bullet = match depth % 3 {
                            0 => BULLET_TOP,
                            1 => BULLET_NESTED,
                            _ => BULLET_DEEP,
                        };
                        match task_status {
                            TaskStatus::None => state.push_text(&format!("{bullet} ")),
                            TaskStatus::Todo => state.push_text(CHECKBOX_TODO),
                            TaskStatus::Done => state.push_text(CHECKBOX_DONE),
                        }
                    }
                }
                Event::ListEnd => {
                    list_stack.pop();
                }

                Event::ThematicBreak => state.push_text(THEMATIC_BREAK),

                Event::DisplayMathStart { .. } => {
                    state.open_entity(EntityKind::Pre, None, None);
                }

                _ => {}
            }
        }

        let end = state.current_utf16_offset;
        let dangling: Vec<MessageEntity> = state
            .active_stack
            .drain(..)
            .rev()
            .filter_map(|active| active.close_at(end))
            .collect();
        state.entities.extend(dangling);
        state
            .entities
            .sort_by_key(|entity| (entity.offset, Reverse(entity.length)));

        MessageChunk {
            text: state.text,
            entities: state.entities,
        }
    }
}

#[must_use]
pub fn split_message_with_entities(
    text: &str,
    entities: &[MessageEntity],
    limit: usize,
) -> Vec<MessageChunk> {
    if text.is_empty() || limit == 0 {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut start_byte = 0_usize;
    let mut start_utf16 = 0_usize;

    while start_byte < text.len() {
        let rest = text.get(start_byte..).unwrap_or_default();
        let mut cut = cut_within_limit(rest, limit);
        cut = clamp_to_entity_budget(rest, entities, start_utf16, cut);

        let end_utf16 = start_utf16.saturating_add(cut.utf16);
        let chunk = rest.get(..cut.byte).unwrap_or_default();
        let chunk_entities = clip_entities(entities, start_utf16, end_utf16);

        if !chunk.trim().is_empty() {
            result.push(MessageChunk {
                text: chunk.to_string(),
                entities: chunk_entities,
            });
        }

        start_byte = start_byte.saturating_add(cut.byte);
        start_utf16 = end_utf16;
    }

    result
}

#[derive(Debug, Clone, Copy)]
struct Cut {
    byte: usize,
    utf16: usize,
}

fn cut_within_limit(rest: &str, limit: usize) -> Cut {
    let earliest_useful = limit / 2;
    let mut taken = Cut { byte: 0, utf16: 0 };
    let mut after_space: Option<Cut> = None;
    let mut after_newline: Option<Cut> = None;
    let mut truncated = false;

    for cluster in rest.graphemes(true) {
        let width = cluster.encode_utf16().count();
        if taken.utf16.saturating_add(width) > limit && taken.byte > 0 {
            truncated = true;
            break;
        }

        taken = Cut {
            byte: taken.byte.saturating_add(cluster.len()),
            utf16: taken.utf16.saturating_add(width),
        };

        if cluster.ends_with('\n') {
            after_newline = Some(taken);
        } else if cluster.ends_with(' ') {
            after_space = Some(taken);
        }
    }

    if !truncated {
        return taken;
    }

    after_newline
        .or(after_space)
        .filter(|candidate| candidate.utf16 >= earliest_useful)
        .unwrap_or(taken)
}

fn clamp_to_entity_budget(
    rest: &str,
    entities: &[MessageEntity],
    start_utf16: usize,
    cut: Cut,
) -> Cut {
    let end_utf16 = start_utf16.saturating_add(cut.utf16);
    let mut overlapping = entities
        .iter()
        .filter(|entity| covers(entity, start_utf16, end_utf16));
    let Some(overflowing) = overlapping.nth(MAX_ENTITIES) else {
        return cut;
    };

    let boundary = overflowing.offset;
    if boundary <= start_utf16 || boundary >= end_utf16 {
        return cut;
    }

    let wanted = boundary.saturating_sub(start_utf16);
    let mut shortened = Cut { byte: 0, utf16: 0 };
    for cluster in rest.graphemes(true) {
        if shortened.utf16 >= wanted {
            break;
        }
        shortened = Cut {
            byte: shortened.byte.saturating_add(cluster.len()),
            utf16: shortened
                .utf16
                .saturating_add(cluster.encode_utf16().count()),
        };
    }

    if shortened.byte == 0 { cut } else { shortened }
}

fn covers(entity: &MessageEntity, start_utf16: usize, end_utf16: usize) -> bool {
    let entity_end = entity.offset.saturating_add(entity.length);
    entity.offset < end_utf16 && entity_end > start_utf16
}

fn clip_entities(
    entities: &[MessageEntity],
    start_utf16: usize,
    end_utf16: usize,
) -> Vec<MessageEntity> {
    entities
        .iter()
        .filter(|entity| covers(entity, start_utf16, end_utf16))
        .filter_map(|entity| {
            let entity_end = entity.offset.saturating_add(entity.length);
            let overlap_start = entity.offset.max(start_utf16);
            let overlap_end = entity_end.min(end_utf16);
            let length = overlap_end.saturating_sub(overlap_start);
            if length == 0 {
                return None;
            }
            Some(MessageEntity {
                kind: entity.kind,
                offset: overlap_start.saturating_sub(start_utf16),
                length,
                url: entity.url.clone(),
                language: entity.language.clone(),
            })
        })
        .collect()
}

#[must_use]
pub fn process_markdown(markdown: &str, with_photo: bool) -> Vec<MessageChunk> {
    let mut parser = LlmMarkdownParser::new();
    let chunk_result = parser.push_chunk(markdown);
    let end_result = parser.end();

    let mut builder = TelegramEntityBuilder::new();
    for action in chunk_result.actions.into_iter().chain(end_result.actions) {
        builder.push_action(action);
    }

    let whole = builder.build();
    let limit = if with_photo {
        CAPTION_LIMIT
    } else {
        MESSAGE_LIMIT
    };

    split_message_with_entities(&whole.text, &whole.entities, limit)
}

#[cfg(test)]
mod tests {
    use super::{
        BuildState,
        EntityKind,
        Event,
        MessageEntity,
        Script,
        apply_script,
        is_block_mappable,
        label_is_empty,
        split_message_with_entities,
        to_subscript,
        to_superscript,
    };

    fn state() -> BuildState {
        BuildState {
            text: String::new(),
            entities: Vec::new(),
            active_stack: Vec::new(),
            current_utf16_offset: 0,
        }
    }

    #[test]
    fn digits_and_letters_map_to_scripts() {
        assert_eq!(to_superscript('2'), Some('²'));
        assert_eq!(to_superscript('n'), Some('ⁿ'));
        assert_eq!(to_subscript('2'), Some('₂'));
        assert_eq!(to_subscript('x'), Some('ₓ'));
    }

    #[test]
    fn characters_without_a_script_form_map_to_nothing() {
        for character in ['q', 'C', 'F', 'ж', '😀', '/'] {
            assert_eq!(to_superscript(character), None);
        }
        for character in ['b', 'z', 'Q', 'ж', '😀', '/'] {
            assert_eq!(to_subscript(character), None);
        }
    }

    #[test]
    fn the_innermost_script_decides_how_text_is_mapped() {
        assert_eq!(apply_script(&[], "x2"), "x2");
        assert_eq!(apply_script(&[Some(Script::Super)], "12"), "¹²");
        assert_eq!(apply_script(&[Some(Script::Sub)], "12"), "₁₂");
        assert_eq!(
            apply_script(&[Some(Script::Super), Some(Script::Sub)], "1"),
            "₁"
        );
        assert_eq!(apply_script(&[Some(Script::Super), None], "1"), "¹");
    }

    #[test]
    fn unmappable_characters_pass_through_a_script() {
        assert_eq!(apply_script(&[Some(Script::Super)], "qC"), "qC");
    }

    #[test]
    fn a_block_is_mappable_only_when_every_character_is() {
        let mappable = [
            Event::Text("12".to_string()),
            Event::SuperscriptEnd,
            Event::Text("rest".to_string()),
        ];
        assert!(is_block_mappable(&mappable, 0, true));

        let unmappable = [Event::Text("1q".to_string()), Event::SuperscriptEnd];
        assert!(!is_block_mappable(&unmappable, 0, true));
    }

    #[test]
    fn an_unclosed_script_is_not_mappable() {
        let events = [Event::Text("12".to_string())];
        assert!(!is_block_mappable(&events, 0, true));
    }

    #[test]
    fn a_nested_script_is_closed_by_its_own_end() {
        let events = [
            Event::SuperscriptStart,
            Event::Text("1".to_string()),
            Event::SuperscriptEnd,
            Event::Text("2".to_string()),
            Event::SuperscriptEnd,
        ];
        assert!(is_block_mappable(&events, 0, true));
    }

    #[test]
    fn a_label_is_empty_when_it_holds_no_visible_text() {
        assert!(label_is_empty(&[Event::LinkEnd], 0));
        assert!(label_is_empty(
            &[Event::Text("   ".to_string()), Event::LinkEnd],
            0
        ));
        assert!(!label_is_empty(
            &[Event::Text("alt".to_string()), Event::LinkEnd],
            0
        ));
    }

    #[test]
    fn an_unterminated_label_counts_as_empty() {
        assert!(label_is_empty(&[Event::BoldStart], 0));
    }

    #[test]
    fn text_is_measured_in_utf16_units() {
        let mut build = state();
        build.push_text("ж😀");
        assert_eq!(build.current_utf16_offset, 3);
    }

    #[test]
    fn closing_an_entity_that_was_never_opened_does_nothing() {
        let mut build = state();
        build.push_text("text");
        build.close_entity(EntityKind::Bold);
        assert_eq!(build.entities, Vec::new());
    }

    #[test]
    fn an_entity_covering_nothing_is_dropped() {
        let mut build = state();
        build.open_entity(EntityKind::Bold, None, None);
        build.close_entity(EntityKind::Bold);
        assert_eq!(build.entities, Vec::new());
    }

    #[test]
    fn popping_a_newline_clips_the_entities_that_covered_it() {
        let mut build = state();
        build.open_entity(EntityKind::Pre, None, None);
        build.push_text("code\n");
        build.close_entity(EntityKind::Pre);

        assert!(build.pop_newline());
        assert_eq!(build.text, "code");
        assert_eq!(build.entities.len(), 1);
        assert_eq!(build.entities[0].length, 4);
    }

    #[test]
    fn popping_a_newline_that_is_not_there_changes_nothing() {
        let mut build = state();
        build.push_text("code");
        assert!(!build.pop_newline());
        assert_eq!(build.text, "code");
    }

    #[test]
    fn splitting_empty_text_produces_no_chunks() {
        assert_eq!(split_message_with_entities("", &[], 10), Vec::new());
    }

    #[test]
    fn an_entity_is_clipped_to_the_chunk_that_holds_it() {
        let entity = MessageEntity {
            kind: EntityKind::Bold,
            offset: 0,
            length: 10,
            url: None,
            language: None,
        };
        let chunks = split_message_with_entities("abcdefghij", &[entity], 4);

        assert_eq!(chunks.len(), 3);
        for chunk in &chunks {
            let length = chunk.text.encode_utf16().count();
            assert_eq!(chunk.entities.len(), 1);
            assert_eq!(chunk.entities[0].offset, 0);
            assert_eq!(chunk.entities[0].length, length);
        }
    }
}
