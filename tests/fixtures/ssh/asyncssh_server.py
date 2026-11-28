"""Disposable AsyncSSH server for the Docker interop matrix."""

from __future__ import annotations

import asyncio
import hmac
import logging
import os
from pathlib import Path
from typing import TYPE_CHECKING

import asyncssh

if TYPE_CHECKING:
    from collections.abc import AsyncIterator

FIXTURE = Path("/fixture")
PASSWORD = (FIXTURE / "password").read_text().strip()
HOME = Path("/home/tfm")
LOG = logging.getLogger(__name__)
UNPRIVILEGED = 65534


class _Login(asyncssh.SSHServer):
    def begin_auth(self, username: str) -> bool:
        if username == "tfm":
            self._conn.set_authorized_keys(str(FIXTURE / "authorized_keys"))
        return True

    def connection_made(self, conn: asyncssh.SSHServerConnection) -> None:
        self._conn = conn

    def password_auth_supported(self) -> bool:
        return True

    def validate_password(self, username: str, password: str) -> bool:
        return username == "tfm" and hmac.compare_digest(password, PASSWORD)


class _Files(asyncssh.SFTPServer):
    async def scandir(self, path: bytes) -> AsyncIterator[asyncssh.SFTPName]:
        async for entry in super().scandir(path):
            if entry.filename == b"missing-metadata":
                entry.attrs = asyncssh.SFTPAttrs(permissions=entry.attrs.permissions)
            yield entry


async def _main() -> None:
    # Keep the root-owned denied directory inaccessible to the fixture server.
    os.setgroups([])
    os.setgid(UNPRIVILEGED)
    os.setuid(UNPRIVILEGED)
    logging.basicConfig(level=logging.INFO)
    LOG.info("AsyncSSH %s", asyncssh.__version__)
    server = await asyncssh.create_server(
        _Login,
        port=2022,
        config=None,
        server_host_keys=[FIXTURE / "host"],
        sftp_factory=lambda chan: _Files(chan, chroot=os.fsencode(HOME)),
    )
    async with server:
        await server.wait_closed()


if __name__ == "__main__":
    asyncio.run(_main())
