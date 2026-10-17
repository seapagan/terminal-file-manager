# Reference provenance

## M1 local browsing

The local metadata, sorting and ignore-rule boundaries were adapted from
[`lsplus d30ea4a`](https://github.com/seapagan/lsplus/tree/d30ea4a4d8f382bce1fd761f62b470926906f847),
particularly `src/platform/{unix,windows}.rs`, `src/utils/{sort,gitignore}.rs`
and the associated `tests/crate/` filesystem and ignore suites (MIT, Grant
Ramsay). TFM retains native path identity, separates recursive eligibility from
interactive directory entry, rebuilds rule matchers on refresh and reports
unknown rule status instead of silently treating errors as unignored files.
The native regression cases were reviewed and adapted to these contracts.

The correctness follow-up compares isolated real Git repositories and linked
worktrees against `git check-ignore --no-index --verbose --non-matching -z`,
distinguishing explicit negations from ignored paths. TFM retains its existing
excluded-parent check: lsplus's combined `matched_path_or_any_parents` call
returns a direct child negation before checking an excluded parent, which fails
this matrix. No lifetime cache or swallowed rule errors were adopted. Invalid
patterns that Git treats literally can instead produce an explicit unknown
state in the `ignore` library; matching describes rules independently of Git's
tracked-file index. Global discovery follows the library's supported Git config
lookup rather than promising all of Git's configuration directives.

The terminal lifecycle, native PTY/ConPTY session driver, colour mapping and
semantic styling adapt [Labeldeck 8c0fdea](https://github.com/seapagan/labeldeck/tree/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d)
(`src/edit/color.rs`, `src/edit/ui/{theme,render}.rs`, terminal guards and
`tests/common/terminal*.rs`; MIT, Grant Ramsay). Associated named/indexed/RGB,
monochrome, palette, resize and restoration tests were inspected before adapting
the applicable regressions. TFM uses colored_text public detection/resolution
APIs directly, not copied internal colour logic. Its native metadata boundary
uses the lsplus Win32 query pattern with function-scoped approved unsafe code,
attribute validation, lossless UTF-16 and owned handle cleanup.

Ratatui/Crossterm and the direct `colored_text` dependency supply the M1 terminal
foundation. The only approved dependency-policy exceptions are Zlib for
`foldhash 0.2.0`, Unicode-3.0 for `unicode-ident 1.0.27`, and `syn 2.0.119`
alongside `3.0.9`. The level-triggered Unix input path additionally requires
`crossterm -> filedescriptor 0.8.3 -> thiserror 1.0.69 -> thiserror-impl 1.0.69`,
alongside Ratatui/kasuari's `thiserror`/`thiserror-impl 2.0.21`. The two older
versions have exact duplicate exceptions, to remove when filedescriptor adopts
thiserror 2. This keeps pending input readable when resize and keyboard readiness
coincide; the default Mio source can lose that readiness edge. Other duplicate
versions and unknown sources remain denied; licence permissions are unchanged.

The M0 ports used complete reference files and tests. The private implementation
ledger records differences, classifications and execution proof. The ports did
not use the deleted historical M0-B implementation as a template.

| Source revision | Adopted files / design |
| --- | --- |
| [colored_text 8b53ee9](https://github.com/seapagan/colored_text/tree/8b53ee9df7cce63efb12c2422b63072627455a6b) | Full Makefile/exact-MSRV gate; canonical checker and all 18 tests; Ruff ALL policy; deny; native Rust/quality and Zizmor workflows; contributor structure |
| [keyhold 0cbd710](https://github.com/seapagan/keyhold/tree/0cbd7108cd46aae929b5d39b77ecf3290fa913e9) | Renovate; security-policy reporting structure; distinct legacy checker assertions |
| [git-diff-out c95d7e7](https://github.com/seapagan/git-diff-out/tree/c95d7e7e2772fcd58ee481e0108e4eb3dc264b1e) | Strict maintainability/host guards and mandatory Linux x86_64 MSVC xwin tasks; complete weekly policy workflow and ShellCheck installation; legacy checker assertions |
| [labeldeck 6883a31](https://github.com/seapagan/labeldeck/tree/6883a3126d9c66c1b64cc1023d67f37a753d9485) | Cargo metadata/binstall shape; both full licences/notices; complete eight-target release framework and reviewed actionlint configuration |
| [labeldeck 8c0fdea](https://github.com/seapagan/labeldeck/tree/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d) | Complete ELF/PE verifier implementations and their original 20 ELF / seven PE regression cases; last relevant file revision [ae92a29](https://github.com/seapagan/labeldeck/commit/ae92a29b276bc04d88c901c098521c433d08f342) |

Mechanical adaptations cover TFM identifiers, approved scaffold MSRV, paths,
verified tool/action versions, std-only/inapplicable library profiles and the
manual nonpublishing boundary. Existing reviewed suppressions are retained
unchanged. The original M0-B corrections covered approved raw-path/XML/CSV/
source reconciliation, verified Linux Rustup bootstrap, and exact asset inventory
including zero-byte rejection. Later approved corrections appear below.

Lizard 1.23.0 remains the audited parser; a 1.24.1 upgrade needs separate
compatibility proof. M0 native distribution, installer and selected-dependency
feasibility checks have passed. Production dependency adoption and immutable
publication remain later gates. [Acceptance](acceptance.md) links the executed
checkpoints and distinguishes them from configured or future checks.

## M0-C1: binary verifier port

The [ELF helper](https://github.com/seapagan/labeldeck/blob/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d/scripts/verify-release-binary.sh)
was copied unchanged, including the GNU GLIBC 2.28 ceiling and separate MUSL
pre-strip / static post-strip modes. The
[PE helper](https://github.com/seapagan/labeldeck/blob/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d/scripts/verify-release-binary.ps1)
initially changed only the descriptive executable name. The complete
[ELF suite](https://github.com/seapagan/labeldeck/blob/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d/tests/release/verify.sh)
and [PE suite](https://github.com/seapagan/labeldeck/blob/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d/tests/release/verify.ps1)
retain every original assertion with TFM fixture/temp names.

Approved D1 additions test ELF input/type/argument/inspection failures and
missing MUSL signatures, PE signature/offset rejection, and the actual existing
inline Mach-O workflow blocks. Later approved corrections added ELF64 and
little-endian checks in
[`2855f6c`](https://github.com/seapagan/terminal-file-manager/commit/2855f6c)
and PowerShell named-argument/output fixes in
[`85d5974`](https://github.com/seapagan/terminal-file-manager/commit/85d5974).
Fake readelf output, synthetic PE bytes and injected file
descriptions prove parser/control-flow behavior; they do not prove foreign-native
execution, real stripping or an eight-target rehearsal. The native M0 rehearsal
provides that separate executable and installer proof.

## M0-C2: installer port and approved corrections

The complete [Unix installer](https://github.com/seapagan/labeldeck/blob/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d/install.sh),
[PowerShell installer](https://github.com/seapagan/labeldeck/blob/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d/install.ps1),
[Unix suite](https://github.com/seapagan/labeldeck/blob/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d/tests/installers/unix.sh)
and [Windows suite](https://github.com/seapagan/labeldeck/blob/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d/tests/installers/windows.ps1)
were copied before adaptation; their reference file revisions were
`ae92a29b276bc04d88c901c098521c433d08f342`. Every original assertion is retained;
the Unix harness separates its full inherited case sequence from new security
cases to keep files within the existing maintainability limit.

Mechanical changes distinguish `terminal-file-manager` archives from the `tfm`
command and use TFM environment names. Approved D2–D8 changes enforce archive
member/type policy, destination checks, strict sidecars, HTTPS/stable tags,
executable identity, Windows transactional replacement, portable fixture hashing,
correct native test paths/version fixtures and fail-fast native CI routing.
[Keyhold's Unix suite](https://github.com/seapagan/keyhold/blob/0cbd7108cd46aae929b5d39b77ecf3290fa913e9/tests/installers/unix.sh)
supplies checksum-first/sentinel and detector-independent libc override assertions
(last relevant revision `cd037693aa268956bb963aba19faf3b97d777270`).

Windows native architecture discovery uses
[IsWow64Process2](https://learn.microsoft.com/en-us/windows/win32/api/wow64apiset/nf-wow64apiset-iswow64process2):
[GetNativeSystemInfo alone](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getnativesysteminfo)
can return an emulated architecture. Replacement uses the Framework-compatible
[File.Replace overload](https://learn.microsoft.com/en-us/dotnet/api/system.io.file.replace?view=netframework-4.8.1)
with a same-directory owned backup. The approved preservation correction
[`df3919a`](https://github.com/seapagan/terminal-file-manager/commit/df3919a701dee4c21d22c64df7a52e0603889331)
adds exclusive non-truncating replacement preflight and separate cleanup-failure
recovery. Native Windows x64 PowerShell 5.1/7+ passed the complete preservation
suites. The later M0 rehearsal passed native x64 and ARM64 Windows suites and
real-archive acceptance under both PowerShell versions.

The revised C2 timeout approval preserves synchronous exact version validation
without a deadline or a new consumer runtime. That C2 change retained the release
matrix, bootstrap, inventory, pins, QC and nonpublishing boundary.
Checksums establish downloaded-byte integrity, not publisher authenticity.

## M0-C3: repository-side assets and offline real-archive hooks

D9 changes only the macOS packaging hash to native `shasum -a 256`. D10's
`scripts/verify_release_assets.py` is new approved standard-library code, rather
than a reference port. It inspects physical tar headers and ZIP metadata without
extraction or execution, requires exact four-file inventories and checksum
records, compares checkout documents and, before upload, verified build bytes.
The aggregate omits build-byte comparison and never executes foreign payloads.

The existing eight-target workflow, frozen images/actions/bootstrap, pre/post
object checks, checksum aggregation and recursive inventory remain authoritative.
The separately approved C3 correction changes only the bootstrap checksum
option from GNU `--check` to portable `-c`: the pinned MUSL image uses BusyBox.
The same four-target corruption/positive regression runs inside each controlled
Linux image, retaining verification before executable permission or execution.
Real-archive harness modes replace only transport with owned local fixtures;
production archive, architecture, version and destination checks still run.
Added tests are repository tooling; no consumer runtime or publishing stage is
introduced. [Run 38625643530](https://github.com/seapagan/terminal-file-manager/actions/runs/38625643530)
passed prepare, eight native targets and aggregate at `b3c31aa` on 2026-10-14.
[Acceptance](acceptance.md) records the remaining final-release gates.

## Later foundation changes

The approved CI cleanup in
[`e3c51df`](https://github.com/seapagan/terminal-file-manager/commit/e3c51df)
kept infrastructure tests available through `verify-infra` and manual workflows.
The LLVM JSON/LCOV merger in
[`2694f84`](https://github.com/seapagan/terminal-file-manager/commit/2694f84)
replaced cross-platform LCOV-ID aggregation with validated source identities.
See [contributor guidance](../CONTRIBUTING.md#testing-and-coverage) and
[fixture provenance](../tests/fixtures/llvm-coverage/provenance.md).

The selected-dependency probe and its removal have a
[tracked handover](m0-dependency-feasibility.md). The Unix installer diagnostic
in [`67a25ac`](https://github.com/seapagan/terminal-file-manager/commit/67a25ac)
adds conditional `noexec`/`TMPDIR` guidance without changing staging or validation.
