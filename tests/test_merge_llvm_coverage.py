"""Behavioural regressions for native LLVM coverage aggregation."""

from __future__ import annotations

import copy
import json
import tempfile
import unittest
from pathlib import Path
from typing import Any

from scripts import merge_llvm_coverage as merger

FIXTURES = Path(__file__).parent / "fixtures" / "llvm-coverage"
MACRO_DEFINITION_LINES = 2


def function(name: str, line: int, column: int, hits: int) -> dict[str, Any]:
    """Describe one LLVM instantiation in a compact export."""
    return {
        "name": name,
        "count": hits,
        "filenames": ["src/lib.rs"],
        "regions": [[line, column, line, column + 4, hits, 0, 0, 0]],
        "branches": [],
    }


def native(
    functions: list[dict[str, Any]], found: int, hit: int
) -> tuple[str, dict[str, Any]]:
    """Build a small native pair with explicit source-function summary counts."""
    entries = ["SF:src/lib.rs"]
    entries += [f"FN:{f['regions'][0][0]},{f['name']}" for f in functions]
    entries += [f"FNDA:{f['count']},{f['name']}" for f in functions]
    entries += [
        f"FNF:{found}",
        f"FNH:{hit}",
        "DA:10,3",
        "BRF:0",
        "BRH:0",
        "LF:1",
        "LH:1",
        "end_of_record",
    ]
    summary = {
        "functions": {"count": found, "covered": hit},
        "instantiations": {
            "count": len(functions),
            "covered": sum(f["count"] > 0 for f in functions),
        },
        "lines": {"count": 1, "covered": 1},
        "branches": {"count": 0, "covered": 0},
    }
    document = {
        "type": "llvm.coverage.json.export",
        "version": "3.1.0",
        "data": [
            {
                "files": [
                    {
                        "filename": "src/lib.rs",
                        "summary": summary,
                        "branches": [],
                        "expansions": [],
                        "segments": [
                            [10, 1, 3, True, True, False],
                            [10, 5, 0, False, False, False],
                        ],
                    }
                ],
                "functions": functions,
                "totals": copy.deepcopy(summary),
            }
        ],
    }
    return "\n".join(entries) + "\n", document


def external_macros(platform: str) -> tuple[str, dict[str, Any]]:
    """Move real expansion definitions to filtered platform-specific sources."""
    document = json.loads((FIXTURES / "branches.json").read_text(encoding="utf-8"))
    unit = document["data"][0]
    for record in [*unit["functions"], *unit["files"][0]["expansions"]]:
        if len(record["filenames"]) > 1:
            record["filenames"][1:] = [
                f"src/{platform}_outer.rs",
                f"src/{platform}_inner.rs",
            ]
    report = unit["files"][0]
    report["branches"] = [branch for branch in report["branches"] if branch[6] == 0]
    report["segments"] = [
        segment for segment in report["segments"] if segment[0] > MACRO_DEFINITION_LINES
    ]
    lcov = (FIXTURES / "branches.lcov").read_text(encoding="utf-8")
    removed = ("DA:1,", "DA:2,", "BRDA:1,")
    return "\n".join(
        line for line in lcov.splitlines() if not line.startswith(removed)
    ) + "\n", document


class MergeCoverageTests(unittest.TestCase):
    """Exercise the public file merger without processes or platform mocks."""

    def setUp(self) -> None:
        """Own all input and output files."""
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.output = self.root / "combined.lcov"

    def merge(self, pairs: list[tuple[str, dict[str, Any]]]) -> str:
        """Write native pairs and return the combined public report."""
        inputs = []
        for index, (lcov, document) in enumerate(pairs):
            trace = self.root / f"{index}.lcov"
            export = self.root / f"{index}.json"
            trace.write_text(lcov, encoding="utf-8")
            export.write_text(json.dumps(document), encoding="utf-8")
            inputs.append((str(index), str(trace), str(export)))
        merger.merge(inputs, self.output)
        return self.output.read_text(encoding="utf-8")

    def test_disambiguators_do_not_inflate_two_source_functions(self) -> None:
        """Three native 2/2 reports remain 2/2, including zero-hit instances."""
        pairs = [
            native(
                [
                    function(f"_RNvCs{os}a", 10, 1, 0),
                    function(f"_RNvCs{os}b", 10, 1, 3),
                    function(f"_RNvCs{os}c", 20, 1, 2),
                ],
                2,
                2,
            )
            for os in ("linux", "macos", "windows")
        ]
        result = self.merge(pairs)
        self.assertIn("FNF:2\nFNH:2\n", result)
        self.assertIn("DA:10,9\n", result)
        self.assertIn("BRF:0\nBRH:0\n", result)

    def test_shared_function_uses_coverage_union(self) -> None:
        """A function missed on one OS is covered when another executes it."""
        result = self.merge(
            [
                native([function("linux", 10, 1, 0)], 1, 0),
                native([function("macos", 10, 1, 4)], 1, 1),
            ]
        )
        self.assertIn("FNF:1\nFNH:1\n", result)

    def test_same_line_closures_remain_separate(self) -> None:
        """Overlapping ranges with different start columns are distinct functions."""
        pair = native(
            [function("outer", 10, 1, 1), function("closure", 10, 2, 0)], 2, 1
        )
        result = self.merge([pair])
        self.assertIn("FNF:2\nFNH:1\n", result)

    def test_generic_instantiations_count_as_one_function(self) -> None:
        """Generic types and build disambiguators are instantiations, not functions."""
        pair = native(
            [function("generic_u8", 10, 1, 0), function("generic_u64", 10, 1, 2)], 1, 1
        )
        self.assertIn("FNF:1\nFNH:1\n", self.merge([pair]))

    def test_excluded_json_functions_are_not_extra_source_files(self) -> None:
        """LLVM exports functions from filtered test files outside its file table."""
        pair = native([function("run", 10, 1, 1)], 1, 1)
        excluded = function("test", 1, 1, 1)
        excluded["filenames"] = ["tests/cli.rs"]
        pair[1]["data"][0]["functions"].append(excluded)
        self.assertIn("FNF:1\nFNH:1\n", self.merge([pair]))

    def test_main_view_region_need_not_be_first(self) -> None:
        """Expansion-file regions must not replace the main-view identity."""
        pair = native([function("macro", 10, 1, 1)], 1, 1)
        fn = pair[1]["data"][0]["functions"][0]
        fn["filenames"] += ["src/macro.rs"]
        fn["regions"] = [[3, 1, 3, 4, 1, 1, 0, 0], [10, 1, 10, 5, 1, 0, 1, 1]]
        pair[1]["data"][0]["files"][0]["expansions"] = [
            {
                "filenames": fn["filenames"],
                "source_region": fn["regions"][1],
                "target_regions": fn["regions"],
                "branches": [],
            }
        ]
        pair = (pair[0].replace("FN:10,", "FN:3,"), pair[1])
        self.assertIn("FN:10,", self.merge([pair]))

    def test_platform_only_files_and_windows_paths_survive(self) -> None:
        """Normalize separators while retaining the source-file union."""
        common = native([function("shared", 10, 1, 1)], 1, 1)
        windows = native([function("windows", 10, 1, 1)], 1, 1)
        windows = (windows[0].replace("src/lib.rs", "src/windows.rs"), windows[1])
        windows[1]["data"][0]["files"][0]["filename"] = "src\\windows.rs"
        windows[1]["data"][0]["functions"][0]["filenames"] = ["src\\windows.rs"]
        result = self.merge([common, windows])
        self.assertIn("SF:src/windows.rs\n", result)
        self.assertIn("SF:src/lib.rs\n", result)

    def test_checksums_are_preserved_and_conflicts_rejected(self) -> None:
        """A source checksum cannot disappear or disagree across builds."""
        pair = native([function("run", 10, 1, 1)], 1, 1)
        checked = (pair[0].replace("DA:10,3", "DA:10,3,abc"), pair[1])
        self.assertIn("DA:10,6,abc\n", self.merge([pair, checked]))
        conflict = (pair[0].replace("DA:10,3", "DA:10,3,def"), pair[1])
        with self.assertRaises(ValueError):
            self.merge([checked, conflict])

    def test_branch_ids_are_matched_by_source_position(self) -> None:
        """An OS-only condition shifts IDs without shifting shared branch identity."""
        first = native([function("linux", 10, 1, 1)], 1, 1)
        second = native([function("macos", 10, 1, 1)], 1, 1)
        for pair, branches, records, found, hit in (
            (
                first,
                [[10, 8, 10, 9, 2, 0, 0, 0, 4]],
                "BRDA:10,0,0,2\nBRDA:10,0,1,0\n",
                2,
                1,
            ),
            (
                second,
                [[10, 4, 10, 5, 0, 0, 0, 0, 4], [10, 8, 10, 9, 0, 3, 0, 0, 4]],
                "BRDA:10,0,0,-\nBRDA:10,0,1,-\nBRDA:10,1,2,0\nBRDA:10,1,3,3\n",
                4,
                1,
            ),
        ):
            data = pair[1]["data"][0]
            data["files"][0]["branches"] = branches
            data["functions"][0]["branches"] = branches
            data["files"][0]["summary"]["branches"] = {"count": found, "covered": hit}
            data["totals"]["branches"] = {"count": found, "covered": hit}
            replacement = records + f"BRF:{found}\nBRH:{hit}"
            if pair is first:
                first = (pair[0].replace("BRF:0\nBRH:0", replacement), pair[1])
            else:
                second = (pair[0].replace("BRF:0\nBRH:0", replacement), pair[1])
        result = self.merge([first, second])
        self.assertIn("BRF:4\nBRH:2\n", result)
        self.assertIn("BRDA:10,1,2,2\nBRDA:10,1,3,3\n", result)
        self.assertIn("BRDA:10,0,0,-\nBRDA:10,0,1,-\n", result)

    def test_conflicting_or_unsupported_input_fails_before_output(self) -> None:
        """Corruption and unsupported LCOV fields must never produce a report."""
        pair = native([function("run", 10, 1, 1)], 1, 1)
        for before, after in (
            ("FNF:1", "FNF:2"),
            ("FNDA:1,run", "FNDA:2,run"),
            ("LH:1", "LH:0"),
            ("BRF:0", "BRF:2"),
            ("end_of_record", ""),
            ("DA:10,3", "DA:10,-1"),
            ("LF:1", "XX:1\nLF:1"),
        ):
            with self.subTest(after=after), self.assertRaises(ValueError):
                self.merge([(pair[0].replace(before, after), pair[1])])
            self.assertFalse(self.output.exists())

    def test_json_summary_conflict_is_rejected(self) -> None:
        """Matching LCOV and reconstructed groups cannot hide a false JSON summary."""
        pair = native([function("run", 10, 1, 1)], 1, 1)
        pair[1]["data"][0]["files"][0]["summary"]["functions"]["covered"] = 0
        with self.assertRaises(ValueError):
            self.merge([pair])

    def test_platform_only_function_in_shared_file_survives(self) -> None:
        """The common function merges while a conditional function retains its miss."""
        common = native([function("shared", 10, 1, 1)], 1, 1)
        windows = native(
            [function("shared_windows", 10, 1, 2), function("windows_only", 20, 1, 0)],
            2,
            1,
        )
        self.assertIn("FNF:2\nFNH:1\n", self.merge([common, windows]))

    def test_input_order_does_not_change_combined_records(self) -> None:
        """Artifact download order cannot change identifiers, counters or checksums."""
        pairs = [
            native([function("one", 10, 1, 2)], 1, 1),
            native([function("two", 10, 1, 0), function("extra", 20, 1, 1)], 2, 1),
        ]
        self.assertEqual(self.merge(pairs), self.merge(list(reversed(pairs))))

    def test_malformed_json_schema_regions_and_totals_fail_closed(self) -> None:
        """Reject future schemas, invalid IDs, negative counters and empty coverage."""
        original = native([function("run", 10, 1, 1)], 1, 1)
        for corruption in ("schema", "file_id", "negative", "totals", "mcdc", "empty"):
            pair = copy.deepcopy(original)
            unit = pair[1]["data"][0]
            with self.subTest(corruption=corruption):
                if corruption == "schema":
                    pair[1]["version"] = "4.0.0"
                elif corruption == "file_id":
                    unit["functions"][0]["regions"][0][5] = 4
                elif corruption == "negative":
                    unit["functions"][0]["count"] = -1
                elif corruption == "totals":
                    unit["totals"]["functions"]["count"] = 2
                elif corruption == "mcdc":
                    unit["functions"][0]["mcdc_records"] = [[1]]
                else:
                    unit["files"] = []
                with self.assertRaises(ValueError):
                    self.merge([pair])
                self.assertFalse(self.output.exists())

    def test_boolean_and_float_counters_fail_before_output(self) -> None:
        """JSON booleans and integral floats must never masquerade as counters."""
        for value in (True, False, 1.0, 0.0):
            pair = native([function("run", 10, 1, int(value))], 1, int(value))
            pair[1]["data"][0]["functions"][0]["count"] = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                self.merge([pair])
            self.assertFalse(self.output.exists())

    def test_numeric_segment_flags_fail_before_output(self) -> None:
        """Changing boolean flag checks must not admit integers or floats."""
        for value in (0, 1, 0.0, 1.0):
            pair = native([function("run", 10, 1, 1)], 1, 1)
            pair[1]["data"][0]["files"][0]["segments"][0][3] = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                self.merge([pair])
            self.assertFalse(self.output.exists())

    def test_real_rust_generics_and_same_line_closures(self) -> None:
        """Real LLVM exports have five source functions and three unique DA lines."""
        pair = (
            (FIXTURES / "overlapping.lcov").read_text(encoding="utf-8"),
            json.loads((FIXTURES / "overlapping.json").read_text(encoding="utf-8")),
        )
        result = self.merge([pair])
        self.assertIn("FNF:5\nFNH:5\n", result)
        self.assertIn("LF:3\nLH:3\n", result)
        self.assertIn("DA:1,2\nDA:2,1\nDA:3,1\n", result)

    def test_real_nested_expansions_and_branch_instantiations(self) -> None:
        """Preserve real LLVM branch edges, including definition and call sites."""
        pair = (
            (FIXTURES / "branches.lcov").read_text(encoding="utf-8"),
            json.loads((FIXTURES / "branches.json").read_text(encoding="utf-8")),
        )
        result = self.merge([pair, pair])
        self.assertIn("FNF:2\nFNH:2\n", result)
        self.assertIn("BRF:8\nBRH:5\n", result)
        self.assertIn("LF:4\nLH:4\n", result)
        self.assertIn("BRDA:3,0,0,2\nBRDA:3,0,1,2\n", result)
        self.assertIn("BRDA:3,1,2,2\nBRDA:3,1,3,0\n", result)

    def test_expansion_counter_conflicts_fail_before_output(self) -> None:
        """An LCOV/expansion agreement cannot override its owning function counter."""
        document = json.loads((FIXTURES / "branches.json").read_text(encoding="utf-8"))
        document["data"][0]["files"][0]["expansions"][0]["branches"][0][4] = 200
        lcov = (FIXTURES / "branches.lcov").read_text(encoding="utf-8")
        lcov = lcov.replace("BRDA:3,1,2,1", "BRDA:3,1,2,200")
        with self.assertRaises(ValueError):
            self.merge([(lcov, document)])
        self.assertFalse(self.output.exists())

    def test_missing_expansions_fail_before_output(self) -> None:
        """Missing call-site branches must be detected from the function mapping."""
        document = json.loads((FIXTURES / "branches.json").read_text(encoding="utf-8"))
        document["data"][0]["files"][0]["expansions"] = []
        lcov = (FIXTURES / "branches.lcov").read_text(encoding="utf-8")
        lcov = (
            "\n".join(
                line for line in lcov.splitlines() if not line.startswith("BRDA:3,1,")
            )
            + "\n"
        )
        with self.assertRaises(ValueError):
            self.merge([(lcov, document)])
        self.assertFalse(self.output.exists())

    def test_platform_macro_definition_files_remain_distinct(self) -> None:
        """Equal coordinates in distinct definition files must not collapse edges."""
        pairs = [external_macros("linux"), external_macros("windows")]
        result = self.merge(pairs)
        self.assertIn("BRF:8\nBRH:5\n", result)
        self.assertEqual(result, self.merge(list(reversed(pairs))))

    def test_intermediate_expansion_files_remain_distinct(self) -> None:
        """Separate wrapper definitions retain their paths to a shared inner macro."""
        pairs = [external_macros("linux"), external_macros("windows")]
        for _, document in pairs:
            unit = document["data"][0]
            for record in [*unit["functions"], *unit["files"][0]["expansions"]]:
                if len(record["filenames"]) > 1:
                    record["filenames"][2] = "src/shared_inner.rs"
        self.assertIn("BRF:8\nBRH:5\n", self.merge(pairs))


if __name__ == "__main__":
    unittest.main()
