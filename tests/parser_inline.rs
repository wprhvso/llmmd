
mod common;

use _native::from_markdown::Event;
use common::{events, resolve};

fn text_of(markdown: &str) -> String {
    events(markdown)
        .iter()
        .filter_map(|event| match event {
            Event::Text(value) => Some(value.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn plain_text_round_trips() {
    assert_eq!(
        events("hello world"),
        vec![Event::Text("hello world".into())]
    );
}

#[test]
fn empty_input_produces_no_events() {
    assert_eq!(events(""), []);
}

#[test]
fn bold_italic_and_bold_italic() {
    assert_eq!(
        events("**b**"),
        vec![Event::BoldStart, Event::Text("b".into()), Event::BoldEnd]
    );
    assert_eq!(
        events("*i*"),
        vec![
            Event::ItalicStart,
            Event::Text("i".into()),
            Event::ItalicEnd
        ]
    );
    assert_eq!(
        events("***bi***"),
        vec![
            Event::BoldStart,
            Event::ItalicStart,
            Event::Text("bi".into()),
            Event::ItalicEnd,
            Event::BoldEnd,
        ]
    );
}

#[test]
fn underscore_emphasis_is_not_applied_inside_words() {

    assert_eq!(text_of("a_b_c"), "a_b_c");
    assert!(!events("a_b_c").contains(&Event::ItalicStart));

    assert_eq!(
        events("_i_"),
        vec![
            Event::ItalicStart,
            Event::Text("i".into()),
            Event::ItalicEnd
        ]
    );
}

#[test]
fn more_than_three_markers_stay_literal() {
    assert_eq!(text_of("****x****"), "****x****");
}

#[test]
fn unclosed_emphasis_stays_literal() {
    assert_eq!(text_of("a **b c"), "a **b c");
    assert_eq!(text_of("abc __"), "abc __");
}

#[test]
fn strikethrough_spoiler_and_underline() {
    assert_eq!(
        events("~~s~~"),
        vec![
            Event::StrikethroughStart,
            Event::Text("s".into()),
            Event::StrikethroughEnd,
        ]
    );
    assert_eq!(
        events("||s||"),
        vec![
            Event::SpoilerStart,
            Event::Text("s".into()),
            Event::SpoilerEnd,
        ]
    );
    assert_eq!(
        events("<u>s</u>"),
        vec![
            Event::UnderlineStart,
            Event::Text("s".into()),
            Event::UnderlineEnd,
        ]
    );
}

#[test]
fn single_tilde_is_literal() {
    assert_eq!(text_of("a ~ b"), "a ~ b");
    assert_eq!(text_of("a ~~~ b"), "a ~~~ b");
}

#[test]
fn inline_code_is_opaque_to_markup() {
    assert_eq!(
        events("`a*b*c`"),
        vec![Event::InlineCode("a*b*c".into())],
        "inline code must not be re-interpreted as emphasis"
    );
    assert_eq!(events("``a`b``"), vec![Event::InlineCode("a`b".into())]);
}

#[test]
fn inline_math_variants() {
    assert_eq!(
        events("$x+1$"),
        vec![Event::InlineMath {
            delimiter: "$".into(),
            content: "x+1".into(),
        }]
    );
    assert_eq!(
        events("\\(x+1\\)"),
        vec![Event::InlineMath {
            delimiter: "\\(".into(),
            content: "x+1".into(),
        }]
    );
}

#[test]
fn currency_amounts_are_not_math() {
    assert_eq!(text_of("costs $5 and $10 total"), "costs $5 and $10 total");
    assert_eq!(text_of("$100"), "$100");
}

#[test]
fn closing_math_delimiter_needs_non_space_before_it() {

    assert_eq!(text_of("a $x $ y"), "a $x $ y");
    assert!(
        !events("a $x $ y")
            .iter()
            .any(|e| matches!(e, Event::InlineMath { .. }))
    );
}

#[test]
fn links_and_images() {
    assert_eq!(
        events("[label](https://example.com)"),
        vec![
            Event::LinkStart {
                url: "https://example.com".into()
            },
            Event::Text("label".into()),
            Event::LinkEnd,
        ]
    );
    assert_eq!(
        events("![alt](https://example.com/a.png)"),
        vec![
            Event::ImageStart {
                url: "https://example.com/a.png".into()
            },
            Event::Text("alt".into()),
            Event::ImageEnd,
        ]
    );
}

#[test]
fn link_label_may_contain_markup() {
    assert_eq!(
        events("[**bold**](https://e.com)"),
        vec![
            Event::LinkStart {
                url: "https://e.com".into()
            },
            Event::BoldStart,
            Event::Text("bold".into()),
            Event::BoldEnd,
            Event::LinkEnd,
        ]
    );
}

#[test]
fn bracket_without_destination_stays_literal() {
    assert_eq!(text_of("a [b] c"), "a [b] c");
    assert_eq!(text_of("a [b](http"), "a [b](http");
    assert_eq!(text_of("[a](http://x y)"), "[a](http://x y)");
}

#[test]
fn bang_without_bracket_stays_literal() {
    assert_eq!(text_of("a! b"), "a! b");
    assert_eq!(text_of("wow!"), "wow!");
}

#[test]
fn nested_inline_content_is_not_duplicated() {

    assert_eq!(
        events("**bold with `code` inside**"),
        vec![
            Event::BoldStart,
            Event::Text("bold with ".into()),
            Event::InlineCode("code".into()),
            Event::Text(" inside".into()),
            Event::BoldEnd,
        ]
    );
    assert_eq!(
        events("*italic with [link](http://a.b) here*"),
        vec![
            Event::ItalicStart,
            Event::Text("italic with ".into()),
            Event::LinkStart {
                url: "http://a.b".into()
            },
            Event::Text("link".into()),
            Event::LinkEnd,
            Event::Text(" here".into()),
            Event::ItalicEnd,
        ]
    );
    assert_eq!(
        events("**a $x$ b**"),
        vec![
            Event::BoldStart,
            Event::Text("a ".into()),
            Event::InlineMath {
                delimiter: "$".into(),
                content: "x".into(),
            },
            Event::Text(" b".into()),
            Event::BoldEnd,
        ]
    );
}

#[test]
fn deeply_nested_emphasis_terminates() {

    let depth = 200;
    let markdown = format!("{}x{}", "*".repeat(depth), "*".repeat(depth));
    let rendered = text_of(&markdown);
    assert!(rendered.contains('x'), "content must survive: {rendered:?}");
}

#[test]
fn html_like_text_is_preserved_verbatim() {

    assert_eq!(text_of("x < y"), "x < y");
    assert_eq!(text_of("a <b c>d"), "a <b c>d");
    assert_eq!(text_of("a <verylongtag>b"), "a <verylongtag>b");
    assert_eq!(text_of("a <u>b"), "a <u>b");
}

#[test]
fn superscript_and_subscript_events() {
    assert_eq!(
        events("x<sup>2</sup>"),
        vec![
            Event::Text("x".into()),
            Event::SuperscriptStart,
            Event::Text("2".into()),
            Event::SuperscriptEnd,
        ]
    );
    assert_eq!(
        events("x<sub>i</sub>"),
        vec![
            Event::Text("x".into()),
            Event::SubscriptStart,
            Event::Text("i".into()),
            Event::SubscriptEnd,
        ]
    );
}

#[test]
fn trailing_delimiters_are_never_swallowed() {

    assert_eq!(text_of("abc ###"), "abc ###");
    assert_eq!(text_of("###"), "###");
    assert_eq!(text_of("abc `"), "abc `");
    assert_eq!(text_of("abc ``"), "abc ``");
    assert_eq!(text_of("abc $"), "abc $");
    assert_eq!(text_of("abc \\"), "abc \\");
    assert_eq!(text_of("abc !"), "abc !");
    assert_eq!(text_of("abc <"), "abc <");
}

#[test]
fn rollback_counts_match_the_events_they_undo() {

    for markdown in [
        "**b**",
        "a `c` b",
        "[l](http://x)",
        "![i](http://x)",
        "***bi***",
        "~~s~~ and ||sp||",
        "<u>u</u> <sup>2</sup> <sub>3</sub>",
        "**outer *inner* rest**",
        "$m$ and \\(n\\)",
        "| a | b |\n|---|---|\n| 1 | 2 |\n",
    ] {
        let resolved = resolve(common::actions(markdown));
        assert!(
            !resolved.is_empty(),
            "{markdown:?} resolved to nothing at all"
        );
        assert!(
            !resolved.iter().any(
                |event| matches!(event, Event::Text(t) if t.contains("**") || t.contains("]("))
            ),
            "{markdown:?} left literal markup behind: {resolved:?}"
        );
    }
}
