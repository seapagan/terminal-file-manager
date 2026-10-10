"""Analyse a Cargo package archive and render its publishability report."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tarfile
from dataclasses import asdict, dataclass
from pathlib import Path, PurePosixPath
from typing import Any

MIN_MEMBER_PARTS = 2
PACKAGE_LIMIT_BYTES = 9_500_000
MARKER = "<!-- tfm-package-checks -->"
REPOSITORY_ROOT = Path(__file__).parents[1]
STATIC_REQUIRED_PATHS = (
    ".cargo_vcs_info.json",
    "Cargo.lock",
    "Cargo.toml",
    "Cargo.toml.orig",
    "LICENSE-APACHE",
    "LICENSE-MIT",
    "README.md",
)
FORBIDDEN_PREFIXES = (
    ".github",
    ".vscode",
    "docs",
    "specs",
    "plans",
    "reviews",
    "scripts",
    "target",
    "AGENTS.md",
    ".gitattributes",
    ".gitignore",
    ".markdownlint.yaml",
    ".rustfmt.toml",
    "CHANGELOG.md",
    "Makefile.toml",
    "TODO.md",
    "clippy.toml",
    "deny.toml",
    "renovate.json",
)


class ArchiveError(Exception):
    """An operational failure that prevents trustworthy package analysis."""


def distribution_path(path: str) -> bool:
    """Identify tracked source files and intended root Rust integration tests."""
    return path.startswith("src/") or (
        path.startswith("tests/") and path.count("/") == 1 and path.endswith(".rs")
    )


def forbidden_member(path: str, files: set[str], required: tuple[str, ...]) -> bool:
    """Reject private roots and files outside the actual intended inventory."""
    if any(
        path == prefix or path.startswith(f"{prefix}/") for prefix in FORBIDDEN_PREFIXES
    ):
        return True
    if path in files:
        return path not in required
    return not (path in {"src", "tests"} or path.startswith("src/"))


def tracked_distribution_path(entry: bytes) -> str:
    """Validate one Git index entry and return its archive-form path."""
    try:
        metadata, raw_path = entry.split(b"\t", 1)
        mode, _object_id, stage = metadata.split(b" ", 2)
        path = raw_path.decode("utf-8")
    except (UnicodeDecodeError, ValueError) as error:
        message = "invalid tracked distribution entry from git"
        raise ArchiveError(message) from error
    if stage != b"0":
        message = f"unmerged tracked distribution entry: {path}"
        raise ArchiveError(message)
    if mode not in {b"100644", b"100755"}:
        mode_label = mode.decode(errors="replace")
        message = f"unsupported tracked distribution entry mode {mode_label}: {path}"
        raise ArchiveError(message)
    normalized = PurePosixPath(path).as_posix()
    if normalized != path or "\\" in path or not distribution_path(path):
        message = f"invalid tracked distribution path: {path!r}"
        raise ArchiveError(message)
    return normalized


def repository_distribution_paths(repository: Path) -> tuple[str, ...]:
    """Return tracked regular files in the distributable repository trees."""
    command = (
        "git",
        "-C",
        str(repository),
        "ls-files",
        "--stage",
        "-z",
        "--",
        "src",
        ":(glob)tests/*.rs",
    )
    try:
        # Controlled Git argv, no shell; reads the caller-selected repository index.
        completed = subprocess.run(  # noqa: S603
            command,
            check=True,
            capture_output=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", b"")
        message = detail.decode("utf-8", errors="replace").strip()
        suffix = f": {message}" if message else ""
        message = f"cannot inspect tracked distribution files in {repository}{suffix}"
        raise ArchiveError(message) from error

    paths: set[str] = set()
    for entry in completed.stdout.split(b"\0"):
        if entry:
            paths.add(tracked_distribution_path(entry))
    return tuple(sorted(paths))


def required_paths(repository: Path = REPOSITORY_ROOT) -> tuple[str, ...]:
    """Combine special package files with repository-derived runtime files."""
    return STATIC_REQUIRED_PATHS + repository_distribution_paths(repository)


@dataclass(frozen=True)
class PackageResult:
    """Structured package policy results."""

    archive: str
    archive_root: str
    size_bytes: int
    limit_bytes: int
    headroom_bytes: int
    exceeded_by_bytes: int
    required_paths: tuple[str, ...]
    missing_paths: tuple[str, ...]
    forbidden_prefixes: tuple[str, ...]
    forbidden_paths: tuple[str, ...]

    @property
    def size_ok(self) -> bool:
        """Return whether the archive is within the configured ceiling."""
        return self.size_bytes <= self.limit_bytes

    @property
    def contents_ok(self) -> bool:
        """Return whether all required and forbidden-path checks pass."""
        return not self.missing_paths and not self.forbidden_paths

    @property
    def policy_ok(self) -> bool:
        """Return whether every blocking package policy passes."""
        return self.size_ok and self.contents_ok

    def to_dict(self) -> dict[str, Any]:
        """Return deterministic JSON-ready data, including derived status."""
        return {
            **asdict(self),
            "contents_ok": self.contents_ok,
            "policy_ok": self.policy_ok,
            "size_ok": self.size_ok,
        }


def discover_archive(package_directory: Path) -> Path:
    """Return the only current crate archive in a package directory."""
    try:
        archives = sorted(
            path
            for path in package_directory.iterdir()
            if path.is_file() and path.suffix == ".crate"
        )
    except OSError as error:
        message = f"cannot inspect package directory {package_directory}: {error}"
        raise ArchiveError(message) from error
    if len(archives) != 1:
        message = (
            f"expected exactly one .crate in {package_directory}, found {len(archives)}"
        )
        raise ArchiveError(message)
    return archives[0]


def normalized_member_path(name: str) -> tuple[str, str]:
    """Validate a tar member and return its archive root and relative path."""
    if not name or "\\" in name:
        message = f"invalid archive member path: {name!r}"
        raise ArchiveError(message)
    path = PurePosixPath(name)
    if path.is_absolute() or ".." in path.parts:
        message = f"unsafe archive member path: {name!r}"
        raise ArchiveError(message)
    parts = tuple(part for part in path.parts if part not in ("", "."))
    if len(parts) < MIN_MEMBER_PARTS:
        message = f"member is not beneath one archive root: {name!r}"
        raise ArchiveError(message)
    return parts[0], PurePosixPath(*parts[1:]).as_posix()


def package_members(package: tarfile.TarFile) -> list[tarfile.TarInfo]:
    """Read all tar members and reject an empty package."""
    members = package.getmembers()
    if not members:
        message = "package archive is empty"
        raise ArchiveError(message)
    return members


def record_member(
    member: tarfile.TarInfo, roots: set[str], seen: set[str], files: set[str]
) -> None:
    """Validate one member and record its root, identity and regular-file status."""
    root, relative = normalized_member_path(member.name)
    roots.add(root)
    if relative in seen:
        message = f"duplicate archive member path: {relative}"
        raise ArchiveError(message)
    seen.add(relative)
    if member.isfile():
        files.add(relative)
    elif not member.isdir():
        message = f"unsupported archive member type: {member.name}"
        raise ArchiveError(message)


def archive_contents(archive: Path) -> tuple[str, set[str], set[str]]:
    """Read the archive, returning its root, files, and all member paths."""
    roots: set[str] = set()
    seen: set[str] = set()
    files: set[str] = set()
    try:
        with tarfile.open(archive, mode="r:gz") as package:
            for member in package_members(package):
                record_member(member, roots, seen, files)
    except ArchiveError:
        raise
    except (OSError, tarfile.TarError) as error:
        message = f"cannot read package archive {archive}: {error}"
        raise ArchiveError(message) from error
    if len(roots) != 1:
        message = f"expected one archive root, found {len(roots)}: {sorted(roots)}"
        raise ArchiveError(message)
    return roots.pop(), files, seen


def analyse_archive(
    archive: Path,
    limit_bytes: int = PACKAGE_LIMIT_BYTES,
    repository: Path = REPOSITORY_ROOT,
) -> PackageResult:
    """Measure an archive and evaluate all package-content policies."""
    if limit_bytes < 0:
        message = "package limit must not be negative"
        raise ValueError(message)
    try:
        size_bytes = archive.stat().st_size
    except OSError as error:
        message = f"cannot measure package archive {archive}: {error}"
        raise ArchiveError(message) from error
    root, files, members = archive_contents(archive)
    required = required_paths(repository)
    missing = tuple(path for path in required if path not in files)
    forbidden = tuple(
        sorted(path for path in members if forbidden_member(path, files, required))
    )
    return PackageResult(
        archive=archive.name,
        archive_root=root,
        size_bytes=size_bytes,
        limit_bytes=limit_bytes,
        headroom_bytes=max(limit_bytes - size_bytes, 0),
        exceeded_by_bytes=max(size_bytes - limit_bytes, 0),
        required_paths=required,
        missing_paths=missing,
        forbidden_prefixes=FORBIDDEN_PREFIXES,
        forbidden_paths=forbidden,
    )


def decimal_mb(byte_count: int) -> str:
    """Render bytes as decimal megabytes with stable precision."""
    return f"{byte_count / 1_000_000:.2f} MB"


def render_content_status(result: PackageResult) -> list[str]:
    """Render required and forbidden package-content results."""
    if result.contents_ok:
        return ["Required package contents verified."]
    lines = ["Package-content policy failed."]
    if result.missing_paths:
        lines.extend(["", "Missing required paths:", ""])
        lines.extend(f"- `{path}`" for path in result.missing_paths)
    if result.forbidden_paths:
        lines.extend(["", "Forbidden paths present:", ""])
        lines.extend(f"- `{path}`" for path in result.forbidden_paths)
    return lines


def render_markdown(result: PackageResult) -> str:
    """Render deterministic Markdown for summaries and sticky comments."""
    heading = "✅" if result.policy_ok else "❌"
    lines = [
        MARKER,
        "",
        f"## Package checks {heading}",
        "",
        (
            f"Source package archive: **{decimal_mb(result.size_bytes)}** / "
            f"**{decimal_mb(result.limit_bytes)}** limit"
        ),
    ]
    if result.size_ok:
        percent = (
            result.headroom_bytes / result.limit_bytes * 100
            if result.limit_bytes > 0
            else 0.0
        )
        lines.append(
            f"Headroom: **{decimal_mb(result.headroom_bytes)} ({percent:.1f}%)**"
        )
    else:
        lines.append(f"Exceeded by: **{decimal_mb(result.exceeded_by_bytes)}**")
    lines.extend(["", *render_content_status(result)])
    if not result.size_ok:
        lines.extend(["", "The package exceeds the configured size ceiling."])
    lines.extend(
        [
            "",
            "> [!IMPORTANT]",
            (
                "> crates.io currently limits `.crate` uploads to **10 MiB**. "
                "This project's lower CI ceiling is intentional to preserve "
                "publishing headroom."
            ),
        ]
    )
    return "\n".join(lines) + "\n"


def write_outputs(result: PackageResult, json_path: Path, markdown_path: Path) -> None:
    """Write deterministic structured data and its pre-rendered Markdown."""
    json_path.write_text(
        json.dumps(result.to_dict(), indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    markdown_path.write_text(render_markdown(result), encoding="utf-8")


def result_fields(data: object) -> dict[str, Any]:
    """Extract the reference report fields from an object-shaped JSON root."""
    if not isinstance(data, dict):
        message = "JSON root must be an object"
        raise TypeError(message)
    fields = {
        key: value
        for key, value in data.items()
        if key not in {"contents_ok", "policy_ok", "size_ok"}
    }
    for key in (
        "required_paths",
        "missing_paths",
        "forbidden_prefixes",
        "forbidden_paths",
    ):
        fields[key] = tuple(fields[key])
    return fields


def load_result(path: Path) -> PackageResult:
    """Load and validate structured analysis for the separate policy gate."""
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
        result = PackageResult(**result_fields(data))
    except (KeyError, OSError, TypeError, ValueError, json.JSONDecodeError) as error:
        message = f"cannot load package analysis {path}: {error}"
        raise ArchiveError(message) from error
    normalized = json.loads(json.dumps(result.to_dict()))
    if normalized != data:
        message = f"inconsistent package analysis: {path}"
        raise ArchiveError(message)
    return result


def parse_args() -> argparse.Namespace:
    """Parse analysis or enforcement command arguments."""
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    analyse = commands.add_parser("analyse")
    analyse.add_argument("package_directory", type=Path)
    analyse.add_argument("json_output", type=Path)
    analyse.add_argument("markdown_output", type=Path)
    enforce = commands.add_parser("enforce")
    enforce.add_argument("json_input", type=Path)
    return parser.parse_args()


def main() -> int:
    """Create trustworthy reports first, or enforce an existing report."""
    args = parse_args()
    try:
        if args.command == "analyse":
            archive = discover_archive(args.package_directory)
            write_outputs(
                analyse_archive(archive),
                args.json_output,
                args.markdown_output,
            )
            return 0
        result = load_result(args.json_input)
    except ArchiveError as error:
        sys.stderr.write(f"package report error: {error}\n")
        return 2
    if result.policy_ok:
        return 0
    sys.stderr.write("package publishability policy failed\n")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
