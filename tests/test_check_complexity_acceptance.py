"""Actual-scanner acceptance for fixed limits, discovery and fatal failures."""

from __future__ import annotations

import csv
import io
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from test_check_complexity import CHECKER
from test_check_complexity_remediation import LIMITS

# Rust 1.23.0 counts each match token, not each arm: ordinary has three
# outcomes but CCN 2. Nested adds two matches; guarded adds the guard; closure
# adds one match and one if. Macro/string/comment tokens are not functions.
# Python structural match counts all three cases; this f-string's conditional
# is ignored in 1.23.0. The separately gated 1.24.1 upgrade changes these metrics.
RUST_CORPUS = """fn ordinary(x: i32) -> i32 {
    match x { 0 => 10, 1 => 20, _ => 30 }
}
fn nested(x: i32) -> i32 {
    match x { 0 => match x { 1 => 2, _ => 3 }, _ => 4 }
}
fn guarded(x: i32) -> i32 {
    match x { n if n > 0 => 1, 0 | -1 => 2, _ => 3 }
}
fn closure(x: i32) -> i32 {
    let f = |n| match n { 0 => 1, _ => 2 };
    if x > 0 { f(x) } else { 0 }
}
macro_rules! choose { ($x:expr) => { match $x { 0 => 1, _ => 2 } }; }
fn expanded(x: i32) -> i32 { choose!(x) }
#[test]
fn adjacent() { let text = "match { => }"; /* match => */ assert!(true); }
"""
PYTHON_CORPUS = """def structural(x):
    match x:
        case 0: return 1
        case 1 | 2: return 2
        case _: return 3

def formatted(x):
    return f"value {x if x else 0}"
"""

SCRIPT = Path(__file__).parents[1] / "scripts" / "check_complexity.py"


class ScannerAcceptanceTests(unittest.TestCase):
    """Run isolated Git discovery and both actual audited scanner formats."""

    def setUp(self) -> None:
        """Isolate all fixture writes and inherited signing configuration."""
        temporary = tempfile.TemporaryDirectory(prefix="tfm-checker-acceptance-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        environment = patch.dict(
            os.environ,
            {**LIMITS, "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1"},
        )
        environment.start()
        self.addCleanup(environment.stop)
        CHECKER._run(["git", "init", "-q"], cwd=self.root)

    def _write(self, name: str, source: str) -> None:
        (self.root / name).write_text(source, encoding="utf-8")

    def _write_boundaries(self) -> None:
        """Build explicit source metrics around all four fixed limits."""
        for value in (9, 10, 11):
            self._write(
                f"ccn-{value}.py",
                "def branches(x):\n" + "    if x: pass\n" * (value - 1),
            )
        for value in (49, 50, 51):
            self._write(
                f"nloc-{value}.py",
                "def lines():\n" + "    pass\n" * (value - 1),
            )
        for value in (7, 8, 9):
            params = ", ".join(f"p{index}" for index in range(value))
            self._write(f"params-{value}.py", f"def args({params}):\n    pass\n")
        for value in (499, 500, 501):
            self._write(f"file-{value}.py", "value = 1\n" * value)
        self._write("empty.rs", "")

    def test_real_threshold_boundaries(self) -> None:
        """Real metrics below/equal/above each immutable limit stay advisory."""
        self._write_boundaries()
        files = CHECKER._source_files(self.root)
        functions, nloc = CHECKER._scan_sources(self.root, files)
        by_path = {metric.path: metric for metric in functions}
        self.assertEqual([by_path[f"ccn-{v}.py"].ccn for v in (9, 10, 11)], [9, 10, 11])
        self.assertEqual(
            [by_path[f"nloc-{v}.py"].nloc for v in (49, 50, 51)], [49, 50, 51]
        )
        self.assertEqual(
            [by_path[f"params-{v}.py"].parameters for v in (7, 8, 9)], [7, 8, 9]
        )
        self.assertEqual(
            [nloc[f"file-{v}.py"] for v in (499, 500, 501)], [499, 500, 501]
        )
        self.assertEqual(nloc["empty.rs"], 0)
        findings = CHECKER._collect_findings(
            functions, nloc, CHECKER.Thresholds(10, 50, 8, 500)
        )
        self.assertEqual(
            {(f.path, f.metric) for f in findings},
            {
                ("ccn-11.py", "CCN"),
                ("nloc-51.py", "function NLOC"),
                ("params-9.py", "parameters"),
                ("file-501.py", "file NLOC"),
            },
        )
        output = CHECKER._run([sys.executable, "-B", str(SCRIPT)], cwd=self.root)
        self.assertIn("4 advisory finding(s)", output)
        self.assertEqual(
            output, CHECKER._run([sys.executable, "-B", str(SCRIPT)], cwd=self.root)
        )

    def test_tracked_ignored_and_nested_discovery(self) -> None:
        """Index ownership wins over ignores; untracked ignored input stays out."""
        self._write("tracked.rs", "fn tracked() {}\n")
        CHECKER._run(["git", "add", "tracked.rs"], cwd=self.root)
        self._write(".gitignore", "tracked.rs\nignored.py\n")
        self._write("ignored.py", "invalid source never selected")
        self._write("untracked.py", "pass\n")
        (self.root / "nested").mkdir()
        files = CHECKER._source_files(self.root)
        self.assertEqual(files, ["tracked.rs", "untracked.py"])
        previous = Path.cwd()
        try:
            os.chdir(self.root / "nested")
            self.assertEqual(CHECKER._source_files(), files)
            output = CHECKER._run([sys.executable, "-B", str(SCRIPT)])
        finally:
            os.chdir(previous)
        self.assertIn("no advisory findings", output)

    def test_real_special_names_and_snapshot_consistency(self) -> None:
        """ASCII aliases preserve exact identity and immutable CSV/XML input."""
        names = ["-leading.rs", "space name.rs", "comma,name.rs", "quote'name.rs"]
        if os.name != "nt":
            names += ["new\nline.rs", "control-\x01.rs"]
        for name in names:
            self._write(name, "fn original() {}\n")
        original_run = CHECKER._run
        snapshots: list[list[bytes]] = []

        def run(command: list[str], *, cwd: Path | None = None) -> str:
            output: str = original_run(command, cwd=cwd)
            if command[0] == "lizard" and cwd is not None:
                snapshots.append([path.read_bytes() for path in sorted(cwd.iterdir())])
                for name in names:
                    self._write(name, "fn changed(x: i32) { if x > 0 {} }\n")
            return output

        with patch.object(CHECKER, "_run", side_effect=run):
            functions, nloc = CHECKER._scan_sources(self.root, sorted(names))
        self.assertEqual(snapshots[0], snapshots[1])
        self.assertEqual(set(nloc), set(names))
        self.assertEqual({metric.name for metric in functions}, {"original"})
        self.assertEqual({metric.path for metric in functions}, set(names))

    def test_full_process_failures_are_blocking(self) -> None:
        """Actual entry-point status blocks invalid configuration and setup."""
        self._write("source.rs", "fn source() {}\n")
        cases = (
            ({"COMPLEXITY_MAX_CCN": "0"}, "positive integer"),
            ({"COMPLEXITY_LIZARD_VERSION": "1.24.1"}, "version mismatch"),
            (
                {"PATH": str(self.root / "absent-tools")},
                "required executable not found",
            ),
        )
        for environment, message in cases:
            with (
                self.subTest(environment=environment),
                patch.dict(os.environ, environment),
                self.assertRaisesRegex(
                    CHECKER.CheckerError,
                    f"exit code 1: error: complexity checker: .*{message}",
                ),
            ):
                CHECKER._run([sys.executable, "-B", str(SCRIPT)], cwd=self.root)
        with self.assertRaisesRegex(
            CHECKER.CheckerError, "exit code 17: scanner failed"
        ):
            CHECKER._run(
                [
                    sys.executable,
                    "-c",
                    "import sys; sys.stderr.write('scanner failed'); sys.exit(17)",
                ]
            )

    def test_actual_csv_corruption_is_rejected(self) -> None:
        """Every field, row shape, identity and bounds is validated."""
        self._write("source.rs", "fn source() {}\n")
        output = CHECKER._run(
            CHECKER._lizard_command(["-V", "--csv"], ["source.rs"]), cwd=self.root
        )
        rows = list(csv.reader(io.StringIO(output)))
        for index in (0, 1, 2, 3, 4, 9, 10):
            for value in ("bad", "-1"):
                damaged = [row.copy() for row in rows]
                damaged[1][index] = value
                stream = io.StringIO()
                csv.writer(stream).writerows(damaged)
                with (
                    self.subTest(index=index, value=value),
                    self.assertRaises(CHECKER.CheckerError),
                ):
                    CHECKER._function_metrics(stream.getvalue(), {"source.rs"})
        corruptions = (
            output.replace("start,end", "end,start"),
            output + output.splitlines()[1] + "\n",
            output.replace("source.rs", "extra.rs"),
            output.splitlines()[0] + "\n" + ",".join(rows[1][:-1]),
            output.splitlines()[0] + "\n" + ",".join(rows[1] + ["extra"]),
            output.replace("source@1-1@source.rs", "source@2-2@source.rs"),
        )
        for index, damaged in enumerate(corruptions):
            with self.subTest(index=index), self.assertRaises(CHECKER.CheckerError):
                CHECKER._function_metrics(damaged, {"source.rs"})

    def test_actual_xml_corruption_is_rejected(self) -> None:
        """Real XML must satisfy the full audited schema and CSV identities."""
        self._write("source.rs", "fn source() {}\n")
        files = ["source.rs"]
        csv_output = CHECKER._run(
            CHECKER._lizard_command(["-V", "--csv"], files), cwd=self.root
        )
        functions = CHECKER._function_metrics(csv_output, set(files))
        xml = CHECKER._run(CHECKER._lizard_command(["--xml"], files), cwd=self.root)
        mutations = (
            ("<cppncss>", '<cppncss extra="1">'),
            ('type="Function"', 'type="Unknown"'),
            ('type="File"', 'type="Function"'),
            ("<labels>", '<labels extra="1">'),
            ("<label>Nr.</label>", "<label>CCN</label>"),
            ("<label>NCSS</label>", '<label extra="1">NCSS</label>'),
            ('name="source(...) at source.rs:1"', 'name=""'),
            ('name="source(...) at source.rs:1"', 'name="other(...) at source.rs:1"'),
            ('<item name="source.rs">', '<item name="extra.rs">'),
            ('<item name="source.rs">', '<item name="source.rs" extra="1">'),
            ("<value>1</value>", "<value>-1</value>"),
            ("<value>1</value>", "<value>NaN</value>"),
            ("<value>1</value>", "<value>2</value>"),
            ("<value>1</value>", '<value extra="1">1</value>'),
            ("<value>1</value>", "<value><nested/>1</value>"),
            ("<value>1</value>", ""),
            ("<value>1</value>", "<value>1</value><value>1</value>"),
            ('label="NCSS" value="1.0"', 'label="unknown" value="1.0"'),
            ('label="NCSS" value="1.0"', 'label="NCSS" value="NaN"'),
            ('label="NCSS" value="1.0"', 'label="NCSS" value="-1"'),
            ('label="NCSS" value="1.0"', 'label="NCSS" value="bad"'),
            ('<sum label="NCSS" value="1"/>', '<sum label="NCSS" value="1.5"/>'),
            ("</cppncss>", "<extra/></cppncss>"),
            ("</cppncss>", ""),
        )
        for index, (before, after) in enumerate(mutations):
            damaged = xml.replace(before, after, 1)
            self.assertNotEqual(xml, damaged)
            with self.subTest(index=index), self.assertRaises(CHECKER.CheckerError):
                CHECKER._file_metrics(damaged, set(files), functions)

    def test_representative_language_boundaries(self) -> None:
        """Freeze audited parser behaviour without claiming corrected Rust CCN."""
        self._write("corpus.rs", RUST_CORPUS)
        self._write("corpus.py", PYTHON_CORPUS)
        functions, nloc = CHECKER._scan_sources(self.root, ["corpus.py", "corpus.rs"])
        observed = {m.name: (m.line, m.nloc, m.ccn, m.parameters) for m in functions}
        self.assertEqual(
            observed,
            {
                "ordinary": (1, 3, 2, 1),
                "nested": (4, 3, 3, 1),
                "guarded": (7, 3, 3, 1),
                "closure": (10, 4, 3, 1),
                "adjacent": (17, 1, 1, 0),
                "expanded": (15, 1, 1, 1),
                "structural": (1, 5, 4, 1),
                "formatted": (7, 2, 1, 1),
            },
        )
        self.assertEqual(nloc, {"corpus.py": 7, "corpus.rs": 17})


if __name__ == "__main__":
    unittest.main()
