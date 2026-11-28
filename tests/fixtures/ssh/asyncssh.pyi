# Typing surface of the Docker fixture's AsyncSSH dependency.

from collections.abc import AsyncIterator, Callable
from pathlib import Path
from types import TracebackType

__version__: str

class SSHServerConnection:
    def set_authorized_keys(self, authorized_keys: str) -> None: ...

class SSHServer: ...

class SFTPAttrs:
    def __init__(self, *, permissions: int | None = None) -> None: ...
    permissions: int | None

class SFTPName:
    filename: bytes
    attrs: SFTPAttrs

class SFTPServer:
    def __init__(self, chan: object, chroot: bytes) -> None: ...
    def scandir(self, path: bytes) -> AsyncIterator[SFTPName]: ...

class SSHAcceptor:
    async def __aenter__(self) -> object: ...
    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc_value: BaseException | None,
        traceback: TracebackType | None,
    ) -> None: ...
    async def wait_closed(self) -> None: ...

async def create_server(
    server_factory: Callable[[], SSHServer],
    *,
    port: int,
    config: None,
    server_host_keys: list[Path],
    sftp_factory: Callable[[object], SFTPServer],
) -> SSHAcceptor: ...
