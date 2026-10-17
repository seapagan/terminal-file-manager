# Contributing

Keep changes focused, preserve the current CLI and platform contracts, and add
behavioural regression tests for functionality or bug fixes. Prefer tests of
observable behaviour over tests of implementation details. Document new public
APIs, and avoid speculative infrastructure or dependencies without a concrete
need.

## Source and package

The [repository](https://github.com/seapagan/terminal-file-manager) contains
application source, tests, installer and release tooling, and CI configuration.
The Cargo package includes Rust sources, application/native tests, README,
both licences and required Cargo metadata. Local planning material and development support
utilities are excluded. Inspect the package with:

```sh
cargo make package-list
```

`cargo make package` validates the package. It requires clean packaged sources;
`cargo package --locked --allow-dirty` is only an interim development check,
not a replacement for the final verification gate.

## Rust version and dependencies

The minimum supported Rust version (MSRV) is declared in `Cargo.toml` as
`rust-version = "1.95.0"`. TFM uses Rust edition 2024 and a version 4 lockfile.
The manifest is the authority for the supported floor; developer tooling may
need a newer compiler. Install the exact MSRV toolchain for compatibility checks:

```sh
rustup toolchain install 1.95.0 --profile minimal --component clippy,rustfmt
```

Before adding or upgrading dependencies, check upstream release notes, licence
and security policy, compiler requirements and target compatibility. Keep
`Cargo.lock` reproducible and run `cargo make msrv`. Changing the declared MSRV
requires explicit approval. SSH/SFTP integration must additionally validate the
actual adopted dependency graph and its supported native targets; historical
feasibility results do not certify a future shipping build.

## Current CLI

The Cargo package is `terminal-file-manager`; the executable is `tfm`.
`tfm` browses the shell cwd; `tfm DIRECTORY [OTHER_DIRECTORY]` accepts up to two
native paths. `-2` / `--two-panes`, two paths or `two_panes = true` request dual
startup. One explicit path initializes both panes. Browsing requires a terminal
on stdin/stdout. `--` accepts leading-hyphen paths. Explicit-path failures never
substitute another path.
`tfm --help` / `-h` and `tfm --version` / `-V` print output and return 0;
unsupported arguments and startup failures report stderr and return 2.
M1 browsing is read-only; Enter never launches files. See README for bindings
and lazy preferences. `Q` saves current local locations/layout; ordinary quit
never replaces that session. `--restore-session` or `restore_session = true`
opts into restoration; explicit paths override the configured default.
`--save-session` validates and records startup locations/layout without a TUI.
Tests use owned fixtures and isolated HOME/config values.

Native PTY tests use portable-pty on Unix and ConPTY on Windows. Their ignored
`terminal_child` entry point is deliberately invoked by the parent restoration
test; it is not an untested feature skip. Windows-only metadata tests must run
on Windows; Linux xwin supplies compile/lint/build evidence only. Rendering and
input tests use cached state/TestBackend, independently of a real terminal.

## Local verification

Install stable Rust with `rustfmt`, Clippy and `llvm-tools-preview`, plus
`cargo-make`, `cargo-nextest`, `cargo-llvm-cov`, `cargo-audit`, `cargo-deny`,
`actionlint`, `ShellCheck` and `zizmor`. Full verification also uses Python
3.10+, Ruff, mypy and Lizard 1.23.0 for static support-code checks and source
complexity analysis. Install and verify required tools before running the
corresponding tasks; missing prerequisites must not be treated as passing.

| Command | Purpose |
| --- | --- |
| `cargo make verify-fast` | Native Rust formatting, checks, tests, doctests, docs, builds and package validation |
| `cargo make verify` | Complete host-applicable development gate: Rust, MSRV, coverage, complexity, static support-code checks, dependency and workflow security, packaging and required cross-checks |
| `cargo make verify-infra` | Explicit tooling self-tests: Python checks/tests, installer and binary-verifier regressions, CI/release harness tests |
| `cargo make test` | Native Rust tests through Nextest |
| `cargo make doctest` | Independent Rust documentation tests |
| `cargo make coverage` | Instrumented Rust test coverage and LCOV |
| `cargo make coverage-html` | HTML coverage report (`cargo make test-html` is an alias) |
| `cargo make msrv` | Full check against the exact declared Rust version |
| `cargo make audit` / `cargo make deny` | Dependency advisory and licence/source/ban policy |
| `cargo make complexity` | Source complexity report, including maintained tests |

`cargo make verify` selects `verify-linux`, `verify-macos` or `verify-windows`
for the host; `cargo make quality` follows the same routing. It does **not** run
tooling self-tests. Run `verify-infra` when support scripts, installer/verifier
code, or CI harnesses change, and when explicitly checking that infrastructure.
Its preflight reports missing requirements before testing and distinguishes
foreign-host suites as *not applicable*. All retained individual Make tasks
remain available for focused work.

### Linux Windows cross-verification

**Full Linux `cargo make verify` always includes `cargo make verify-xwin`.**
This requires `cargo-xwin` 0.23.1, `clang-cl`, `lld-link`, `llvm-lib`, and the
active Rust toolchain's `x86_64-pc-windows-msvc` target:

```sh
rustup target add x86_64-pc-windows-msvc
```

The task performs locked check, warning-denying Clippy and build validation for
Windows MSVC. It does not execute Windows binaries or replace native Windows
CI. On non-Linux hosts, run the appropriate native full gate and report that
Linux `xwin` verification has not been performed.

### Warnings and completion

A successful exit code does not mean a warning-free result. Inspect and report
non-fatal findings from `cargo audit`, `cargo deny`, complexity analysis and
other verification tools; investigate worthwhile findings rather than quietly
ignoring them. Do not weaken gates or introduce suppressions simply to obtain
green results. Run the full applicable `cargo make verify` gate before committing,
and rerun it if relevant source, tests or configuration changes afterward.

## Testing and coverage

Use isolated, deterministic tests with disposable paths and configuration.
Test filesystem behaviour on appropriate native operating systems, including
platform-specific file metadata, permissions and links. Never run destructive
tests against real user data or credentials. Keep Rust unit, integration and
doctest coverage aligned with the behaviour actually implemented.

Rust CI tests on **Linux, macOS and Windows**. Each native job exports LCOV and
LLVM JSON from the same test profiles. The aggregation job validates both formats
and merges source-function groups and branch locations into `coverage.lcov`.
It retains the `coverage-linux`, `coverage-macos`, `coverage-windows` and
`coverage-combined` artifacts for 14 days. The combined artifact includes
`coverage-summary.json` with the native LLVM summaries and combined LCOV counts.
It also uploads coverage to
Codacy when `CODACY_PROJECT_TOKEN` is available in an eligible context. Coverage
collection and artifact validation do not depend on that token. No percentage
threshold is currently enforced; set a measured, justified threshold once
there is representative application code. `cargo make coverage` writes
`target/llvm-cov/coverage.lcov` and `target/llvm-cov/coverage.json`;
`cargo make coverage-html` writes HTML to `target/llvm-cov/html`.

LLVM groups instantiations by the first region in a function's main-view file.
The merger reproduces that rule and checks native function summaries and every
instantiation's LCOV name, line and execution count against JSON. Branch IDs are
local to an export; the merger checks LLVM's native enumeration before combining
branches by function location, branch range, definition files and expansion call
sites, including intermediate definition files. It checks complete expansion
records and counters against the owning functions before merging. It supports
LLVM's unified and separate branch-instantiation exports. It rejects conflicting
checksums, incomplete reports, unsupported LCOV fields and ambiguous identities.
LLVM JSON 3.1.0 is the supported schema. MC/DC and folded branch exports whose
records cannot reproduce native summaries fail explicitly.

Native LLVM line and branch summaries describe function groups; their counts can
differ from detailed records when functions overlap or macros expand. The merger
preserves these native summaries in its JSON diagnostics, checks line records
against LLVM segments, and calculates combined LCOV summaries from the emitted
unique records. Generic instantiations and build-specific Rust symbols therefore
do not inflate the combined function denominator.

Infrastructure regressions are retained locally under `verify-infra` rather
than executed in every routine CI run. The Python support matrix is available
as a manually dispatched workflow. Developer-facing quality, MSRV, policy,
workflow security and package checks remain ordinary CI responsibilities.

## Release and installer checks

Release archives are named `terminal-file-manager-vX.Y.Z-<target>.tar.gz` on
Unix and `.zip` on Windows, each with an exact-name `.sha256` sidecar. Each
archive contains four root-level regular files: `tfm` (or `tfm.exe`),
`LICENSE-MIT`, `LICENSE-APACHE` and `README.md`.

The nonpublishing eight-target release rehearsal is **manual-only**. It checks
binary architecture and ABI requirements, checksums, archive contents and
native installer acceptance before any public release is considered. Linux GNU
builds target a maximum GLIBC requirement of 2.28; MUSL builds are checked for
static linkage. The [acceptance record](docs/acceptance.md) links the completed
M0 native rehearsal. Repeat distribution acceptance for the final shipping graph
and release artifacts; historical scaffold or dependency-probe runs do not
certify a finished release.

The Unix and Windows installers validate downloaded archive hashes, executable
architecture and exact `tfm --version` before replacement. They use staging and
preservation/recovery safeguards; do not weaken them when changing installers.
Unix candidate execution can fail when the temporary filesystem restricts
execution, for example with `noexec`. In that case, set `TMPDIR` to a writable
directory on an executable filesystem and retry. Other launch failures need
their own diagnosis. The installers reject linked destination ancestors; use
an explicit physical installation path when that restriction applies.

For focused offline acceptance using an existing release archive:

```sh
sh tests/installers/unix.sh --real-archive ARCHIVE TARGET VERSION BINARY
pwsh -NoProfile -File tests/installers/windows.ps1 -RealArchive ARCHIVE -Target TARGET -Version VERSION -Binary BINARY
```

Use only disposable destinations. The matching native host and necessary tools
are required. Installer tests must not download public releases by default.
These checks belong to infrastructure verification or release certification,
not every ordinary Rust CI run.

## Changes, commits and reviews

Keep commits coherent and use imperative Conventional Commit subjects.
Inspect both staged and unstaged diffs before committing. Follow the repository
and global agent instructions for approvals, GPG signing, sign-off and delivery.
Do not commit local agent instructions or private plans. `CHANGELOG.md` is
maintainer-owned: do not edit, reformat or regenerate it.

Update README, CLI help and other public documentation when behaviour changes.
When reporting completion, distinguish actual local checks from hosted results,
list any outstanding warnings or untested platforms, and never describe an
unexecuted test as passed.
