#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Text(String),
    CodeBlockStart(String),
    CodeBlockEnd,
    InlineCode(String),
    DisplayMathStart { delimiter: String },
    DisplayMathEnd { delimiter: String },
    InlineMath { delimiter: String, content: String },
    HeadingStart { level: u8 },
    HeadingEnd,
    BlockquoteStart,
    BlockquoteEnd,
    ListStart { ordered: bool, start: u64 },
    ListEnd,
    ListItemStart { task_status: TaskStatus },
    ListItemEnd,
    TableStart,
    TableEnd,
    TableRowStart,
    TableRowEnd,
    TableCellStart { is_header: bool },
    TableCellEnd,
    ThematicBreak,
    BoldStart,
    BoldEnd,
    ItalicStart,
    ItalicEnd,
    StrikethroughStart,
    StrikethroughEnd,
    SpoilerStart,
    SpoilerEnd,
    UnderlineStart,
    UnderlineEnd,
    SuperscriptStart,
    SuperscriptEnd,
    SubscriptStart,
    SubscriptEnd,
    LinkStart { url: String },
    LinkEnd,
    ImageStart { url: String },
    ImageEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    None,
    Todo,
    Done,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Emit(Event),
    Rollback(usize),
}

#[derive(Debug, PartialEq, Eq)]
pub struct ChunkResult {
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    NormalText,
    CheckingBackticks {
        count: u8,
        is_line_start: bool,
    },
    ReadingCodeInfo {
        opening_count: u8,
    },
    InsideCodeBlock {
        opening_count: u8,
    },
    CheckingBlockEnd {
        opening_count: u8,
        current_count: u8,
    },
    CheckingDollar {
        char_before: char,
    },
    CheckingSlash,
    VerifyInlineMathDollarEnd,
    InsideDisplayMathDollar,
    CheckingDisplayMathDollarEnd,
    InsideDisplayMathBracket,
    CheckingDisplayMathBracketEnd,
    CheckingHeading {
        count: u8,
        is_line_start: bool,
    },
    InsideHeading,
    CheckingStar {
        count: u8,
        char_before: char,
        marker: char,
    },
    CheckingTilde {
        count: u8,
        char_before: char,
    },
    CheckingPipe {
        count: u8,
        char_before: char,
    },
    ReadingHtmlTag,
    CheckingBang,
    CheckingLinkUrl {
        kind: SpeculationKind,
        spec_idx: usize,
    },
    ReadingLinkUrl {
        kind: SpeculationKind,
        spec_idx: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    Blockquote,
    List { ordered: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskBox {
    Bracket,
    Marker,
    Close,
    Space,
}

#[derive(Debug, PartialEq, Eq)]
enum PrefixState {
    Scan,
    CheckDash,
    ReadDigits,
    CheckDot,
    StrictScan {
        quotes_stripped: u8,
        space_allowed: bool,
        indent_stripped: u8,
    },
    CheckingThematicBreak {
        marker: char,
        count: u8,
    },
    CheckingTaskBox {
        step: TaskBox,
        status: TaskStatus,
    },
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpeculationKind {
    MathDollar,
    MathParenthesis,
    Code(u8),
    Bold,
    Italic,
    BoldItalic,
    Strikethrough,
    Spoiler,
    Underline,
    Superscript,
    Subscript,
    LinkLabel,
    ImageLabel,
}

const MAX_HTML_TAG_LEN: usize = 10;

const SPACES_PER_LIST_LEVEL: u8 = 2;
const TAB_WIDTH: u8 = 4;
const MIN_THEMATIC_BREAK_MARKERS: u8 = 3;
const MIN_FENCE_BACKTICKS: u8 = 3;
const MAX_EMPHASIS_MARKERS: u8 = 3;
const MAX_HEADING_LEVEL: u8 = 6;

const INLINE_RECURSION_LIMIT: u16 = 32;

const INLINE_REPARSE_BUDGET_PER_CHAR: usize = 8;

const fn is_verbatim(state: &State) -> bool {
    matches!(
        state,
        State::InsideCodeBlock { .. }
            | State::CheckingBlockEnd { .. }
            | State::InsideDisplayMathDollar
            | State::CheckingDisplayMathDollarEnd
            | State::InsideDisplayMathBracket
            | State::CheckingDisplayMathBracketEnd
    )
}

const fn can_close(previous: char) -> bool {
    !previous.is_whitespace()
}

const fn can_open(next: char) -> bool {
    !next.is_whitespace()
}

#[derive(Debug, Clone)]
struct Speculation {
    kind: SpeculationKind,
    start_event_index: usize,
    raw_content: String,
}

#[expect(clippy::struct_excessive_bools)]
#[derive(Debug)]
pub struct LlmMarkdownParser {
    state: State,
    buffer: String,
    at_line_start: bool,
    last_char: char,
    open_containers: Vec<Container>,
    line_containers: Vec<Container>,
    prefix_state: PrefixState,
    prefix_buffer: String,
    current_indent: u8,
    explicit_list_marker: bool,
    found_thematic_break: bool,
    inline_only: bool,
    current_task_status: TaskStatus,
    current_list_start: u64,
    global_event_counter: usize,
    speculations: Vec<Speculation>,
    current_line_raw: String,
    last_line_raw: String,
    current_line_event_index: usize,
    last_line_event_index: usize,
    in_table: bool,
    skip_math_newline: bool,
    pending_carriage_return: bool,
    html_tag: String,
    link_url: String,
    block_floor: usize,
    depth: u16,
    reparse_budget: usize,
}

impl Default for LlmMarkdownParser {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmMarkdownParser {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: State::NormalText,
            buffer: String::new(),
            at_line_start: true,
            last_char: '\n',
            open_containers: Vec::new(),
            line_containers: Vec::new(),
            prefix_state: PrefixState::Scan,
            prefix_buffer: String::new(),
            current_indent: 0,
            explicit_list_marker: false,
            found_thematic_break: false,
            inline_only: false,
            current_task_status: TaskStatus::None,
            current_list_start: 1,
            global_event_counter: 0,
            speculations: Vec::new(),
            current_line_raw: String::new(),
            last_line_raw: String::new(),
            current_line_event_index: 0,
            last_line_event_index: 0,
            in_table: false,
            skip_math_newline: false,
            pending_carriage_return: false,
            html_tag: String::new(),
            link_url: String::new(),
            block_floor: 0,
            depth: INLINE_RECURSION_LIMIT,
            reparse_budget: 0,
        }
    }

    const fn is_block_event(event: &Event) -> bool {
        matches!(
            event,
            Event::CodeBlockStart(_)
                | Event::CodeBlockEnd
                | Event::DisplayMathStart { .. }
                | Event::DisplayMathEnd { .. }
                | Event::HeadingStart { .. }
                | Event::HeadingEnd
                | Event::BlockquoteStart
                | Event::BlockquoteEnd
                | Event::ListStart { .. }
                | Event::ListEnd
                | Event::ListItemStart { .. }
                | Event::ListItemEnd
                | Event::TableStart
                | Event::TableEnd
                | Event::TableRowStart
                | Event::TableRowEnd
                | Event::TableCellStart { .. }
                | Event::TableCellEnd
                | Event::ThematicBreak
        )
    }

    fn list_indent(&self) -> u8 {
        let levels = self
            .open_containers
            .iter()
            .filter(|&&container| matches!(container, Container::List { .. }))
            .count();
        u8::try_from(levels.saturating_mul(usize::from(SPACES_PER_LIST_LEVEL))).unwrap_or(u8::MAX)
    }

    fn inherit_indented_parents(&mut self) {
        let base = self.line_containers.len();
        let levels = usize::from(self.current_indent / SPACES_PER_LIST_LEVEL);
        let keep = base.saturating_add(levels).min(self.open_containers.len());
        for index in base..keep {
            if let Some(&container) = self.open_containers.get(index) {
                self.line_containers.push(container);
            }
        }
    }

    const fn is_thematic_marker(marker: char) -> bool {
        matches!(marker, '-' | '_' | '*')
    }

    const fn is_table_event(event: &Event) -> bool {
        matches!(
            event,
            Event::TableStart
                | Event::TableEnd
                | Event::TableRowStart
                | Event::TableRowEnd
                | Event::TableCellStart { .. }
                | Event::TableCellEnd
        )
    }

    fn emit(&mut self, event: Event, out: &mut Vec<Action>) {
        self.global_event_counter = self.global_event_counter.saturating_add(1);
        out.push(Action::Emit(event));
    }

    fn push_event(&mut self, event: Event, out: &mut Vec<Action>) {
        if Self::is_block_event(&event) {
            self.speculations.clear();

            if self.in_table && !Self::is_table_event(&event) {
                self.in_table = false;
                self.emit(Event::TableEnd, out);
                self.current_line_event_index = self.global_event_counter;
                self.block_floor = self.global_event_counter;
            }
            self.emit(event, out);

            self.block_floor = self.global_event_counter;
            return;
        }
        self.emit(event, out);
    }

    fn close_containers_from(&mut self, floor: usize, out: &mut Vec<Action>) {
        while self.open_containers.len() > floor {
            match self.open_containers.pop() {
                Some(Container::Blockquote) => self.push_event(Event::BlockquoteEnd, out),
                Some(Container::List { .. }) => {
                    self.push_event(Event::ListItemEnd, out);
                    self.push_event(Event::ListEnd, out);
                }
                None => break,
            }
        }
    }

    fn rollback_to(&mut self, index: usize, out: &mut Vec<Action>) {
        let count = self.global_event_counter.saturating_sub(index);
        if count > 0 {
            self.push_rollback(count, out);
        }
    }

    fn push_rollback(&mut self, count: usize, out: &mut Vec<Action>) {
        self.global_event_counter = self.global_event_counter.saturating_sub(count);

        self.current_line_event_index =
            self.current_line_event_index.min(self.global_event_counter);
        self.last_line_event_index = self.last_line_event_index.min(self.global_event_counter);
        self.block_floor = self.block_floor.min(self.global_event_counter);
        out.push(Action::Rollback(count));
    }

    fn flush_text(&mut self, out: &mut Vec<Action>) {
        if !self.buffer.is_empty() {
            let t = std::mem::take(&mut self.buffer);
            for spec in &mut self.speculations {
                spec.raw_content.push_str(&t);
            }
            self.push_event(Event::Text(t), out);
        }
    }

    fn start_speculation(&mut self, kind: SpeculationKind) {
        self.speculations.push(Speculation {
            kind,
            start_event_index: self.global_event_counter,
            raw_content: String::new(),
        });
    }

    fn has_speculation(&self, kind: SpeculationKind) -> bool {
        self.speculations.iter().any(|s| s.kind == kind)
    }

    fn abort_speculation(&mut self, spec_idx: usize) {
        if let Some(pos) = self
            .speculations
            .iter()
            .position(|s| s.start_event_index == spec_idx)
        {
            self.speculations.remove(pos);
        }
    }

    fn resolve_link(
        &mut self,
        spec_idx: usize,
        url: &str,
        kind: SpeculationKind,
        out: &mut Vec<Action>,
    ) {
        self.flush_text(out);
        if let Some(pos) = self
            .speculations
            .iter()
            .position(|s| s.start_event_index == spec_idx)
        {
            let spec = self.speculations.remove(pos);
            self.speculations.truncate(pos);

            self.rollback_to(spec.start_event_index, out);

            let content = spec.raw_content;

            let prefix = match kind {
                SpeculationKind::LinkLabel => "[",
                SpeculationKind::ImageLabel => "![",
                _ => "",
            };
            let suffix = format!("]({url})");

            let c1 = content.strip_prefix(prefix).unwrap_or(&content);
            let c2 = c1.strip_suffix(&suffix).unwrap_or(c1);
            let label = c2.to_string();

            match kind {
                SpeculationKind::LinkLabel => {
                    self.push_event(
                        Event::LinkStart {
                            url: url.to_string(),
                        },
                        out,
                    );
                    let inner_actions = self.parse_inline(&label);
                    for ev in inner_actions {
                        self.push_event(ev, out);
                    }
                    self.push_event(Event::LinkEnd, out);
                }
                SpeculationKind::ImageLabel => {
                    self.push_event(
                        Event::ImageStart {
                            url: url.to_string(),
                        },
                        out,
                    );
                    let inner_actions = self.parse_inline(&label);
                    for ev in inner_actions {
                        self.push_event(ev, out);
                    }
                    self.push_event(Event::ImageEnd, out);
                }
                _ => {}
            }
        }
    }

    fn resolve_speculation(
        &mut self,
        kind: SpeculationKind,
        start_delim: &str,
        end_delim: &str,
        out: &mut Vec<Action>,
    ) {
        self.flush_text(out);
        if let Some(idx) = self.speculations.iter().rposition(|s| s.kind == kind) {
            let spec = self.speculations.remove(idx);
            self.speculations.truncate(idx);

            self.rollback_to(spec.start_event_index, out);

            let content = spec.raw_content;

            let c1 = content.strip_prefix(start_delim).unwrap_or(&content);
            let c2 = c1.strip_suffix(end_delim).unwrap_or(c1);
            let content_str = c2.to_string();

            match kind {
                SpeculationKind::MathDollar => {
                    self.push_event(
                        Event::InlineMath {
                            delimiter: "$".into(),
                            content: content_str,
                        },
                        out,
                    );
                }
                SpeculationKind::MathParenthesis => {
                    self.push_event(
                        Event::InlineMath {
                            delimiter: "\\(".into(),
                            content: content_str,
                        },
                        out,
                    );
                }
                SpeculationKind::Code(_) => {
                    self.push_event(Event::InlineCode(content_str), out);
                }
                SpeculationKind::BoldItalic => {
                    self.push_event(Event::BoldStart, out);
                    self.push_event(Event::ItalicStart, out);
                    let inner_actions = self.parse_inline(&content_str);
                    for ev in inner_actions {
                        self.push_event(ev, out);
                    }
                    self.push_event(Event::ItalicEnd, out);
                    self.push_event(Event::BoldEnd, out);
                }
                _ => {
                    let (start_ev, end_ev) = match kind {
                        SpeculationKind::Bold => (Event::BoldStart, Event::BoldEnd),
                        SpeculationKind::Italic => (Event::ItalicStart, Event::ItalicEnd),
                        SpeculationKind::Strikethrough =>
                            (Event::StrikethroughStart, Event::StrikethroughEnd),
                        SpeculationKind::Spoiler => (Event::SpoilerStart, Event::SpoilerEnd),
                        SpeculationKind::Underline => (Event::UnderlineStart, Event::UnderlineEnd),
                        SpeculationKind::Superscript =>
                            (Event::SuperscriptStart, Event::SuperscriptEnd),
                        SpeculationKind::Subscript => (Event::SubscriptStart, Event::SubscriptEnd),
                        _ => unreachable!(),
                    };

                    self.push_event(start_ev, out);
                    let inner_actions = self.parse_inline(&content_str);
                    for ev in inner_actions {
                        self.push_event(ev, out);
                    }
                    self.push_event(end_ev, out);
                }
            }
        }
    }

    fn normalize_newlines(&mut self, chunk: &str) -> String {
        let mut normalized = String::with_capacity(chunk.len());
        for character in chunk.chars() {
            if character == '\r' {
                self.pending_carriage_return = true;
                normalized.push('\n');
            } else if character == '\n' && self.pending_carriage_return {
                self.pending_carriage_return = false;
            } else {
                self.pending_carriage_return = false;
                normalized.push(character);
            }
        }
        normalized
    }

    fn is_delimiter_row(s: &str) -> bool {
        let s = s.trim();
        if s.is_empty() || !s.contains('|') || !s.contains('-') {
            return false;
        }
        s.chars()
            .all(|c| c == '|' || c == '-' || c == ':' || c.is_whitespace())
    }

    fn is_table_row(s: &str) -> bool {
        let s = s.trim();
        !s.is_empty() && s.contains('|')
    }

    fn parse_inline(&mut self, text: &str) -> Vec<Event> {
        if text.is_empty() {
            return Vec::new();
        }
        if self.depth == 0 || self.reparse_budget < text.len() {
            return vec![Event::Text(text.to_string())];
        }
        self.reparse_budget = self.reparse_budget.saturating_sub(text.len());

        let mut inner = Self::new();
        inner.prefix_state = PrefixState::Done;
        inner.inline_only = true;
        inner.depth = self.depth.saturating_sub(1);
        inner.reparse_budget = self.reparse_budget;

        let mut actions = inner.push_chunk(text).actions;
        actions.extend(inner.end().actions);

        self.reparse_budget = inner.reparse_budget;

        let mut events = Vec::new();
        for action in actions {
            match action {
                Action::Emit(event) => events.push(event),
                Action::Rollback(count) => {
                    let new_len = events.len().saturating_sub(count);
                    events.truncate(new_len);
                }
            }
        }
        events
    }

    fn split_row_cells(row: &str) -> Vec<String> {
        let mut cells = Vec::new();
        let mut cell = String::new();
        let mut escaped = false;

        for character in row.chars() {
            if escaped {
                if character != '|' {
                    cell.push('\\');
                }
                cell.push(character);
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '|' {
                cells.push(std::mem::take(&mut cell));
            } else {
                cell.push(character);
            }
        }
        if escaped {
            cell.push('\\');
        }
        cells.push(cell);

        if cells.first().is_some_and(|first| first.trim().is_empty()) && cells.len() > 1 {
            cells.remove(0);
        }
        if cells.last().is_some_and(|last| last.trim().is_empty()) && cells.len() > 1 {
            cells.pop();
        }
        cells
    }

    fn emit_parsed_table_row(&mut self, row: &str, is_header: bool, out: &mut Vec<Action>) {
        self.push_event(Event::TableRowStart, out);
        for cell in Self::split_row_cells(row.trim()) {
            self.push_event(Event::TableCellStart { is_header }, out);
            let cell_events = self.parse_inline(&cell);
            for ev in cell_events {
                self.push_event(ev, out);
            }
            self.push_event(Event::TableCellEnd, out);
        }
        self.push_event(Event::TableRowEnd, out);
    }

    pub fn push_chunk(&mut self, chunk: &str) -> ChunkResult {
        let mut actions = Vec::new();

        if !self.inline_only {
            self.reparse_budget = self
                .reparse_budget
                .saturating_add(chunk.len().saturating_mul(INLINE_REPARSE_BUDGET_PER_CHAR));
        }

        let chunk = self.normalize_newlines(chunk);

        for c in chunk.chars() {
            let mut process_as_text = false;

            let in_strict_block = is_verbatim(&self.state);

            if self.prefix_state == PrefixState::Done || self.inline_only {
                process_as_text = !self.found_thematic_break;
            } else {
                match self.prefix_state {
                    PrefixState::StrictScan {
                        quotes_stripped,
                        space_allowed,
                        indent_stripped,
                    } => {
                        let expected = u8::try_from(
                            self.open_containers
                                .iter()
                                .filter(|&&con| con == Container::Blockquote)
                                .count(),
                        )
                        .unwrap_or(u8::MAX);

                        if c == '>' && quotes_stripped < expected {
                            self.prefix_state = PrefixState::StrictScan {
                                quotes_stripped: quotes_stripped.saturating_add(1),
                                space_allowed: true,
                                indent_stripped,
                            };
                            continue;
                        } else if (c == ' ' || c == '\t') && space_allowed {
                            self.prefix_state = PrefixState::StrictScan {
                                quotes_stripped,
                                space_allowed: false,
                                indent_stripped,
                            };
                            continue;
                        } else if (c == ' ' || c == '\t') && quotes_stripped < expected {
                            continue;
                        } else if (c == ' ' || c == '\t') && indent_stripped < self.list_indent() {
                            let width = if c == '\t' { 4 } else { 1 };
                            self.prefix_state = PrefixState::StrictScan {
                                quotes_stripped,
                                space_allowed,
                                indent_stripped: indent_stripped.saturating_add(width),
                            };
                            continue;
                        }
                        self.prefix_state = PrefixState::Done;
                    }
                    PrefixState::Scan => {
                        if c == ' ' {
                            self.current_indent = self.current_indent.saturating_add(1);
                            continue;
                        } else if c == '\t' {
                            self.current_indent = self.current_indent.saturating_add(TAB_WIDTH);
                            continue;
                        } else if c == '>' {
                            self.inherit_indented_parents();
                            self.line_containers.push(Container::Blockquote);
                            self.current_indent = 0;
                            continue;
                        } else if c == '*' || c == '_' {
                            self.prefix_buffer.push(c);
                            self.prefix_state = PrefixState::CheckingThematicBreak {
                                marker: c,
                                count: 1,
                            };
                            continue;
                        } else if c == '-' || c == '+' {
                            self.prefix_buffer.push(c);
                            self.prefix_state = PrefixState::CheckDash;
                            continue;
                        } else if c.is_ascii_digit() {
                            self.prefix_buffer.push(c);
                            self.prefix_state = PrefixState::ReadDigits;
                            continue;
                        }
                        self.prefix_state = PrefixState::Done;
                    }
                    PrefixState::CheckDash => {
                        if c == ' ' || c == '\t' {
                            self.prefix_buffer.push(c);
                            self.prefix_state = PrefixState::CheckingThematicBreak {
                                marker: self.prefix_buffer.chars().next().unwrap_or('-'),
                                count: 1,
                            };
                            continue;
                        } else if c == self.prefix_buffer.chars().next().unwrap_or('-') {
                            self.prefix_buffer.push(c);
                            self.prefix_state = PrefixState::CheckingThematicBreak {
                                marker: c,
                                count: 2,
                            };
                            continue;
                        }
                        self.prefix_state = PrefixState::Done;
                    }
                    PrefixState::CheckingThematicBreak { marker, count } =>
                        if c == marker {
                            self.prefix_buffer.push(c);
                            self.prefix_state = PrefixState::CheckingThematicBreak {
                                marker,
                                count: count.saturating_add(1),
                            };
                            continue;
                        } else if c == ' ' || c == '\t' {
                            self.prefix_buffer.push(c);
                            continue;
                        } else if c == '\n' {
                            if count >= MIN_THEMATIC_BREAK_MARKERS
                                && Self::is_thematic_marker(marker)
                            {
                                self.found_thematic_break = true;
                                self.prefix_buffer.clear();
                            }
                            self.prefix_state = PrefixState::Done;
                        } else if count == 1
                            && (marker == '-' || marker == '*' || marker == '+')
                            && self.prefix_buffer.ends_with(|ch: char| ch.is_whitespace())
                        {
                            self.inherit_indented_parents();
                            self.line_containers
                                .push(Container::List { ordered: false });
                            self.explicit_list_marker = true;
                            self.prefix_buffer.clear();
                            self.current_indent = 0;

                            if c == '[' {
                                self.prefix_buffer.push(c);
                                self.prefix_state = PrefixState::CheckingTaskBox {
                                    step: TaskBox::Marker,
                                    status: TaskStatus::None,
                                };
                                continue;
                            }

                            self.prefix_state = PrefixState::Done;
                        } else {
                            self.prefix_state = PrefixState::Done;
                        },
                    PrefixState::ReadDigits => {
                        if c.is_ascii_digit() {
                            self.prefix_buffer.push(c);
                            continue;
                        } else if c == '.' || c == ')' {
                            self.prefix_buffer.push(c);
                            self.prefix_state = PrefixState::CheckDot;
                            continue;
                        }
                        self.prefix_state = PrefixState::Done;
                    }
                    PrefixState::CheckDot => {
                        if c == ' ' || c == '\t' {
                            self.inherit_indented_parents();
                            self.line_containers.push(Container::List { ordered: true });
                            self.explicit_list_marker = true;

                            self.current_list_start = self
                                .prefix_buffer
                                .trim_end_matches(['.', ')'])
                                .parse()
                                .unwrap_or(1);
                            self.prefix_buffer.clear();
                            self.current_indent = 0;
                            self.prefix_state = PrefixState::CheckingTaskBox {
                                step: TaskBox::Bracket,
                                status: TaskStatus::None,
                            };
                            continue;
                        }
                        self.prefix_state = PrefixState::Done;
                    }
                    PrefixState::CheckingTaskBox { step, status } => match step {
                        TaskBox::Bracket => {
                            if c == '[' {
                                self.prefix_buffer.push(c);
                                self.prefix_state = PrefixState::CheckingTaskBox {
                                    step: TaskBox::Marker,
                                    status,
                                };
                                continue;
                            }
                            self.prefix_state = PrefixState::Done;
                        }
                        TaskBox::Marker => {
                            if c == ' ' {
                                self.prefix_buffer.push(c);
                                self.prefix_state = PrefixState::CheckingTaskBox {
                                    step: TaskBox::Close,
                                    status: TaskStatus::Todo,
                                };
                                continue;
                            } else if c == 'x' || c == 'X' {
                                self.prefix_buffer.push(c);
                                self.prefix_state = PrefixState::CheckingTaskBox {
                                    step: TaskBox::Close,
                                    status: TaskStatus::Done,
                                };
                                continue;
                            }
                            self.prefix_state = PrefixState::Done;
                        }
                        TaskBox::Close => {
                            if c == ']' {
                                self.prefix_buffer.push(c);
                                self.prefix_state = PrefixState::CheckingTaskBox {
                                    step: TaskBox::Space,
                                    status,
                                };
                                continue;
                            }
                            self.prefix_state = PrefixState::Done;
                        }
                        TaskBox::Space => {
                            if c == ' ' || c == '\t' {
                                self.current_task_status = status;
                                self.prefix_buffer.clear();
                                self.prefix_state = PrefixState::Scan;
                                continue;
                            }
                            self.prefix_state = PrefixState::Done;
                        }
                    },
                    PrefixState::Done => unreachable!(),
                }

                if self.prefix_state == PrefixState::Done {
                    if in_strict_block {
                        self.line_containers = self.open_containers.clone();
                    } else if self.line_containers.is_empty() && !self.explicit_list_marker {
                        let is_lazy = !self.buffer.is_empty() && c != '\n';
                        if is_lazy || self.current_indent >= 2 {
                            self.line_containers = self.open_containers.clone();
                        } else if c == '\n' && self.prefix_buffer.is_empty() {
                            let keep = self
                                .open_containers
                                .iter()
                                .position(|container| *container == Container::Blockquote)
                                .unwrap_or(self.open_containers.len());
                            self.line_containers =
                                self.open_containers.get(..keep).unwrap_or(&[]).to_vec();
                        }
                    }

                    let common = self
                        .open_containers
                        .iter()
                        .zip(&self.line_containers)
                        .take_while(|(a, b)| a == b)
                        .count();

                    if common < self.open_containers.len()
                        || self.explicit_list_marker
                        || self.found_thematic_break
                    {
                        self.flush_text(&mut actions);

                        self.close_containers_from(common, &mut actions);
                    }

                    let to_open = self.line_containers.get(common..).unwrap_or(&[]).to_vec();
                    let innermost = to_open.len().saturating_sub(1);
                    for (index, container) in to_open.into_iter().enumerate() {
                        match container {
                            Container::Blockquote => {
                                self.push_event(Event::BlockquoteStart, &mut actions);
                            }
                            Container::List { ordered } => {
                                let start = if index == innermost {
                                    self.current_list_start
                                } else {
                                    1
                                };
                                self.push_event(Event::ListStart { ordered, start }, &mut actions);
                                self.push_event(
                                    Event::ListItemStart {
                                        task_status: self.current_task_status,
                                    },
                                    &mut actions,
                                );
                            }
                        }
                    }

                    if self.explicit_list_marker
                        && common > 0
                        && common == self.line_containers.len()
                        && matches!(
                            self.open_containers.get(common.saturating_sub(1)),
                            Some(Container::List { .. })
                        )
                    {
                        self.push_event(Event::ListItemEnd, &mut actions);
                        self.push_event(
                            Event::ListItemStart {
                                task_status: self.current_task_status,
                            },
                            &mut actions,
                        );
                    }

                    self.open_containers = self.line_containers.clone();

                    self.current_line_event_index = self.global_event_counter;

                    if self.found_thematic_break {
                        self.push_event(Event::ThematicBreak, &mut actions);
                    } else {
                        let failed = std::mem::take(&mut self.prefix_buffer);
                        for fc in failed.chars() {
                            self.current_line_raw.push(fc);
                            self.push_char(fc, &mut actions);
                        }
                        process_as_text = true;
                    }
                }
            }
            if process_as_text {
                self.push_char(c, &mut actions);
            }

            self.last_char = c;
            self.current_line_raw.push(c);

            if c == '\n' {
                self.flush_text(&mut actions);

                if !in_strict_block && self.current_line_raw.trim().is_empty() {
                    self.speculations.clear();
                }

                let next_in_strict = is_verbatim(&self.state);

                if self.inline_only {
                    self.prefix_state = PrefixState::Done;
                    self.at_line_start = false;
                } else {
                    if !next_in_strict
                        && self.prefix_state == PrefixState::Done
                        && !self.found_thematic_break
                    {
                        if !self.in_table
                            && Self::is_delimiter_row(&self.current_line_raw)
                            && Self::is_table_row(&self.last_line_raw)
                            && self.state == State::NormalText
                            && self.last_line_event_index >= self.block_floor
                        {
                            self.rollback_to(self.last_line_event_index, &mut actions);

                            self.push_event(Event::TableStart, &mut actions);
                            let row = self.last_line_raw.clone();
                            self.emit_parsed_table_row(&row, true, &mut actions);
                            self.in_table = true;
                        } else if self.in_table {
                            if Self::is_table_row(&self.current_line_raw) {
                                self.rollback_to(self.current_line_event_index, &mut actions);
                                let row = self.current_line_raw.clone();
                                self.emit_parsed_table_row(&row, false, &mut actions);
                            } else {
                                self.in_table = false;
                                self.rollback_to(self.current_line_event_index, &mut actions);
                                self.push_event(Event::TableEnd, &mut actions);

                                let raw_line = self.current_line_raw.clone();
                                let text_events = self.parse_inline(&raw_line);
                                for ev in text_events {
                                    self.push_event(ev, &mut actions);
                                }
                            }
                        }
                    } else if self.in_table && !next_in_strict {
                        self.in_table = false;
                        self.push_event(Event::TableEnd, &mut actions);
                    }

                    if next_in_strict {
                        self.prefix_state = PrefixState::StrictScan {
                            quotes_stripped: 0,
                            space_allowed: false,
                            indent_stripped: 0,
                        };
                    } else {
                        self.prefix_state = PrefixState::Scan;
                    }
                    self.line_containers.clear();
                    self.current_indent = 0;
                    self.explicit_list_marker = false;
                    self.found_thematic_break = false;
                    self.current_task_status = TaskStatus::None;
                    self.current_list_start = 1;
                    self.prefix_buffer.clear();

                    self.last_line_raw = std::mem::take(&mut self.current_line_raw);
                    self.last_line_event_index = self.current_line_event_index;
                    self.current_line_event_index = self.global_event_counter;
                    self.at_line_start = true;
                }
            } else if !c.is_whitespace() && self.prefix_state == PrefixState::Done {
                self.at_line_start = false;
            }
        }

        if !matches!(self.state, State::ReadingCodeInfo { .. }) {
            self.flush_text(&mut actions);
        }
        ChunkResult { actions }
    }

    pub fn end(&mut self) -> ChunkResult {
        let mut actions = Vec::new();

        if matches!(
            self.state,
            State::CheckingStar { .. }
                | State::CheckingTilde { .. }
                | State::CheckingPipe { .. }
                | State::CheckingDollar { .. }
                | State::VerifyInlineMathDollarEnd
                | State::CheckingSlash
                | State::CheckingBang
                | State::ReadingHtmlTag
                | State::CheckingLinkUrl { .. }
                | State::ReadingLinkUrl { .. }
                | State::CheckingHeading { .. }
        ) || matches!(
            self.state,
            State::CheckingBackticks { count, is_line_start }
                if !(is_line_start && count >= MIN_FENCE_BACKTICKS)
        ) {
            self.push_char('\n', &mut actions);
            if self.buffer.ends_with('\n') {
                let _ = self.buffer.pop();
            }
        }

        if let PrefixState::CheckingThematicBreak { marker, count } = self.prefix_state
            && count >= MIN_THEMATIC_BREAK_MARKERS
            && Self::is_thematic_marker(marker)
        {
            self.found_thematic_break = true;
            self.prefix_buffer.clear();
            self.prefix_state = PrefixState::Done;

            let common = self.open_containers.len().saturating_sub(1);
            self.flush_text(&mut actions);
            self.close_containers_from(common, &mut actions);
            self.push_event(Event::ThematicBreak, &mut actions);
        }

        if matches!(self.state, State::ReadingCodeInfo { .. }) {
            let language = std::mem::take(&mut self.buffer).trim().to_string();
            self.push_event(Event::CodeBlockStart(language), &mut actions);
            self.push_event(Event::CodeBlockEnd, &mut actions);
            self.state = State::NormalText;
        }

        if let State::CheckingBlockEnd {
            opening_count,
            current_count,
        } = self.state
            && current_count < opening_count
        {
            self.buffer
                .push_str(&"`".repeat(usize::from(current_count)));
        }

        self.flush_text(&mut actions);

        if self.in_table {
            if self.state == State::NormalText && Self::is_table_row(&self.current_line_raw) {
                self.rollback_to(self.current_line_event_index, &mut actions);
                let row = self.current_line_raw.clone();
                self.emit_parsed_table_row(&row, false, &mut actions);
            }
            self.in_table = false;
            self.push_event(Event::TableEnd, &mut actions);
        }

        match self.state {
            State::CheckingBackticks {
                count,
                is_line_start,
            } if is_line_start && count >= MIN_FENCE_BACKTICKS => {
                self.push_event(Event::CodeBlockStart(String::new()), &mut actions);
                self.push_event(Event::CodeBlockEnd, &mut actions);
            }
            State::InsideCodeBlock { .. } | State::CheckingBlockEnd { .. } => {
                self.push_event(Event::CodeBlockEnd, &mut actions);
            }
            State::InsideDisplayMathDollar | State::CheckingDisplayMathDollarEnd => {
                self.push_event(
                    Event::DisplayMathEnd {
                        delimiter: "$$".to_string(),
                    },
                    &mut actions,
                );
            }
            State::InsideDisplayMathBracket | State::CheckingDisplayMathBracketEnd => {
                self.push_event(
                    Event::DisplayMathEnd {
                        delimiter: "\\]".to_string(),
                    },
                    &mut actions,
                );
            }
            State::InsideHeading => {
                self.push_event(Event::HeadingEnd, &mut actions);
            }
            _ => {}
        }

        self.close_containers_from(0, &mut actions);
        self.rewind();

        ChunkResult { actions }
    }

    fn rewind(&mut self) {
        self.state = State::NormalText;
        self.prefix_state = PrefixState::Scan;
        self.prefix_buffer.clear();
        self.line_containers.clear();
        self.speculations.clear();
        self.buffer.clear();
        self.current_line_raw.clear();
        self.last_line_raw.clear();
        self.at_line_start = true;
        self.last_char = '\n';
        self.current_indent = 0;
        self.explicit_list_marker = false;
        self.found_thematic_break = false;
        self.current_task_status = TaskStatus::None;
        self.current_list_start = 1;
        self.in_table = false;
        self.skip_math_newline = false;
        self.pending_carriage_return = false;
        self.html_tag.clear();
        self.link_url.clear();
    }

    fn push_char(&mut self, c: char, out: &mut Vec<Action>) {
        let mut reprocess = true;

        while reprocess {
            reprocess = false;
            let current_state = self.state;

            match current_state {
                State::NormalText =>
                    if c == '`' {
                        self.flush_text(out);
                        self.state = State::CheckingBackticks {
                            count: 1,
                            is_line_start: self.at_line_start,
                        };
                    } else if c == '#' && !self.inline_only {
                        self.flush_text(out);
                        self.state = State::CheckingHeading {
                            count: 1,
                            is_line_start: self.at_line_start,
                        };
                    } else if c == '$' {
                        self.flush_text(out);
                        self.state = State::CheckingDollar {
                            char_before: self.last_char,
                        };
                    } else if c == '\\' {
                        self.flush_text(out);
                        self.state = State::CheckingSlash;
                    } else if c == '*' || c == '_' {
                        self.flush_text(out);
                        self.state = State::CheckingStar {
                            count: 1,
                            char_before: self.last_char,
                            marker: c,
                        };
                    } else if c == '~' {
                        self.flush_text(out);
                        self.state = State::CheckingTilde {
                            count: 1,
                            char_before: self.last_char,
                        };
                    } else if c == '|' {
                        self.flush_text(out);
                        self.state = State::CheckingPipe {
                            count: 1,
                            char_before: self.last_char,
                        };
                    } else if c == '<' {
                        self.flush_text(out);
                        self.html_tag.clear();
                        self.html_tag.push('<');
                        self.state = State::ReadingHtmlTag;
                    } else if c == '!' {
                        self.flush_text(out);
                        self.state = State::CheckingBang;
                    } else if c == '[' {
                        self.flush_text(out);
                        self.start_speculation(SpeculationKind::LinkLabel);
                        self.buffer.push('[');
                        self.flush_text(out);
                        self.state = State::NormalText;
                    } else if c == ']' {
                        let maybe_spec = self
                            .speculations
                            .iter()
                            .rev()
                            .find(|s| {
                                s.kind == SpeculationKind::LinkLabel
                                    || s.kind == SpeculationKind::ImageLabel
                            })
                            .map(|s| (s.kind, s.start_event_index));

                        self.buffer.push(']');
                        if let Some((kind, spec_idx)) = maybe_spec {
                            self.flush_text(out);
                            self.state = State::CheckingLinkUrl { kind, spec_idx };
                        }
                    } else {
                        self.buffer.push(c);
                    },

                State::CheckingBang =>
                    if c == '[' {
                        self.start_speculation(SpeculationKind::ImageLabel);
                        self.buffer.push_str("![");
                        self.flush_text(out);
                        self.state = State::NormalText;
                    } else {
                        self.buffer.push('!');
                        self.state = State::NormalText;
                        reprocess = true;
                    },

                State::CheckingLinkUrl { kind, spec_idx } =>
                    if c == '(' {
                        self.buffer.push('(');
                        self.flush_text(out);
                        self.link_url.clear();
                        self.state = State::ReadingLinkUrl { kind, spec_idx };
                    } else {
                        self.abort_speculation(spec_idx);
                        self.state = State::NormalText;
                        reprocess = true;
                    },

                State::ReadingLinkUrl { kind, spec_idx } =>
                    if c == ')' {
                        self.buffer.push(')');
                        let url = std::mem::take(&mut self.link_url);
                        self.resolve_link(spec_idx, &url, kind, out);
                        self.state = State::NormalText;
                    } else if c.is_whitespace() {
                        self.abort_speculation(spec_idx);
                        self.state = State::NormalText;
                        reprocess = true;
                    } else {
                        self.link_url.push(c);
                        self.buffer.push(c);
                        self.flush_text(out);
                    },

                State::ReadingHtmlTag => {
                    if c != '>' && (c.is_whitespace() || self.html_tag.len() >= MAX_HTML_TAG_LEN) {
                        let tag = std::mem::take(&mut self.html_tag);
                        self.buffer.push_str(&tag);
                        self.flush_text(out);
                        self.state = State::NormalText;
                        reprocess = true;
                        continue;
                    }

                    self.html_tag.push(c);
                    if c == '>' {
                        let tag = std::mem::take(&mut self.html_tag);
                        if tag == "<u>" {
                            self.start_speculation(SpeculationKind::Underline);
                            self.buffer.push_str(&tag);
                            self.flush_text(out);
                        } else if tag == "</u>" && self.has_speculation(SpeculationKind::Underline)
                        {
                            self.buffer.push_str(&tag);
                            self.resolve_speculation(
                                SpeculationKind::Underline,
                                "<u>",
                                "</u>",
                                out,
                            );
                        } else if tag == "<sup>" {
                            self.start_speculation(SpeculationKind::Superscript);
                            self.buffer.push_str(&tag);
                            self.flush_text(out);
                        } else if tag == "</sup>"
                            && self.has_speculation(SpeculationKind::Superscript)
                        {
                            self.buffer.push_str(&tag);
                            self.resolve_speculation(
                                SpeculationKind::Superscript,
                                "<sup>",
                                "</sup>",
                                out,
                            );
                        } else if tag == "<sub>" {
                            self.start_speculation(SpeculationKind::Subscript);
                            self.buffer.push_str(&tag);
                            self.flush_text(out);
                        } else if tag == "</sub>"
                            && self.has_speculation(SpeculationKind::Subscript)
                        {
                            self.buffer.push_str(&tag);
                            self.resolve_speculation(
                                SpeculationKind::Subscript,
                                "<sub>",
                                "</sub>",
                                out,
                            );
                        } else {
                            self.buffer.push_str(&tag);
                            self.flush_text(out);
                        }
                        self.state = State::NormalText;
                    }
                }

                State::CheckingStar {
                    count,
                    char_before,
                    marker,
                } =>
                    if c == marker {
                        self.state = State::CheckingStar {
                            count: count.saturating_add(1),
                            char_before,
                            marker,
                        };
                    } else {
                        if count > MAX_EMPHASIS_MARKERS {
                            self.buffer
                                .push_str(&marker.to_string().repeat(usize::from(count)));
                            self.flush_text(out);
                            self.state = State::NormalText;
                            reprocess = true;
                            continue;
                        }

                        let mut right_flanking = can_close(char_before);
                        let mut left_flanking = can_open(c);

                        if marker == '_' {
                            if char_before.is_alphanumeric() {
                                left_flanking = false;
                            }
                            if c.is_alphanumeric() {
                                right_flanking = false;
                            }
                        }

                        let kind = match count {
                            1 => SpeculationKind::Italic,
                            2 => SpeculationKind::Bold,
                            _ => SpeculationKind::BoldItalic,
                        };
                        let delim = marker.to_string().repeat(usize::from(count));

                        if right_flanking && self.has_speculation(kind) {
                            self.buffer.push_str(&delim);
                            self.resolve_speculation(kind, &delim, &delim, out);
                        } else if left_flanking {
                            self.start_speculation(kind);
                            self.buffer.push_str(&delim);
                            self.flush_text(out);
                        } else {
                            self.buffer.push_str(&delim);
                            self.flush_text(out);
                        }

                        self.state = State::NormalText;
                        reprocess = true;
                    },

                State::CheckingTilde { count, char_before } =>
                    if c == '~' {
                        self.state = State::CheckingTilde {
                            count: count.saturating_add(1),
                            char_before,
                        };
                    } else {
                        if count == 2 {
                            let right_flanking = can_close(char_before);
                            let left_flanking = can_open(c);
                            if right_flanking
                                && self.has_speculation(SpeculationKind::Strikethrough)
                            {
                                self.buffer.push_str("~~");
                                self.resolve_speculation(
                                    SpeculationKind::Strikethrough,
                                    "~~",
                                    "~~",
                                    out,
                                );
                            } else if left_flanking {
                                self.start_speculation(SpeculationKind::Strikethrough);
                                self.buffer.push_str("~~");
                                self.flush_text(out);
                            } else {
                                self.buffer.push_str("~~");
                                self.flush_text(out);
                            }
                        } else {
                            self.buffer.push_str(&"~".repeat(usize::from(count)));
                            self.flush_text(out);
                        }
                        self.state = State::NormalText;
                        reprocess = true;
                    },

                State::CheckingPipe { count, char_before } =>
                    if c == '|' {
                        self.state = State::CheckingPipe {
                            count: count.saturating_add(1),
                            char_before,
                        };
                    } else {
                        if count == 2 {
                            if self.has_speculation(SpeculationKind::Spoiler) {
                                self.buffer.push_str("||");
                                self.resolve_speculation(SpeculationKind::Spoiler, "||", "||", out);
                            } else {
                                self.start_speculation(SpeculationKind::Spoiler);
                                self.buffer.push_str("||");
                                self.flush_text(out);
                            }
                        } else {
                            self.buffer.push_str(&"|".repeat(usize::from(count)));
                            self.flush_text(out);
                        }
                        self.state = State::NormalText;
                        reprocess = true;
                    },

                State::CheckingBackticks {
                    count,
                    is_line_start,
                } =>
                    if c == '`' {
                        self.state = State::CheckingBackticks {
                            count: count.saturating_add(1),
                            is_line_start,
                        };
                    } else if is_line_start && count >= MIN_FENCE_BACKTICKS && !self.inline_only {
                        self.state = State::ReadingCodeInfo {
                            opening_count: count,
                        };
                        reprocess = true;
                    } else {
                        let delim = "`".repeat(usize::from(count));
                        let kind = SpeculationKind::Code(count);
                        if self.has_speculation(kind) {
                            self.buffer.push_str(&delim);
                            self.resolve_speculation(kind, &delim, &delim, out);
                        } else {
                            self.start_speculation(kind);
                            self.buffer.push_str(&delim);
                            self.flush_text(out);
                        }
                        self.state = State::NormalText;
                        reprocess = true;
                    },

                State::CheckingDollar { char_before } =>
                    if c == '$' && !self.inline_only {
                        self.push_event(
                            Event::DisplayMathStart {
                                delimiter: "$$".to_string(),
                            },
                            out,
                        );
                        self.skip_math_newline = true;
                        self.state = State::InsideDisplayMathDollar;
                    } else {
                        let right_flanking = can_close(char_before);
                        if right_flanking && self.has_speculation(SpeculationKind::MathDollar) {
                            self.state = State::VerifyInlineMathDollarEnd;
                        } else {
                            if can_open(c) {
                                self.start_speculation(SpeculationKind::MathDollar);
                            }
                            self.buffer.push('$');
                            self.flush_text(out);
                            self.state = State::NormalText;
                        }
                        reprocess = true;
                    },

                State::VerifyInlineMathDollarEnd => {
                    self.buffer.push('$');
                    if c.is_ascii_digit() {
                        self.flush_text(out);
                    } else {
                        self.resolve_speculation(SpeculationKind::MathDollar, "$", "$", out);
                    }
                    self.state = State::NormalText;
                    reprocess = true;
                }

                State::CheckingSlash =>
                    if c == '[' && !self.inline_only {
                        self.push_event(
                            Event::DisplayMathStart {
                                delimiter: "\\[".to_string(),
                            },
                            out,
                        );
                        self.skip_math_newline = true;
                        self.state = State::InsideDisplayMathBracket;
                    } else if c == '(' {
                        self.start_speculation(SpeculationKind::MathParenthesis);
                        self.buffer.push_str("\\(");
                        self.flush_text(out);
                        self.state = State::NormalText;
                    } else if c == ')' && self.has_speculation(SpeculationKind::MathParenthesis) {
                        self.buffer.push_str("\\)");
                        self.resolve_speculation(
                            SpeculationKind::MathParenthesis,
                            "\\(",
                            "\\)",
                            out,
                        );
                        self.state = State::NormalText;
                    } else {
                        self.buffer.push('\\');
                        self.state = State::NormalText;
                        reprocess = true;
                    },

                State::CheckingHeading {
                    count,
                    is_line_start,
                } =>
                    if c == '#' {
                        if count < MAX_HEADING_LEVEL {
                            self.state = State::CheckingHeading {
                                count: count.saturating_add(1),
                                is_line_start,
                            };
                        } else {
                            self.buffer.push_str(&"#".repeat(usize::from(count) + 1));
                            self.state = State::NormalText;
                        }
                    } else if c == ' ' || c == '\t' {
                        if is_line_start {
                            self.push_event(Event::HeadingStart { level: count }, out);
                            self.state = State::InsideHeading;
                        } else {
                            self.buffer.push_str(&"#".repeat(usize::from(count)));
                            self.buffer.push(c);
                            self.state = State::NormalText;
                        }
                    } else {
                        self.buffer.push_str(&"#".repeat(usize::from(count)));
                        self.state = State::NormalText;
                        reprocess = true;
                    },

                State::InsideHeading =>
                    if c == '\n' {
                        self.flush_text(out);
                        self.push_event(Event::HeadingEnd, out);
                        self.state = State::NormalText;
                        reprocess = true;
                    } else {
                        self.buffer.push(c);
                    },

                State::ReadingCodeInfo { opening_count } =>
                    if c == '\n' {
                        let language = std::mem::take(&mut self.buffer).trim().to_string();
                        self.push_event(Event::CodeBlockStart(language), out);
                        self.state = State::InsideCodeBlock { opening_count };
                    } else {
                        self.buffer.push(c);
                    },

                State::InsideCodeBlock { opening_count } =>
                    if self.at_line_start && c == '`' {
                        self.flush_text(out);
                        self.state = State::CheckingBlockEnd {
                            opening_count,
                            current_count: 1,
                        };
                    } else {
                        self.buffer.push(c);
                        if c == '\n' {
                            self.flush_text(out);
                        }
                    },

                State::CheckingBlockEnd {
                    opening_count,
                    current_count,
                } =>
                    if c == '`' {
                        self.state = State::CheckingBlockEnd {
                            opening_count,
                            current_count: current_count.saturating_add(1),
                        };
                    } else {
                        if current_count >= opening_count {
                            self.flush_text(out);
                            self.push_event(Event::CodeBlockEnd, out);
                            self.state = State::NormalText;
                        } else {
                            self.buffer
                                .push_str(&"`".repeat(usize::from(current_count)));
                            self.state = State::InsideCodeBlock { opening_count };
                        }
                        reprocess = true;
                    },

                State::InsideDisplayMathDollar =>
                    if std::mem::take(&mut self.skip_math_newline) && c == '\n' {
                    } else if c == '$' {
                        self.flush_text(out);
                        self.state = State::CheckingDisplayMathDollarEnd;
                    } else {
                        self.buffer.push(c);
                    },

                State::CheckingDisplayMathDollarEnd =>
                    if c == '$' {
                        self.push_event(
                            Event::DisplayMathEnd {
                                delimiter: "$$".to_string(),
                            },
                            out,
                        );
                        self.state = State::NormalText;
                    } else {
                        self.buffer.push('$');
                        self.state = State::InsideDisplayMathDollar;
                        reprocess = true;
                    },

                State::InsideDisplayMathBracket =>
                    if std::mem::take(&mut self.skip_math_newline) && c == '\n' {
                    } else if c == '\\' {
                        self.flush_text(out);
                        self.state = State::CheckingDisplayMathBracketEnd;
                    } else {
                        self.buffer.push(c);
                    },

                State::CheckingDisplayMathBracketEnd =>
                    if c == ']' {
                        self.push_event(
                            Event::DisplayMathEnd {
                                delimiter: "\\]".to_string(),
                            },
                            out,
                        );
                        self.state = State::NormalText;
                    } else {
                        self.buffer.push('\\');
                        self.state = State::InsideDisplayMathBracket;
                        reprocess = true;
                    },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Event, LlmMarkdownParser};

    fn cells(row: &str) -> Vec<String> {
        LlmMarkdownParser::split_row_cells(row)
    }

    #[test]
    fn a_delimiter_row_needs_pipes_and_dashes() {
        assert!(LlmMarkdownParser::is_delimiter_row("|---|---|"));
        assert!(LlmMarkdownParser::is_delimiter_row(" | :--- | ---: | "));
        assert!(!LlmMarkdownParser::is_delimiter_row("|   |   |"));
        assert!(!LlmMarkdownParser::is_delimiter_row("-----"));
        assert!(!LlmMarkdownParser::is_delimiter_row(""));
        assert!(!LlmMarkdownParser::is_delimiter_row("| a | b |"));
    }

    #[test]
    fn a_table_row_is_any_non_empty_line_with_a_pipe() {
        assert!(LlmMarkdownParser::is_table_row("| a |"));
        assert!(LlmMarkdownParser::is_table_row("a | b"));
        assert!(!LlmMarkdownParser::is_table_row("   "));
        assert!(!LlmMarkdownParser::is_table_row("no pipe here"));
    }

    #[test]
    fn cells_split_on_unescaped_pipes_only() {
        assert_eq!(cells("| a | b |"), vec![" a ", " b "]);
        assert_eq!(cells("a|b"), vec!["a", "b"]);
        assert_eq!(cells(r"| a \| b |"), vec![" a | b "]);
        assert_eq!(cells(r"| a \n b |"), vec![r" a \n b "]);
    }

    #[test]
    fn a_row_without_outer_pipes_keeps_all_its_cells() {
        assert_eq!(cells("a | b | c"), vec!["a ", " b ", " c"]);
        assert_eq!(cells("only"), vec!["only"]);
        assert_eq!(cells("|"), vec![""]);
    }

    #[test]
    fn a_trailing_backslash_is_kept_in_the_last_cell() {
        assert_eq!(cells(r"a\"), vec![r"a\"]);
    }

    #[test]
    fn a_thematic_marker_is_one_of_three_characters() {
        for marker in ['-', '_', '*'] {
            assert!(LlmMarkdownParser::is_thematic_marker(marker));
        }
        for marker in ['+', '=', '#', 'a'] {
            assert!(!LlmMarkdownParser::is_thematic_marker(marker));
        }
    }

    #[test]
    fn block_events_are_told_apart_from_inline_ones() {
        assert!(LlmMarkdownParser::is_block_event(&Event::BlockquoteStart));
        assert!(LlmMarkdownParser::is_block_event(&Event::ThematicBreak));
        assert!(!LlmMarkdownParser::is_block_event(&Event::BoldStart));
        assert!(!LlmMarkdownParser::is_block_event(&Event::Text(
            "x".to_string()
        )));
    }

    #[test]
    fn table_events_are_told_apart_from_other_block_events() {
        assert!(LlmMarkdownParser::is_table_event(&Event::TableRowStart));
        assert!(LlmMarkdownParser::is_table_event(&Event::TableCellEnd));
        assert!(!LlmMarkdownParser::is_table_event(&Event::BlockquoteStart));
    }

    #[test]
    fn an_inline_reparse_stops_when_the_budget_runs_out() {
        let mut parser = LlmMarkdownParser::new();
        parser.reparse_budget = 0;

        let events = parser.parse_inline("**bold**");
        assert_eq!(events, vec![Event::Text("**bold**".to_string())]);
    }

    #[test]
    fn an_inline_reparse_of_nothing_produces_nothing() {
        let mut parser = LlmMarkdownParser::new();
        assert_eq!(parser.parse_inline(""), Vec::new());
    }

    #[test]
    fn an_inline_reparse_stops_at_the_recursion_limit() {
        let mut parser = LlmMarkdownParser::new();
        parser.reparse_budget = 1024;
        parser.depth = 0;

        let events = parser.parse_inline("*i*");
        assert_eq!(events, vec![Event::Text("*i*".to_string())]);
    }

    #[test]
    fn the_list_indent_grows_with_the_open_lists() {
        let mut parser = LlmMarkdownParser::new();
        assert_eq!(parser.list_indent(), 0);

        parser.open_containers.push(super::Container::Blockquote);
        assert_eq!(parser.list_indent(), 0);

        parser
            .open_containers
            .push(super::Container::List { ordered: false });
        assert_eq!(parser.list_indent(), 2);

        parser
            .open_containers
            .push(super::Container::List { ordered: true });
        assert_eq!(parser.list_indent(), 4);
    }
}
