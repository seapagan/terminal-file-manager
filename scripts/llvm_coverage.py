"""LLVM export validation and source identities used by the LCOV merger.

LLVM 23.1.1 CoverageMapping.cpp defines main-view files and instantiation
sets; CoverageExporterLcov.cpp defines branch sorting and true/false IDs.
Native summaries count instantiations by source group, so overlapping
functions can contribute a source line more than once. Combined LCOV
summaries instead describe its unique line and source-edge records.
"""

from __future__ import annotations

import json
from itertools import groupby
from pathlib import Path, PurePosixPath
from typing import Any

Location = tuple[int, int]
BranchKey = tuple[
    tuple[Location, ...], tuple[int, ...], tuple[str, int, int], tuple[str, ...]
]
REGION_WIDTH = 8
BRANCH_WIDTH = 9
SEGMENT_WIDTH = 6
PERCENT_TOLERANCE = 0.01
EXPANSION = 1
BRANCH_KINDS = {4, 6}


def require(condition: object, message: str) -> None:
    """Reject untrustworthy or unsupported coverage instead of approximating it."""
    if not condition:
        raise ValueError(message)


def number(value: object) -> int:
    """Require an unsigned JSON integer, excluding bool and float coercions."""
    require(
        isinstance(value, int) and not isinstance(value, bool) and value >= 0,
        f"Invalid LLVM counter: {value!r}",
    )
    return int(value) if isinstance(value, int) else 0


def source_path(value: str) -> str:
    """Normalize native separators while rejecting aliases and external sources."""
    require(isinstance(value, str), "Invalid source path")
    normalized = value.replace("\\", "/")
    path = PurePosixPath(normalized)
    require(
        normalized.startswith("src/")
        and normalized == path.as_posix()
        and ".." not in path.parts,
        f"Invalid coverage source path: {value}",
    )
    require(
        not any(char in normalized for char in "\r\n\0"),
        "Invalid source path characters",
    )
    return normalized


def summary_pair(summary: dict[str, Any]) -> tuple[int, int]:
    """Validate a native LLVM summary, including optional redundant fields."""
    count, covered = number(summary["count"]), number(summary["covered"])
    require(covered <= count, "Covered summary exceeds found")
    if "notcovered" in summary:
        require(
            number(summary["notcovered"]) == count - covered,
            "Invalid notcovered summary",
        )
    if "percent" in summary:
        percent = summary["percent"]
        require(
            isinstance(percent, (int, float))
            and not isinstance(percent, bool)
            and abs(percent - (100 * covered / count if count else 0))
            < PERCENT_TOLERANCE,
            "Invalid percentage summary",
        )
    return count, covered


def validate_function(fn: dict[str, Any]) -> None:
    """Validate region references before deriving any source identity."""
    require(
        isinstance(fn["name"], str) and bool(fn["name"]), "Invalid LLVM function name"
    )
    number(fn["count"])
    names = fn["filenames"]
    require(isinstance(names, list) and bool(names), "Missing function filenames")
    require(
        all(isinstance(name, str) and name for name in names),
        "Invalid function filenames",
    )
    fn["filenames"] = [name.replace("\\", "/") for name in names]
    require(bool(fn["regions"]), "Missing function regions")
    for region in fn["regions"]:
        validate_region(region, names)
    require(
        not fn.get("mcdc_records"),
        "MCDC coverage cannot be represented in this LCOV format",
    )


def validate_region(region: list[int], names: list[str]) -> None:
    """Validate one code, skipped, gap or expansion region."""
    require(
        isinstance(region, list) and len(region) == REGION_WIDTH,
        "Unsupported region layout",
    )
    for value in region:
        number(value)
    require(
        region[0] > 0 and region[1] > 0 and tuple(region[:2]) <= tuple(region[2:4]),
        "Invalid region coordinates",
    )
    require(region[5] < len(names) and region[6] < len(names), "Invalid region file ID")
    require(region[7] in {0, 1, 2, 3}, "Unsupported function region kind")


def function_location(fn: dict[str, Any]) -> tuple[str, Location]:
    """Use the first region in the first file that is not an expansion target."""
    main = main_file_id(fn)
    region = next((region for region in fn["regions"] if region[5] == main), None)
    require(region is not None, "Function has no main-view region")
    if region is None:
        message = "Missing main-view region"
        raise ValueError(message)
    return fn["filenames"][main], (region[0], region[1])


def main_file_id(fn: dict[str, Any]) -> int:
    """Find LLVM's first file ID that is not an expansion target."""
    expanded = {region[6] for region in fn["regions"] if region[7] == EXPANSION}
    main = next(
        (index for index in range(len(fn["filenames"])) if index not in expanded), None
    )
    require(main is not None, "Function has no main-view file")
    if main is None:
        message = "Missing main-view file"
        raise ValueError(message)
    return main


def read_export(
    path: str,
) -> tuple[dict[str, dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    """Require one full LLVM export, matching source/totals summaries."""
    doc = json.loads(
        Path(path).read_text(encoding="utf-8"), object_pairs_hook=unique_object
    )
    require(
        doc["type"] == "llvm.coverage.json.export" and doc["version"] == "3.1.0",
        "Unsupported LLVM JSON schema",
    )
    require(
        isinstance(doc["data"], list) and len(doc["data"]) == 1,
        "Expected one coverage export unit",
    )
    unit = doc["data"][0]
    files = {}
    require(
        isinstance(unit["files"], list) and bool(unit["files"]), "Empty LLVM file table"
    )
    for report in unit["files"]:
        source = source_path(report["filename"])
        require(source not in files, f"Duplicate JSON source: {source}")
        require(not report.get("mcdc_records"), "Unsupported MCDC file coverage")
        files[source] = report
    functions = unit["functions"]
    require(isinstance(functions, list), "Missing LLVM function table")
    for fn in functions:
        validate_function(fn)
        function_location(fn)
    require(
        number(unit["totals"]["functions"]["count"]) > 0,
        "Empty native function coverage",
    )
    for category in ("functions", "instantiations", "lines", "branches"):
        pairs = [summary_pair(report["summary"][category]) for report in files.values()]
        require(
            summary_pair(unit["totals"][category])
            == (sum(pair[0] for pair in pairs), sum(pair[1] for pair in pairs)),
            f"LLVM totals conflict: {category}",
        )
    return files, functions, unit["totals"]


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    """Reject duplicate JSON keys rather than accepting the last counter."""
    result: dict[str, Any] = {}
    for key, value in pairs:
        require(key not in result, f"Duplicate JSON key: {key}")
        result[key] = value
    return result


def line_segments(segments: list[list[Any]]) -> dict[int, list[list[Any]]]:
    """Validate the sorted native file segment stream."""
    byline: dict[int, list[list[Any]]] = {}
    previous = (0, 0)
    for segment in segments:
        require(len(segment) == SEGMENT_WIDTH, "Unsupported segment layout")
        line, column, count = (number(value) for value in segment[:3])
        require(
            line > 0 and column > 0 and (line, column) > previous,
            "Unsorted or duplicate segments",
        )
        require(
            all(isinstance(value, bool) for value in segment[3:]),
            "Invalid segment flags",
        )
        byline.setdefault(line, []).append([line, column, count, *segment[3:]])
        previous = line, column
    return byline


def line_stat(current: list[list[Any]], wrapped: list[Any] | None) -> int | None:
    """Apply LLVM's mapped-line and maximum region-entry counter rules."""
    entries = [segment for segment in current if region_entry(segment)]
    skipped = bool(current and skipped_entry(current[0]))
    wrapped_count, has_count = (wrapped[2], wrapped[3]) if wrapped else (0, False)
    if not entries and (skipped or not has_count):
        return None
    counts = [segment[2] for segment in entries if not segment[5]]
    return int(max([wrapped_count, *counts]))


def region_entry(segment: list[Any]) -> bool:
    """Identify a region entry carrying a counter, including gap entries."""
    return bool(segment[3] and segment[4])


def skipped_entry(segment: list[Any]) -> bool:
    """Identify a skipped region entry without a counter."""
    return bool(not segment[3] and segment[4])


def line_counts(segments: list[list[Any]]) -> dict[int, int]:
    """Reproduce LLVM LineCoverageStats from exported file segments."""
    byline = line_segments(segments)
    result = {}
    wrapped = None
    for line in range(1, max(byline, default=0) + 1):
        current = byline.get(line, [])
        count = line_stat(current, wrapped)
        if count is not None:
            result[line] = count
        if current:
            wrapped = current[-1]
    return result


def branch_region(region: list[int], names: list[str]) -> None:
    """Require the supported LLVM true/false branch representation."""
    require(len(region) == BRANCH_WIDTH, "Unsupported branch layout")
    for value in region:
        number(value)
    require(
        region[0] > 0 and region[1] > 0 and tuple(region[:2]) <= tuple(region[2:4]),
        "Invalid branch coordinates",
    )
    require(region[6] < len(names) and region[7] < len(names), "Invalid branch file ID")
    require(region[8] in BRANCH_KINDS, "Unsupported branch kind")


def function_owner(fn: dict[str, Any]) -> tuple[str, int, int]:
    """Identify the owning source function without build-specific symbols."""
    filename, (line, column) = function_location(fn)
    return filename, line, column


def direct_branches(
    report: dict[str, Any], functions: list[dict[str, Any]]
) -> list[tuple[BranchKey, int, int]]:
    """Reproduce LLVM's function-ordered file branches, keeping owner context."""
    source = source_path(report["filename"])
    regions = []
    result = []
    for fn in functions:
        for region in fn["branches"]:
            branch_region(region, fn["filenames"])
            if fn["filenames"][region[6]] == source:
                regions.append(region)
                key: BranchKey = (
                    ((region[0], region[1]),),
                    tuple(region[:4]),
                    function_owner(fn),
                    (fn["filenames"][region[6]],),
                )
                result.append((key, region[4], region[5]))
    require(
        regions == report["branches"],
        "File/function branch conflict or unsupported folded branch",
    )
    return result


def expansion_owner(
    expansion: dict[str, Any], functions: list[dict[str, Any]]
) -> tuple[str, int, int]:
    """Resolve the complete expansion mapping before matching nested branches."""
    owners = {
        function_owner(fn)
        for fn in functions
        if fn["filenames"] == expansion["filenames"]
        and fn["regions"] == expansion["target_regions"]
    }
    require(len(owners) == 1, "Ambiguous or missing expansion function identity")
    return next(iter(owners))


def expanded_branches(
    fn: dict[str, Any], file_id: int, visited: frozenset[int]
) -> list[list[int]]:
    """Reproduce LLVM's depth-first nested expansion branch collection."""
    require(file_id not in visited, "Cyclic expansion file graph")
    result = []
    for region in fn["regions"]:
        if region[5] == file_id and region[7] == EXPANSION:
            result.extend(expanded_branches(fn, region[6], visited | {file_id}))
    result.extend(branch for branch in fn["branches"] if branch[6] == file_id)
    return result


def validate_expansions(
    report: dict[str, Any], functions: list[dict[str, Any]]
) -> None:
    """Require the complete expansion export derived from owning function records."""
    source = source_path(report["filename"])
    expected: list[dict[str, Any]] = []
    for fn in functions:
        main = main_file_id(fn)
        if fn["filenames"][main] != source:
            continue
        expected.extend(
            {
                "filenames": fn["filenames"],
                "source_region": region,
                "target_regions": fn["regions"],
                "branches": expanded_branches(fn, region[6], frozenset()),
            }
            for region in fn["regions"]
            if region[5] == main and region[7] == EXPANSION
        )
    for expansion in report["expansions"]:
        expansion["filenames"] = [
            name.replace("\\", "/") for name in expansion["filenames"]
        ]
    require(expected == report["expansions"], "Expansion/function coverage conflict")


def expansion_path(
    region: list[int], expansion: dict[str, Any]
) -> tuple[tuple[Location, ...], tuple[str, ...]]:
    """Recover nested expansion call sites from the exported region file graph."""
    root = expansion["source_region"]
    edges: dict[int, list[int]] = {}
    for mapped in expansion["target_regions"]:
        if mapped[7] == EXPANSION:
            require(mapped[6] not in edges, "Ambiguous expansion file graph")
            edges[mapped[6]] = mapped
    path = []
    names = expansion["filenames"]
    filenames = []
    file_id = region[6]
    visited = set()
    while file_id != root[6]:
        require(
            file_id not in visited and file_id in edges,
            "Invalid nested branch expansion",
        )
        visited.add(file_id)
        edge = edges[file_id]
        path.append((edge[0], edge[1]))
        filenames.append(names[edge[5]])
        file_id = edge[5]
    return (
        ((root[0], root[1]), *reversed(path)),
        (names[root[5]], *reversed(filenames), names[region[6]]),
    )


def native_branches(
    report: dict[str, Any], functions: list[dict[str, Any]]
) -> list[tuple[BranchKey, int, int]]:
    """Recover LLVM's stable sort paths and source identities for branch pairs."""
    result = direct_branches(report, functions)
    validate_expansions(report, functions)
    for expansion in report["expansions"]:
        names = [name.replace("\\", "/") for name in expansion["filenames"]]
        expansion["filenames"] = names
        owner = expansion_owner(expansion, functions)
        validate_region(expansion["source_region"], names)
        require(
            expansion["source_region"] in expansion["target_regions"]
            and expansion["source_region"][7] == EXPANSION,
            "Invalid expansion source region",
        )
        for region in expansion["branches"]:
            branch_region(region, names)
            path, filenames = expansion_path(region, expansion)
            key = (path, tuple(region[:4]), owner, filenames)
            result.append((key, region[4], region[5]))
    # LLVM sorts by expansion path, retaining input order at identical paths.
    return sorted(result, key=lambda item: item[0][0])


def reconcile_branches(
    report: dict[str, Any],
    functions: list[dict[str, Any]],
    records: dict[tuple[int, int, int], int | None],
) -> dict[BranchKey, tuple[int, int]]:
    """Verify each native BRDA before unifying equivalent source edges."""
    native = native_branches(report, functions)
    expected = branch_records(native)
    unified = []
    for _, group in groupby(native, key=lambda item: item[0][0]):
        items = list(group)
        unified.append(
            (
                items[0][0],
                sum(item[1] for item in items),
                sum(item[2] for item in items),
            )
        )
    require(
        records == expected or records == branch_records(unified),
        "BRDA/LLVM JSON conflict (folded or unknown branch exports are unsupported)",
    )
    validate_branch_summary(report, functions)
    merged: dict[BranchKey, tuple[int, int]] = {}
    for key, true, false in native:
        old_true, old_false = merged.get(key, (0, 0))
        merged[key] = old_true + true, old_false + false
    return merged


def branch_records(
    branches: list[tuple[BranchKey, int, int]],
) -> dict[tuple[int, int, int], int | None]:
    """Reproduce LLVM's per-line true/false pair enumeration."""
    records = {}
    pairs: dict[int, int] = {}
    for key, true, false in branches:
        line = key[0][0][0]
        pair = pairs.get(line, 0)
        for edge, count in enumerate((true, false)):
            records[line, pair, pair * 2 + edge] = count if true or false else None
        pairs[line] = pair + 1
    return records


def validate_branch_summary(
    report: dict[str, Any], functions: list[dict[str, Any]]
) -> None:
    """LLVM takes maximum found/hit branch totals within each instantiation set."""
    groups: dict[Location, tuple[int, int]] = {}
    source = source_path(report["filename"])
    for fn in functions:
        filename, location = function_location(fn)
        if filename != source:
            continue
        branches = fn["branches"]
        for region in branches:
            branch_region(region, fn["filenames"])
        found = len(branches) * 2
        hit = sum(region[4] > 0 for region in branches) + sum(
            region[5] > 0 for region in branches
        )
        old_found, old_hit = groups.get(location, (0, 0))
        groups[location] = max(found, old_found), max(hit, old_hit)
    require(
        summary_pair(report["summary"]["branches"])
        == (
            sum(pair[0] for pair in groups.values()),
            sum(pair[1] for pair in groups.values()),
        ),
        "Native branch summary conflict or unsupported folded branch",
    )
