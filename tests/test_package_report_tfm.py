"""TFM-specific source inventory and source-package regressions."""

from __future__ import annotations

import io
import tarfile
import tempfile
import unittest
from pathlib import Path

from test_package_report import initialise_repository, write_archive

from scripts import package_report as report


class TfmPackageTests(unittest.TestCase):
    """Verify the TFM inventory adaptations with controlled fixture indexes."""

    def setUp(self) -> None:
        """Create an isolated archive and a controlled disposable Git index."""
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.archive = self.root / "terminal-file-manager-0.1.0.crate"
        self.repository = self.root / "repository"
        initialise_repository(self.repository)
        self.required = report.required_paths(self.repository)

    def analyse(self, files: tuple[str, ...] | None = None) -> report.PackageResult:
        """Analyse a real controlled source archive with the TFM policy."""
        write_archive(self.archive, self.required if files is None else files)
        return report.analyse_archive(self.archive, repository=self.repository)

    def test_licenses_notices_and_readme_are_required(self) -> None:
        """Require both full licences, third-party notices and the public README."""
        for required in (
            "LICENSE-APACHE",
            "LICENSE-MIT",
            "THIRD_PARTY_NOTICES.md",
            "README.md",
        ):
            with self.subTest(required=required):
                files = tuple(path for path in self.required if path != required)
                self.assertEqual(self.analyse(files).missing_paths, (required,))

    def test_rust_integration_tests_are_required(self) -> None:
        """A tracked root Rust integration test must survive packaging."""
        files = tuple(path for path in self.required if path != "tests/cli.rs")
        self.assertEqual(self.analyse(files).missing_paths, ("tests/cli.rs",))

    def test_private_and_support_files_are_forbidden(self) -> None:
        """Private material, test support and build artefacts fail policy."""
        forbidden = (
            "specs/private.md",
            "plans/private.md",
            "reviews/private.md",
            "AGENTS.md",
            "scripts/check_complexity.py",
            "tests/test_support.py",
            "tests/ci/test.sh",
            "target/output",
            "unrelated.txt",
            "tests/nested/test.rs",
        )
        result = self.analyse(self.required + forbidden)
        self.assertEqual(result.forbidden_paths, tuple(sorted(forbidden)))

    def test_only_tracked_rust_tests_enter_dynamic_inventory(self) -> None:
        """Git discovery includes nested Rust modules and omits support files."""
        repository = self.root / "mixed-repository"
        initialise_repository(
            repository,
            (
                "src/main.rs",
                "tests/cli.rs",
                "tests/test_support.py",
                "tests/ci/a.sh",
                "tests/nested/a.rs",
                "plans/private.md",
            ),
        )
        self.assertEqual(
            report.repository_distribution_paths(repository),
            ("src/main.rs", "tests/cli.rs", "tests/nested/a.rs"),
        )

    def test_real_size_ceiling_boundaries(self) -> None:
        """Measure actual compressed file sizes below, at and above 9,500,000."""
        for size in (9_499_999, 9_500_000, 9_500_001):
            with self.subTest(size=size):
                write_archive(self.archive, self.required)
                with self.archive.open("ab") as stream:
                    stream.write(bytes(size - self.archive.stat().st_size))
                result = report.analyse_archive(
                    self.archive, repository=self.repository
                )
                self.assertEqual(result.size_bytes, size)
                self.assertEqual(result.size_ok, size <= report.PACKAGE_LIMIT_BYTES)
                self.assertEqual(
                    result.headroom_bytes, max(report.PACKAGE_LIMIT_BYTES - size, 0)
                )

    def test_member_types_are_rejected(self) -> None:
        """Hard links, devices and FIFOs cannot supply required files."""
        for member_type in (
            tarfile.LNKTYPE,
            tarfile.CHRTYPE,
            tarfile.BLKTYPE,
            tarfile.FIFOTYPE,
        ):
            with self.subTest(member_type=member_type):
                with tarfile.open(self.archive, "w:gz") as archive:
                    member = tarfile.TarInfo("package/src/main.rs")
                    member.type = member_type
                    member.linkname = "other"
                    archive.addfile(member)
                with self.assertRaisesRegex(report.ArchiveError, "unsupported"):
                    report.analyse_archive(self.archive, repository=self.repository)

    def test_absolute_and_backslash_paths_are_rejected(self) -> None:
        """Unsafe paths are operational errors, before policy rendering."""
        for name in ("/package/Cargo.toml", "package\\Cargo.toml"):
            with self.subTest(name=name):
                with tarfile.open(self.archive, "w:gz") as archive:
                    archive.addfile(tarfile.TarInfo(name), io.BytesIO())
                with self.assertRaises(report.ArchiveError):
                    report.analyse_archive(self.archive, repository=self.repository)

    def test_source_report_marker_and_wording(self) -> None:
        """Identify this source crate independently from binary release archives."""
        markdown = report.render_markdown(self.analyse())
        self.assertEqual(markdown.splitlines()[0], "<!-- tfm-package-checks -->")
        self.assertIn("Source package archive:", markdown)
        self.assertNotIn("Published package:", markdown)
