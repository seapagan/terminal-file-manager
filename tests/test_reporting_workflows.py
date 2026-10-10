"""Isolated execution of reporting workflow shells and comment ownership."""

from __future__ import annotations

import json
import os
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from test_check_complexity import CHECKER

ROOT = Path(__file__).parents[1]
REPORTS = ("package", "quality")
GH_MOCK = """#!/usr/bin/env bash
set -euo pipefail
printf '%s\\n' "$@" >> "$MOCK_CALLS"
if [[ "$*" == *'--method '* ]]; then exit 0; fi
while (( $# > 0 )); do
    if [[ "$1" == '--jq' ]]; then
        jq -r "$2" "$MOCK_COMMENTS"
        exit 0
    fi
    shift
done
exit 1
"""


def workflow(name: str) -> str:
    """Read the actual TFM reporting workflow."""
    return (ROOT / ".github" / "workflows" / f"{name}.yml").read_text("utf-8")


def step_script(source: str, name: str) -> str:
    """Extract one literal production shell step without a YAML dependency."""
    step = source.split(f"      - name: {name}\n", 1)[1]
    step = step.split("\n      - name:", 1)[0]
    body = step.split("        run: ", 1)[1]
    if body.startswith("|\n"):
        return "\n".join(line[10:] for line in body.splitlines()[1:]) + "\n"
    return body.strip() + "\n"


@unittest.skipUnless(
    sys.platform == "linux", "Workflow shell mocks exercise the Linux comment jobs"
)
class ReportingWorkflowTests(unittest.TestCase):
    """Run only literal workflow shells against mocked GitHub endpoints."""

    def setUp(self) -> None:
        """Create disposable reports and a no-network GitHub CLI mock."""
        temporary = tempfile.TemporaryDirectory(prefix="tfm-reporting-workflow-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.calls = self.root / "calls.txt"
        self.comments = self.root / "comments.json"
        self.comments.write_text("[]", "utf-8")
        gh = self.root / "gh"
        gh.write_text(GH_MOCK, "utf-8")
        gh.chmod(0o755)
        environment = mock.patch.dict(
            os.environ,
            {
                "PATH": str(self.root) + os.pathsep + os.environ.get("PATH", ""),
                "GITHUB_REPOSITORY": "seapagan/terminal-file-manager",
                "PR_NUMBER": "42",
                "GH_TOKEN": "mock-token",
                "MOCK_CALLS": str(self.calls),
                "MOCK_COMMENTS": str(self.comments),
            },
        )
        environment.start()
        self.addCleanup(environment.stop)

    def run_step(self, name: str, step: str) -> None:
        """Execute the actual workflow shell using the existing test process helper."""
        script = step_script(workflow(name), step)
        CHECKER._run(
            ["bash", "--noprofile", "--norc", "-euo", "pipefail", "-c", script],
            cwd=self.root,
        )

    def report(self, name: str) -> str:
        """Write a trusted same-run artifact with literal shell metacharacters."""
        identifier = "package-checks" if name == "package" else "advisory-quality"
        marker = f"<!-- tfm-{identifier} -->"
        body = marker + "\n$(touch executed) `touch executed` $HOME\n"
        (self.root / f"{name}-comment.md").write_text(body, "utf-8")
        return marker

    def test_creates_comment_when_no_owned_comment_exists(self) -> None:
        """Both report jobs POST safely when no marker-owned bot comment exists."""
        for name in REPORTS:
            with self.subTest(workflow=name):
                self.report(name)
                self.run_step(
                    name,
                    "Verify "
                    + ("package" if name == "package" else "advisory")
                    + " comment",
                )
                self.run_step(name, "Create or update sticky comment")
                calls = self.calls.read_text("utf-8")
                self.assertIn("POST\n", calls)
                self.assertIn("issues/42/comments", calls)
                self.assertFalse((self.root / "executed").exists())
                self.calls.unlink()

    def test_updates_only_exact_first_line_bot_owned_comment(self) -> None:
        """Quoted markers and human comments do not confer sticky ownership."""
        for name in REPORTS:
            with self.subTest(workflow=name):
                marker = self.report(name)
                comments = [
                    {"id": 11, "user": {"login": "human"}, "body": marker},
                    {
                        "id": 12,
                        "user": {"login": "github-actions[bot]"},
                        "body": "quoted\n" + marker,
                    },
                    {
                        "id": 13,
                        "user": {"login": "github-actions[bot]"},
                        "body": marker + " suffix",
                    },
                    {
                        "id": 14,
                        "user": {"login": "github-actions[bot]"},
                        "body": marker + "\nold",
                    },
                ]
                self.comments.write_text(json.dumps(comments), "utf-8")
                self.run_step(name, "Create or update sticky comment")
                calls = self.calls.read_text("utf-8")
                self.assertIn("PATCH\n", calls)
                self.assertIn("issues/comments/14", calls)
                self.assertNotIn("POST\n", calls)
                self.calls.unlink()

    def test_missing_and_empty_artifacts_fail_before_commenting(self) -> None:
        """A missing or empty same-run artifact cannot reach the write endpoint."""
        for name in REPORTS:
            step = (
                "Verify "
                + ("package" if name == "package" else "advisory")
                + " comment"
            )
            for exists in (False, True):
                with self.subTest(workflow=name, exists=exists):
                    if exists:
                        (self.root / f"{name}-comment.md").touch()
                    with self.assertRaises(CHECKER.CheckerError):
                        self.run_step(name, step)
                    self.assertFalse(self.calls.exists())

    def test_comment_jobs_are_same_repository_pr_only(self) -> None:
        """Fork PRs and non-PR events cannot satisfy either write-job guard."""
        for name in REPORTS:
            source = workflow(name)
            section = source.split(f"  {name}-report:\n", 1)[1]
            guard = section.split("    if: >-\n", 1)[1].split("    needs:", 1)[0]
            self.assertIn("github.event_name == 'pull_request'", guard)
            self.assertIn(
                "github.event.pull_request.head.repo.full_name == github.repository",
                guard,
            )
            self.assertNotIn("uses: actions/checkout", section)
            self.assertNotIn("pull_request_target", source)
            self.assertEqual(source.count("pull-requests: write"), 1)

    def test_actual_guards_skip_forks_and_non_pr_events(self) -> None:
        """Evaluate the production guard comparisons with isolated event contexts."""
        for name in REPORTS:
            section = workflow(name).split(f"  {name}-report:\n", 1)[1]
            guard = section.split("    if: >-\n", 1)[1].split("    needs:", 1)[0]
            comparisons = [
                part.strip() for part in guard.split("&&") if part.strip() != "always()"
            ]
            for event, head, ready, expected in (
                ("pull_request", "owner/tfm", "true", True),
                ("pull_request", "fork/tfm", "true", False),
                ("push", "owner/tfm", "true", False),
                ("pull_request", "owner/tfm", "false", name != "package"),
            ):
                values = {
                    "github.event_name": event,
                    "github.event.pull_request.head.repo.full_name": head,
                    "github.repository": "owner/tfm",
                    "needs.package.outputs.report_ready": ready,
                }
                matches = []
                for comparison in comparisons:
                    left, right = (part.strip() for part in comparison.split("=="))
                    matches.append(values[left] == values.get(right, right.strip("'")))
                with self.subTest(workflow=name, event=event, head=head, ready=ready):
                    self.assertEqual(all(matches), expected)

    def test_package_report_precedes_enforcement_and_uploads_json(self) -> None:
        """Policy failures retain both report artifacts before blocking the job."""
        source = workflow("package")
        self.assertLess(
            source.index("Analyse package"), source.index("Upload package comment")
        )
        self.assertLess(
            source.index("Upload package comment"),
            source.index("Enforce package policy"),
        )
        self.assertIn(
            "package-report.json", source.split("Upload package comment", 1)[1]
        )
        self.assertIn("cargo make package\n", source)
        self.assertIn("cargo make package-list", source)
        self.assertNotIn("--allow-dirty", source)

    def test_advisory_compiler_failure_cannot_generate_success_report(self) -> None:
        """A failing compiler pipeline prevents a misleading empty-findings report."""
        cargo = self.root / "cargo"
        cargo.write_text(
            "#!/usr/bin/env bash\necho 'error: mock compiler failure' >&2\nexit 23\n",
            "utf-8",
        )
        cargo.chmod(0o755)
        with self.assertRaises(CHECKER.CheckerError):
            self.run_step("quality", "Run advisory quality checks")
        self.assertIn(
            "mock compiler failure", (self.root / "quality.log").read_text("utf-8")
        )
        source = workflow("quality")
        build_step = source.split("      - name: Build advisory report", 1)[1].split(
            "      - name:", 1
        )[0]
        self.assertNotIn("always()", build_step)
        self.assertNotIn("continue-on-error", source)

    def test_production_shells_pass_shellcheck(self) -> None:
        """Lint the literal comment scripts independently from their workflow YAML."""
        self.assertIsNotNone(shutil.which("shellcheck"))
        for name in REPORTS:
            script = self.root / f"{name}.sh"
            script.write_text(
                "#!/usr/bin/env bash\n"
                + step_script(workflow(name), "Create or update sticky comment"),
                "utf-8",
            )
            CHECKER._run(["shellcheck", str(script)])


if __name__ == "__main__":
    unittest.main()
