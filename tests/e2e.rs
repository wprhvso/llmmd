mod common;

use _native::{
    limits::{CAPTION_LIMIT, MESSAGE_LIMIT},
    to_telegram::{MessageEntity, process_llm_markdown_sync},
};
use common::{assert_entities_valid, slice_utf16};

const ANSWER: &str = "\
# Отчёт

Кратко: **всё готово**, но есть *нюансы*.

## Что сделано

1. Разобран парсер
2. Написаны тесты
   - модульные
   - интеграционные

- [x] собрать
- [ ] выкатить

> Замечание: лимит Telegram — 4096 символов.

Формула: $E = mc^2$, а блок:

$$
a^2 + b^2 = c^2
$$

Код:

```python
def main() -> int:
    return 0
```

| Метрика | Значение |
|---------|----------|
| Тесты   | 128      |
| Время   | 3s       |

Подробности в [документации](https://example.com/docs), картинка ![схема](https://example.com/d.png).

Ещё ~~зачёркнуто~~, ||спойлер|| и <u>подчёркнуто</u>, x<sup>2</sup> и H<sub>2</sub>O.

---

Конец.
";

fn only_chunk(markdown: &str) -> (String, Vec<MessageEntity>) {
    let mut chunks = process_llm_markdown_sync(markdown, false);
    assert_eq!(chunks.len(), 1, "the answer should fit a single message");
    chunks.remove(0)
}

fn entity_covering<'a>(
    text: &str,
    entities: &'a [MessageEntity],
    kind: &str,
    expected: &str,
) -> &'a MessageEntity {
    entities
        .iter()
        .find(|entity| {
            entity.r#type == kind && slice_utf16(text, entity.offset, entity.length) == expected
        })
        .unwrap_or_else(|| panic!("no {kind} entity covering {expected:?} in {text:?}"))
}

#[test]
fn a_realistic_answer_keeps_its_entities_valid() {
    let (text, entities) = only_chunk(ANSWER);
    assert_entities_valid(&text, &entities);
    assert_ne!(entities.len(), 0);
}

#[test]
fn a_realistic_answer_loses_every_markup_marker() {
    let (text, _) = only_chunk(ANSWER);
    for marker in ["**", "~~", "||", "<u>", "</u>", "```", "](", "$$"] {
        assert!(
            !text.contains(marker),
            "{marker:?} survived into the message text: {text:?}"
        );
    }
}

#[test]
fn a_realistic_answer_maps_every_construct_to_an_entity() {
    let (text, entities) = only_chunk(ANSWER);

    entity_covering(&text, &entities, "bold", "всё готово");
    entity_covering(&text, &entities, "italic", "нюансы");
    entity_covering(&text, &entities, "strikethrough", "зачёркнуто");
    entity_covering(&text, &entities, "spoiler", "спойлер");
    entity_covering(&text, &entities, "underline", "подчёркнуто");
    entity_covering(&text, &entities, "code", "E = mc^2");
    entity_covering(&text, &entities, "bold", "Отчёт");
    entity_covering(&text, &entities, "bold", "Что сделано");

    let link = entity_covering(&text, &entities, "text_link", "документации");
    assert_eq!(link.url.as_deref(), Some("https://example.com/docs"));

    let image = entity_covering(&text, &entities, "text_link", "схема");
    assert_eq!(image.url.as_deref(), Some("https://example.com/d.png"));

    let code = entities
        .iter()
        .find(|entity| entity.language.as_deref() == Some("python"))
        .expect("the fenced block keeps its language");
    assert_eq!(code.r#type, "pre");
    assert!(slice_utf16(&text, code.offset, code.length).contains("return 0"));

    assert!(
        entities.iter().any(|entity| entity.r#type == "blockquote"),
        "the quote became a blockquote entity"
    );
}

#[test]
fn a_realistic_answer_renders_its_block_structure() {
    let (text, _) = only_chunk(ANSWER);

    assert!(text.contains("# Отчёт"));
    assert!(text.contains("## Что сделано"));
    assert!(text.contains("1. Разобран парсер"));
    assert!(text.contains("2. Написаны тесты"));
    assert!(text.contains("модульные"));
    assert!(text.contains("☑ собрать"));
    assert!(text.contains("☐ выкатить"));
    assert!(text.contains("──────────"));
    assert!(text.contains("x² и H₂O"));
    assert!(text.contains('│'), "the table is drawn with box characters");
    assert!(text.contains("Метрика"));
    assert!(text.contains("128"));
    assert!(text.contains("Конец."));
}

#[test]
fn a_long_answer_is_delivered_as_several_valid_messages() {
    let document = ANSWER.repeat(20);
    let chunks = process_llm_markdown_sync(&document, false);
    assert!(chunks.len() > 1);

    for (text, entities) in &chunks {
        assert!(text.encode_utf16().count() <= MESSAGE_LIMIT);
        assert_ne!(text.trim(), "");
        assert_entities_valid(text, entities);
    }
}

#[test]
fn a_photo_caption_uses_the_smaller_limit() {
    let chunks = process_llm_markdown_sync(ANSWER, true);
    assert_ne!(chunks.len(), 0);

    for (text, entities) in &chunks {
        assert!(text.encode_utf16().count() <= CAPTION_LIMIT);
        assert_entities_valid(text, entities);
    }
}
