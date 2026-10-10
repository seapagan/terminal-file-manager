"""TFM advisory scope, JSON aggregation and failure regressions."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from test_advisory_quality_report import TOO_MANY_LINES_DIAGNOSTIC, finding

from scripts import advisory_quality_report as report


class TfmAdvisoryTests(unittest.TestCase):
    """Verify the two-lint report is a supplement to strict verification."""

    def test_combined_json_preserves_common_and_specific_findings(self) -> None:
        """JSON retains exact platform ownership alongside the Markdown grouping."""
        shared = finding()
        linux = finding("src/linux.rs", 20, "linux_only")
        results = [
            report.PlatformResult("Linux", (shared, linux)),
            report.PlatformResult("Windows", (shared,)),
        ]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "combined.json"
            report.write_combined_result(path, results)
            data = json.loads(path.read_text("utf-8"))
        by_function = {item["function"]: item["platforms"] for item in data["findings"]}
        self.assertEqual(
            by_function,
            {"classify_and_decode": ["Linux", "Windows"], "linux_only": ["Linux"]},
        )
        self.assertEqual(data["thresholds"], {"lines": 50, "arguments": 8})

    def test_combined_cli_writes_both_outputs(self) -> None:
        """The workflow aggregation entry point writes JSON and Markdown together."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            platform = root / "linux.json"
            markdown = root / "combined.md"
            structured = root / "combined.json"
            report.write_platform_result(platform, "Linux", [finding()])
            with mock.patch.object(
                sys,
                "argv",
                [
                    "report",
                    "combine",
                    str(markdown),
                    str(platform),
                    "--json-output",
                    str(structured),
                ],
            ):
                report.main()
            self.assertEqual(
                markdown.read_text("utf-8").splitlines()[0],
                "<!-- tfm-advisory-quality -->",
            )
            self.assertEqual(
                json.loads(structured.read_text("utf-8"))["platforms"], ["Linux"]
            )

    def test_negative_and_unrelated_lints_are_not_reported(self) -> None:
        """Other Clippy diagnostics are outside this report's actual scope."""
        diagnostic = TOO_MANY_LINES_DIAGNOSTIC.replace(
            "#too_many_lines", "#cognitive_complexity"
        )
        self.assertEqual(report.extract_findings(diagnostic), [])

    def test_corrupt_platform_json_blocks_aggregation(self) -> None:
        """Invalid artifacts cannot become a healthy combined report."""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "corrupt.json"
            path.write_text("not JSON", "utf-8")
            with self.assertRaises(json.JSONDecodeError):
                report.load_platform_result(path)

    def test_report_scope_and_policy_are_explicit(self) -> None:
        """The report covers only two Clippy metrics at TFM's approved 50/8 policy."""
        markdown = report.render_report("Linux", [])
        self.assertIn("50 lines / 8 arguments", markdown)
        self.assertIn(
            "only `clippy::too_many_lines` and `clippy::too_many_arguments`", markdown
        )
        combined = report.render_combined_comment(
            [report.PlatformResult("Linux", (finding(),))]
        )
        self.assertIn("supplements the blocking quality gates", combined)
