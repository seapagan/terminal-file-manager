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
