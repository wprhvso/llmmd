from typing import TypedDict

class MessageEntity(TypedDict):
    type: str
    offset: int
    length: int
    url: str | None
    language: str | None

class MessageChunk(TypedDict):
    text: str
    entities: list[MessageEntity]

def process_markdown(markdown: str, with_photo: bool = False) -> list[MessageChunk]: ...
