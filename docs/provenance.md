# Reference provenance

Full actual reference files and tests were inspected before porting. The private
implementation ledger records full differences, classifications and execution
proof. The deleted historical M0-B implementation was not used as a template.

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
unchanged. Substantive corrections are limited to approved raw-path/XML/CSV/
source reconciliation, verified Linux Rustup bootstrap, and exact asset inventory
including zero-byte rejection. New regressions accompany these boundaries.

Lizard 1.23.0 remains the audited parser. The prospective 1.24.1 upgrade, embedded
SSH graph, native distribution/installers and immutable publication remain
separate proof gates. Action source/release notes and frozen image digests were
verified; configuration is not evidence that hosted jobs ran.

## M0-C1: binary verifier port

The [ELF helper](https://github.com/seapagan/labeldeck/blob/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d/scripts/verify-release-binary.sh)
is copied unchanged, including the GNU GLIBC 2.28 ceiling and separate MUSL
pre-strip / static post-strip modes. The
[PE helper](https://github.com/seapagan/labeldeck/blob/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d/scripts/verify-release-binary.ps1)
changes only the descriptive executable name. The complete
[ELF suite](https://github.com/seapagan/labeldeck/blob/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d/tests/release/verify.sh)
and [PE suite](https://github.com/seapagan/labeldeck/blob/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d/tests/release/verify.ps1)
retain every original assertion with TFM fixture/temp names.

Approved D1 additions test ELF input/type/argument/inspection failures and
missing MUSL signatures, PE signature/offset rejection, and the actual existing
inline Mach-O workflow blocks. Production verifier logic and release workflow
blocks are unchanged. Fake readelf output, synthetic PE bytes and injected file
descriptions prove parser/control-flow behavior; they do not prove foreign-native
execution, real stripping or an eight-target rehearsal. Installers and real-asset
rehearsal proof remain separate milestones.

## M0-C2: installer port and approved corrections

The complete [Unix installer](https://github.com/seapagan/labeldeck/blob/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d/install.sh),
[PowerShell installer](https://github.com/seapagan/labeldeck/blob/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d/install.ps1),
[Unix suite](https://github.com/seapagan/labeldeck/blob/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d/tests/installers/unix.sh)
and [Windows suite](https://github.com/seapagan/labeldeck/blob/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d/tests/installers/windows.ps1)
were copied before adaptation; latest relevant file revisions remain
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
with a same-directory owned backup. Native proof remains pending.

The revised C2 timeout approval preserves synchronous exact version validation
without a deadline or a new consumer runtime. No release matrix, bootstrap,
inventory, action/tool pin, canonical QC or publication infrastructure changes.
Checksums establish downloaded-byte integrity, not publisher authenticity.
