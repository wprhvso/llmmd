from typing import Final

from llmmd.types import MessageChunk

MESSAGE_LIMIT: Final[int]
CAPTION_LIMIT: Final[int]
MAX_ENTITIES: Final[int]

def process_markdown(markdown: str, with_photo: bool = False) -> list[MessageChunk]: ...
