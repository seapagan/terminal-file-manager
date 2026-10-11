# M0 selected dependency feasibility — local proof, hosted gates pending

Status on 2026-10-11: **local compiler/policy and x86_64 Linux ABI proof pass;
source provenance is corrected; hosted native proof remains unmet.**
The accepted std-only rehearsal (`10e4be1`, run `38153439044`) was not repeated.
No staging, commit, push, hosted dispatch, installer/product/release change,
publication or product MSRV change occurred. PR #1 remains draft.

## Selected source and graph

The isolated unpublished workspace is `probes/m0-dependencies/`. It does not
join the product workspace. Its lockfile contains 246 packages, SHA256
`d94748671869250b4b8468c9e96608893d61dbf6b109d9f832fc0f13d61ca7bd`.
Full resolved versions/sources/features are in `evidence/graph.json` and
`evidence/features.txt`; registry checksums are in Cargo.lock.

Yazi SFTP is the extracted stable `v26.9.1` snapshot at
`8dd895c695a5950330c2623eb43debf323b60654`. The fetched codeload archive SHA256 is
`ff68e645d81c5b275e12f7be5493e8cae170026fce9537f40c67a786cfac235c`.
`PROVENANCE.md`, `evidence/source-hashes.json` and `evidence/snapshot-delta.txt`
record original/current files and modifications. Long-term ownership remains
open for M3; the intended narrow fork/full-revision Cargo Git dependency was
not created during M0.

Direct selection: russh =0.64.1, corrected Yazi 26.9.1, Ratatui =0.30.2 with
`crossterm_0_29`, Crossterm =0.29.0, Tokio =1.53.2 and ring =0.17.14.
Necessary Yazi dependencies are bitflags =2.13.2, hashbrown =0.17.1,
parking_lot =0.12.5, serde =1.0.229 and typed-path =0.12.3. Live upstream release
and compatibility metadata were inspected before these selections.
Both russh edges disable defaults and enable only `ring`; upstream's
russh 0.63.1/RSA and Tokio `full` selections were not inherited. Tokio enables
only rt-multi-thread/io-util/net/sync/time/macros. Transitive feature unification
is recorded rather than assumed absent.

russh's registry release corresponds to upstream revision
`e5d80a07dc554480b6ea3b2e14adfc190407b627`. Resolved russh features are exactly
`["ring"]`. No rsa, aws-lc-rs or aws-lc-sys package exists; ssh-key has no RSA
feature. This disables SSH RSA support; ring itself has intrinsic RSA APIs that
cannot be removed through a ring Cargo feature. No RSA-only SSH interoperability
is promised. ssh-key 0.7.0-rc.11 remains a prerelease dependency.

The executable exercises russh Ed25519 signing/verification and identification
exchange via connect_stream; ring AES-256-GCM encryption/decryption; Crossterm
backend rendering; and Yazi's actual private SFTP codec. Direct ring use reaches
the same native backend instance used by russh. This is an offline dependency
smoke, without an SSH server harness, authenticated SFTP session or transfer
engine. Production lifecycle/authentication acceptance remains M3.

## Approved HANDLE and lint changes

Eleven imported files preserve opaque HANDLE bytes as Vec<u8>, Cow<[u8]> or
&[u8]. Request extension names remain strings. Three codec regressions cover
binary HANDLE response round trips, all seven handle-consuming requests, and
separate invalid-UTF8 versus literal replacement-character paths. The first
HANDLE regression failed before the fix with invalid UTF-8; all five workspace
tests pass afterward (three codec tests, crypto/terminal, SSH banner).

The approved allowance is confined to the imported trait declaration's one
`AsSftpPath::as_sftp_path(self)` method in `vendor/yazi-sftp/src/path.rs`:
`#[allow(clippy::wrong_self_convention)]`. No implementation/caller was renamed.
It preserves upstream API ownership/naming and avoids 18 unnecessary changes.
Upstream manifest lint allowances remain unchanged. TFM-owned code receives
strict Clippy; no root/workspace allowance or Clippy --no-deps bypass exists.
Rust 1.95.0 strict workspace Clippy passes on host and both controlled Linux
libcs, with no TFM-owned or imported Yazi warnings.

## Corrected source provenance and preserved licence texts

The exact upstream package
[LICENSE](https://github.com/sxyazi/yazi/blob/8dd895c695a5950330c2623eb43debf323b60654/yazi-sftp/LICENSE)
is Apache-2.0, while its workspace manifest declares MIT and its root licence is
MIT. The earlier extraction incorrectly replaced the package Apache text with
MIT. After explicit approval, the original package LICENSE was restored and
the root MIT text retained separately as LICENSE-MIT. Both upstream texts are
unchanged, including whitespace and notices. The isolated manifest retains the
upstream inherited MIT metadata; this is an accurate record of the inconsistency,
not a newly asserted dual-licence grant or a long-term ownership decision.

Exact hashes are `c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4`
(Apache) and `e66519d3d006dc76760d00d40361d24d2b97ba80ab93438885e78bc5a70bdd9c`
(MIT). Each matches the retained archive, exact revision's live raw source and
rebuilt .crate contents byte-for-byte. All snapshot hashes match; the lockfile is
unchanged. `evidence/licence-provenance-restored.json` records these assertions.
The rebuilt Yazi package contains 53 files and compiles under Rust 1.95.0.
All four cargo-deny checks pass again; no additional allowance/waiver was added.
These focused checks close the local notice-preservation blocker; final shipping
ownership/notices treatment remains M3/M8.

## Policy evidence

Audit 0.22.2: zero vulnerabilities/warnings against RustSec database revision
`7eebec69c352c7191b1f13eb95dd510eeca5d1de` (1,296 advisories).
Deny 0.20.2: advisories, licences, bans and sources pass with no warnings.
These are dated results for the frozen graph, not permanent security claims.
The project allowlist adds only approved ISC, BSD-3-Clause, Zlib and Unicode-3.0.
Global multiple-version denial, wildcard/source/advisory checks remain intact.

Probe-only exact duplicate skips are getrandom =0.2.17, hashbrown =0.16.1,
sha3 =0.11.0, syn =2.0.119 and windows-sys =0.52.0. The companion versions are
0.4.3, 0.17.1, 0.12.0, 3.0.8 and 0.61.2 respectively. Every actual reverse edge,
requirement and target is in `evidence/duplicate-paths.json`.
Disjoint requirements are ring ^0.2/russh ^0.4; kasuari ^0.16/Ratatui+Yazi ^0.17;
ml-kem ^0.11/russh ^0.12; derive macros ^2/^3; ring ^0.52/Tokio+mio ^0.61.
Unification requires upstream API/dependency changes, not a root pin. Only the
named older versions are skipped; unlisted duplicates fail. Re-review on any
lockfile change. No advisory waiver or broader suppression was added.

Verbose release builds expose registry-only compiler/deprecation/build warnings
in russh, ring and other transitive crates; exact messages are retained in
`evidence/registry-warnings.json` and full Linux build logs. Cargo's normal
registry-dependency lint capping applies; it was not modified. No allowance or
upstream edit was added for these warnings. Do not describe the entire registry
graph as warning-free. The strict local workspace/probe gates pass.

## Compiler floor and source consumability

Rust 1.95.0 (`59807616e`, 2026-04-14; LLVM 22.1.2) passes workspace check,
warning-denying Clippy, all-target/all-feature tests, doctests, warning-denying
workspace docs, release linking and smoke. All five tests pass; no doctests are
present. The same full gate passes with native 1.95.0 GNU and MUSL host
compilers in the controlled images. Rust 1.99.0 previously passed debug tests
and also passed an initial controlled GNU release/ABI run.

Normal Rust 1.94.0 rejects the probe/Yazi manifest's declared 1.95.0 floor.
Diagnostic-only `check --ignore-rust-version` nevertheless passes the whole graph
with 1.94.0 (`4a4ef493e`, 2026-03-02). Therefore 1.95.0 is the supported declared
floor, **not an evidenced intrinsic language/compiler minimum**. Metadata gives
support constraints, not an exact intrinsic boundary. No lower compiler is
promised; no product or snapshot rust-version was lowered. Unique-platform
native minimum paths still require the proposed hosted jobs.

A copied workspace passes `cargo +1.95.0 check --frozen` with existing registry
cache. `cargo package -p yazi-sftp --locked --allow-dirty` initially packaged 52 files and
verifies compilation of the unpacked package. After licence restoration, the focused recheck packages 53 files and again
verifies compilation, with both original texts in the package. These remain
uncommitted source-consumption checks. Packaging the root probe
fails because corrected `yazi-sftp =26.9.1` is not published (registry has 26.5.6).
The approved unpublished path workspace is source-consumable, not a publishable
registry application package. Final shipping/source arrangement remains M3/M8.

## Actual eight-target evidence

| Target | Actual compiler/link result | Native run and object/ABI evidence | Remaining gate |
| --- | --- | --- | --- |
| x86_64-unknown-linux-gnu | Rust 1.95.0; GCC 14.2.1; GNU ld/as 2.41; release linked | Full five-test gate; smoke pre/post-strip in pinned manylinux; x86_64 ELF; GLIBC max 2.28 both; dynamic libc/libgcc_s/libpthread/libm/libdl/loader | Hosted route confirmation |
| x86_64-unknown-linux-musl | Rust 1.95.0 native MUSL host; Alpine GCC 14.2.0; GNU ld/as 2.44; release linked | Full five-test gate; smoke pre/post-strip in pinned musllinux; x86_64 ELF; __init_libc before strip; no PT_INTERP/NEEDED before/after | Hosted route confirmation |
| aarch64-unknown-linux-gnu | Not built; compiler unobserved | None | Native ARM container link/test/smoke; GLIBC <=2.28 |
| aarch64-unknown-linux-musl | Not built; compiler unobserved | None | Native ARM static pre/post-strip and smoke |
| x86_64-apple-darwin | Not built; compiler/SDK unobserved | None | Native Intel link/tests/smoke; Mach-O machine, minOS/load libraries |
| aarch64-apple-darwin | Not built; compiler/SDK unobserved | None | Native Apple Silicon equivalent |
| x86_64-pc-windows-msvc | Local cross-link: Rust 1.95.0; cargo-xwin 0.23.1; Clang/LLD 21.1.8; cached CRT 14.44, SDK 10.0.26100 | Existing PE verifier and llvm-readobj confirm AMD64 0x8664; imports recorded; no native Windows execution | Native MSVC runner full gate/smoke and runtime floor |
| aarch64-pc-windows-msvc | Not built; compiler/SDK unobserved | None | Native ARM MSVC+Clang/ring link/tests/PE/imports/smoke |

Windows cross-build imports include KERNEL32, oleaut32, ntdll,
api-ms-win-core-synch-l1-2-0, bcryptprimitives, VCRUNTIME140 and UCRT API sets.
PE header OS version 6.0 does not prove that runtime floor; native baseline and
DLL availability remain untested. macOS/Windows support floors need maintainer
selection after observed toolchain/dependency evidence. Ring requires native
C compilation and target assembly; its packaged pregenerated material avoids
assuming an extra Perl/NASM toolchain. Windows ARM ring explicitly switches to
Clang when needed. No system OpenSSL dependency appears in this graph.

## Temporary hosted route (prepared, not dispatched)

`.github/workflows/m0-dependency-probe.yml` is workflow_dispatch-only, isolated
and temporary. It reuses the eight existing runners, four exact image digests,
Rustup 1.29.1 bootstrap hashes and existing ELF/PE verifiers. It uses exact
Rust 1.95.0 with explicit RUSTUP_TOOLCHAIN to prevent the product's 1.99.0
rust-toolchain.toml from overriding the probe. Release linking and native smoke
run alongside check/test/docs/Clippy; Linux smoke runs inside its image.
Mach-O load commands/minOS and PE headers/imports are retained for review,
with compiler/linker/assembler/SDK inventory and cc debug output. There are no
installer, packaging/rehearsal, coverage, release or publication jobs.

Existing routes: ubuntu-24.04/ubuntu-24.04-arm native digest-pinned containers;
macos-26-intel/macos-26; windows-2025/windows-11-vs2026-arm. The workflow retains
only dependency logs/binaries as short-lived evidence artifacts. Existing
checkout/setup-rust/upload-artifact full-SHA pins were verified against live
release tags. Actionlint, extracted Bash syntax and PowerShell AST checks pass; all eight
route identities match the existing workflow (evidence/workflow-static-checks.txt).
**This is linted proposed routing, not hosted
execution proof.** No hosted job link exists. Remove the workflow after proof
acceptance; do not turn it into a permanent QC framework.

## Gate disposition and shortest continuation

| Named gate | Disposition |
| --- | --- |
| Accepted M0-C std-only distribution rehearsal | Met previously; not repeated |
| Locked ring/RSA-disabled selected dependency graph | Met locally |
| Binary HANDLE/raw-path compatibility; real crypto/SSH/SFTP codec smoke | Met locally |
| Scoped strict Clippy and local declared-MSRV full gate | Met locally at 1.95.0; intrinsic minimum unproven, native foreign paths unmet |
| Audit/deny/lockfile policy | Met locally; no broad waiver |
| Source provenance/notices | Met locally: both original texts restored/preserved and package bytes verified |
| Source/package consumption | Met locally for approved source snapshot; root registry package unsupported by this unpublished route |
| Eight-target selected graph ABI/native acceptance | Unmet: two native Linux rows and one Windows cross-link; five targets unbuilt, foreign native paths absent |
| Temporary eight-target workflow preparation | Met locally as linted draft; dispatch not authorized |
| M0-D hosted delivery and maintainer acceptance | Unmet; separate signed Git delivery/push/dispatch approval required |
| Terminal/UI, mutations/Trash, session lifecycle/auth, transfers | Deferred by plan to M1/M2/M3/M4 |
| Long-term Yazi ownership and final shipping graph/publication | Deferred/open for M3/M8 |
| Genuine direct transfer | Deferred post-v1 |

Shortest continuation: separately authorize explicit-path signed/signed-off Git
delivery, push and the dependency-only eight-target dispatch. Review its actual conclusions
before proposing M0-D acceptance; retain PR #1 as draft until accepted.
No merge/tag/release/publication authorization is implied. Deferred installer
triage and CI simplification remain outside this probe.

## Reproduction commands actually run

Run from the repository root; build/cache output is isolated under
`/tmp/tfm-m0-dependencies-20261011`. Corresponding logs are under probe evidence.

```sh
rtk proxy env CARGO_TARGET_DIR=/tmp/tfm-m0-dependencies-20261011/target-msrv cargo +1.95.0 clippy --workspace --all-targets --all-features --locked --manifest-path probes/m0-dependencies/Cargo.toml -- -D warnings
rtk proxy env CARGO_TARGET_DIR=/tmp/tfm-m0-dependencies-20261011/target-msrv cargo +1.95.0 test --workspace --all-targets --all-features --locked --manifest-path probes/m0-dependencies/Cargo.toml
rtk proxy env CARGO_TARGET_DIR=/tmp/tfm-m0-dependencies-20261011/target-msrv cargo +1.95.0 test --workspace --doc --all-features --locked --manifest-path probes/m0-dependencies/Cargo.toml
rtk proxy env CARGO_TARGET_DIR=/tmp/tfm-m0-dependencies-20261011/target-msrv RUSTDOCFLAGS="-D warnings" cargo +1.95.0 doc --workspace --no-deps --all-features --locked --manifest-path probes/m0-dependencies/Cargo.toml
rtk proxy env CARGO_TARGET_DIR=/tmp/tfm-m0-dependencies-20261011/target-msrv cargo +1.95.0 build --release --workspace --all-targets --all-features --locked --manifest-path probes/m0-dependencies/Cargo.toml
rtk proxy env CARGO_TARGET_DIR=/tmp/tfm-m0-dependencies-20261011/target-previous cargo +1.94.0 check --workspace --all-targets --all-features --locked --manifest-path probes/m0-dependencies/Cargo.toml
rtk proxy env CARGO_TARGET_DIR=/tmp/tfm-m0-dependencies-20261011/target-previous cargo +1.94.0 check --ignore-rust-version --workspace --all-targets --all-features --locked --manifest-path probes/m0-dependencies/Cargo.toml
rtk proxy env CARGO_TARGET_DIR=/tmp/tfm-m0-dependencies-20261011/target-msrv cargo +1.95.0 package -p yazi-sftp --locked --allow-dirty --manifest-path probes/m0-dependencies/Cargo.toml
rtk proxy env CARGO_TARGET_DIR=/tmp/tfm-m0-dependencies-20261011/target-msrv cargo +1.95.0 package -p tfm-m0-dependency-probe --locked --allow-dirty --manifest-path probes/m0-dependencies/Cargo.toml
rtk proxy env CARGO_TARGET_DIR=/tmp/tfm-m0-dependencies-20261011/target-msrv cargo +1.95.0 check --workspace --all-targets --all-features --frozen --manifest-path /tmp/tfm-m0-dependencies-20261011/source-copy/Cargo.toml
rtk proxy env CARGO_TARGET_DIR=/tmp/tfm-m0-dependencies-20261011/target-xwin cargo +1.95.0 xwin build --release --locked --target x86_64-pc-windows-msvc --manifest-path probes/m0-dependencies/Cargo.toml
rtk proxy llvm-readobj --file-headers --coff-imports /tmp/tfm-m0-dependencies-20261011/target-xwin/x86_64-pc-windows-msvc/release/tfm-m0-dependency-probe.exe
rtk proxy pwsh -NoProfile -File scripts/verify-release-binary.ps1 -Arch x86_64 -Binary /tmp/tfm-m0-dependencies-20261011/target-xwin/x86_64-pc-windows-msvc/release/tfm-m0-dependency-probe.exe
rtk proxy cargo audit --file probes/m0-dependencies/Cargo.lock --json
rtk proxy cargo deny --manifest-path probes/m0-dependencies/Cargo.toml --locked --config probes/m0-dependencies/deny.toml check
rtk proxy actionlint .github/workflows/m0-dependency-probe.yml
```

The exact local Linux body is retained in `evidence/linux-local-command.sh`.
It was invoked through Docker with the repository mounted read-only at /work,
per-libc output at /proof, and the established image digest/bootstrap SHA.
GNU 1.95.0 full proof passes. MUSL full build/tests passed, then BusyBox rejected
GNU-style stat --format before strip; the portable stat -c continuation passed
strip/static/smoke. The proposed workflow uses the corrected portable form.
An initial GNU invocation selected 1.99.0 through product rust-toolchain.toml;
it was repeated with explicit 1.95.0 selection, whose log is the cited MSRV proof.
MUSL's initial bootstrap attempt also selected 1.99.0 and an edited temporary
script interrupted that run; neither is counted as the final minimum proof.
Cargo/rustup/xwin cache read-only failures received identical elevated retries;
no credentials/network diagnosis or policy bypass was inferred.

New probe functions previously measured CCN <=8/NLOC <=29 with Lizard's
10/50/8 thresholds; TFM-owned files are below 500 NLOC. Imported source was not
broadly reformatted. Original upstream hotspots/allowances were preserved.

The focused licence correction reran the exact Yazi package command above and
the exact probe cargo-deny command. Logs: evidence/package-vendor-restored.txt
and evidence/deny-restored.txt. Cargo-deny first hit the read-only advisory lock;
the identical elevated retry passed. No Rust source, manifest, features,
duplicate-policy exception or lockfile changed during this correction.
