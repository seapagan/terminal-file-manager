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
