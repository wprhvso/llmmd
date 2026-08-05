#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Text(String),
    CodeBlockStart(String),
    // CodeText(String),
    CodeBlockEnd,
    InlineCode(String),
    DisplayMathStart { delimiter: String },
    DisplayMathText(String),
    DisplayMathEnd { delimiter: String },
    InlineMath { delimiter: String, content: String },
    HeadingStart { level: u8 },
    HeadingText(String),
    HeadingEnd,
    BlockquoteStart,
    BlockquoteEnd,
    ListStart { ordered: bool },
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

#[derive(Debug, Clone, PartialEq, Eq)]
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
    CheckingDollar,
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
    CheckingHtmlTag {
        tag: String,
    },
    CheckingBang,
    CheckingLinkUrl {
        kind: SpeculationKind,
        spec_idx: usize,
    },
    ReadingLinkUrl {
        kind: SpeculationKind,
        spec_idx: usize,
        url: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    Blockquote,
    List { ordered: bool },
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
    },
    CheckingThematicBreak {
        marker: char,
        count: u8,
    },
    CheckingTaskBox {
        step: u8,
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

#[derive(Debug, Clone)]
struct Speculation {
    kind: SpeculationKind,
    start_event_index: usize,
    raw_content: String,
}

#[allow(clippy::struct_excessive_bools)]
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
    global_event_counter: usize,
    speculations: Vec<Speculation>,
    current_line_raw: String,
    last_line_raw: String,
    current_line_event_index: usize,
    last_line_event_index: usize,
    in_table: bool,
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
            global_event_counter: 0,
            speculations: Vec::new(),
            current_line_raw: String::new(),
            last_line_raw: String::new(),
            current_line_event_index: 0,
            last_line_event_index: 0,
            in_table: false,
        }
    }

    fn push_event(&mut self, event: Event, out: &mut Vec<Action>) {
        self.global_event_counter = self.global_event_counter.saturating_add(1);
        out.push(Action::Emit(event));
    }

    fn push_rollback(&mut self, count: usize, out: &mut Vec<Action>) {
        self.global_event_counter = self.global_event_counter.saturating_sub(count);
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

    #[allow(clippy::too_many_lines)]
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

            let rollback = self
                .global_event_counter
                .saturating_sub(spec.start_event_index);
            if rollback > 0 {
                self.push_rollback(rollback, out);
            }

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
                    let inner_actions = Self::parse_inline(&label);
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
                    let inner_actions = Self::parse_inline(&label);
                    for ev in inner_actions {
                        self.push_event(ev, out);
                    }
                    self.push_event(Event::ImageEnd, out);
                }
                _ => {}
            }
        }
    }

    #[allow(clippy::too_many_lines)]
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

            let rollback = self
                .global_event_counter
                .saturating_sub(spec.start_event_index);
            if rollback > 0 {
                self.push_rollback(rollback, out);
            }

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
                    let inner_actions = Self::parse_inline(&content_str);
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
                        SpeculationKind::Strikethrough => {
                            (Event::StrikethroughStart, Event::StrikethroughEnd)
                        }
                        SpeculationKind::Spoiler => (Event::SpoilerStart, Event::SpoilerEnd),
                        SpeculationKind::Underline => (Event::UnderlineStart, Event::UnderlineEnd),
                        SpeculationKind::Superscript => {
                            (Event::SuperscriptStart, Event::SuperscriptEnd)
                        }
                        SpeculationKind::Subscript => (Event::SubscriptStart, Event::SubscriptEnd),
                        _ => unreachable!(),
                    };

                    self.push_event(start_ev, out);
                    let inner_actions = Self::parse_inline(&content_str);
                    for ev in inner_actions {
                        self.push_event(ev, out);
                    }
                    self.push_event(end_ev, out);
                }
            }
        }
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

    fn parse_inline(text: &str) -> Vec<Event> {
        let mut p = Self::new();
        p.prefix_state = PrefixState::Done;
        p.inline_only = true;
        let mut res = p.push_chunk(text);
        res.actions.extend(p.end().actions);
        res.actions
            .into_iter()
            .filter_map(|a| match a {
                Action::Emit(e) => Some(e),
                Action::Rollback(_) => None,
            })
            .collect()
    }

    fn emit_parsed_table_row(&mut self, row: &str, is_header: bool, out: &mut Vec<Action>) {
        self.push_event(Event::TableRowStart, out);
        let mut row = row.trim();
        if row.starts_with('|') {
            row = &row[1..];
        }
        if row.ends_with('|') {
            row = &row[..row.len().saturating_sub(1)];
        }
        for cell in row.split('|') {
            self.push_event(Event::TableCellStart { is_header }, out);
            let cell_events = Self::parse_inline(cell);
            for ev in cell_events {
                self.push_event(ev, out);
            }
            self.push_event(Event::TableCellEnd, out);
        }
        self.push_event(Event::TableRowEnd, out);
    }

    #[allow(clippy::too_many_lines)]
    pub fn push_chunk(&mut self, chunk: &str) -> ChunkResult {
        let mut actions = Vec::new();

        for c in chunk.chars() {
            let mut process_as_text = false;

            let in_strict_block = matches!(
                self.state,
                State::InsideCodeBlock { .. }
                    | State::CheckingBlockEnd { .. }
                    | State::InsideDisplayMathDollar
                    | State::CheckingDisplayMathDollarEnd
                    | State::InsideDisplayMathBracket
                    | State::CheckingDisplayMathBracketEnd
            );

            if self.prefix_state == PrefixState::Done || self.inline_only {
                process_as_text = !self.found_thematic_break;
            } else {
                match self.prefix_state {
                    PrefixState::StrictScan {
                        quotes_stripped,
                        space_allowed,
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
                            };
                            continue;
                        } else if (c == ' ' || c == '\t') && space_allowed {
                            self.prefix_state = PrefixState::StrictScan {
                                quotes_stripped,
                                space_allowed: false,
                            };
                            continue;
                        } else if (c == ' ' || c == '\t') && quotes_stripped < expected {
                            continue;
                        }
                        self.prefix_state = PrefixState::Done;
                    }
                    PrefixState::Scan => {
                        if c == ' ' {
                            self.current_indent = self.current_indent.saturating_add(1);
                            continue;
                        } else if c == '\t' {
                            self.current_indent = self.current_indent.saturating_add(4);
                            continue;
                        } else if c == '>' {
                            if self.line_containers.is_empty() {
                                let parent_levels = usize::from(self.current_indent / 2);
                                let keep = parent_levels.min(self.open_containers.len());
                                for &container in self.open_containers.iter().take(keep) {
                                    self.line_containers.push(container);
                                }
                            }
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
                    PrefixState::CheckingThematicBreak { marker, count } => {
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
                            if count >= 3 {
                                self.found_thematic_break = true;
                                self.prefix_buffer.clear();
                            }
                            self.prefix_state = PrefixState::Done;
                        } else if count == 1
                            && (marker == '-' || marker == '*' || marker == '+')
                            && self.prefix_buffer.ends_with(|ch: char| ch.is_whitespace())
                        {
                            if self.line_containers.is_empty() {
                                let parent_levels = usize::from(self.current_indent / 2);
                                let keep = parent_levels.min(self.open_containers.len());
                                for &container in self.open_containers.iter().take(keep) {
                                    self.line_containers.push(container);
                                }
                            }
                            self.line_containers
                                .push(Container::List { ordered: false });
                            self.explicit_list_marker = true;
                            self.prefix_buffer.clear();
                            self.current_indent = 0;

                            if c == '[' {
                                self.prefix_buffer.push(c);
                                self.prefix_state = PrefixState::CheckingTaskBox {
                                    step: 1,
                                    status: TaskStatus::None,
                                };
                                continue;
                            }

                            self.prefix_state = PrefixState::Done;
                        } else {
                            self.prefix_state = PrefixState::Done;
                        }
                    }
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
                            if self.line_containers.is_empty() {
                                let parent_levels = usize::from(self.current_indent / 2);
                                let keep = parent_levels.min(self.open_containers.len());
                                for &container in self.open_containers.iter().take(keep) {
                                    self.line_containers.push(container);
                                }
                            }
                            self.line_containers.push(Container::List { ordered: true });
                            self.explicit_list_marker = true;
                            self.prefix_buffer.clear();
                            self.current_indent = 0;
                            self.prefix_state = PrefixState::CheckingTaskBox {
                                step: 0,
                                status: TaskStatus::None,
                            };
                            continue;
                        }
                        self.prefix_state = PrefixState::Done;
                    }
                    PrefixState::CheckingTaskBox { step, status } => {
                        if step == 0 {
                            if c == '[' {
                                self.prefix_buffer.push(c);
                                self.prefix_state =
                                    PrefixState::CheckingTaskBox { step: 1, status };
                                continue;
                            }
                            self.prefix_state = PrefixState::Done;
                        } else if step == 1 {
                            if c == ' ' {
                                self.prefix_buffer.push(c);
                                self.prefix_state = PrefixState::CheckingTaskBox {
                                    step: 2,
                                    status: TaskStatus::Todo,
                                };
                                continue;
                            } else if c == 'x' || c == 'X' {
                                self.prefix_buffer.push(c);
                                self.prefix_state = PrefixState::CheckingTaskBox {
                                    step: 2,
                                    status: TaskStatus::Done,
                                };
                                continue;
                            }
                            self.prefix_state = PrefixState::Done;
                        } else if step == 2 {
                            if c == ']' {
                                self.prefix_buffer.push(c);
                                self.prefix_state =
                                    PrefixState::CheckingTaskBox { step: 3, status };
                                continue;
                            }
                            self.prefix_state = PrefixState::Done;
                        } else if step == 3 {
                            if c == ' ' || c == '\t' {
                                self.current_task_status = status;
                                self.prefix_buffer.clear();
                                self.prefix_state = PrefixState::Scan;
                                continue;
                            }
                            self.prefix_state = PrefixState::Done;
                        }
                    }
                    PrefixState::Done => unreachable!(),
                }

                if self.prefix_state == PrefixState::Done {
                    if c != '\n'
                        && matches!(self.prefix_state, PrefixState::CheckingThematicBreak { count, .. } if count >= 3)
                    {
                        self.found_thematic_break = true;
                        self.prefix_buffer.clear();
                    }

                    if in_strict_block {
                        self.line_containers = self.open_containers.clone();
                    } else if self.line_containers.is_empty() && !self.explicit_list_marker {
                        let is_lazy = !self.buffer.is_empty() && c != '\n';
                        if is_lazy || self.current_indent >= 2 {
                            self.line_containers = self.open_containers.clone();
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

                        let to_close = self.open_containers.get(common..).unwrap_or(&[]).to_vec();
                        for container in to_close.into_iter().rev() {
                            match container {
                                Container::Blockquote => {
                                    self.push_event(Event::BlockquoteEnd, &mut actions);
                                }
                                Container::List { .. } => {
                                    self.push_event(Event::ListItemEnd, &mut actions);
                                    self.push_event(Event::ListEnd, &mut actions);
                                }
                            }
                        }
                    }

                    let to_open = self.line_containers.get(common..).unwrap_or(&[]).to_vec();
                    for container in to_open {
                        match container {
                            Container::Blockquote => {
                                self.push_event(Event::BlockquoteStart, &mut actions);
                            }
                            Container::List { ordered } => {
                                self.push_event(Event::ListStart { ordered }, &mut actions);
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

                    if self.found_thematic_break {
                        self.push_event(Event::ThematicBreak, &mut actions);
                    } else {
                        let failed = std::mem::take(&mut self.prefix_buffer);
                        for fc in failed.chars() {
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

                let next_in_strict = matches!(
                    self.state,
                    State::InsideCodeBlock { .. }
                        | State::CheckingBlockEnd { .. }
                        | State::InsideDisplayMathDollar
                        | State::CheckingDisplayMathDollarEnd
                        | State::InsideDisplayMathBracket
                        | State::CheckingDisplayMathBracketEnd
                );

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
                        {
                            let rollback_count = self
                                .global_event_counter
                                .saturating_sub(self.last_line_event_index);
                            if rollback_count > 0 {
                                self.push_rollback(rollback_count, &mut actions);
                            }

                            self.push_event(Event::TableStart, &mut actions);
                            let row = self.last_line_raw.clone();
                            self.emit_parsed_table_row(&row, true, &mut actions);
                            self.in_table = true;
                        } else if self.in_table {
                            if Self::is_table_row(&self.current_line_raw) {
                                let rollback_count = self
                                    .global_event_counter
                                    .saturating_sub(self.current_line_event_index);
                                if rollback_count > 0 {
                                    self.push_rollback(rollback_count, &mut actions);
                                }
                                let row = self.current_line_raw.clone();
                                self.emit_parsed_table_row(&row, false, &mut actions);
                            } else {
                                self.in_table = false;
                                let rollback_count = self
                                    .global_event_counter
                                    .saturating_sub(self.current_line_event_index);
                                if rollback_count > 0 {
                                    self.push_rollback(rollback_count, &mut actions);
                                }
                                self.push_event(Event::TableEnd, &mut actions);

                                let text_events = Self::parse_inline(&self.current_line_raw);
                                for ev in text_events {
                                    self.push_event(ev, &mut actions);
                                }
                            }
                        }
                    } else if self.in_table && self.found_thematic_break {
                        self.in_table = false;
                        self.push_event(Event::TableEnd, &mut actions);
                    }

                    if next_in_strict {
                        self.prefix_state = PrefixState::StrictScan {
                            quotes_stripped: 0,
                            space_allowed: false,
                        };
                    } else {
                        self.prefix_state = PrefixState::Scan;
                    }
                    self.line_containers.clear();
                    self.current_indent = 0;
                    self.explicit_list_marker = false;
                    self.found_thematic_break = false;
                    self.current_task_status = TaskStatus::None;
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

        self.flush_text(&mut actions);
        ChunkResult { actions }
    }

    #[allow(clippy::too_many_lines)]
    pub fn end(&mut self) -> ChunkResult {
        let mut actions = Vec::new();

        if let PrefixState::CheckingThematicBreak { count, .. } = self.prefix_state
            && count >= 3
        {
            self.found_thematic_break = true;
            self.prefix_buffer.clear();
            self.prefix_state = PrefixState::Done;

            let common = self.open_containers.len().saturating_sub(1);
            self.flush_text(&mut actions);
            let to_close = self.open_containers.get(common..).unwrap_or(&[]).to_vec();
            for container in to_close.into_iter().rev() {
                match container {
                    Container::Blockquote => {
                        self.push_event(Event::BlockquoteEnd, &mut actions);
                    }
                    Container::List { .. } => {
                        self.push_event(Event::ListItemEnd, &mut actions);
                        self.push_event(Event::ListEnd, &mut actions);
                    }
                }
            }
            self.push_event(Event::ThematicBreak, &mut actions);
        }

        self.flush_text(&mut actions);

        if self.in_table {
            if Self::is_table_row(&self.current_line_raw) {
                let rollback_count = self
                    .global_event_counter
                    .saturating_sub(self.current_line_event_index);
                if rollback_count > 0 {
                    self.push_rollback(rollback_count, &mut actions);
                }
                let row = self.current_line_raw.clone();
                self.emit_parsed_table_row(&row, false, &mut actions);
            }
            self.push_event(Event::TableEnd, &mut actions);
        }

        match self.state {
            State::CheckingBackticks {
                count,
                is_line_start,
            } if is_line_start && count >= 3 => {
                self.push_event(Event::CodeBlockStart(String::new()), &mut actions);
                self.push_event(Event::CodeBlockEnd, &mut actions);
            }
            State::ReadingCodeInfo { .. } => {
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

        let to_close = self.open_containers.clone();
        for container in to_close.into_iter().rev() {
            match container {
                Container::Blockquote => self.push_event(Event::BlockquoteEnd, &mut actions),
                Container::List { .. } => {
                    self.push_event(Event::ListItemEnd, &mut actions);
                    self.push_event(Event::ListEnd, &mut actions);
                }
            }
        }
        self.open_containers.clear();
        self.state = State::NormalText;

        ChunkResult { actions }
    }

    #[allow(clippy::too_many_lines)]
    fn push_char(&mut self, c: char, out: &mut Vec<Action>) {
        let mut reprocess = true;

        while reprocess {
            reprocess = false;
            let current_state = self.state.clone();

            match current_state {
                State::NormalText => {
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
                        self.state = State::CheckingDollar;
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
                        self.state = State::CheckingHtmlTag {
                            tag: String::from("<"),
                        };
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
                    }
                }

                State::CheckingBang => {
                    if c == '[' {
                        self.start_speculation(SpeculationKind::ImageLabel);
                        self.buffer.push_str("![");
                        self.flush_text(out);
                        self.state = State::NormalText;
                    } else {
                        self.buffer.push('!');
                        self.state = State::NormalText;
                        reprocess = true;
                    }
                }

                State::CheckingLinkUrl { kind, spec_idx } => {
                    if c == '(' {
                        self.buffer.push('(');
                        self.flush_text(out);
                        self.state = State::ReadingLinkUrl {
                            kind,
                            spec_idx,
                            url: String::new(),
                        };
                    } else {
                        self.abort_speculation(spec_idx);
                        self.state = State::NormalText;
                        reprocess = true;
                    }
                }

                State::ReadingLinkUrl {
                    kind,
                    spec_idx,
                    mut url,
                } => {
                    if c == ')' {
                        self.buffer.push(')');
                        self.resolve_link(spec_idx, &url, kind, out);
                        self.state = State::NormalText;
                    } else if c.is_whitespace() {
                        self.buffer.push(c);
                        self.flush_text(out);
                        self.abort_speculation(spec_idx);
                        self.state = State::NormalText;
                    } else {
                        url.push(c);
                        self.buffer.push(c);
                        self.flush_text(out);
                        self.state = State::ReadingLinkUrl {
                            kind,
                            spec_idx,
                            url,
                        };
                    }
                }

                State::CheckingHtmlTag { mut tag } => {
                    tag.push(c);
                    if c == '>' {
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
                    } else if c.is_whitespace() || tag.len() > 10 {
                        self.buffer.push_str(&tag);
                        self.flush_text(out);
                        self.state = State::NormalText;
                        reprocess = true;
                    } else {
                        self.state = State::CheckingHtmlTag { tag };
                    }
                }

                State::CheckingStar {
                    count,
                    char_before,
                    marker,
                } => {
                    if c == marker {
                        self.state = State::CheckingStar {
                            count: count.saturating_add(1),
                            char_before,
                            marker,
                        };
                    } else {
                        if count > 3 {
                            self.buffer
                                .push_str(&marker.to_string().repeat(usize::from(count)));
                            self.flush_text(out);
                            self.state = State::NormalText;
                            reprocess = true;
                            continue;
                        }

                        let mut right_flanking = !char_before.is_whitespace()
                            && char_before != '\n'
                            && char_before != '\0';
                        let mut left_flanking = !c.is_whitespace() && c != '\n';

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
                    }
                }

                State::CheckingTilde { count, char_before } => {
                    if c == '~' {
                        self.state = State::CheckingTilde {
                            count: count.saturating_add(1),
                            char_before,
                        };
                    } else {
                        if count == 2 {
                            let right_flanking = !char_before.is_whitespace()
                                && char_before != '\n'
                                && char_before != '\0';
                            let left_flanking = !c.is_whitespace() && c != '\n';
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
                    }
                }

                State::CheckingPipe { count, char_before } => {
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
                    }
                }

                State::CheckingBackticks {
                    count,
                    is_line_start,
                } => {
                    if c == '`' {
                        self.state = State::CheckingBackticks {
                            count: count.saturating_add(1),
                            is_line_start,
                        };
                    } else if is_line_start && count >= 3 && !self.inline_only {
                        self.state = State::ReadingCodeInfo {
                            opening_count: count,
                        };
                        reprocess = true;
                    } else {
                        let delim = "`".repeat(usize::from(count));
                        let kind = SpeculationKind::Code(count);
                        if self.has_speculation(kind) {
                            self.resolve_speculation(kind, &delim, &delim, out);
                        } else {
                            self.start_speculation(kind);
                            self.buffer.push_str(&delim);
                            self.flush_text(out);
                        }
                        self.state = State::NormalText;
                        reprocess = true;
                    }
                }

                State::CheckingDollar => {
                    if c == '$' && !self.inline_only {
                        self.push_event(
                            Event::DisplayMathStart {
                                delimiter: "$$".to_string(),
                            },
                            out,
                        );
                        self.state = State::InsideDisplayMathDollar;
                    } else {
                        let right_flanking_ok = !self.last_char.is_whitespace();
                        if right_flanking_ok && self.has_speculation(SpeculationKind::MathDollar) {
                            self.state = State::VerifyInlineMathDollarEnd;
                        } else {
                            if !c.is_whitespace() {
                                self.start_speculation(SpeculationKind::MathDollar);
                            }
                            self.buffer.push('$');
                            self.flush_text(out);
                            self.state = State::NormalText;
                        }
                        reprocess = true;
                    }
                }

                State::VerifyInlineMathDollarEnd => {
                    if c.is_ascii_digit() {
                        self.buffer.push('$');
                        self.flush_text(out);
                    } else {
                        self.resolve_speculation(SpeculationKind::MathDollar, "$", "$", out);
                    }
                    self.state = State::NormalText;
                    reprocess = true;
                }

                State::CheckingSlash => {
                    if c == '[' && !self.inline_only {
                        self.push_event(
                            Event::DisplayMathStart {
                                delimiter: "\\[".to_string(),
                            },
                            out,
                        );
                        self.state = State::InsideDisplayMathBracket;
                    } else if c == '(' {
                        self.start_speculation(SpeculationKind::MathParenthesis);
                        self.buffer.push_str("\\(");
                        self.flush_text(out);
                        self.state = State::NormalText;
                    } else if c == ')' && self.has_speculation(SpeculationKind::MathParenthesis) {
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
                    }
                }

                State::CheckingHeading {
                    count,
                    is_line_start,
                } => {
                    if c == '#' {
                        if count < 6 {
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
                    }
                }

                State::InsideHeading => {
                    if c == '\n' {
                        self.flush_text(out);
                        self.push_event(Event::HeadingEnd, out);
                        self.state = State::NormalText;
                        reprocess = true;
                    } else {
                        self.buffer.push(c);
                    }
                }

                State::ReadingCodeInfo { opening_count } => {
                    if c == '\n' {
                        let language = std::mem::take(&mut self.buffer).trim().to_string();
                        self.push_event(Event::CodeBlockStart(language), out);
                        self.state = State::InsideCodeBlock { opening_count };
                    } else {
                        self.buffer.push(c);
                    }
                }

                State::InsideCodeBlock { opening_count } => {
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
                    }
                }

                State::CheckingBlockEnd {
                    opening_count,
                    current_count,
                } => {
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
                    }
                }

                State::InsideDisplayMathDollar => {
                    if c == '$' {
                        self.flush_text(out);
                        self.state = State::CheckingDisplayMathDollarEnd;
                    } else {
                        self.buffer.push(c);
                    }
                }

                State::CheckingDisplayMathDollarEnd => {
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
                    }
                }

                State::InsideDisplayMathBracket => {
                    if c == '\\' {
                        self.flush_text(out);
                        self.state = State::CheckingDisplayMathBracketEnd;
                    } else {
                        self.buffer.push(c);
                    }
                }

                State::CheckingDisplayMathBracketEnd => {
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
                    }
                }
            }
        }
    }
}
