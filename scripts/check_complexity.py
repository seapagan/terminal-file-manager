"""Report Codacy-aligned source complexity findings from Lizard output."""

from __future__ import annotations

import csv
import io
import math
import os
import re
import shutil

# Security suppressions below are intentional and tool-specific: Bandit/Codacy
# consumes the B-coded ``nosec`` comments; matching Ruff S-coded suppressions
# remain for equivalent Ruff security rules when enabled. Do not remove either
# without rerunning Bandit/Codacy and Ruff.
# Subprocess use is the reviewed boundary for invoking Git and Lizard.
import subprocess  # nosec B404
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING

# XML input is produced by the locally invoked Lizard process.
from xml.etree.ElementTree import Element, ParseError, fromstring  # nosec B405

MINIMUM_PYTHON = (3, 10)

if sys.version_info < MINIMUM_PYTHON:
    print("check_complexity.py requires Python 3.10 or newer", file=sys.stderr)
    raise SystemExit(1)

if TYPE_CHECKING:
    from collections.abc import Sequence


@dataclass(frozen=True)
class Thresholds:
    """Configured upper bounds for each reported complexity metric."""

    ccn: int
    function_nloc: int
    parameters: int
    file_nloc: int


@dataclass(frozen=True)
class FunctionMetric:
    """Lizard metrics for one source function."""

    path: str
    name: str
    line: int
    nloc: int
    ccn: int
    parameters: int


@dataclass(frozen=True, order=True)
class Finding:
    """One deterministically sortable advisory finding."""

    path: str
    line: int
    metric: str
    message: str


class CheckerError(Exception):
    """Raised when the checker cannot produce a trustworthy report."""


def _limit(name: str) -> int:
    value = os.environ.get(name, "")
    if not re.fullmatch(r"[1-9][0-9]*", value):
        message = f"{name} must be a positive integer, got {value!r}"
        raise CheckerError(message)
    return int(value)


def _run(command: Sequence[str], *, cwd: Path | None = None) -> str:
    try:
        # Controlled argv boundary: fixed executable, separate arguments, no shell.
        result = subprocess.run(  # noqa: S603  # nosec B603
            command,
            cwd=cwd,
            capture_output=True,
            text=True,
            errors="surrogateescape",
            check=False,
        )
    except FileNotFoundError as error:
        message = f"required executable not found: {command[0]}"
        raise CheckerError(message) from error
    if result.returncode != 0:
        detail = (
            result.stderr.strip() or result.stdout.strip() or "no diagnostic output"
        )
        message = (
            f"{' '.join(command[:2])} failed with exit code "
            f"{result.returncode}: {detail}"
        )
        raise CheckerError(message)
    return result.stdout


def _normalized(path: str) -> str:
    return Path(path.removeprefix("./")).as_posix()


def _csv_text(row: dict[str, str | None], field: str) -> str:
    value = row.get(field)
    if not value:
        message = f"invalid CSV {field} value: {value!r}"
        raise CheckerError(message)
    return value


def _integer(row: dict[str, str | None], field: str) -> int:
    raw_value = row.get(field)
    try:
        value = int(raw_value) if raw_value is not None else -1
    except ValueError as error:
        message = f"invalid CSV {field} value: {raw_value!r}"
        raise CheckerError(message) from error
    if value < 0:
        message = f"invalid CSV {field} value: {raw_value!r}"
        raise CheckerError(message)
    return value


def _repository_root() -> Path:
    return Path(_run(["git", "rev-parse", "--show-toplevel"]).removesuffix("\n"))


def _source_files(root: Path | None = None) -> list[str]:
    if root is None:
        root = _repository_root()
    output = _run(
        [
            "git",
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "*.rs",
            "*.py",
        ],
        cwd=root,
    )
    files = sorted(
        {_normalized(path) for path in output.split("\0") if (root / path).is_file()}
    )
    if not files:
        message = "no non-ignored Rust or Python source files found"
        raise CheckerError(message)
    return files


def _lizard_command(output_options: Sequence[str], files: Sequence[str]) -> list[str]:
    return [
        "lizard",
        "-l",
        "rust",
        "-l",
        "python",
        "-i",
        "-1",
        *output_options,
        "--",
        *files,
    ]


def _function_metrics(output: str, files: set[str]) -> list[FunctionMetric]:
    reader = csv.DictReader(io.StringIO(output), strict=True)
    required = [
        "NLOC",
        "CCN",
        "token",
        "PARAM",
        "length",
        "location",
        "file",
        "function",
        "long_name",
        "start",
        "end",
    ]
    if reader.fieldnames != required:
        message = f"unexpected CSV columns: {reader.fieldnames!r}"
        raise CheckerError(message)

    metrics = []
    identities: set[tuple[str, str, int]] = set()
    for row in reader:
        if set(row) != set(required) or any(value is None for value in row.values()):
            message = "invalid CSV row width"
            raise CheckerError(message)
        path = _normalized(_csv_text(row, "file"))
        name = _csv_text(row, "function")
        if path not in files:
            message = f"unexpected CSV function record: {row!r}"
            raise CheckerError(message)
        start = _integer(row, "start")
        if start == 0:
            message = f"invalid CSV start value: {start}"
            raise CheckerError(message)
        _csv_bounds(row, start, name)
        identity = (path, name, start)
        if identity in identities:
            message = f"duplicate CSV function record: {identity!a}"
            raise CheckerError(message)
        identities.add(identity)
        metrics.append(
            FunctionMetric(
                path=path,
                name=name,
                line=start,
                nloc=_integer(row, "NLOC"),
                ccn=_integer(row, "CCN"),
                parameters=_integer(row, "PARAM"),
            ),
        )
    return metrics


def _csv_bounds(row: dict[str, str | None], start: int, name: str) -> None:
    for field in ("NLOC", "CCN", "PARAM", "token", "length", "end"):
        _integer(row, field)
    end = _integer(row, "end")
    location = f"{name}@{start}-{end}@{_csv_text(row, 'file')}"
    if end < start or _integer(row, "length") != end - start + 1:
        message = "inconsistent CSV source bounds"
        raise CheckerError(message)
    if _csv_text(row, "location") != location:
        message = "inconsistent CSV location"
        raise CheckerError(message)
    _csv_text(row, "long_name")


def _xml_file_measure(
    output: str,
    functions: Sequence[FunctionMetric] | None = None,
) -> tuple[Element, list[str | None], int]:
    if re.search(r"<!\s*(?:DOCTYPE|ENTITY)\b", output, re.IGNORECASE):
        message = "invalid Lizard XML: declarations are forbidden"
        raise CheckerError(message)
    try:
        # Lizard's reported version is checked before its output is parsed here.
        root = fromstring(output)  # noqa: S314  # nosec B314
    except (ParseError, UnicodeError) as error:
        message = f"invalid Lizard XML: {error}"
        raise CheckerError(message) from error

    if functions is not None:
        _reconcile_xml(root, functions)

    measures = root.findall(".//measure[@type='File']")
    if len(measures) != 1:
        message = f"expected one XML File measure, found {len(measures)}"
        raise CheckerError(message)
    measure = measures[0]
    labels = [label.text for label in measure.findall("./labels/label")]
    if "NCSS" not in labels:
        message = f"XML File measure lacks NCSS: {labels!r}"
        raise CheckerError(message)
    return measure, labels, labels.index("NCSS")


def _xml_ncss(path: str, raw_nloc: str | None) -> int:
    try:
        nloc = int(raw_nloc) if raw_nloc is not None else -1
    except ValueError as error:
        message = f"invalid XML NCSS value for {path!r}: {raw_nloc!r}"
        raise CheckerError(message) from error
    if nloc < 0:
        message = f"invalid XML NCSS value for {path!r}: {raw_nloc!r}"
        raise CheckerError(message)
    return nloc


def _xml_file_record(
    item: Element,
    labels: Sequence[str | None],
    nloc_index: int,
) -> tuple[str, int]:
    path = _normalized(item.get("name", ""))
    values = [value.text for value in item.findall("./value")]
    if not path or len(values) != len(labels):
        message = f"unexpected XML file record for {path!r}"
        raise CheckerError(message)
    return path, _xml_ncss(path, values[nloc_index])


def _file_metrics(
    output: str,
    expected_files: set[str],
    functions: Sequence[FunctionMetric] | None = None,
) -> dict[str, int]:
    measure, labels, nloc_index = _xml_file_measure(output, functions)

    metrics: dict[str, int] = {}
    for item in measure.findall("./item"):
        path, nloc = _xml_file_record(item, labels, nloc_index)
        if path in metrics:
            message = f"unexpected XML file record for {path!r}"
            raise CheckerError(message)
        metrics[path] = nloc

    actual_files = set(metrics)
    if actual_files != expected_files:
        missing = sorted(expected_files - actual_files)
        unexpected = sorted(actual_files - expected_files)
        message = (
            f"source analysis file mismatch; missing={missing!r}, "
            f"unexpected={unexpected!r}"
        )
        raise CheckerError(message)
    return metrics


def _xml_require(condition: object, message: str) -> None:
    if not condition:
        detail = f"invalid Lizard XML schema: {message}"
        raise CheckerError(detail)


def _xml_labels(measure: Element) -> list[str]:
    kind = measure.get("type")
    expected = {"Nr.", "NCSS", "CCN"}
    if kind == "File":
        expected.add("Functions")
    containers = measure.findall("labels")
    _xml_require(len(containers) == 1, "expected one labels container")
    container = containers[0]
    labels = [label.text or "" for label in container]
    _xml_require(not container.attrib, "unexpected labels attributes")
    _xml_require(
        len(labels) == len(expected) and set(labels) == expected, "unexpected labels"
    )
    for label in container:
        _xml_require(
            label.tag == "label" and not label.attrib and not len(label),
            "invalid label",
        )
    return labels


def _xml_auxiliary(item: Element, labels: list[str]) -> None:
    _xml_require(
        set(item.attrib) == {"label", "value"} and not len(item), "invalid summary"
    )
    _xml_require(item.get("label") in set(labels) - {"Nr."}, "unknown summary label")
    try:
        number = float(item.get("value", ""))
    except ValueError as error:
        message = "invalid Lizard XML summary value"
        raise CheckerError(message) from error
    _xml_require(math.isfinite(number) and number >= 0, "invalid summary value")
    if item.tag == "sum":
        _xml_require(number.is_integer(), "noninteger sum")


def _xml_records(measure: Element) -> dict[str, dict[str, int]]:
    labels = _xml_labels(measure)
    records: dict[str, dict[str, int]] = {}
    for item in measure:
        _xml_require(
            item.tag in {"labels", "item", "average", "sum"}, "unexpected record"
        )
        if item.tag == "labels":
            continue
        if item.tag != "item":
            _xml_auxiliary(item, labels)
            continue
        name = item.get("name", "")
        _xml_require(
            set(item.attrib) == {"name"} and bool(name), "invalid item identity"
        )
        _xml_require(
            name not in records and len(item) == len(labels),
            "duplicate or invalid item",
        )
        values = {}
        for label, value in zip(labels, item, strict=True):
            _xml_require(
                value.tag == "value" and not value.attrib and not len(value),
                "invalid value",
            )
            values[label] = _xml_ncss(name, value.text)
        _xml_require(values["Nr."] == len(records) + 1, "invalid ordinal")
        records[name] = values
    return records


def _xml_measures(root: Element) -> dict[str, Element]:
    _xml_require(
        root.tag == "cppncss"
        and not root.attrib
        and len(root) == len({"File", "Function"}),
        "invalid root",
    )
    measures: dict[str, Element] = {}
    for measure in root:
        kind = measure.get("type", "")
        _xml_require(
            measure.tag == "measure" and set(measure.attrib) == {"type"},
            "invalid measure",
        )
        _xml_require(
            kind in {"File", "Function"} and kind not in measures, "unexpected measure"
        )
        measures[kind] = measure
    return measures


def _reconcile_xml(root: Element, functions: Sequence[FunctionMetric]) -> None:
    measures = _xml_measures(root)
    files = _xml_records(measures["File"])
    actual = _xml_records(measures["Function"])
    expected = {
        f"{item.name}(...) at {item.path}:{item.line}": item for item in functions
    }
    _xml_require(
        actual.keys() == expected.keys(), "function identities differ from CSV"
    )
    for name, metric in expected.items():
        _xml_require(
            actual[name]["NCSS"] == metric.nloc and actual[name]["CCN"] == metric.ccn,
            "function metrics differ from CSV",
        )
    for path, values in files.items():
        selected = [item for item in functions if item.path == path]
        _xml_require(
            values["Functions"] == len(selected), "file function count differs from CSV"
        )
        _xml_require(
            values["CCN"] == sum(item.ccn for item in selected),
            "file CCN differs from CSV",
        )


def _scan_sources(
    root: Path, files: Sequence[str]
) -> tuple[list[FunctionMetric], dict[str, int]]:
    with tempfile.TemporaryDirectory(prefix="tfm-lizard-") as directory:
        snapshot = Path(directory)
        aliases = {
            f"source-{index:08d}{Path(path).suffix}": path
            for index, path in enumerate(files)
        }
        for alias, path in aliases.items():
            shutil.copyfile(root / path, snapshot / alias)
        selected = set(aliases)
        functions = _function_metrics(
            _run(_lizard_command(["-V", "--csv"], list(aliases)), cwd=snapshot),
            selected,
        )
        file_nloc = _file_metrics(
            _run(_lizard_command(["--xml"], list(aliases)), cwd=snapshot),
            selected,
            functions,
        )
    mapped = [
        FunctionMetric(
            aliases[item.path],
            item.name,
            item.line,
            item.nloc,
            item.ccn,
            item.parameters,
        )
        for item in functions
    ]
    return mapped, {aliases[path]: nloc for path, nloc in file_nloc.items()}


def _collect_findings(
    functions: Sequence[FunctionMetric],
    file_nloc: dict[str, int],
    thresholds: Thresholds,
) -> list[Finding]:
    findings = []
    for function in functions:
        for metric, value, configured in (
            ("CCN", function.ccn, thresholds.ccn),
            ("function NLOC", function.nloc, thresholds.function_nloc),
            ("parameters", function.parameters, thresholds.parameters),
        ):
            if value > configured:
                message = (
                    f"{function.path!a}:{function.line}: function {function.name!a}: "
                    f"{metric} actual={value} limit={configured}"
                )
                findings.append(Finding(function.path, function.line, metric, message))
    for path, value in file_nloc.items():
        if value > thresholds.file_nloc:
            message = f"{path!a}: file NLOC actual={value} limit={thresholds.file_nloc}"
            findings.append(Finding(path, 0, "file NLOC", message))
    return sorted(findings)


def _report(
    version: str,
    thresholds: Thresholds,
    findings: Sequence[Finding],
) -> None:
    print(f"Lizard version: {version}")
    print(
        "Effective thresholds: "
        f"CCN <= {thresholds.ccn}; "
        f"function NLOC <= {thresholds.function_nloc}; "
        f"parameters <= {thresholds.parameters}; "
        f"file NLOC <= {thresholds.file_nloc}",
    )
    for finding in findings:
        print(finding.message)
    if findings:
        print(
            f"Complexity: {len(findings)} advisory finding(s); "
            "findings do not fail verification.",
        )
    else:
        print("Complexity: no advisory findings.")


def main() -> None:
    """Run the complexity checker and print its advisory report."""
    expected_version = os.environ.get("COMPLEXITY_LIZARD_VERSION", "")
    if not expected_version:
        message = "COMPLEXITY_LIZARD_VERSION must not be empty"
        raise CheckerError(message)
    thresholds = Thresholds(
        ccn=_limit("COMPLEXITY_MAX_CCN"),
        function_nloc=_limit("COMPLEXITY_MAX_FUNCTION_NLOC"),
        parameters=_limit("COMPLEXITY_MAX_PARAMETERS"),
        file_nloc=_limit("COMPLEXITY_MAX_FILE_NLOC"),
    )

    actual_version = _run(["lizard", "--version"]).strip()
    if actual_version != expected_version:
        message = (
            f"Lizard version mismatch: expected {expected_version}, "
            f"got {actual_version!r}"
        )
        raise CheckerError(message)

    root = _repository_root()
    files = _source_files(root)
    try:
        functions, file_nloc = _scan_sources(root, files)
    except (OSError, csv.Error) as error:
        message = f"cannot analyze source snapshot: {error!a}"
        raise CheckerError(message) from error
    _report(
        actual_version, thresholds, _collect_findings(functions, file_nloc, thresholds)
    )


if __name__ == "__main__":
    try:
        main()
    except CheckerError as error:
        print(f"error: complexity checker: {error}", file=sys.stderr)
        sys.exit(1)
