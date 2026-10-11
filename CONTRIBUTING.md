# Contributing

Keep changes focused and preserve the approved milestone boundaries and CLI
contract. Use standard Rust formatting and document every new public item. The
M0-B scaffold uses only the standard library. Later dependencies require the
approved compatibility, security and licence proof.

## Published package and full source

The Cargo package includes Cargo metadata/lockfile, Rust sources, CLI smoke
tests, README and both licences. Repository support tooling and private
planning material are excluded. Inspect `cargo package --list --locked`.

For the complete test suite and tooling, use the
[GitHub repository](https://github.com/seapagan/terminal-file-manager).

## Rust support

`Cargo.toml`'s `rust-version` is the single source of truth for the MSRV. The
approved M0-B floor is exact Rust 1.95.0, with edition 2024 and lockfile v4.
The future embedded SSH dependency graph still needs exact-floor and native
verification before adoption. Developer tool bootstrap requirements do not
change the package floor.

Before updating dependencies, inspect their compiler requirements and release
notes, retain a reproducible lockfile, and run `cargo make msrv`. An MSRV
increase requires an explicit support-policy decision and documentation update.
Consumer projects, test dependencies, lockfile format, and the bootstrap
requirements of developer tools do not set the package/source MSRV.

## Local tools and checks

Install current stable Rust with rustfmt, Clippy, and llvm-tools-preview, plus
cargo-make, cargo-nextest, cargo-llvm-cov, cargo-audit, cargo-deny, actionlint,
ShellCheck, and Zizmor. The MSRV task uses cargo-make's embedded Duckscript on
every platform, including native Windows. It reads Cargo metadata from
`Cargo.toml`, requires an exact stable `major.minor.patch` rust-version, and
invokes that toolchain for formatting, checking, Clippy, tests, doctests,
documentation, builds, and packaging. No external scripting runtime is required.
Install the exact declared Rust toolchain with Clippy and rustfmt before running
the task:

```console
rustup toolchain install 1.95.0 --profile minimal --component clippy,rustfmt
cargo make verify
```

`verify` selects `verify-linux`, `verify-macos` or `verify-windows` for the
current host. Each runs every platform-independent gate; Linux also requires
the Rustup-bootstrap and release-inventory regressions, plus `verify-xwin`.
Distribution regressions also run on their native hosts: ELF/Mach-O and Unix
installers on Linux, Mach-O and Unix installers on macOS, PE and Windows
installers on Windows. Host guards reject explicit use on an incompatible
host. `quality` follows the same routing. `verify-fast` runs the native Rust,
documentation, build and clean-package checks without the support/security
suite, distribution regressions or xwin cross-verification. `fmt` is a check-only alias for `format`;
`release-build` builds the optimized binary, and `package-list` reports the locked crate inventory.

The full gate checks formatting, all targets/features, Clippy and rustdoc with
warnings denied, unit/integration tests and doctests, builds, packaging, the
exact declared MSRV, advisories/licenses/sources, workflow syntax/security,
and coverage, plus the mandatory Python support tasks below. Packaging requires
a clean committed tree. During development, run individual tasks and use
`cargo package --locked --allow-dirty` as an
interim check; run the canonical gate again after committing.

```console
cargo make coverage
cargo make coverage-html
```

The M0-B baseline measures the actual help/version/error paths. LCOV uses
repository-relative paths at `target/llvm-cov/coverage.lcov`; HTML is under
`target/llvm-cov/html`. `test-html` aliases the HTML task. Check nonempty
records for every production source. Numeric floors are set after M1 provides
representative code and maintainer-reviewed measurements; library-specific
thresholds are not copied. Coverage does not prove future filesystem or SSH
behaviour.

On Linux, full verification also requires cargo-xwin 0.23.1, LLVM tools
`clang-cl`, `lld-link` and `llvm-lib` on PATH, Clippy, and the active toolchain's
`x86_64-pc-windows-msvc` standard library. Install the Rust target with
`rustup target add x86_64-pc-windows-msvc`. `cargo make verify-xwin` runs the
reference locked all-target/all-feature check, warning-denying Clippy and build.
Missing prerequisites fail; these cross-checks do not execute Windows tests.
The pinned reference supports only x86_64 xwin verification; Windows ARM64
remains in the eight-target release framework. Hosted CI retains separate native
Windows verification and has no dedicated xwin job.

Release archives use `terminal-file-manager-vX.Y.Z-<target>.tar.gz` on Unix and
`.zip` on Windows, with exact-name `.sha256` sidecars. Archives contain the
`tfm` / `tfm.exe`, `LICENSE-MIT`, `LICENSE-APACHE` and `README.md` as four
root-level regular files. C1 verifiers and C2 installers have regression tasks;
The C3 source-side archive checks and real-archive rehearsal hooks are implemented;
full eight-target hosted acceptance remains pending.
No public TFM binary release is implied.

`cargo make installers-shellcheck` follows all Unix harness sources;
`cargo make unix-installer-test` retains the 58 inherited control-flow cases
and adds security cases plus real native Rust executable fixtures. `rustc`
is a test prerequisite, not an installer dependency. Tests use private PATH
shims and local assets; they never download public releases. Hashing uses
`sha256sum` on Linux and `shasum -a 256` on macOS.

Windows full verification runs `cargo make windows-installer-test` with
PowerShell 7 and `cargo make windows-installer-test-legacy` with Windows
PowerShell 5.1. Both require the named shell; missing tools fail. Their native
success group uses the local `release-build` output. The security suite covers
ZIP/sidecar/PE rejection, destination checks and replacement/recovery; running
it with Linux PowerShell proves parser behavior and simulated transactions only.
Native Windows CI invokes the two shell tasks independently.

`cargo make release-assets-test` validates distribution archive names, strict
sidecars, machine headers, member types/modes and checkout/build byte identity
using Python's standard library. Full verification includes it; `verify-fast`
is unchanged. `cargo make release-hash-test` executes the actual macOS workflow
hash block with `shasum -a 256`; no macOS coreutils prerequisite is added.
`cargo make release-hook-test` executes the literal Unix real-archive workflow
step with an owned native archive, checks fresh/reinstall/preservation evidence
and rejects corrupted or missing archives. It builds the native release fixture
and runs in full Unix verification and native CI.

The manual nonpublishing rehearsal validates every archive before upload and
again in the foreign-asset aggregate. Its existing checksum and recursive
sixteen-file inventory blocks remain mandatory. It runs real-archive installer
acceptance on each matching native runner, including explicit GNU/MUSL selection
and both Windows shells. Local offline entry points are:

```sh
sh tests/installers/unix.sh --real-archive ARCHIVE TARGET VERSION BINARY
pwsh -NoProfile -File tests/installers/windows.ps1 -RealArchive ARCHIVE -Target TARGET -Version VERSION -Binary BINARY
```

Use only disposable installer destinations created by these harnesses. Unix
real-archive tests require platform Perl to bound the owned version process;
this is a test prerequisite, never an installer requirement. Controlled Linux
build-image runtime tests require their existing `timeout` utility. Real Windows
archive tests use Framework-compatible process deadlines in PowerShell 5.1/7+.
No process-tree containment is claimed. Source inspection and local static tests
do not prove Windows ARM64, macOS Intel or the complete hosted rehearsal.

Consumer installers verify architecture and exact `tfm --version` before
replacement. Version validation is synchronous without a deadline; no Bash,
Python, GNU timeout or process-tree supervisor is required. Native installer
acceptance checks prompt version termination. Destination checks are best-effort
pathname rechecks, not universal race prevention. Windows uses destination-local
exclusive staging, no-overwrite move for fresh installs and `File.Replace` plus
an owned backup for replacements; failed recovery retains its path for recovery.

CI runs the native Rust checks on Linux, macOS, and Windows with current
stable Rust. A separate Linux job derives and verifies the exact MSRV from
`Cargo.toml`. Dependency/workflow audits and coverage run on Linux. Actions are
pinned to full commit SHAs; verify upstream stable releases and compatibility
before updating pins.

### Mandatory Python support tooling

Python 3.10+ is the support-code floor. The canonical checker and all 18
original tests come from
[colored_text](https://github.com/seapagan/colored_text/tree/8b53ee9df7cce63efb12c2422b63072627455a6b),
with distinct legacy and documented defect regressions retained. Install:

```sh
python -m pip install 'lizard==1.23.0' 'ruff==0.17.0' 'mypy==2.4.0'
```

`cargo make verify` requires every support tool: absent tools fail rather than
skip. Ruff uses the reference ALL policy and reviewed exceptions. Strict mypy
checks Python 3.10 compatibility. Windows uses `python` for tests and the
checker; Unix uses `python3`. Native Rust CI remains independent of Python.
Linux quality CI uses exact Python 3.14.8. Separate support CI uses the same
three native OS runners with 3.14.8, plus provisionable Linux 3.10.21 as a
legacy floor compatibility test; it is not the recommended development runtime.
The final Python 3.10 release is 3.10.22; the current setup-python manifest
has no corresponding artifact. Lizard stays at audited 1.23.0 until the full
separately approved upgrade gates pass.

Run `cargo make complexity` for the Codacy-aligned report. Git discovers tracked
(including tracked-but-ignored) and nonignored untracked Rust (`*.rs`) and
Python (`*.py`) sources, **including maintained tests**. Tests are included in
analysis. Deleted files are omitted, and conflicted index entries are
deduplicated.

The defaults are overrideable through the environment:

| Variable | Default |
| --- | --- |
| `COMPLEXITY_LIZARD_VERSION` | `1.23.0` |
| `COMPLEXITY_MAX_CCN` | `10` |
| `COMPLEXITY_MAX_FUNCTION_NLOC` | `50` |
| `COMPLEXITY_MAX_PARAMETERS` | `8` |
| `COMPLEXITY_MAX_FILE_NLOC` | `500` |

The default threshold values are supplied by `Makefile.toml`; direct invocation
of `scripts/check_complexity.py` therefore requires the corresponding environment
variables to be set.

Equality is allowed; only values above a threshold are findings. Complexity
findings are advisory and do not fail verification. Malformed or unexpected
Lizard output, source-set mismatches, version mismatches, invalid configuration,
missing required inputs after task start, and other tooling/setup failures are
fatal. The regression fixture disables local Git commit signing so inherited
global signing/GPG configuration cannot break its temporary repository.

## Commits

Use short imperative Conventional Commit subjects and focused commits. Sign
commits using the existing Git signing setup and add a sign-off with `git commit
-s`. Keep API/test work separate from CI changes. `CHANGELOG.md` is permanently
maintainer-only: never create, edit, format or regenerate it, or invoke a task
that writes it. Do not commit agent instructions or private plans. Git delivery
and publication require separate authorization.
