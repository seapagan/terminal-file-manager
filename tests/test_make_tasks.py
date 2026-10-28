"""Regressions for Cargo Make's resolved two-stage local verification graph."""

from __future__ import annotations

import os
import re
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from test_check_complexity import CHECKER
from test_package_report import initialise_repository

ROOT = Path(__file__).parents[1]
STRICT = {"msrv-version", "msrv", "package", "package-list"}
FAST = {"format", "check", "clippy", "test", "doctest", "doc", "build", "release-build"}
COMMON = FAST | {
    "python-format",
    "python-lint",
    "python-type",
    "complexity",
    "maintainability",
    "audit",
    "deny",
    "actions-lint",
    "zizmor",
    "coverage",
}


def steps(task: str) -> set[str]:
    """Ask Cargo Make to resolve aliases and transitive dependencies."""
    output: str = CHECKER._run(["cargo", "make", "--print-steps", task], cwd=ROOT)
    return set(re.findall(r'^            name: "([^"]+)"', output, re.MULTILINE))


class MakeTaskTests(unittest.TestCase):
    """Keep development gates usable before strict postcommit verification."""

    def test_fast_is_a_development_subset(self) -> None:
        """Resolve every retained fast dependency without packaging or MSRV."""
        self.assertEqual(steps("verify-fast"), FAST | {"init", "end", "verify-fast"})

    def test_host_development_gates_preserve_requirements(self) -> None:
        """Retain shared checks, host guards and mandatory Linux xwin."""
        for host in ("linux", "macos", "windows"):
            with self.subTest(host=host):
                resolved = steps(f"verify-{host}")
                self.assertLessEqual(COMMON | {f"_require-{host}"}, resolved)
                self.assertFalse(STRICT & resolved)
                if host != "windows":
                    self.assertLessEqual(
                        {"release-shellcheck", "installers-shellcheck"}, resolved
                    )
                if host == "linux":
                    self.assertLessEqual(
                        {"verify-xwin", "xwin-check", "xwin-clippy", "xwin-build"},
                        resolved,
                    )

    def test_quality_and_verify_resolve_the_same_host_graph(self) -> None:
        """The quality alias must retain the complete development gate."""
        verify = steps("verify") - {"verify"}
        self.assertEqual(steps("quality") - {"quality"}, verify)
        self.assertLessEqual(COMMON, verify)
        self.assertFalse(STRICT & verify)

    def test_postcommit_children_are_sequenced_inside_the_guarded_task(self) -> None:
        """Resolve only the wrapper; execution tests prove its ordered children."""
        self.assertEqual(
            steps("verify-postcommit"), {"init", "end", "verify-postcommit"}
        )


class PostcommitGuardTests(unittest.TestCase):
    """Execute the production guard and sequence against disposable Git trees."""

    def setUp(self) -> None:
        """Create a clean fixture commit without using developer Git settings."""
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.repository = self.root / "repository"
        self.calls = self.root / "children.log"
        environment = mock.patch.dict(
            os.environ,
            {
                "GIT_CONFIG_GLOBAL": os.devnull,
                "GIT_CONFIG_NOSYSTEM": "1",
            },
        )
        environment.start()
        self.addCleanup(environment.stop)
        initialise_repository(self.repository, ("tracked.txt", ".gitignore"))
        (self.repository / ".gitignore").write_text("ignored/\n", encoding="utf-8")
        self.git("add", ".gitignore")
        self.git(
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=" + str(self.root),
            "commit",
            "--quiet",
            "-m",
            "fixture",
        )

    def git(self, *args: str) -> str:
        """Run controlled Git commands only inside this disposable repository."""
        result: str = CHECKER._run(["git", *args], cwd=self.repository)
        return result

    def run_gate(self, failed: str = "", *, missing_git: bool = False) -> None:
        """Retain the actual wrapper and override only its expensive children."""
        makefile = self.root / "tasks.toml"
        source = f'extend = "{(ROOT / "Makefile.toml").as_posix()}"\n'
        for task in ("msrv-version", "msrv", "package"):
            clear = "" if task == "msrv" else "clear = true\n"
            source += (
                f"[tasks.{task}]\n{clear}"
                "script_runner = \"@duckscript\"\nscript = '''\n"
                f'appendfile "{self.calls.as_posix()}" "{task},"\n'
                f"exit {1 if task == failed else 0}\n'''\n"
            )
        if missing_git:
            source += '[tasks.verify-postcommit.env]\nPATH = ""\n'
        makefile.write_text(source, encoding="utf-8")
        cargo = shutil.which("cargo")
        self.assertIsNotNone(cargo)
        CHECKER._run(
            [cargo, "make", "--makefile", str(makefile), "verify-postcommit"],
            cwd=self.repository,
        )

    def test_clean_and_ignored_only_run_both_children_in_order(self) -> None:
        """Ignored local instructions do not prevent the strict checks."""
        self.run_gate()
        ignored = self.repository / "ignored"
        ignored.mkdir()
        (ignored / "AGENTS.md").write_text("local", encoding="utf-8")
        self.run_gate()
        self.assertEqual(
            self.calls.read_text(),
            "msrv-version,msrv,package,msrv-version,msrv,package,",
        )

    def test_unstaged_tracked_changes_stop_before_children(self) -> None:
        """Refuse tracked changes even when Cargo would exclude that file."""
        (self.repository / "tracked.txt").write_text("changed", encoding="utf-8")
        self.assert_guard_refuses()

    def test_staged_only_changes_stop_before_children(self) -> None:
        """Refuse an index change with no unstaged difference."""
        (self.repository / "tracked.txt").write_text("changed", encoding="utf-8")
        self.git("add", "tracked.txt")
        self.assert_guard_refuses()

    def test_untracked_source_stops_before_children(self) -> None:
        """Refuse a new nonignored source file that packaging might overlook."""
        (self.repository / "new.rs").write_text("new", encoding="utf-8")
        self.assert_guard_refuses()

    def assert_guard_refuses(self) -> None:
        """Require the actionable diagnostic and no child side effects."""
        with self.assertRaisesRegex(CHECKER.CheckerError, "clean committed checkout"):
            self.run_gate()
        self.assertFalse(self.calls.exists())

    def test_git_inspection_error_stops_before_children(self) -> None:
        """A status error after HEAD inspection must not become clean success."""
        (self.repository / ".git" / "index").write_bytes(b"invalid index")
        with self.assertRaisesRegex(CHECKER.CheckerError, "cannot inspect checkout"):
            self.run_gate()
        self.assertFalse(self.calls.exists())

    def test_missing_git_stops_before_children(self) -> None:
        """The guard fails closed when Git cannot be resolved."""
        with self.assertRaisesRegex(CHECKER.CheckerError, "git is unavailable"):
            self.run_gate(missing_git=True)
        self.assertFalse(self.calls.exists())

    def test_unborn_checkout_stops_before_children(self) -> None:
        """An empty status is insufficient without a committed HEAD."""
        self.repository = self.root / "unborn"
        self.repository.mkdir()
        self.git("init", "--quiet")
        with self.assertRaisesRegex(CHECKER.CheckerError, "no committed HEAD"):
            self.run_gate()
        self.assertFalse(self.calls.exists())

    def test_either_child_failure_stops_the_sequence(self) -> None:
        """MSRV failure blocks package; package failure also fails the aggregate."""
        for failed, expected in (
            ("msrv", "msrv-version,msrv,"),
            ("package", "msrv-version,msrv,package,"),
        ):
            with self.subTest(failed=failed):
                self.calls.unlink(missing_ok=True)
                with self.assertRaises(CHECKER.CheckerError):
                    self.run_gate(failed)
                self.assertEqual(self.calls.read_text(), expected)
