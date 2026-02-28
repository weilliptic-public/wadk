"""Streaming response from the platform (execute_with_streaming)."""

from typing import AsyncIterator


class ByteStream:
    """Async iterator of bytes from execute_with_streaming.

    Use with: async for chunk in byte_stream: ...
    """

    def __init__(self, stream: AsyncIterator[bytes]) -> None:
        self._stream = stream

    def __aiter__(self) -> AsyncIterator[bytes]:
        return self._stream
