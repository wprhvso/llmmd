mod common;

use common::{assert_entities_valid, render, slice_utf16, spans, text};

fn checked(markdown: &str) -> (String, Vec<_native::to_telegram::MessageEntity>) {
    let (rendered, entities) = render(markdown);
    assert_entities_valid(&rendered, &entities);
    (rendered, entities)
}

#[test]
fn emphasis_becomes_entities_and_markers_disappear() {
    let (rendered, entities) = checked("**b** *i* ~~s~~ ||sp|| <u>u</u>");
    assert_eq!(rendered, "b i s sp u");

    let kinds: Vec<&str> = entities
        .iter()
        .map(|entity| entity.r#type.as_str())
        .collect();
    for expected in ["bold", "italic", "strikethrough", "spoiler", "underline"] {
        assert!(kinds.contains(&expected), "missing {expected} in {kinds:?}");
    }
}

#[test]
fn entity_offsets_point_at_the_right_substring() {
    let (rendered, entities) = checked("plain **bold** and *italic* end");
    assert_eq!(rendered, "plain bold and italic end");

    for entity in &entities {
        let covered = slice_utf16(&rendered, entity.offset, entity.length);
        match entity.r#type.as_str() {
            "bold" => assert_eq!(covered, "bold"),
            "italic" => assert_eq!(covered, "italic"),
            other => panic!("unexpected entity {other}"),
        }
    }
}

#[test]
fn offsets_are_utf16_units_not_bytes_or_chars() {
    let (rendered, _) = checked("ж😀 **b**");
    assert_eq!(rendered, "ж😀 b");
    assert_eq!(spans("ж😀 **b**", "bold"), vec![(4, 1)]);
    assert_eq!(slice_utf16(&rendered, 4, 1), "b");
}

#[test]
fn nested_emphasis_produces_nested_spans() {
    let (rendered, _) = checked("**bold *and italic* rest**");
    assert_eq!(rendered, "bold and italic rest");
    assert_eq!(spans("**bold *and italic* rest**", "bold"), vec![(0, 20)]);
    assert_eq!(spans("**bold *and italic* rest**", "italic"), vec![(5, 10)]);
}

#[test]
fn links_carry_their_url() {
    let (rendered, entities) = checked("see [docs](https://example.com/a?b=1) now");
    assert_eq!(rendered, "see docs now");
    let link = entities
        .iter()
        .find(|entity| entity.r#type == "text_link")
        .expect("a text_link entity");
    assert_eq!(link.url.as_deref(), Some("https://example.com/a?b=1"));
    assert_eq!(slice_utf16(&rendered, link.offset, link.length), "docs");
}

#[test]
fn images_keep_their_url_as_a_link() {
    let (rendered, entities) = checked("![alt text](https://example.com/a.png)");
    assert_eq!(rendered, "alt text");
    let link = entities
        .iter()
        .find(|entity| entity.r#type == "text_link")
        .expect("an image must survive as a link");
    assert_eq!(link.url.as_deref(), Some("https://example.com/a.png"));
}

#[test]
fn an_image_without_alt_text_falls_back_to_its_url() {
    let (rendered, entities) = checked("![](https://example.com/a.png)");
    assert_eq!(rendered, "https://example.com/a.png");
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].r#type, "text_link");
}

#[test]
fn inline_code_and_math_become_code_entities() {
    let (rendered, _) = checked("a `x` b $y$ c");
    assert_eq!(rendered, "a x b y c");
    assert_eq!(spans("a `x` b $y$ c", "code"), vec![(2, 1), (6, 1)]);
}

#[test]
fn code_blocks_become_pre_with_a_language() {
    let (rendered, entities) = checked("```python\nprint(1)\n```\n");
    assert_eq!(rendered, "print(1)\n");
    let pre = entities
        .iter()
        .find(|entity| entity.r#type == "pre")
        .expect("a pre entity");
    assert_eq!(pre.language.as_deref(), Some("python"));
    assert_eq!(slice_utf16(&rendered, pre.offset, pre.length), "print(1)");
}

#[test]
fn a_code_block_without_a_language_has_none() {
    let (_, entities) = checked("```\nx\n```\n");
    let pre = entities
        .iter()
        .find(|entity| entity.r#type == "pre")
        .expect("a pre entity");
    assert_eq!(pre.language, None);
}

#[test]
fn headings_render_their_marker_and_are_bold() {
    let (rendered, _) = checked("## Sub\n");
    assert_eq!(rendered, "## Sub\n");
    assert_eq!(spans("## Sub\n", "bold"), vec![(3, 3)]);
}

#[test]
fn bullets_and_numbering() {
    assert_eq!(text("- a\n- b\n"), "• a\n• b\n");
    assert_eq!(text("1. a\n2. b\n"), "1. a\n2. b\n");
    assert_eq!(text("- a\n  - b\n    - c\n"), "• a\n  ◦ b\n    ▪ c\n");
}

#[test]
fn task_items_render_checkboxes() {
    assert_eq!(text("- [ ] a\n- [x] b\n"), "☐ a\n☑ b\n");
    assert_eq!(text("1. [ ] a\n2. [x] b\n"), "1. ☐ a\n2. ☑ b\n");
}

#[test]
fn blockquotes_become_blockquote_entities() {
    let (rendered, entities) = checked("> quoted\n");
    assert_eq!(rendered, "quoted\n");
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].r#type, "blockquote");
}

#[test]
fn thematic_break_renders_a_rule() {
    assert!(text("a\n\n---\n\nb").contains("──────────"));
}

#[test]
fn superscript_maps_to_unicode_when_every_character_can() {
    assert_eq!(text("x<sup>2</sup>"), "x²");
    assert_eq!(text("H<sub>2</sub>O"), "H₂O");
    assert_eq!(text("x<sup>n+1</sup>"), "xⁿ⁺¹");
}

#[test]
fn superscript_falls_back_to_tags_when_it_cannot_map() {
    assert_eq!(text("x<sup>Q</sup>"), "x<sup>Q</sup>");
}

#[test]
fn tables_render_as_monospace_code_lines() {
    let (rendered, entities) = checked("| a | b |\n|---|---|\n| 1 | 2 |\n");
    assert!(rendered.contains('a') && rendered.contains('1'));
    assert!(
        !rendered.contains('|'),
        "pipes must be replaced by box drawing"
    );
    assert!(
        entities.iter().all(|entity| entity.r#type == "code"),
        "table lines are code spans: {entities:?}"
    );
    for entity in &entities {
        let line = slice_utf16(&rendered, entity.offset, entity.length);
        assert!(!line.contains('\n'), "a code span must not span lines");
    }
}

#[test]
fn text_after_a_table_is_preserved() {
    let rendered = text("| a |\n|---|\n| 1 |\nafter\n");
    assert!(rendered.ends_with("after\n"), "{rendered:?}");
}

#[test]
fn entity_invariants_hold_for_a_mixed_document() {
    let document = "\
# Report

Intro with **bold**, *italic*, `code`, $math$ and a [link](https://example.com).

- one
- two
  - nested
- [x] done

> quoted **text**

| col a | col b |
|-------|-------|
| 1     | 2     |

```rust
fn main() {}
```

x<sup>2</sup> + H<sub>2</sub>O

---

![diagram](https://example.com/d.png)
";
    let (rendered, entities) = checked(document);
    assert_ne!(rendered, "");
    assert!(
        entities.iter().any(|entity| entity.r#type == "pre"),
        "the fenced block must survive"
    );
    assert!(
        entities
            .iter()
            .any(|entity| entity.url.as_deref() == Some("https://example.com/d.png")),
        "the image url must survive"
    );
}

#[test]
fn no_input_produces_no_output() {
    let (rendered, entities) = checked("");
    assert_eq!(rendered, "");
    assert_eq!(entities, []);
}
