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
