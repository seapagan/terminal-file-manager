"""Positive and adversarial distribution fixtures, without member extraction."""

from __future__ import annotations

import gzip
import io
import os
import tarfile
import tempfile
import unittest
import warnings
import zipfile
from pathlib import Path
from unittest import mock

from scripts import check_complexity as checker
from scripts import verify_release_assets as assets


def executable(target: str) -> bytes:
    """Create structural headers; these are not runnable-binary evidence."""
    header = bytearray(256)
    arm = target.startswith("aarch64")
    if "linux" in target:
        header[:7] = b"\x7fELF\x02\x01\x01"
        header[16:18] = (2).to_bytes(2, "little")
        header[18:20] = (183 if arm else 62).to_bytes(2, "little")
    elif "darwin" in target:
        header[:4] = b"\xcf\xfa\xed\xfe"
        header[4:8] = (0x100000C if arm else 0x1000007).to_bytes(4, "little")
        header[12:16] = (2).to_bytes(4, "little")
    else:
        header[:2] = b"MZ"
        header[60:64] = (128).to_bytes(4, "little")
        header[128:132] = b"PE\0\0"
        header[132:134] = (0xAA64 if arm else 0x8664).to_bytes(2, "little")
    return bytes(header)


class ReleaseAssetsTests(unittest.TestCase):
    """Require complete identity for tar/ZIP and reject unsafe metadata."""

    def setUp(self) -> None:
        """Create isolated fixture storage and a checkout-byte document set."""
        temporary = tempfile.TemporaryDirectory(prefix="tfm-release-assets-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.target = assets.TARGETS[0]
        self.payload = executable(self.target)
        self.binary = self.root / "built"
        self.archive = self.root / assets.expected_name(self.target, "0.1.0")

    def members(self) -> list[tuple[str, bytes]]:
        """Return exactly the approved files using genuine checkout docs."""
        name = "tfm.exe" if "windows" in self.target else "tfm"
        return [
            (name, self.payload),
            *[(name, (assets.ROOT / name).read_bytes()) for name in assets.DOCUMENTS],
        ]

    def write(
        self,
        members: list[tuple[str, bytes]] | None = None,
        kind: bytes = tarfile.REGTYPE,
        mode: int = 0o755,
    ) -> None:
        """Build an intentionally controlled archive and valid digest."""
        entries = self.members() if members is None else members
        self.archive = self.root / assets.expected_name(self.target, "0.1.0")
        self.binary.write_bytes(self.payload)
        if "windows" in self.target:
            with (
                warnings.catch_warnings(),
                zipfile.ZipFile(self.archive, "w") as package,
            ):
                warnings.simplefilter("ignore", UserWarning)
                for name, data in entries:
                    info = zipfile.ZipInfo(name)
                    info.external_attr = mode << 16
                    package.writestr(info, data)
        else:
            with tarfile.open(
                self.archive, "w:gz", format=tarfile.USTAR_FORMAT
            ) as package:
                for name, data in entries:
                    tar_info = tarfile.TarInfo(name)
                    tar_info.size = len(data)
                    tar_info.mode = mode
                    tar_info.type = kind
                    package.addfile(tar_info, io.BytesIO(data))
        self.sidecar()

    def sidecar(self) -> Path:
        """Refresh a strict record so malicious content reaches validation."""
        path = Path(f"{self.archive}.sha256")
        path.write_bytes(
            f"{assets.digest_file(self.archive)}  {self.archive.name}\n".encode("ascii")
        )
        return path

    def check(self, *, aggregate: bool = False) -> None:
        """Validate with build identity unless modelling foreign aggregation."""
        assets.verify(
            self.archive, self.target, "0.1.0", None if aggregate else self.binary
        )

    def checkout_documents(self) -> tuple[Path, Path]:
        """Exercise real Git conversions using only owned index/config storage."""
        repository = self.root / "repository"
        repository.mkdir()
        for name in assets.DOCUMENTS:
            (repository / name).write_bytes((assets.ROOT / name).read_bytes())
        (repository / "src").mkdir()
        (repository / "src/main.rs").write_bytes(b"fn main() {}\n")
        (repository / "install.sh").write_bytes(b"#!/bin/sh\n")
        attributes = assets.ROOT / ".gitattributes"
        if attributes.exists():
            (repository / attributes.name).write_bytes(attributes.read_bytes())
        environment = {
            "GIT_DIR": str(repository / ".git"),
            "GIT_WORK_TREE": str(repository),
            "GIT_INDEX_FILE": str(repository / ".git" / "owned-index"),
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_COUNT": "0",
            "GIT_ATTR_NOSYSTEM": "1",
        }
        windows, linux = self.root / "windows", self.root / "linux"
        with mock.patch.dict(os.environ, environment):
            checker._run(["git", "init", "-q"], cwd=repository)
            checker._run(
                ["git", "-c", "core.autocrlf=input", "add", "--", "."],
                cwd=repository,
            )
            for setting, checkout in (("true", windows), ("false", linux)):
                checker._run(
                    [
                        "git",
                        "-c",
                        f"core.autocrlf={setting}",
                        "checkout-index",
                        "--all",
                        f"--prefix={checkout.as_posix()}/",
                    ],
                    cwd=repository,
                )
        return windows, linux

    def test_cross_host_checkout_document_identity(self) -> None:
        """One Windows ZIP must retain literal identity across Git EOL defaults."""
        windows, linux = self.checkout_documents()
        for name in (*assets.DOCUMENTS, "src/main.rs", "install.sh"):
            self.assertEqual((windows / name).read_bytes(), (linux / name).read_bytes())
            self.assertNotIn(b"\r\n", (windows / name).read_bytes())
        self.target = "x86_64-pc-windows-msvc"
        self.payload = executable(self.target)
        members = [
            ("tfm.exe", self.payload),
            *[(name, (windows / name).read_bytes()) for name in assets.DOCUMENTS],
        ]
        self.write(members)
        for checkout in (windows, linux):
            with mock.patch.object(assets, "ROOT", checkout):
                self.check()
                self.check(aggregate=True)
        name, data = members[1]
        members[1] = (name, data[:-1] + bytes([data[-1] ^ 1]))
        self.write(members)
        with (
            mock.patch.object(assets, "ROOT", linux),
            self.assertRaisesRegex(assets.AssetError, "checkout bytes mismatch"),
        ):
            self.check(aggregate=True)

    def test_all_eight_targets_and_aggregate(self) -> None:
        """Each synthetic machine header satisfies its exact declared target."""
        for self.target in assets.TARGETS:
            with self.subTest(target=self.target):
                self.payload = executable(self.target)
                self.write()
                self.check()
                self.check(aggregate=True)

    def test_checksum_grammar(self) -> None:
        """Reject fields, aliases, encoding ambiguity and additional records."""
        self.write()
        sidecar = self.sidecar()
        record = sidecar.read_bytes()
        for invalid in (
            record * 2,
            record.replace(b"  ", b" "),
            record + b"extra",
            record.replace(b"terminal-file-manager", b"tfm"),
            b"\xef\xbb\xbf" + record,
            record.replace(b"\n", b"\r\n"),
            b"g" + record[1:],
            record[:64] + b" *" + record[66:],
        ):
            with self.subTest(record=invalid):
                sidecar.write_bytes(invalid)
                with self.assertRaises(assets.AssetError):
                    self.check()
        sidecar.write_bytes(record.rstrip(b"\n"))
        self.check()

    def test_fixture_sidecar_ascii_lf_record(self) -> None:
        """Fixture records retain exact wire bytes under Windows text defaults."""
        self.write()
        expected = f"{assets.digest_file(self.archive)}  {self.archive.name}\n".encode(
            "ascii"
        )
        self.assertEqual(Path(f"{self.archive}.sha256").read_bytes(), expected)
        self.check()

    def test_checksum_corruption_and_missing_sidecar(self) -> None:
        """Tampering fails before any archive parser runs."""
        self.write()
        self.archive.write_bytes(self.archive.read_bytes() + b"changed")
        with mock.patch.object(assets, "verify_tar") as parser:
            with self.assertRaisesRegex(assets.AssetError, "SHA256 mismatch"):
                self.check()
            parser.assert_not_called()
        Path(f"{self.archive}.sha256").unlink()
        with self.assertRaises(OSError):
            self.check()

    def test_wrong_archive_name_target_and_version(self) -> None:
        """Package names and stable version/target resolution are exact."""
        for target, version in (
            ("unknown", "0.1.0"),
            (self.target, "v0.1.0"),
            (self.target, "0.1.0-rc.1"),
        ):
            with (
                self.subTest(target=target, version=version),
                self.assertRaises(assets.AssetError),
            ):
                assets.expected_name(target, version)
        self.write()
        with self.assertRaises(assets.AssetError):
            assets.verify(self.archive, assets.TARGETS[1], "0.1.0")
        with self.assertRaises(assets.AssetError):
            assets.verify(self.archive, self.target, "0.2.0")

    def test_member_names_and_missing_files(self) -> None:
        """Reject aliases, duplicates, traversal, Windows paths and omissions."""
        for self.target in (assets.TARGETS[0], assets.TARGETS[-1]):
            self.payload = executable(self.target)
            members = self.members()
            for name in (
                "extra",
                "tfm/wrong",
                "./tfm",
                "../outside",
                "/tfm",
                "C:\\tfm.exe",
                "tfm.exe:stream",
                "TFM.exe",
                "tfm.exe.",
                "tfm.exe\x00alias",
            ):
                with self.subTest(target=self.target, name=name):
                    # ZIP writers normalize NUL; check the raw name contract too.
                    with self.assertRaises(assets.AssetError):
                        assets.check_member(name, set(), "tfm")
                    if "\x00" not in name:
                        self.write([*members, (name, b"unsafe")])
                        with self.assertRaises(assets.AssetError):
                            self.check()
            for entries in (members[:-1], members[1:], [*members, members[0]]):
                self.write(entries)
                with self.assertRaises(assets.AssetError):
                    self.check()

    def test_tar_special_types_and_mode(self) -> None:
        """Links, devices, FIFO, directories and extension headers fail closed."""
        for kind in (
            tarfile.SYMTYPE,
            tarfile.LNKTYPE,
            tarfile.CHRTYPE,
            tarfile.BLKTYPE,
            tarfile.FIFOTYPE,
            tarfile.DIRTYPE,
            tarfile.XHDTYPE,
            tarfile.GNUTYPE_LONGNAME,
        ):
            self.write(kind=kind)
            with self.subTest(kind=kind), self.assertRaises(assets.AssetError):
                self.check()
        for mode in (0o644, 0o4755):
            self.write(mode=mode)
            with self.assertRaises(assets.AssetError):
                self.check()

    def test_zip_special_types(self) -> None:
        """Unix link/device types cannot masquerade as Windows regular files."""
        self.target = assets.TARGETS[-1]
        self.payload = executable(self.target)
        for mode in (0o120777, 0o040755, 0o010644, 0o020644, 0o104755):
            self.write(mode=mode)
            with self.subTest(mode=mode), self.assertRaises(assets.AssetError):
                self.check()
        self.write(mode=0o100644)
        self.check()
        original = self.archive.read_bytes()
        central = original.index(b"PK\x01\x02")
        for flag in (0x10, 0x40, 0x400):
            raw = bytearray(original)
            attributes = int.from_bytes(raw[central + 38 : central + 42], "little")
            raw[central + 38 : central + 42] = (attributes | flag).to_bytes(4, "little")
            self.archive.write_bytes(raw)
            self.sidecar()
            with self.assertRaises(assets.AssetError):
                self.check()

    def test_document_and_binary_identity(self) -> None:
        """Same-length alterations and missing bytes invalidate the package."""
        for self.target in (assets.TARGETS[0], assets.TARGETS[-1]):
            self.payload = executable(self.target)
            members = self.members()
            for index, (name, data) in enumerate(members):
                changed = members.copy()
                changed[index] = (name, data[:-1] + bytes([data[-1] ^ 1]))
                self.write(changed)
                with self.subTest(name=name), self.assertRaises(assets.AssetError):
                    self.check()
            self.write()
            self.binary.write_bytes(b"wrong")
            with self.assertRaises(assets.AssetError):
                self.check()

    def test_wrong_machine_format_and_offsets(self) -> None:
        """Architecture checks remain independent of optional build comparison."""
        for self.target in assets.TARGETS:
            wrong = (
                self.target.replace("x86_64", "aarch64")
                if self.target.startswith("x86")
                else self.target.replace("aarch64", "x86_64")
            )
            for self.payload in (executable(wrong), bytes(256), b"short"):
                self.write()
                with (
                    self.subTest(target=self.target),
                    self.assertRaises(assets.AssetError),
                ):
                    self.check(aggregate=True)
        self.target = assets.TARGETS[-1]
        for offset in (0, 255, 0xFFFFFFFF):
            header = bytearray(executable(self.target))
            header[60:64] = offset.to_bytes(4, "little")
            self.payload = bytes(header)
            self.write()
            with self.assertRaises(assets.AssetError):
                self.check(aggregate=True)

    def test_truncated_malformed_archives(self) -> None:
        """Valid hashes do not rescue invalid compression or header records."""
        for self.target in (assets.TARGETS[0], assets.TARGETS[-1]):
            self.payload = executable(self.target)
            self.write()
            original = self.archive.read_bytes()
            for malformed in (b"", b"not an archive", original[: len(original) // 2]):
                self.archive.write_bytes(malformed)
                self.sidecar()
                with (
                    self.subTest(target=self.target),
                    self.assertRaises(
                        (
                            assets.AssetError,
                            OSError,
                            EOFError,
                            tarfile.TarError,
                            zipfile.BadZipFile,
                        )
                    ),
                ):
                    self.check()

    def test_tar_end_and_padding(self) -> None:
        """Reject truncated end markers, hidden trailing headers and bad padding."""
        self.write()
        original = gzip.decompress(self.archive.read_bytes())
        for raw in (
            original[:511],
            original + b"hidden",
            original[:768] + b"x" + original[769:],
        ):
            self.archive.write_bytes(gzip.compress(raw))
            self.sidecar()
            with self.assertRaises((assets.AssetError, tarfile.TarError)):
                self.check()

    def test_executable_size_limit_before_payload_reads(self) -> None:
        """Tar and ZIP ceilings apply with and without a verified build."""
        for self.target in (assets.TARGETS[0], assets.TARGETS[-1]):
            for size, accepted in ((256, True), (257, False)):
                self.payload = executable(self.target) + bytes(size - 256)
                self.write()
                with mock.patch.object(assets, "MAX_BINARY_SIZE", 256, create=True):
                    for aggregate in (False, True):
                        with self.subTest(
                            target=self.target, size=size, aggregate=aggregate
                        ):
                            if accepted:
                                self.check(aggregate=aggregate)
                            else:
                                with self.assertRaisesRegex(
                                    assets.AssetError, "executable size limit"
                                ):
                                    self.check(aggregate=aggregate)

    def test_archive_size_limit_before_hashing(self) -> None:
        """An oversized archive is rejected before its checksum consumes input."""
        for self.target in (assets.TARGETS[0], assets.TARGETS[-1]):
            self.payload = executable(self.target)
            self.write()
            size = self.archive.stat().st_size
            with mock.patch.object(assets, "MAX_ARCHIVE_SIZE", size, create=True):
                self.check()
            with (
                mock.patch.object(assets, "MAX_ARCHIVE_SIZE", size - 1, create=True),
                mock.patch.object(
                    assets,
                    "digest_file",
                    side_effect=AssertionError("oversized archive hashed"),
                ),
                self.assertRaisesRegex(assets.AssetError, "archive size limit"),
            ):
                self.check()

    def test_tar_zero_padding_limit(self) -> None:
        """Zero gzip trailers cannot bypass either source or aggregate ceilings."""
        self.write()
        original = gzip.decompress(self.archive.read_bytes())
        self.archive.write_bytes(gzip.compress(original + bytes(65537)))
        self.sidecar()
        for aggregate in (False, True):
            with (
                self.subTest(aggregate=aggregate),
                self.assertRaisesRegex(assets.AssetError, "tar padding limit"),
            ):
                self.check(aggregate=aggregate)

    def test_hidden_tar_alias_and_bad_header_checksum(self) -> None:
        """Physical name fields cannot hide aliases behind a NUL terminator."""
        self.write()
        original = gzip.decompress(self.archive.read_bytes())
        header = bytearray(original[:512])
        header[:9] = b"tfm\0alias"
        header[148:156] = b" " * 8
        checksum = sum(header)
        header[148:156] = f"{checksum:06o}\0 ".encode("ascii")
        self.archive.write_bytes(gzip.compress(bytes(header) + original[512:]))
        self.sidecar()
        with self.assertRaisesRegex(assets.AssetError, "hidden alias"):
            self.check()
        header[148:156] = b"000000\0 "
        self.archive.write_bytes(gzip.compress(bytes(header) + original[512:]))
        self.sidecar()
        with self.assertRaises(tarfile.TarError):
            self.check()

    def test_zip_crc_encryption_and_local_name(self) -> None:
        """Corrupt data, encrypted entries and conflicting local metadata fail."""
        self.target = assets.TARGETS[-1]
        self.payload = executable(self.target)
        self.write()
        original = self.archive.read_bytes()
        raw = bytearray(original)
        first_data = 30 + int.from_bytes(raw[26:28], "little")
        first_data += int.from_bytes(raw[28:30], "little")
        crc = raw.copy()
        crc[first_data] ^= 1
        encrypted = raw.copy()
        central = encrypted.index(b"PK\x01\x02")
        encrypted[central + 8] |= 1
        name = raw.copy()
        name[30] ^= 1
        for invalid in (crc, encrypted, name):
            self.archive.write_bytes(invalid)
            self.sidecar()
            with self.assertRaises((assets.AssetError, zipfile.BadZipFile)):
                self.check()

    def test_pe_header_across_read_boundaries(self) -> None:
        """PE offsets spanning chunks are inspected without unbounded reads."""
        self.target = assets.TARGETS[-1]
        offset = assets.CHUNK + 62
        original = executable(self.target)
        header = bytearray(offset + 6)
        header[:64] = original[:64]
        header[60:64] = offset.to_bytes(4, "little")
        header[offset : offset + 6] = original[128:134]
        self.payload = bytes(header)
        self.write()
        self.check()
