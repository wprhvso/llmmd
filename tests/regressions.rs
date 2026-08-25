mod common;

use _native::from_markdown::Event;
use common::{assert_balanced, assert_entities_valid, events, render, text};

fn assert_well_formed(markdown: &str) {
    assert_balanced(markdown);

    let (rendered, entities) = render(markdown);
    assert_entities_valid(&rendered, &entities);
}

#[test]
fn nested_markup_is_not_duplicated_by_the_inner_reparse() {
    assert_eq!(
        text("*italic with [l](http://a.b) here*"),
        "italic with l here"
    );
    assert_eq!(text("**bold with `code` inside**"), "bold with code inside");
}

#[test]
fn a_closing_delimiter_reaches_the_enclosing_speculation() {
    assert_eq!(text("**a `b` c**"), "a b c");
    assert_eq!(text("**a $x$ b**"), "a x b");
    assert_eq!(text("**a \\(x\\) b**"), "a x b");
}

#[test]
fn trailing_markers_at_end_of_input_are_not_swallowed() {
    for (markdown, expected) in [
        ("abc ###", "abc ###"),
        ("###", "###"),
        ("abc `", "abc `"),
        ("abc ``", "abc ``"),
    ] {
        assert_eq!(text(markdown), expected);
    }
}

#[test]
fn a_nul_byte_closes_a_delimiter_like_any_other_word_character() {
    for (with_nul, with_letter) in [
        ("**a\0**b", "**az**b"),
        ("~~a\0~~b", "~~az~~b"),
        ("$a\0$b", "$az$b"),
    ] {
        assert_eq!(text(with_nul).replace('\0', "z"), text(with_letter));
    }
}

#[test]
fn dashes_followed_by_text_stay_literal() {
    for markdown in ["--- x", "*** x", "___ x", "a\n\n--- x\n\nb"] {
        assert!(!events(markdown).contains(&Event::ThematicBreak));
    }
    assert_eq!(text("--- x"), "--- x");
}

#[test]
fn an_unterminated_link_destination_does_not_grow_a_newline() {
    assert_eq!(text("a [b](http"), "a [b](http");
}

#[test]
fn an_html_like_run_does_not_duplicate_its_terminator() {
    assert_eq!(text("x < y"), "x < y");
    assert_eq!(text("a <b c>d"), "a <b c>d");
    assert_eq!(text("a <verylongtag>b"), "a <verylongtag>b");
}

#[test]
fn an_unterminated_fence_keeps_its_info_string_out_of_the_body() {
    assert_eq!(text("```rust"), "");
    assert_eq!(
        events("```rust"),
        vec![Event::CodeBlockStart("rust".into()), Event::CodeBlockEnd]
    );
}

#[test]
fn images_are_rendered_as_links_instead_of_being_dropped() {
    let (rendered, entities) = render("![alt](https://e.com/a.png)");
    assert_eq!(rendered, "alt");
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].url.as_deref(), Some("https://e.com/a.png"));
}

#[test]
fn trimming_the_trailing_newline_does_not_leave_entities_dangling() {
    assert_well_formed("> quoted\n```");
    assert_well_formed("> a\n$$");
    let (rendered, entities) = render("> quoted\n```");
    assert_entities_valid(&rendered, &entities);
}

#[test]
fn a_speculation_never_resolves_across_a_block_boundary() {
    assert_eq!(text("**a\n\n# h\n\nb**"), "**a\n\n# h\n\nb**");
    assert_eq!(text("> ||a\n\nb||"), "||a\n\nb||");
    assert_well_formed("***1) \n1. #||\\(<u>***    ~~| a |+[1]   1) ---");
    assert_well_formed("- rust***)***[x]```~~\\(_| a ||---|\n\n<sub>rust<u>\t   ```a</u>");
}

#[test]
fn an_unmatched_delimiter_does_not_reach_across_a_blank_line() {
    let (rendered, entities) = render("Use `foo to do X.\n\nThen run `bar` later.");
    assert_eq!(rendered, "Use `foo to do X.\n\nThen run bar later.");
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].r#type, "code");
}

#[test]
fn a_thematic_break_at_end_of_input_closes_containers_exactly_once() {
    assert_well_formed(">> a\n---");
    assert_well_formed("> a\n---");
    assert_well_formed("- a\n***");
}

#[test]
fn a_blank_line_does_not_restart_list_numbering() {
    assert_eq!(
        text("1. first\n\n2. second\n\n3. third\n"),
        "1. first\n\n2. second\n\n3. third\n"
    );
    assert_eq!(text("- a\n\n- b\n"), "• a\n\n• b\n");

    let (rendered, entities) = render("> a\n\nb\n");
    assert_eq!(rendered, "a\n\nb\n");
    let quote = entities
        .iter()
        .find(|entity| entity.r#type == "blockquote")
        .expect("a blockquote entity");
    assert_eq!(quote.length, 2, "the quote must not swallow the next block");
}

#[test]
fn plus_is_a_bullet_marker_not_a_thematic_break() {
    assert_eq!(text("+++\n"), "+++\n");
    assert_eq!(text("+ a\n+ b\n"), "• a\n• b\n");
    for rule in ["---\n", "***\n", "___\n"] {
        assert!(text(rule).contains("──────────"), "{rule:?}");
    }
}

#[test]
fn a_table_row_starting_with_digits_keeps_its_first_cell() {
    let rendered = text("Year | Revenue\n-----|--------\n2024 | 100\n2025 | 200\n");
    assert!(rendered.contains("2024"), "{rendered}");
    assert!(rendered.contains("2025"), "{rendered}");
    assert!(rendered.contains("100"), "{rendered}");
}

#[test]
fn text_before_a_table_keeps_its_line_break() {
    let rendered = text("Intro\n| A | B |\n|---|---|\n| 1 | 2 |\n");
    assert!(rendered.starts_with("Intro\n"), "{rendered:?}");
}

#[test]
fn a_table_inside_a_blockquote_keeps_the_blockquote() {
    let (rendered, entities) = render("> | a | b |\n> |---|---|\n> | 1 | 2 |\n");
    assert_entities_valid(&rendered, &entities);
    assert!(
        entities.iter().any(|entity| entity.r#type == "blockquote"),
        "the quote around the table must survive: {entities:?}"
    );
    assert_well_formed("> | a | b |\n> |---|---|\n> | 1 | 2 |\n");
}

#[test]
fn a_list_before_a_table_is_not_reopened_by_it() {
    assert_well_formed("- item\n| A | B |\n|---|---|\n| 1 | 2 |\n\n- second\n- third\n");
    let rendered = text("- item\n| A | B |\n|---|---|\n| 1 | 2 |\n\n- second\n- third\n");
    assert!(rendered.contains("• second"), "{rendered:?}");
    assert!(rendered.contains("• third"), "{rendered:?}");
}

#[test]
fn a_block_after_a_table_terminates_it_rather_than_being_swallowed() {
    for markdown in [
        "| A | B |\n|---|---|\n| 1 | 2 |\n```rust\nlet x = 1;\n```\ndone\n",
        "| A |\n|---|\n| 1 |\n$$\nx = 1\n$$\nend\n",
        "| A |\n|---|\n| 1 |\n- item one\n- item two\n",
        "| A |\n|---|\n| 1 |\n> quoted\n",
        "| A |\n|---|\n| 1 |\n# heading\n",
    ] {
        assert_well_formed(markdown);
    }

    let (rendered, entities) =
        render("| A | B |\n|---|---|\n| 1 | 2 |\n```rust\nlet x = 1;\n```\ndone\n");
    assert!(rendered.contains("let x = 1;"), "{rendered:?}");
    assert!(
        entities
            .iter()
            .any(|entity| entity.language.as_deref() == Some("rust")),
        "the code block must keep its language: {entities:?}"
    );

    let rendered = text("| A |\n|---|\n| 1 |\n- item one\n- item two\n");
    assert!(rendered.contains("• item one"), "{rendered:?}");
}

#[test]
fn a_thematic_break_after_a_table_is_rendered() {
    let rendered = text("| A |\n|---|\n| 1 |\n---\nAfter\n");
    assert!(rendered.contains("──────────"), "{rendered:?}");
    assert!(rendered.ends_with("After\n"), "{rendered:?}");
}

#[test]
fn indentation_nests_lists_inside_a_blockquote_too() {
    assert_eq!(text("> - a\n>   - b\n"), "• a\n  ◦ b\n");
    assert_eq!(text("- a\n  - b\n"), "• a\n  ◦ b\n");
}

#[test]
fn a_fence_inside_a_list_does_not_keep_the_list_indentation() {
    let (rendered, entities) = render("- item\n  ```py\n  code()\n  ```\n- next\n");
    assert_eq!(rendered, "• item\ncode()\n• next\n");
    let pre = entities
        .iter()
        .find(|entity| entity.r#type == "pre")
        .expect("a pre entity");
    assert_eq!(pre.language.as_deref(), Some("py"));
}

#[test]
fn an_escaped_pipe_stays_inside_its_table_cell() {
    let rendered = text("| a \\| b | c |\n|---|---|\n| 1 | 2 |\n");
    assert!(rendered.contains("a | b"), "{rendered:?}");

    assert!(
        rendered.contains('1') && rendered.contains('2'),
        "{rendered:?}"
    );
}

#[test]
fn a_surrogate_pair_survives_message_splitting() {
    use _native::to_telegram::split_message_with_entities;

    let text = "😀".repeat(40);
    for limit in 2..40 {
        let joined: String = split_message_with_entities(&text, &[], limit)
            .into_iter()
            .map(|(chunk, _)| chunk)
            .collect();
        assert_eq!(joined, text, "limit {limit} corrupted the text");
    }
}

#[test]
fn deeply_nested_inline_markup_finishes_quickly() {
    let depth = 200;
    let markdown = format!("{}x{}", "[".repeat(depth), "](u)".repeat(depth));
    let rendered = text(&markdown);
    assert!(rendered.contains('x'), "content must survive");

    let markdown = format!("{}x{}", "**".repeat(depth), "**".repeat(depth));
    assert!(text(&markdown).contains('x'));
}

#[test]
fn an_empty_link_or_image_label_falls_back_to_the_url() {
    for markdown in ["[](https://e.com)", "![](https://e.com)"] {
        let (rendered, entities) = render(markdown);
        assert_eq!(rendered, "https://e.com", "{markdown:?}");
        assert_eq!(entities.len(), 1, "{markdown:?}");
        assert_eq!(entities[0].url.as_deref(), Some("https://e.com"));
    }
}

#[test]
fn an_image_inside_a_link_yields_one_link_entity() {
    let (rendered, entities) = render("[![alt](https://i/x.png)](https://e.com)");
    assert_eq!(rendered, "alt");
    assert_eq!(entities.len(), 1, "{entities:?}");
    assert_eq!(entities[0].url.as_deref(), Some("https://e.com"));
}

#[test]
fn nested_blockquotes_produce_a_single_entity() {
    let (rendered, entities) = render("> a\n> > b\n");
    assert_eq!(rendered, "a\nb\n");
    assert_eq!(
        entities
            .iter()
            .filter(|entity| entity.r#type == "blockquote")
            .count(),
        1,
        "{entities:?}"
    );
}

#[test]
fn the_innermost_script_wins_over_an_enclosing_one() {
    assert_eq!(text("x<sup>a<sub>2</sub></sup>"), "xᵃ₂");
    assert_eq!(text("x<sub>a<sup>2</sup></sub>"), "xₐ²");
}

#[test]
fn display_math_does_not_start_with_a_blank_line() {
    let (rendered, entities) = render("$$\nx = 1\n$$\n");
    assert_eq!(rendered, "x = 1\n");
    let pre = entities
        .iter()
        .find(|entity| entity.r#type == "pre")
        .expect("a pre entity");
    assert_eq!(pre.offset, 0);
    assert_eq!(pre.length, 5);

    assert_eq!(text("\\[\nx\n\\]\n"), "x\n");

    assert_eq!(text("$$\n\nx\n$$\n"), "\nx\n");
}

#[test]
fn an_ordered_list_keeps_the_number_it_starts_at() {
    assert_eq!(text("3. a\n4. b\n"), "3. a\n4. b\n");
    assert_eq!(text("5. only\n"), "5. only\n");
    assert_eq!(text("1. a\n2. b\n"), "1. a\n2. b\n");
    assert_eq!(text("1) a\n2) b\n"), "1. a\n2. b\n");

    assert_eq!(text("3. a\n9. b\n"), "3. a\n4. b\n");
}

#[test]
fn windows_line_endings_render_like_unix_ones() {
    let documents = [
        "# Heading\n\nA **bold** word.\n",
        "- a\n- b\n  - c\n",
        "```python\nprint(1)\n```\n",
        "> quote\n> more\n",
        "| a | b |\n|---|---|\n| 1 | 2 |\n",
        "text\n\nmore text\n",
    ];
    for document in documents {
        let windows = document.replace('\n', "\r\n");
        assert_eq!(
            render(&windows),
            render(document),
            "{document:?} rendered differently with CRLF line endings"
        );
    }
}

#[test]
fn a_carriage_return_never_reaches_the_message_text() {
    for document in ["a\r\nb\r\n", "# h\r\n", "**b**\r\n", "a\rb\r"] {
        let (text, entities) = render(document);
        assert!(
            !text.contains('\r'),
            "{document:?} kept a carriage return in {text:?}"
        );
        assert_entities_valid(&text, &entities);
    }
}

#[test]
fn a_lone_carriage_return_breaks_the_line() {
    assert_eq!(render("a\rb").0, "a\nb");
}

#[test]
fn a_link_without_a_label_shows_its_url_inside_a_table() {
    let text = render("| a |\n|---|\n| [](https://e.com/x) |\n").0;
    assert!(
        text.contains("https://e.com/x"),
        "the url disappeared from the table: {text:?}"
    );
}

#[test]
fn a_link_inside_a_table_keeps_its_label() {
    let text = render("| a |\n|---|\n| [label](https://e.com/x) |\n").0;
    assert!(text.contains("label"), "the label disappeared: {text:?}");
}
