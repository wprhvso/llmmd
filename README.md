# llmmd

Превращает markdown, написанный LLM, в текст и entities для Telegram Bot API. Режет длинные ответы на сообщения по лимиту.

## Установка

```
pip install llmmd
```

## Использование

```python
from llmmd import process_markdown

for chunk in process_markdown("# Привет\n\nЭто **жирный** текст."):
    bot.send_message(chat_id, chunk["text"], entities=chunk["entities"])
```

Для подписи к фото лимит меньше — передайте `with_photo=True`:

```python
chunks = process_markdown(text, with_photo=True)
```

Лимиты доступны как `llmmd.MESSAGE_LIMIT` и `llmmd.CAPTION_LIMIT`.

## CLI

```
llmmd file.md
cat file.md | llmmd --with-photo
```

Печатает JSON со списком чанков.

## Лицензия

MIT
