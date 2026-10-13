"""Validate and combine native LLVM JSON/LCOV pairs by source coverage identity.

Usage: python -m scripts.merge_llvm_coverage --input PLATFORM LCOV JSON ...
       --output coverage.lcov
The stdout JSON records native summaries and combined record-based statistics.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from scripts import llvm_coverage as llvm

DA_CHECKSUM_FIELDS = 3
BRDA_FIELDS = 4


@dataclass
class Trace:
    """One complete LLVM LCOV source record."""

    functions: dict[str, int] = field(default_factory=dict)
    hits: dict[str, int] = field(default_factory=dict)
    lines: dict[int, tuple[int, str]] = field(default_factory=dict)
    branches: dict[tuple[int, int, int], int | None] = field(default_factory=dict)
    summary: dict[str, int] = field(default_factory=dict)


def integer(text: str) -> int:
    """Reject negative, fractional and missing LCOV counters."""
    llvm.require(text.isascii() and text.isdigit(), f"Invalid LCOV integer: {text!r}")
    return int(text)


def insert(mapping: dict[Any, Any], key: object, value: object) -> None:
    """Reject repeated records instead of silently overwriting coverage."""
    llvm.require(key not in mapping, f"Duplicate LCOV record: {key}")
    mapping[key] = value


def parse_field(trace: Trace, tag: str, value: str) -> None:
    """Read only supported LLVM LCOV fields, including optional DA checksums."""
    if tag in {"FNF", "FNH", "LF", "LH", "BRF", "BRH"}:
        insert(trace.summary, tag, integer(value))
    elif tag in {"FN", "FNDA"}:
        counter, name = value.split(",", 1)
        llvm.require(bool(name), "Empty function name")
        insert(trace.functions if tag == "FN" else trace.hits, name, integer(counter))
    elif tag == "DA":
        parts = value.split(",")
        llvm.require(len(parts) in {2, 3}, "Invalid DA record")
        line, count = integer(parts[0]), integer(parts[1])
        llvm.require(line > 0, "Invalid DA line")
        insert(
            trace.lines,
            line,
            (count, parts[2] if len(parts) == DA_CHECKSUM_FIELDS else ""),
        )
    elif tag == "BRDA":
        parts = value.split(",")
        llvm.require(len(parts) == BRDA_FIELDS, "Invalid BRDA record")
        key = tuple(integer(part) for part in parts[:3])
        llvm.require(key[0] > 0, "Invalid BRDA line")
        insert(trace.branches, key, None if parts[3] == "-" else integer(parts[3]))
    else:
        message = f"Unsupported LCOV record: {tag}"
        raise ValueError(message)


def read_lcov(path: str) -> dict[str, Trace]:
    """Require nonempty, complete, unique source records and all LLVM summaries."""
    sources: dict[str, Trace] = {}
    trace = None
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        if line == "TN:":
            llvm.require(trace is None, "TN inside source record")
        elif line.startswith("SF:"):
            llvm.require(trace is None, "Unterminated source record")
            trace = Trace()
            insert(sources, llvm.source_path(line[3:]), trace)
        elif line == "end_of_record":
            llvm.require(trace is not None, "Unexpected end_of_record")
            if trace is not None:
                llvm.require(
                    set(trace.summary) == {"FNF", "FNH", "LF", "LH", "BRF", "BRH"},
                    "Missing LCOV summaries",
                )
                llvm.require(
                    trace.functions.keys() == trace.hits.keys(), "FN/FNDA mismatch"
                )
            trace = None
        else:
            llvm.require(
                trace is not None and ":" in line, "Expected LCOV source record"
            )
            if trace is not None:
                parse_field(trace, *line.split(":", 1))
    llvm.require(trace is None and bool(sources), "Incomplete or empty LCOV input")
    return sources


def reconcile(
    trace: Trace, source: str, report: dict[str, Any], functions: list[dict[str, Any]]
) -> dict[tuple[int, int], int]:
    """Compare native records and source-function groups with LLVM's own summaries."""
    summary = report["summary"]
    for category, found, hit in (
        ("functions", "FNF", "FNH"),
        ("lines", "LF", "LH"),
        ("branches", "BRF", "BRH"),
    ):
        llvm.require(
            llvm.summary_pair(summary[category])
            == (trace.summary[found], trace.summary[hit]),
            f"{source}: JSON/LCOV {category} conflict",
        )
    reconcile_symbols(trace, source, functions)
    instances = [fn for fn in functions if llvm.function_location(fn)[0] == source]
    groups = source_groups(instances)
    llvm.require(
        (len(groups), sum(count > 0 for count in groups.values()))
        == llvm.summary_pair(summary["functions"]),
        f"{source}: function grouping conflict",
    )
    llvm.require(
        (len(instances), sum(fn["count"] > 0 for fn in instances))
        == llvm.summary_pair(summary["instantiations"]),
        f"{source}: instantiation summary conflict",
    )
    llvm.require(
        llvm.line_counts(report["segments"])
        == {line: count for line, (count, _) in trace.lines.items()},
        f"{source}: DA/JSON conflict",
    )
    return groups


def reconcile_symbols(
    trace: Trace, source: str, functions: list[dict[str, Any]]
) -> None:
    """Check every native instantiation name, start line and execution count."""
    raw = [fn for fn in functions if fn["filenames"][0] == source]
    llvm.require(
        len({fn["name"] for fn in raw}) == len(raw),
        f"{source}: duplicate LLVM function symbols",
    )
    llvm.require(
        {fn["name"]: fn["regions"][0][0] for fn in raw} == trace.functions,
        f"{source}: FN/JSON conflict",
    )
    llvm.require(
        {fn["name"]: fn["count"] for fn in raw} == trace.hits,
        f"{source}: FNDA/JSON conflict",
    )


def source_groups(functions: list[dict[str, Any]]) -> dict[tuple[int, int], int]:
    """Sum instantiation counts by LLVM's main-view source location."""
    groups: dict[tuple[int, int], int] = {}
    for fn in functions:
        _, location = llvm.function_location(fn)
        groups[location] = groups.get(location, 0) + fn["count"]
    return groups


def merge_lines(destination: Trace, trace: Trace) -> None:
    """Sum native line counters and retain compatible optional source checksums."""
    for line, (count, checksum) in trace.lines.items():
        old_count, old_checksum = destination.lines.get(line, (0, ""))
        llvm.require(
            not checksum or not old_checksum or checksum == old_checksum,
            f"Conflicting checksum at line {line}",
        )
        destination.lines[line] = (old_count + count, checksum or old_checksum)


def record_summary(trace: Trace) -> dict[str, int]:
    """Count the combined report's unique records and covered records."""
    return {
        "FNF": len(trace.functions),
        "FNH": sum(h > 0 for h in trace.hits.values()),
        "LF": len(trace.lines),
        "LH": sum(h > 0 for h, _ in trace.lines.values()),
        "BRF": len(trace.branches),
        "BRH": sum(h is not None and h > 0 for h in trace.branches.values()),
    }


def render(source: str, trace: Trace) -> str:
    """Emit sorted LCOV with summaries calculated from its combined records."""
    records = ["TN:", f"SF:{source}"]
    records += [f"FN:{line},{name}" for name, line in sorted(trace.functions.items())]
    records += [f"FNDA:{hits},{name}" for name, hits in sorted(trace.hits.items())]
    trace.summary = record_summary(trace)
    records += [f"FNF:{trace.summary['FNF']}", f"FNH:{trace.summary['FNH']}"]
    for line, (count, checksum) in sorted(trace.lines.items()):
        suffix = f",{checksum}" if checksum else ""
        records.append(f"DA:{line},{count}{suffix}")
    for (line, block, branch), taken in sorted(trace.branches.items()):
        records.append(
            f"BRDA:{line},{block},{branch},{'-' if taken is None else taken}"
        )
    records += [f"{tag}:{trace.summary[tag]}" for tag in ("BRF", "BRH", "LF", "LH")]
    return "\n".join([*records, "end_of_record"]) + "\n"


def combine_branches(
    branches: dict[llvm.BranchKey, tuple[int, int]],
) -> dict[tuple[int, int, int], int | None]:
    """Allocate deterministic LCOV IDs after merging true/false source edges."""
    result = {}
    pairs: dict[int, int] = {}
    for key, (true, false) in sorted(branches.items()):
        line = key[0][0][0]
        pair = pairs.get(line, 0)
        for edge, count in enumerate((true, false)):
            result[line, pair, pair * 2 + edge] = count if true or false else None
        pairs[line] = pair + 1
    return result


def merge(inputs: list[tuple[str, str, str]], output: str | Path) -> dict[str, Any]:
    """Validate every input before writing the combined report or its statistics."""
    llvm.require(
        bool(inputs) and len({p for p, _, _ in inputs}) == len(inputs),
        "Empty or duplicate platforms",
    )
    combined: dict[str, Trace] = {}
    branch_union: dict[str, dict[llvm.BranchKey, tuple[int, int]]] = {}
    stats = {}
    for platform, lcov_path, json_path in inputs:
        sources = read_lcov(lcov_path)
        files, functions, totals = llvm.read_export(json_path)
        llvm.require(
            sources.keys() == files.keys(),
            f"{platform}: JSON/LCOV source union conflict",
        )
        stats[platform] = totals
        for source, trace in sources.items():
            groups = reconcile(trace, source, files[source], functions)
            branches = llvm.reconcile_branches(files[source], functions, trace.branches)
            destination = combined.setdefault(source, Trace())
            merge_lines(destination, trace)
            for (line, column), count in groups.items():
                name = f"llvm_source_{line}_{column}"
                destination.functions[name] = line
                destination.hits[name] = destination.hits.get(name, 0) + count
            target = branch_union.setdefault(source, {})
            for key, (true, false) in branches.items():
                old_true, old_false = target.get(key, (0, 0))
                target[key] = old_true + true, old_false + false
    for source, destination in combined.items():
        destination.branches = combine_branches(branch_union[source])
    content = "".join(render(source, combined[source]) for source in sorted(combined))
    Path(output).write_text(content, encoding="utf-8")
    return {
        "native": stats,
        "combined": {
            source: trace.summary for source, trace in sorted(combined.items())
        },
    }


def main() -> None:
    """Merge native artifact pairs and report precise diagnostic failures."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--input",
        nargs=3,
        action="append",
        required=True,
        metavar=("PLATFORM", "LCOV", "JSON"),
    )
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    try:
        stats = merge([tuple(value) for value in args.input], args.output)
    except (ValueError, KeyError, TypeError, IndexError, OSError) as error:
        parser.exit(1, f"Coverage merge failed: {error}\n")
    sys.stdout.write(json.dumps(stats, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
