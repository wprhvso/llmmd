use _native::from_markdown::{Event, TaskStatus};

use crate::support::{events, text_of};

#[test]
fn atx_headings() {
    assert_eq!(
        events("# Title\n"),
        vec![
            Event::HeadingStart { level: 1 },
            Event::Text("Title".into()),
            Event::HeadingEnd,
            Event::Text("\n".into()),
        ]
    );
    for level in 1_u8..=6 {
        let markdown = format!("{} H\n", "#".repeat(usize::from(level)));
        assert_eq!(
            events(&markdown).first(),
            Some(&Event::HeadingStart { level }),
            "level {level} heading"
        );
    }
}

#[test]
fn seven_hashes_is_not_a_heading() {
    assert_eq!(text_of("####### x\n"), "####### x\n");
}

#[test]
fn hash_not_at_line_start_is_literal() {
    assert_eq!(text_of("a # b\n"), "a # b\n");
}

#[test]
fn unterminated_heading_is_closed() {
    assert_eq!(
        events("# Title"),
        vec![
            Event::HeadingStart { level: 1 },
            Event::Text("Title".into()),
            Event::HeadingEnd,
        ]
    );
}

#[test]
fn bullet_lists() {
    assert_eq!(
        events("- a\n- b\n"),
        vec![
            Event::ListStart {
                ordered: false,
                start: 1,
            },
            Event::ListItemStart {
                task_status: TaskStatus::None
            },
            Event::Text("a\n".into()),
            Event::ListItemEnd,
            Event::ListItemStart {
                task_status: TaskStatus::None
            },
            Event::Text("b\n".into()),
            Event::ListItemEnd,
            Event::ListEnd,
        ]
    );
}

#[test]
fn ordered_lists_accept_dot_and_paren() {
    for markdown in ["1. a\n2. b\n", "1) a\n2) b\n"] {
        let evs = events(markdown);
        assert_eq!(
            evs.first(),
            Some(&Event::ListStart {
                ordered: true,
                start: 1,
            }),
            "{markdown:?}"
        );
        assert_eq!(evs.last(), Some(&Event::ListEnd), "{markdown:?}");
    }
}

#[test]
fn nested_lists_open_and_close_symmetrically() {
    let evs = events("- a\n  - b\n");
    let starts = evs
        .iter()
        .filter(|e| matches!(e, Event::ListStart { .. }))
        .count();
    let ends = evs.iter().filter(|e| **e == Event::ListEnd).count();
    assert_eq!(starts, 2);
    assert_eq!(starts, ends);
    assert_eq!(
        evs.iter()
            .filter(|e| matches!(e, Event::ListItemStart { .. }))
            .count(),
        evs.iter().filter(|e| **e == Event::ListItemEnd).count()
    );
}

#[test]
fn task_list_items() {
    let unordered = events("- [ ] todo\n- [x] done\n");
    let statuses: Vec<TaskStatus> = unordered
        .iter()
        .filter_map(|e| match e {
            Event::ListItemStart { task_status } => Some(*task_status),
            _ => None,
        })
        .collect();
    assert_eq!(statuses, vec![TaskStatus::Todo, TaskStatus::Done]);

    let ordered = events("1. [ ] todo\n2. [X] done\n");
    let statuses: Vec<TaskStatus> = ordered
        .iter()
        .filter_map(|e| match e {
            Event::ListItemStart { task_status } => Some(*task_status),
            _ => None,
        })
        .collect();
    assert_eq!(statuses, vec![TaskStatus::Todo, TaskStatus::Done]);
}

#[test]
fn blockquotes_nest_and_close() {
    assert_eq!(
        events("> q\n"),
        vec![
            Event::BlockquoteStart,
            Event::Text("q\n".into()),
            Event::BlockquoteEnd,
        ]
    );

    let nested = events("> a\n> > b\n");
    assert_eq!(
        nested
            .iter()
            .filter(|e| **e == Event::BlockquoteStart)
            .count(),
        nested
            .iter()
            .filter(|e| **e == Event::BlockquoteEnd)
            .count()
    );
}

#[test]
fn thematic_breaks() {
    for markdown in ["a\n\n---\n\nb", "a\n\n***\n\nb", "a\n\n___\n\nb"] {
        assert!(
            events(markdown).contains(&Event::ThematicBreak),
            "{markdown:?} should produce a thematic break"
        );
    }

    assert!(!events("a\n\n--\n\nb").contains(&Event::ThematicBreak));
}

#[test]
fn fenced_code_blocks() {
    assert_eq!(
        events("```python\nprint(1)\n```\n"),
        vec![
            Event::CodeBlockStart("python".into()),
            Event::Text("print(1)\n".into()),
            Event::CodeBlockEnd,
            Event::Text("\n".into()),
        ]
    );
}

#[test]
fn code_fence_content_is_not_interpreted() {
    let evs = events("```\n# not a heading\n**not bold**\n```\n");
    assert!(!evs.iter().any(|e| matches!(e, Event::HeadingStart { .. })));
    assert!(!evs.contains(&Event::BoldStart));
    assert!(evs.contains(&Event::Text("# not a heading\n".into())));
}

#[test]
fn unterminated_fence_keeps_its_language() {
    assert_eq!(
        events("```rust"),
        vec![Event::CodeBlockStart("rust".into()), Event::CodeBlockEnd,]
    );
    assert_eq!(
        events("```rust\nfn main() {}"),
        vec![
            Event::CodeBlockStart("rust".into()),
            Event::Text("fn main() {}".into()),
            Event::CodeBlockEnd,
        ]
    );
}

#[test]
fn partial_closing_fence_stays_in_the_body() {
    assert_eq!(
        events("```\ncode\n``"),
        vec![
            Event::CodeBlockStart(String::new()),
            Event::Text("code\n".into()),
            Event::Text("``".into()),
            Event::CodeBlockEnd,
        ]
    );
}

#[test]
fn display_math_blocks() {
    assert_eq!(
        events("$$x+1$$\n"),
        vec![
            Event::DisplayMathStart {
                delimiter: "$$".into()
            },
            Event::Text("x+1".into()),
            Event::DisplayMathEnd {
                delimiter: "$$".into()
            },
            Event::Text("\n".into()),
        ]
    );
    assert_eq!(
        events("\\[x+1\\]\n"),
        vec![
            Event::DisplayMathStart {
                delimiter: "\\[".into()
            },
            Event::Text("x+1".into()),
            Event::DisplayMathEnd {
                delimiter: "\\]".into()
            },
            Event::Text("\n".into()),
        ]
    );
}

#[test]
fn unterminated_display_math_is_closed() {
    let evs = events("$$x+1");
    assert_eq!(
        evs.last(),
        Some(&Event::DisplayMathEnd {
            delimiter: "$$".into()
        })
    );
}

#[test]
fn tables_are_recognised_and_rolled_back() {
    let evs = events("| a | b |\n|---|---|\n| 1 | 2 |\n");
    assert_eq!(evs.first(), Some(&Event::TableStart));
    assert_eq!(evs.last(), Some(&Event::TableEnd));
    assert_eq!(
        evs.iter().filter(|e| **e == Event::TableRowStart).count(),
        2,
        "header row plus one body row: {evs:?}"
    );
    assert!(
        evs.contains(&Event::TableCellStart { is_header: true }),
        "the header row must be marked: {evs:?}"
    );

    assert!(
        !evs.iter()
            .any(|e| matches!(e, Event::Text(t) if t.contains('|')))
    );
}

#[test]
fn a_table_ends_when_the_rows_stop() {
    let evs = events("| a |\n|---|\n| 1 |\nafter\n");
    let end = evs
        .iter()
        .position(|e| *e == Event::TableEnd)
        .expect("table must close");
    assert!(
        evs[end..].contains(&Event::Text("after\n".into())),
        "trailing text must be emitted after the table: {evs:?}"
    );
}

#[test]
fn an_unterminated_table_still_closes() {
    let evs = events("| a | b |\n|---|---|\n| 1 | 2 |");
    assert_eq!(evs.last(), Some(&Event::TableEnd));
}

#[test]
fn a_pipe_line_without_a_delimiter_row_is_not_a_table() {
    let evs = events("a | b\nc | d\n");
    assert!(!evs.contains(&Event::TableStart));
}

#[test]
fn containers_are_always_balanced() {
    for markdown in [
        "- a\n  - b\n    - c\n",
        "> a\n> > b\n> a\n",
        "- a\n  > q\n- b\n",
        "1. a\n   1. b\n2. c\n",
        "> - a\n> - b\n",
        "- a\n\n---\n\n- b\n",
        "- a\n```\ncode\n```\n",
    ] {
        let evs = events(markdown);
        let pairs = [
            (Event::BlockquoteStart, Event::BlockquoteEnd),
            (Event::ListItemEnd, Event::ListItemEnd),
        ];
        let opens = evs.iter().filter(|e| **e == pairs[0].0).count();
        let closes = evs.iter().filter(|e| **e == pairs[0].1).count();
        assert_eq!(opens, closes, "unbalanced blockquotes in {markdown:?}");

        let list_opens = evs
            .iter()
            .filter(|e| matches!(e, Event::ListStart { .. }))
            .count();
        let list_closes = evs.iter().filter(|e| **e == Event::ListEnd).count();
        assert_eq!(list_opens, list_closes, "unbalanced lists in {markdown:?}");

        let item_opens = evs
            .iter()
            .filter(|e| matches!(e, Event::ListItemStart { .. }))
            .count();
        let item_closes = evs.iter().filter(|e| **e == Event::ListItemEnd).count();
        assert_eq!(
            item_opens, item_closes,
            "unbalanced list items in {markdown:?}"
        );
    }
}
