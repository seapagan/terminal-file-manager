"""Verify rehearsal archives without extracting or executing their members."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import re
import stat
import sys
import tarfile
import zipfile
from pathlib import Path
from typing import NoReturn, Protocol

ROOT = Path(__file__).resolve().parents[1]
DOCUMENTS = ("LICENSE-MIT", "LICENSE-APACHE", "THIRD_PARTY_NOTICES.md", "README.md")
TARGETS = (
    "x86_64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-gnu",
    "aarch64-unknown-linux-musl",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
)
CHUNK = 65536
BLOCK = 512
PE_OFFSET = 60
HEADER_SIZE = 64
MACH_EXECUTABLE = 2
EXECUTABLE_BITS = 0o111
# Current rehearsal archives are below 220 KiB; retain ample release-size headroom.
MAX_ARCHIVE_SIZE = 256 * 1024 * 1024
MAX_BINARY_SIZE = 128 * 1024 * 1024
MAX_TAR_PADDING = 65536


class ByteReader(Protocol):
    """The bounded read interface shared by gzip, ZIP and ordinary files."""

    def read(self, size: int = -1, /) -> bytes:
        """Read at most the requested number of bytes."""
        ...


class AssetError(Exception):
    """The asset cannot establish the required distribution contract."""


def reject(message: str) -> NoReturn:
    """Report a contract failure consistently."""
    raise AssetError(message)


def digest_file(path: Path) -> str:
    """Hash a regular file with bounded reads."""
    with path.open("rb") as stream:
        digest = hashlib.sha256()
        while data := stream.read(CHUNK):
            digest.update(data)
    return digest.hexdigest()


def verify_sidecar(archive: Path) -> None:
    """Require precisely the portable two-space checksum record."""
    expected_length = 64 + 2 + len(archive.name)
    with Path(f"{archive}.sha256").open("rb") as stream:
        record = stream.read(expected_length + 2)
        extra = stream.read(1)
    pattern = rb"([0-9a-f]{64})  " + re.escape(archive.name.encode("ascii")) + rb"\n?"
    match = re.fullmatch(pattern, record)
    if extra or match is None:
        reject("malformed checksum record or wrong archive basename")
    if match[1].decode("ascii") != digest_file(archive):
        reject("archive SHA256 mismatch")


def expected_name(target: str, version: str) -> str:
    """Resolve only the eight approved stable-version asset names."""
    if (
        target not in TARGETS
        or re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version) is None
    ):
        reject("unsupported target or unstable version")
    extension = "zip" if "windows" in target else "tar.gz"
    return f"terminal-file-manager-v{version}-{target}.{extension}"


def check_member(name: str, seen: set[str], executable: str) -> None:
    """Reject aliases, unexpected paths and duplicate members."""
    if name not in (*DOCUMENTS, executable) or name in seen:
        reject(f"unexpected or duplicate member: {name!r}")
    seen.add(name)


def read_exact(stream: ByteReader, size: int) -> bytes:
    """Read only a bounded header or padding block, rejecting truncation."""
    data = stream.read(size)
    if len(data) != size:
        reject("truncated archive member")
    return data


def verify_machine(header: bytes, target: str) -> None:
    """Check bounded machine identity for the declared target."""
    arm = target.startswith("aarch64-")
    if "linux" in target:
        valid = valid_elf(header, arm=arm)
    elif "darwin" in target:
        valid = valid_macho(header, arm=arm)
    else:
        machine = 0xAA64 if arm else 0x8664
        valid = (
            header[:4] == b"PE\0\0" and int.from_bytes(header[4:6], "little") == machine
        )
    if not valid:
        reject("packaged executable has incorrect format or architecture")


def valid_elf(header: bytes, *, arm: bool) -> bool:
    """Require little-endian 64-bit executable ELF for the intended machine."""
    machine = 183 if arm else 62
    return (
        header[:7] == b"\x7fELF\x02\x01\x01"
        and int.from_bytes(header[16:18], "little") in (2, 3)
        and int.from_bytes(header[18:20], "little") == machine
    )


def valid_macho(header: bytes, *, arm: bool) -> bool:
    """Require a thin 64-bit executable Mach-O with its real file-type offset."""
    machine = 0x100000C if arm else 0x1000007
    return (
        header[:4] == b"\xcf\xfa\xed\xfe"
        and int.from_bytes(header[4:8], "little") == machine
        and int.from_bytes(header[12:16], "little") == MACH_EXECUTABLE
    )


def verify_payload(stream: ByteReader, size: int, target: str) -> str:
    """Hash a binary and capture its machine header using bounded reads."""
    if size < HEADER_SIZE:
        reject("truncated executable header")
    header = read_exact(stream, 64)
    digest = hashlib.sha256(header)
    offset = 0
    if "windows" in target:
        if header[:2] != b"MZ":
            reject("missing MZ signature")
        offset = int.from_bytes(header[PE_OFFSET:64], "little")
        if offset < HEADER_SIZE or offset + 6 > size:
            reject("invalid PE header offset")
    position = 64
    machine_header = bytearray()
    while position < size:
        data = read_exact(stream, min(CHUNK, size - position))
        digest.update(data)
        start = max(position, offset)
        end = min(position + len(data), offset + 6)
        if start < end:
            machine_header.extend(data[start - position : end - position])
        position += len(data)
    verify_machine(bytes(machine_header) if offset else header, target)
    return digest.hexdigest()


def verify_content(
    stream: ByteReader, name: str, size: int, target: str, binary: Path | None
) -> None:
    """Compare documents to checkout and the executable to its verified build."""
    if name in DOCUMENTS:
        expected = ROOT / name
        if size != expected.stat().st_size:
            reject(f"checkout size mismatch: {name}")
        with expected.open("rb") as checkout:
            remaining = size
            while remaining:
                amount = min(CHUNK, remaining)
                if read_exact(stream, amount) != read_exact(checkout, amount):
                    reject(f"checkout bytes mismatch: {name}")
                remaining -= amount
    else:
        if size > MAX_BINARY_SIZE:
            reject("executable size limit exceeded")
        if binary is not None and size != binary.stat().st_size:
            reject("built executable size mismatch")
        actual = verify_payload(stream, size, target)
        if binary is not None and actual != digest_file(binary):
            reject("packaged executable differs from verified build")


def verify_tar_end(stream: ByteReader) -> None:
    """Require a complete zero end marker and no hidden trailing records."""
    if read_exact(stream, BLOCK) != bytes(BLOCK):
        reject("invalid tar end marker")
    padding = stream.read(MAX_TAR_PADDING + 1)
    if len(padding) > MAX_TAR_PADDING:
        reject("tar padding limit exceeded")
    if any(padding):
        reject("data after tar end marker")


def verify_tar_header(block: bytes, member: tarfile.TarInfo) -> None:
    """Reject unsafe physical metadata before reading any member payload."""
    for field in (block[:100], block[345:500]):
        if b"\0" in field and any(field.split(b"\0", 1)[1]):
            reject("tar name contains a hidden alias")
    if member.type not in (tarfile.REGTYPE, tarfile.AREGTYPE) or member.linkname:
        reject("nonregular tar member")
    if member.size < 0 or member.mode & 0o7000:
        reject("unsafe tar size or mode")
    if member.name == "tfm" and member.mode & 0o111 != EXECUTABLE_BITS:
        reject("Unix executable mode missing")


def verify_tar(archive: Path, target: str, binary: Path | None) -> None:
    """Inspect physical tar headers, including otherwise hidden extension entries."""
    seen: set[str] = set()
    with gzip.open(archive, "rb") as stream:
        while True:
            block = read_exact(stream, BLOCK)
            if block == bytes(BLOCK):
                verify_tar_end(stream)
                break
            member = tarfile.TarInfo.frombuf(block, "utf-8", "strict")
            check_member(member.name, seen, "tfm")
            verify_tar_header(block, member)
            verify_content(stream, member.name, member.size, target, binary)
            if any(read_exact(stream, (-member.size) % BLOCK)):
                reject("nonzero tar member padding")
    if seen != {*DOCUMENTS, "tfm"}:
        reject("missing required tar members")


def verify_zip(archive: Path, target: str, binary: Path | None) -> None:
    """Inspect ZIP metadata and bounded content through standard library APIs."""
    seen: set[str] = set()
    with zipfile.ZipFile(archive) as package:
        for member in package.infolist():
            check_member(member.orig_filename, seen, "tfm.exe")
            mode = member.external_attr >> 16
            if (
                member.is_dir()
                or member.flag_bits & 1
                or member.external_attr & 0x450
                or stat.S_IFMT(mode) not in (0, stat.S_IFREG)
                or mode & 0o7000
            ):
                reject("unsafe ZIP member type or attributes")
            with package.open(member) as stream:
                verify_content(
                    stream, member.filename, member.file_size, target, binary
                )
                if stream.read(1):
                    reject("ZIP member size mismatch")
    if seen != {*DOCUMENTS, "tfm.exe"}:
        reject("missing required ZIP members")


def verify(
    archive: Path, target: str, version: str, binary: Path | None = None
) -> None:
    """Enforce complete source-side asset identity without foreign execution."""
    if archive.name != expected_name(target, version):
        reject("incorrect package, version or target archive basename")
    if archive.stat().st_size > MAX_ARCHIVE_SIZE:
        reject("archive size limit exceeded")
    verify_sidecar(archive)
    if "windows" in target:
        verify_zip(archive, target, binary)
    else:
        verify_tar(archive, target, binary)


def main() -> int:
    """Validate one archive/sidecar pair or return a fail-closed diagnostic."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", required=True, type=Path)
    parser.add_argument("--target", required=True, choices=TARGETS)
    parser.add_argument("--version", required=True)
    parser.add_argument("--binary", type=Path)
    args = parser.parse_args()
    try:
        verify(args.archive, args.target, args.version, args.binary)
    except (
        AssetError,
        OSError,
        EOFError,
        ValueError,
        tarfile.TarError,
        zipfile.BadZipFile,
        RuntimeError,
    ) as error:
        sys.stderr.write(f"release asset verification failed: {error}\n")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
