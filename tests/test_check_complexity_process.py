"""Full entry-point failure controls using captured actual Lizard streams."""

from __future__ import annotations

import json
import os
import re
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from test_check_complexity import CHECKER
from test_check_complexity_remediation import LIMITS

SCRIPT = Path(__file__).parents[1] / "scripts" / "check_complexity.py"
# The child runs the unchanged production __main__ entry point. Only its process
# responses are replaced, with real captured schemas; no shell or tool executes.
DRIVER = r"""import json, runpy, subprocess, sys
from pathlib import Path
from unittest.mock import patch
script, data_file = sys.argv[1:]
data = json.loads(Path(data_file).read_text(encoding='utf-8'))
def result(command, **kwargs):
    if command[0] == 'git':
        output = data['root'] + '\n' if 'rev-parse' in command else 'source.rs\0'
        return subprocess.CompletedProcess(command, 0, output, '')
    if data.get('missing'):
        raise FileNotFoundError('fixture executable missing')
    key = ('version' if '--version' in command else
           'csv' if '--csv' in command else 'xml')
    status = 7 if data.get('failed') == key else 0
    error = 'fixture scanner failed' if status else ''
    return subprocess.CompletedProcess(command, status, data[key], error)
with patch('subprocess.run', side_effect=result):
    runpy.run_path(script, run_name='__main__')
"""


def _corruptions(csv: str, xml: str) -> list[tuple[str, str]]:
    functions = re.search(r'<measure type="Function">.*?</measure>', xml, re.DOTALL)
    files = re.search(r'<measure type="File">.*?</measure>', xml, re.DOTALL)
    if functions is None or files is None:
        message = "actual scanner omitted required measures"
        raise AssertionError(message)
    function = re.search(r"<item.*?</item>", functions[0], re.DOTALL)
    file = re.search(r"<item.*?</item>", files[0], re.DOTALL)
    if function is None or file is None:
        message = "actual scanner omitted required records"
        raise AssertionError(message)
    return [
        ("csv", csv.replace("start,end", "start,end,end")),
        ("csv", csv + csv.splitlines()[1] + "\n"),
        ("csv", csv.splitlines()[0] + "\n"),
        ("csv", csv.replace('"source"', '"unterminated')),
        ("csv", re.sub(r"(^\d+,\d+,)\d+", r"\g<1>bad", csv, flags=re.MULTILINE)),
        ("xml", "<cppncss>"),
        (
            "xml",
            xml.replace("<cppncss>", '<!DOCTYPE cppncss [<!ENTITY x "x">]><cppncss>'),
        ),
        ("xml", xml.replace("<cppncss>", "<cppncss>\udcff")),
        ("xml", xml.replace("<label>NCSS</label>", "<wrong>NCSS</wrong>", 1)),
        ("xml", xml.replace("<labels>", '<labels extra="1">', 1)),
        ("xml", xml.replace("<labels>", "<labels/><labels>", 1)),
        *_record_corruptions(xml, function[0], file[0], functions[0], files[0]),
    ]


def _changed_metric(record: str, index: int) -> str:
    values = record.split("<value>")
    values[index + 1] = values[index + 1].replace("1</value>", "2</value>", 1)
    return "<value>".join(values)


def _record_corruptions(
    xml: str, function: str, file: str, functions: str, files: str
) -> list[tuple[str, str]]:
    return [
        ("xml", xml.replace(file, "")),
        ("xml", xml.replace(file, file * 2)),
        ("xml", xml.replace(file, file.replace("source-00000000.rs", "unexpected.rs"))),
        ("xml", xml.replace(file, _changed_metric(file, 3))),
        ("xml", xml.replace(file, _changed_metric(file, 2))),
        ("xml", xml.replace(function, "")),
        ("xml", xml.replace(function, function * 2)),
        ("xml", xml.replace(function, _changed_metric(function, 1))),
        ("xml", xml.replace(function, _changed_metric(function, 2))),
        ("xml", xml.replace(functions, functions * 2)),
        ("xml", xml.replace(files, "")),
    ]


class ProcessFailureTests(unittest.TestCase):
    """A trustworthy valid control passes; every machine corruption blocks."""

    def test_captured_schema_failures_block_the_real_entry_point(self) -> None:
        """No parsing/process failure is converted into advisory success."""
        with tempfile.TemporaryDirectory(prefix="tfm-process-controls-") as directory:
            root = Path(directory)
            (root / "source.rs").write_text("fn source() {}\n", encoding="utf-8")
            (root / "source-00000000.rs").write_text(
                "fn source() {}\n", encoding="utf-8"
            )
            csv = CHECKER._run(
                CHECKER._lizard_command(["-V", "--csv"], ["source-00000000.rs"]),
                cwd=root,
            )
            xml = CHECKER._run(
                CHECKER._lizard_command(["--xml"], ["source-00000000.rs"]), cwd=root
            )
            version = CHECKER._run(["lizard", "--version"])
            base = {"root": str(root), "version": version, "csv": csv, "xml": xml}
            data = root / "response.json"
            command = [sys.executable, "-B", "-c", DRIVER, str(SCRIPT), str(data)]
            data.write_text(json.dumps(base), encoding="utf-8")
            with patch.dict(os.environ, LIMITS):
                self.assertIn("no advisory findings", CHECKER._run(command))
                for index, (key, value) in enumerate(_corruptions(csv, xml)):
                    self.assertNotEqual(base[key], value)
                    data.write_text(json.dumps({**base, key: value}), encoding="utf-8")
                    with (
                        self.subTest(index=index),
                        self.assertRaisesRegex(
                            CHECKER.CheckerError,
                            "exit code 1: error: complexity checker:",
                        ),
                    ):
                        CHECKER._run(command)
                self._failed_tools(base, data, command)

    def _failed_tools(
        self, base: dict[str, str], data: Path, command: list[str]
    ) -> None:
        """Version/CSV/XML process errors and missing tools all fail closed."""
        for key in ("version", "csv", "xml"):
            data.write_text(json.dumps({**base, "failed": key}), encoding="utf-8")
            with (
                self.subTest(failed=key),
                self.assertRaisesRegex(
                    CHECKER.CheckerError,
                    "exit code 1: error: complexity checker: .*exit code 7",
                ),
            ):
                CHECKER._run(command)
        data.write_text(json.dumps({**base, "missing": True}), encoding="utf-8")
        with self.assertRaisesRegex(
            CHECKER.CheckerError,
            "exit code 1: error: complexity checker: required executable not found",
        ):
            CHECKER._run(command)


if __name__ == "__main__":
    unittest.main()
