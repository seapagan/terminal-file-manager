"""Regressions for the documented raw-path and scanner-schema defects."""

from __future__ import annotations

import contextlib
import io
import os
import re
import sys
import tempfile
import unittest
from pathlib import Path
from typing import TYPE_CHECKING
from unittest.mock import patch

from test_check_complexity import CHECKER

if TYPE_CHECKING:
    from collections.abc import Callable, Sequence

LIMITS = {
    "COMPLEXITY_LIZARD_VERSION": "1.23.0",
    "COMPLEXITY_MAX_CCN": "10",
    "COMPLEXITY_MAX_FUNCTION_NLOC": "50",
    "COMPLEXITY_MAX_PARAMETERS": "8",
    "COMPLEXITY_MAX_FILE_NLOC": "500",
}
CSV = (
    "NLOC,CCN,token,PARAM,length,location,file,function,long_name,start,end\n"
    "12,4,100,2,20,func@5-24@src/main.rs,src/main.rs,func,func(),5,24\n"
)


class SchemaRegressionTests(unittest.TestCase):
    """Preserve the legacy fixture and reject documented CSV corruption."""

    def test_legacy_xml_without_ordinal_label(self) -> None:
        """The distinct keyhold/git-diff-out NCSS/CCN assertion remains intact."""
        output = (
            '<root><measure type="File"><labels><label>NCSS</label>'
            '<label>CCN</label></labels><item name="src/main.rs">'
            "<value>7</value><value>2</value></item></measure></root>"
        )
        self.assertEqual(
            CHECKER._file_metrics(output, {"src/main.rs"}), {"src/main.rs": 7}
        )

    def test_csv_rejects_extra_columns(self) -> None:
        """An extension column cannot silently change the audited schema."""
        output = CSV.replace("start,end", "start,end,extension").replace(
            "5,24\n", "5,24,1\n"
        )
        with self.assertRaises(CHECKER.CheckerError):
            CHECKER._function_metrics(output, {"src/main.rs"})

    def test_csv_rejects_duplicate_function_rows(self) -> None:
        """A duplicate record must not count a function twice."""
        with self.assertRaises(CHECKER.CheckerError):
            CHECKER._function_metrics(CSV + CSV.splitlines()[1] + "\n", {"src/main.rs"})

    def test_csv_validates_auxiliary_numbers_and_bounds(self) -> None:
        """Unused numeric fields and redundant source bounds stay trustworthy."""
        for field, value in ((2, "bad"), (4, "19"), (10, "4")):
            row = CSV.splitlines()[1].split(",")
            row[field] = value
            with self.subTest(field=field), self.assertRaises(CHECKER.CheckerError):
                CHECKER._function_metrics(
                    CSV.splitlines()[0] + "\n" + ",".join(row), {"src/main.rs"}
                )


class RealScannerRegressionTests(unittest.TestCase):
    """Exercise the full checker against actual audited Lizard output."""

    def setUp(self) -> None:
        """Allocate a private Git worktree; restore cwd before its cleanup."""
        temporary = tempfile.TemporaryDirectory(prefix="tfm-checker-regression-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        previous = Path.cwd()
        self.addCleanup(os.chdir, previous)
        os.chdir(self.root)
        CHECKER._run(["git", "init", "-q"])
        (self.root / "source.rs").write_text(
            "fn foo() { let x = 1; }\n", encoding="utf-8"
        )

    def _scan(self, flag: str = "", mutate: Callable[[str], str] | None = None) -> str:
        original = CHECKER._run

        def run(command: Sequence[str], *, cwd: Path | None = None) -> str:
            output: str = original(command, cwd=cwd)
            return mutate(output) if mutate is not None and flag in command else output

        output = io.StringIO()
        with (
            patch.dict(os.environ, LIMITS),
            patch.object(CHECKER, "_run", side_effect=run),
            contextlib.redirect_stdout(output),
        ):
            CHECKER.main()
        return output.getvalue()

    @unittest.skipUnless(
        sys.platform == "linux", "raw invalid-byte filenames require a Linux filesystem"
    )
    def test_raw_filenames_remain_distinct_and_diagnostics_are_ascii_safe(self) -> None:
        """Raw FF/FE and literal replacement characters never alias or break XML."""
        source = "fn complex(x: i32) {\n" + "if x > 0 {}\n" * 10 + "}\n"
        for name in (
            os.fsdecode(b"raw-\xff.rs"),
            os.fsdecode(b"raw-\xfe.rs"),
            "raw-\ufffd.rs",
        ):
            (self.root / name).write_text(source, encoding="utf-8")
        with patch.dict(os.environ, {"PYTHONIOENCODING": "utf-8:strict"}):
            output = self._scan()
            with patch.dict(os.environ, LIMITS):
                process_output = CHECKER._run([sys.executable, "-B", CHECKER.__file__])
        self.assertEqual(process_output, output)
        self.assertEqual(output.count("CCN actual=11 limit=10"), 3)
        self.assertIn("raw-\\udcff.rs", output)
        self.assertIn("raw-\\udcfe.rs", output)
        self.assertIn("raw-\\ufffd.rs", output)

    def test_real_zero_function_and_empty_files_are_reconciled(self) -> None:
        """Legitimate files without functions still receive file metrics."""
        (self.root / "empty.py").write_text("", encoding="utf-8")
        (self.root / "constants.rs").write_text("const X: i32 = 1;\n", encoding="utf-8")
        self.assertIn("Complexity: no advisory findings.", self._scan())

    def test_real_csv_missing_functions_are_blocking(self) -> None:
        """XML function counts detect an emptied function stream."""
        with self.assertRaises(CHECKER.CheckerError):
            self._scan("--csv", lambda output: output.splitlines()[0] + "\n")

    def test_real_xml_rejects_unknown_schema_and_function_mismatch(self) -> None:
        """Schema, entity declarations and inconsistent function metrics fail."""
        mutations = (
            lambda text: text.replace("cppncss", "unknown"),
            lambda text: text.replace(
                "<cppncss>", '<!DOCTYPE cppncss [<!ENTITY x "x">]><cppncss>'
            ),
            lambda text: text.replace("<label>NCSS</label>", "<label>extra</label>", 1),
            lambda text: text.replace("<value>1</value>", "<value>999</value>", 1),
            lambda text: re.sub(
                r'(<item name="foo.*?<value>1</value>\s*<value>)1',
                r"\g<1>99",
                text,
                count=1,
                flags=re.DOTALL,
            ),
            lambda text: text.replace("</cppncss>", "<unexpected /></cppncss>"),
        )
        for index, mutate in enumerate(mutations):
            with self.subTest(index=index), self.assertRaises(CHECKER.CheckerError):
                self._scan("--xml", mutate)


if __name__ == "__main__":
    unittest.main()
