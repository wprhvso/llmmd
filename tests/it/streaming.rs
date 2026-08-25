use crate::support::{
    Rng,
    actions,
    actions_at_boundaries,
    assert_entities_valid,
    corpus_seeded,
    events_chunked,
    merge_text,
    render,
    render_actions,
    render_chunked,
    resolve,
};

const CHUNK_SIZES: &[usize] = &[1, 2, 3, 4, 5, 7, 11, 17, 64, 1024];

const DOCUMENTS: &[&str] = &[
    "# Title\n\nHello **bold** and *italic*.\n",
    "- a\n- b\n  - c\n",
    "1. one\n2. two\n   1. nested\n",
    "```python\nprint(1)\n```\ntail\n",
    "```\nno language\n``\nstill inside\n```\n",
    "> quote\n> more\n>> deeper\n",
    "text with `code` and [a link](https://example.com/x)\n",
    "![alt](https://example.com/i.png) after the image\n",
    "$$x+1$$\n",
    "inline $x^2$ math and \\(y\\) too\n",
    "\\[\nblock math\n\\]\n",
    "x<sup>2</sup> + y<sub>1</sub>\n",
    "<u>underlined</u> and <notatag> and <sup>x?</sup>\n",
    "***bold italic*** ~~struck~~ ||spoiler||\n",
    "| a | b |\n|---|---|\n| 1 | 2 |\n\nafter\n",
    "| a | b |\n|---|---|\n| **x** | [y](https://e.com) |\n",
    "before\n\n---\n\nafter\n",
    "- [ ] todo\n- [x] done\n",
    "a table that never ends\n\n| h |\n|---|\n| v |",
    "unclosed **bold and `code",
    "мультибайт 😀 и **жирный** текст\n",
    "line\n\n\n\nmany blanks\n",
];

fn all_documents() -> Vec<String> {
    let mut documents: Vec<String> = DOCUMENTS.iter().map(|text| (*text).to_string()).collect();
    documents.extend(corpus_seeded(0xC0FF_EE01, 600));
    documents
}

#[test]
fn a_document_renders_the_same_however_it_is_chunked() {
    for document in all_documents() {
        let whole = render(&document);
        for size in CHUNK_SIZES {
            assert_eq!(
                render_chunked(&document, *size),
                whole,
                "{document:?} rendered differently in {size}-char chunks"
            );
        }
    }
}

#[test]
fn a_document_renders_the_same_at_random_split_points() {
    let mut rng = Rng::new(0x5911_7C0D);
    for document in corpus_seeded(0xBEEF_0042, 400) {
        let length = document.chars().count();
        let whole = render(&document);
        for _ in 0..4 {
            let cuts: Vec<usize> = (0..4)
                .map(|_| rng.below(length.saturating_add(1)))
                .collect();
            let rendered = render_actions(actions_at_boundaries(&document, &cuts));
            assert_eq!(
                rendered, whole,
                "{document:?} rendered differently when split at {cuts:?}"
            );
        }
    }
}

#[test]
fn events_are_the_same_however_the_input_is_chunked() {
    for document in all_documents() {
        let whole = merge_text(resolve(actions(&document)));
        for size in CHUNK_SIZES {
            assert_eq!(
                merge_text(events_chunked(&document, *size)),
                whole,
                "{document:?} produced different events in {size}-char chunks"
            );
        }
    }
}

#[test]
fn entities_stay_valid_when_the_input_arrives_in_pieces() {
    for document in corpus_seeded(0x1234_5678, 400) {
        for size in [1_usize, 3, 9] {
            let (text, entities) = render_chunked(&document, size);
            assert_entities_valid(&text, &entities);
        }
    }
}
