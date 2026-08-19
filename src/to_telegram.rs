use comfy_table::{Table, presets::UTF8_FULL};
use serde::{Deserialize, Serialize};

use crate::from_markdown::{Action, Event, LlmMarkdownParser, TaskStatus};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct MessageEntity {
    pub r#type: String,
    pub offset: i64,
    pub length: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

#[derive(Debug)]
struct ActiveEntity {
    r#type: String,
    start_utf16_offset: usize,
    url: Option<String>,
    language: Option<String>,
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

    fn open_entity(&mut self, r#type: &str, url: Option<String>, language: Option<String>) {
        self.active_stack.push(ActiveEntity {
            r#type: r#type.to_string(),
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

        let limit = i64::try_from(self.current_utf16_offset).unwrap_or(i64::MAX);
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

    fn close_entity(&mut self, expected_type: &str) {
        if let Some(idx) = self
            .active_stack
            .iter()
            .rposition(|e| e.r#type == expected_type)
        {
            let active = self.active_stack.remove(idx);
            let length = self
                .current_utf16_offset
                .saturating_sub(active.start_utf16_offset);

            if length > 0 {
                self.entities.push(MessageEntity {
                    r#type: active.r#type,
                    offset: i64::try_from(active.start_utf16_offset).unwrap_or(0),
                    length: i64::try_from(length).unwrap_or(0),
                    url: active.url,
                    language: active.language,
                });
            }
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
            Event::Text(s)
            | Event::InlineCode(s)
            | Event::HeadingText(s)
            | Event::DisplayMathText(s)
            | Event::InlineMath { content: s, .. } =>
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
            Event::Text(s)
            | Event::InlineCode(s)
            | Event::HeadingText(s)
            | Event::InlineMath { content: s, .. }
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
    #[allow(clippy::too_many_lines)]
    pub fn build(&self) -> (String, Vec<MessageEntity>) {
        let mut state = BuildState {
            text: String::new(),
            entities: Vec::new(),
            active_stack: Vec::new(),
            current_utf16_offset: 0,
        };

        let mut list_stack: Vec<Option<u64>> = Vec::new();
        let mut table_state = TableState::default();

        let mut scripts: Vec<Option<Script>> = Vec::new();

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
                                state.open_entity("code", None, None);
                                state.push_text(line);
                                state.close_entity("code");
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
                    Event::SuperscriptStart => {
                        let mappable =
                            is_block_mappable(&self.resolved_events, i.saturating_add(1), true);
                        scripts.push(mappable.then_some(Script::Super));
                        if !mappable {
                            table_state.current_cell.push_str("<sup>");
                        }
                    }
                    Event::SuperscriptEnd =>
                        if scripts.pop() == Some(None) {
                            table_state.current_cell.push_str("</sup>");
                        },
                    Event::SubscriptStart => {
                        let mappable =
                            is_block_mappable(&self.resolved_events, i.saturating_add(1), false);
                        scripts.push(mappable.then_some(Script::Sub));
                        if !mappable {
                            table_state.current_cell.push_str("<sub>");
                        }
                    }
                    Event::SubscriptEnd =>
                        if scripts.pop() == Some(None) {
                            table_state.current_cell.push_str("</sub>");
                        },
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
                        .push_str(&apply_script(&scripts, string_value)),
                    _ => {}
                }
                continue;
            }

            match event {
                Event::TableStart => {
                    table_state.active = true;
                }
                Event::SuperscriptStart => {
                    let mappable =
                        is_block_mappable(&self.resolved_events, i.saturating_add(1), true);
                    scripts.push(mappable.then_some(Script::Super));
                    if !mappable {
                        state.push_text("<sup>");
                    }
                }
                Event::SuperscriptEnd =>
                    if scripts.pop() == Some(None) {
                        state.push_text("</sup>");
                    },
                Event::SubscriptStart => {
                    let mappable =
                        is_block_mappable(&self.resolved_events, i.saturating_add(1), false);
                    scripts.push(mappable.then_some(Script::Sub));
                    if !mappable {
                        state.push_text("<sub>");
                    }
                }
                Event::SubscriptEnd =>
                    if scripts.pop() == Some(None) {
                        state.push_text("</sub>");
                    },
                Event::Text(string_value)
                | Event::HeadingText(string_value)
                | Event::DisplayMathText(string_value) =>
                    state.push_text(&apply_script(&scripts, string_value)),
                Event::BoldStart => state.open_entity("bold", None, None),
                Event::BoldEnd => state.close_entity("bold"),
                Event::ItalicStart => state.open_entity("italic", None, None),
                Event::ItalicEnd => state.close_entity("italic"),
                Event::StrikethroughStart => state.open_entity("strikethrough", None, None),
                Event::StrikethroughEnd => state.close_entity("strikethrough"),
                Event::UnderlineStart => state.open_entity("underline", None, None),
                Event::UnderlineEnd => state.close_entity("underline"),
                Event::SpoilerStart => state.open_entity("spoiler", None, None),
                Event::SpoilerEnd => state.close_entity("spoiler"),
                Event::BlockquoteStart => {
                    if quote_depth == 0 {
                        state.open_entity("blockquote", None, None);
                    }
                    quote_depth = quote_depth.saturating_add(1);
                }
                Event::BlockquoteEnd => {
                    quote_depth = quote_depth.saturating_sub(1);
                    if quote_depth == 0 {
                        state.close_entity("blockquote");
                    }
                }

                Event::LinkStart { url } | Event::ImageStart { url } => {
                    if link_depth == 0 {
                        state.open_entity("text_link", Some(url.clone()), None);
                    }
                    link_depth = link_depth.saturating_add(1);
                    if label_is_empty(&self.resolved_events, i.saturating_add(1)) {
                        state.push_text(url);
                    }
                }
                Event::LinkEnd | Event::ImageEnd => {
                    link_depth = link_depth.saturating_sub(1);
                    if link_depth == 0 {
                        state.close_entity("text_link");
                    }
                }

                Event::CodeBlockStart(lang) => {
                    let language = if lang.trim().is_empty() {
                        None
                    } else {
                        Some(lang.trim().to_string())
                    };
                    state.open_entity("pre", None, language);
                }
                Event::CodeBlockEnd | Event::DisplayMathEnd { .. } => {
                    state.pop_newline();
                    state.close_entity("pre");
                }
                Event::InlineCode(code) | Event::InlineMath { content: code, .. } => {
                    state.open_entity("code", None, None);
                    state.push_text(&apply_script(&scripts, code));
                    state.close_entity("code");
                }

                Event::HeadingStart { level } => {
                    state.push_text(&format!("{} ", "#".repeat(usize::from(*level))));
                    state.open_entity("bold", None, None);
                }
                Event::HeadingEnd => {
                    state.close_entity("bold");
                }

                Event::ListStart { ordered, start } => {
                    list_stack.push(ordered.then_some(*start));
                }
                Event::ListItemStart { task_status } => {
                    let depth = list_stack.len().saturating_sub(1);
                    let indent = "  ".repeat(depth);
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
                            TaskStatus::Todo => state.push_text(&format!("{num_str} ☐ ")),
                            TaskStatus::Done => state.push_text(&format!("{num_str} ☑ ")),
                        }
                    } else if !list_stack.is_empty() {
                        let bullet = match depth % 3 {
                            0 => "•",
                            1 => "◦",
                            _ => "▪",
                        };
                        match task_status {
                            TaskStatus::None => state.push_text(&format!("{bullet} ")),
                            TaskStatus::Todo => state.push_text("☐ "),
                            TaskStatus::Done => state.push_text("☑ "),
                        }
                    }
                }
                Event::ListEnd => {
                    list_stack.pop();
                }

                Event::ThematicBreak => state.push_text("──────────\n"),

                Event::DisplayMathStart { .. } => {
                    state.open_entity("pre", None, None);
                }

                _ => {}
            }
        }

        for active in state.active_stack.into_iter().rev() {
            let length = state
                .current_utf16_offset
                .saturating_sub(active.start_utf16_offset);
            if length > 0 {
                state.entities.push(MessageEntity {
                    r#type: active.r#type,
                    offset: i64::try_from(active.start_utf16_offset).unwrap_or(0),
                    length: i64::try_from(length).unwrap_or(0),
                    url: active.url,
                    language: active.language,
                });
            }
        }

        (state.text, state.entities)
    }
}

#[must_use]
pub fn split_message_with_entities(
    text: &str,
    entities: &[MessageEntity],
    limit: usize,
) -> Vec<(String, Vec<MessageEntity>)> {
    if text.is_empty() {
        return Vec::new();
    }

    let utf16_chars: Vec<u16> = text.encode_utf16().collect();
    let mut result = Vec::new();
    let mut current_start = 0;

    while current_start < utf16_chars.len() {
        let remaining = utf16_chars.len().saturating_sub(current_start);
        let chunk_size = if remaining <= limit {
            remaining
        } else {
            let slice_end = current_start.saturating_add(limit).min(utf16_chars.len());
            let slice = utf16_chars.get(current_start..slice_end).unwrap_or(&[]);
            let mut cut = limit;

            let earliest_useful_cut = limit / 2;
            let break_at = |wanted: u16| {
                slice
                    .iter()
                    .rposition(|&character| character == wanted)
                    .map(|index| index.saturating_add(1))
                    .filter(|candidate| *candidate >= earliest_useful_cut)
            };
            if let Some(candidate) =
                break_at(u16::from(b'\n')).or_else(|| break_at(u16::from(b' ')))
            {
                cut = candidate;
            }

            let cut_idx = current_start.saturating_add(cut);
            if let Some(&character) = utf16_chars.get(cut_idx)
                && (0xDC00..=0xDFFF).contains(&character)
            {
                cut = cut.saturating_sub(1);
            }

            if cut == 0 {
                let starts_pair = utf16_chars
                    .get(current_start)
                    .is_some_and(|&unit| (0xD800..=0xDBFF).contains(&unit));
                cut = if starts_pair { 2 } else { 1 };
            }

            cut.min(remaining)
        };

        let chunk_end = current_start.saturating_add(chunk_size);
        let chunk_utf16 = utf16_chars.get(current_start..chunk_end).unwrap_or(&[]);
        let chunk_text = String::from_utf16_lossy(chunk_utf16);

        let mut chunk_entities = Vec::new();
        for entity in entities {
            let entity_offset = usize::try_from(entity.offset).unwrap_or(0);
            let entity_length = usize::try_from(entity.length).unwrap_or(0);
            let entity_end = entity_offset.saturating_add(entity_length);

            if entity_end <= current_start || entity_offset >= chunk_end {
                continue;
            }

            let new_offset = entity_offset.saturating_sub(current_start);
            let overlap_start = entity_offset.max(current_start);
            let overlap_end = entity_end.min(chunk_end);
            let new_length = overlap_end.saturating_sub(overlap_start);

            if new_length > 0 {
                chunk_entities.push(MessageEntity {
                    r#type: entity.r#type.clone(),
                    offset: i64::try_from(new_offset).unwrap_or(0),
                    length: i64::try_from(new_length).unwrap_or(0),
                    url: entity.url.clone(),
                    language: entity.language.clone(),
                });
            }
        }

        if !chunk_text.trim().is_empty() {
            result.push((chunk_text, chunk_entities));
        }
        current_start = chunk_end;
    }

    result
}

#[must_use]
pub fn process_llm_markdown_sync(
    markdown: &str,
    with_photo: bool,
) -> Vec<(String, Vec<MessageEntity>)> {
    let mut parser = LlmMarkdownParser::new();
    let chunk_result = parser.push_chunk(markdown);
    let end_result = parser.end();

    let mut builder = TelegramEntityBuilder::new();
    for action in chunk_result.actions.into_iter().chain(end_result.actions) {
        builder.push_action(action);
    }

    let (text, entities) = builder.build();
    let limit = if with_photo { 1024 } else { 4096 };

    split_message_with_entities(&text, &entities, limit)
}

#[cfg(test)]
mod tests {
    use super::{
        BuildState,
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
        build.close_entity("bold");
        assert_eq!(build.entities, Vec::new());
    }

    #[test]
    fn an_entity_covering_nothing_is_dropped() {
        let mut build = state();
        build.open_entity("bold", None, None);
        build.close_entity("bold");
        assert_eq!(build.entities, Vec::new());
    }

    #[test]
    fn popping_a_newline_clips_the_entities_that_covered_it() {
        let mut build = state();
        build.open_entity("pre", None, None);
        build.push_text("code\n");
        build.close_entity("pre");

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
            r#type: "bold".to_string(),
            offset: 0,
            length: 10,
            url: None,
            language: None,
        };
        let chunks = split_message_with_entities("abcdefghij", &[entity], 4);

        assert_eq!(chunks.len(), 3);
        for (chunk, entities) in &chunks {
            let length = i64::try_from(chunk.encode_utf16().count()).expect("a small length");
            assert_eq!(entities.len(), 1);
            assert_eq!(entities[0].offset, 0);
            assert_eq!(entities[0].length, length);
        }
    }
}
