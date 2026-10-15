# TFM M0 selected-dependency handover

The maintainer accepted the bounded eight-target dependency proof on 2026-10-11.
This is historical feasibility evidence for the selected minimal stack, not full
shipping-graph certification or an implementation of a file manager/SSH client.
The maintainer removed the temporary source, isolated workspace and hosted
workflow in [`e681a368`](https://github.com/seapagan/terminal-file-manager/commit/e681a368)
on 2026-10-11. That cleanup left product dependencies, MSRV, installers and
distribution unchanged. Later foundation changes appear in
[provenance](provenance.md#later-foundation-changes).

## Handover location and future ownership

This tracked document is the public handover. Its local planning counterpart is
`plans/tfm-m0-dependency-probe-handover.md`; Git excludes that file and the
canonical specification. Repository readers can use the evidence links and fix
guidance below without access to those local files.

During M3, use a **maintained narrow Yazi SFTP fork pinned to a full Cargo Git
revision**. Do not ship the M0 extracted snapshot or assume upstream has these
corrections. Fork creation, shipping ownership and notices interpretation remain
real-implementation work. Embedded russh/SFTP is the selected core direction;
system OpenSSH is optional, not a required core transport. Genuine direct
server-to-server payload transfer remains post-v1; M4 allows labelled client relay.

## Accepted native proof

[Run 38163834667](https://github.com/seapagan/terminal-file-manager/actions/runs/38163834667)
completed successfully at
[`1cf54087729748f087d05d911abab5d603fc7629`](https://github.com/seapagan/terminal-file-manager/commit/1cf54087729748f087d05d911abab5d603fc7629).
All eight jobs perform actual selected crypto/native dependency compilation and
release linking, native smoke with exact successful output, workspace check,
strict Clippy, five focused tests (two probe, three SFTP), doctests (none) and
warning-denying workspace docs. Unix smoke runs before and after strip. Windows
executes the matching native PE executable. No target success is inferred from
metadata or the accepted std-only rehearsal.

| Target / job | Result | Actual native compiler and object/ABI evidence |
| --- | --- | --- |
| [x86_64 Linux GNU](https://github.com/seapagan/terminal-file-manager/actions/runs/38163834667/job/114542138348) | Pass | Rust 1.95.0; GCC 14.2.1; ld/as 2.41; x86_64 ELF, max GLIBC 2.28 before/after strip, native smoke inside GLIBC 2.28 image |
| [aarch64 Linux GNU](https://github.com/seapagan/terminal-file-manager/actions/runs/38163834667/job/114542138245) | Pass | Rust 1.95.0; GCC 14.2.1; ld/as 2.41; AArch64 ELF, max GLIBC 2.28 before/after strip, native ARM smoke inside GLIBC 2.28 image |
| [x86_64 Linux MUSL](https://github.com/seapagan/terminal-file-manager/actions/runs/38163834667/job/114542138299) | Pass | Rust 1.95.0 MUSL host; GCC 14.2.0; ld/as 2.44; MUSL 1.2.5; x86_64 ELF, __init_libc pre-strip and no PT_INTERP/NEEDED pre/post-strip; native smoke both |
| [aarch64 Linux MUSL](https://github.com/seapagan/terminal-file-manager/actions/runs/38163834667/job/114542138252) | Pass | Rust 1.95.0 MUSL host; GCC 14.2.0; ld/as 2.44; MUSL 1.2.5; AArch64 ELF, same static pre/post checks and native ARM smoke |
| [x86_64 macOS](https://github.com/seapagan/terminal-file-manager/actions/runs/38163834667/job/114542138323) | Pass | Rust 1.95.0; Apple Clang/as 21.0.0 (clang-2100.1.1.101), ld 1267, Xcode 26.6 / SDK 26.5; x86_64 Mach-O, LC_VERSION_MIN_MACOSX 10.12 pre/post-strip, native smoke both |
| [aarch64 macOS](https://github.com/seapagan/terminal-file-manager/actions/runs/38163834667/job/114542138238) | Pass | Rust 1.95.0; same Apple compiler/linker/SDK; arm64 Mach-O, LC_BUILD_VERSION minos 11.0 pre/post-strip, native ARM smoke both |
| [x86_64 Windows](https://github.com/seapagan/terminal-file-manager/actions/runs/38163834667/job/114542138163) | Pass; SDK inventory incomplete | Rust 1.95.0 x64 host; MSVC toolset path 14.51.36231, COFF/PE dumper 14.51.36260.0 and PE linker 14.51; machine 8664, native Server 2025 10.0.26100 smoke |
| [aarch64 Windows](https://github.com/seapagan/terminal-file-manager/actions/runs/38163834667/job/114542138335) | Pass; SDK inventory incomplete | Rust 1.95.0 ARM host; LLVM Clang 22.1.8 for ring ARM C/assembly, same MSVC toolset/dumper/linker evidence; machine AA64, native Windows 11 10.0.26200 smoke |

Both macOS objects load only `/usr/lib/libiconv.2.dylib` and
`/usr/lib/libSystem.B.dylib`, with `/usr/lib/dyld`. The object minimum OS is a
load-command result, **not** evidence of execution on that older OS. Both
Windows objects import kernel32/oleaut32/bcryptprimitives/ntdll, the synch API
set, VCRUNTIME140.dll and UCRT math/runtime/stdio/locale/heap API sets. They
therefore require the appropriate architecture's VC runtime/UCRT and system
libraries; they are not self-contained static CRT executables. PE OS/subsystem
6.00 fields do not prove support for Windows Vista or override Rust/API floors.
Ring's ARM Windows native assembly route uses installed Clang; x64 uses the
MSVC toolset and the crate's packaged assembly inputs. No system SSH executable
is needed by the exercised graph.

## Accepted limitations and milestone boundaries

Windows exact SDK inventory was not captured: mixed PowerShell objects formatted
against OS table columns produced blank compiler/SDK rows. Verbose paths and
PE inspection still establish the recorded MSVC/linker evidence. Transcript
header OS versions are not SDK versions. This is an **accepted limitation**, not
an M0 blocker or a reason for another matrix/inventory run.

Rust **1.95.0** (`59807616e`, 2026-04-14; LLVM 22.1.2) is the **verified declared
supported floor**, including all eight native check/test/docs/Clippy/build paths.
It is not a proven intrinsic compiler minimum. Historical normal 1.94.0 rejected
manifest metadata, while diagnostic `--ignore-rust-version` check passed. No
lower compiler is promised, no further minimum experiment is scheduled, and the
product MSRV is unchanged.

M1 terminal lifecycle/raw local identity, M2 planner/staging/conflicts/Trash,
M3 real authenticated SFTP trust/config/session lifecycle/server fixtures,
M4 local/SSH transfer and approved client relay, and M8 final shipping graph,
notices, budgets and release acceptance remain future obligations. Dependency
smoke does not prove authentication, remote file operations, interoperability,
short writes, acknowledged close, cancellation or reconnect safety.

The std-only distribution rehearsal at `10e4be1`,
[run 38153439044](https://github.com/seapagan/terminal-file-manager/actions/runs/38153439044),
was the accepted distribution proof at probe closeout. The later
[run 38625643530](https://github.com/seapagan/terminal-file-manager/actions/runs/38625643530)
passed all eight native distribution targets and aggregate at `b3c31aa` on
2026-10-14. The approved CI cleanup and coverage merger also landed after
closeout; [provenance](provenance.md#later-foundation-changes) records them.
These checkpoints did not publish a release or change the M3/M8 obligations.

## Selected sources, versions and features

- russh `=0.64.1`: checksummed crates.io release, upstream revision
  `e5d80a07dc554480b6ea3b2e14adfc190407b627`; **both edges disable defaults and
  enable only `ring`**. Resolved features exactly `["ring"]`. No `rsa`,
  `aws-lc-rs` or `aws-lc-sys` package; ssh-key has no RSA feature. Ring itself has
  intrinsic RSA APIs/C code; this claim concerns disabled **SSH RSA support**.
  RSA-only SSH servers are not promised compatible. ssh-key `0.7.0-rc.11` is a
  recorded prerelease transitive dependency.
- ring `=0.17.14`: same backend instance used by russh and direct probe AEAD.
- Yazi `v26.9.1`, commit `8dd895c695a5950330c2623eb43debf323b60654`; only yazi-sftp
  was extracted, adapted to explicit manifest entries and corrected below.
  Upstream workspace russh 0.63.1/RSA was deliberately not inherited.
- Ratatui `=0.30.2`: defaults off, `crossterm_0_29` backend only at direct edge.
- Crossterm `=0.29.0`: defaults off, `events`, `windows`.
- Tokio `=1.53.2`: defaults off, `rt-multi-thread`, `io-util`, `net`, `sync`,
  `time`, `macros`; not upstream `full`. Transitive unified features are in the
  resolved graph appendix rather than assumed absent.
- Necessary Yazi libraries: bitflags `=2.13.2` / serde, hashbrown `=0.17.1` /
  serde, parking_lot `=0.12.5`, serde `=1.0.229` / derive, typed-path `=0.12.3`.

The probe exercised Ed25519 sign/verify and changed-message
rejection, ring AES-256-GCM round trip, russh `connect_stream` identification via
an in-memory duplex transport, and terminal APIs. The banner probe is not an
SSH authentication/server harness. All eight compiled the real ring graph;
foreign feature trees and the Linux russh rustc cfgs confirm ring-only.

## Provenance, consumability and policy

Archive: [exact upstream codeload source](https://codeload.github.com/sxyazi/yazi/tar.gz/8dd895c695a5950330c2623eb43debf323b60654).
Archive SHA256: `ff68e645d81c5b275e12f7be5493e8cae170026fce9537f40c67a786cfac235c`.
Probe lock: 246 packages, SHA256
`d94748671869250b4b8468c9e96608893d61dbf6b109d9f832fc0f13d61ca7bd`.
Every hosted checkout matched that exact raw lock hash. Registry source/checksums
and exact locked dependencies remain recoverable from the historical revision;
no runnable lockfile or source snapshot remains in this branch.

Original Yazi package Apache-2.0 LICENSE was restored unchanged, SHA256
`c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4`.
The upstream root MIT text was separately retained byte-identically as
LICENSE-MIT, SHA256
`e66519d3d006dc76760d00d40361d24d2b97ba80ab93438885e78bc5a70bdd9c`.
Upstream package manifest inherits workspace MIT despite the package Apache
text. Record both accurately; do not invent a dual-licence grant. Both texts
matched the exact archive/live revision and verified package bytes. Shipping
ownership/notices interpretation remains M3/M8.

Copied source workspace `check --frozen` passed. Yazi's 53-file package compiled
under exact 1.95.0 with both original texts. Those local `--allow-dirty` package
checks are source-consumption evidence. Root probe registry packaging was
unsupported because corrected yazi-sftp 26.9.1 is not published (registry 26.5.6).
This approved unpublished path workspace is not the eventual Git shipping source.

Audit 0.22.2 found zero vulnerabilities/warnings against RustSec revision
`7eebec69c352c7191b1f13eb95dd510eeca5d1de` (1,296 advisories). Deny 0.20.2 passed
advisories/licences/bans/sources without warnings. These dated frozen-graph
results do not certify future graphs. The probe's approved allowlist was MIT,
Apache-2.0, ISC, BSD-3-Clause, Zlib and Unicode-3.0. At closeout, project
`deny.toml` retained that list with global duplicate/wildcard/source/advisory
enforcement and **no skips**. Subsequent commit `986ae1f` narrowed the current
product allowlist to MIT and Apache-2.0; future dependencies need a fresh
licence-policy review rather than relying on this historical probe approval.

Probe-only exact older-version duplicate exceptions were getrandom 0.2.17,
hashbrown 0.16.1, sha3 0.11.0, syn 2.0.119 and windows-sys 0.52.0, paired with
0.4.3 / 0.17.1 / 0.12.0 / 3.0.8 / 0.61.2 respectively. Actual reverse edges
below establish disjoint requirements; unification would require upstream API
or dependency changes, not a root pin. These exceptions disappear with the
probe and do not authorise shipping policy exceptions or blanket waivers.

Registry-only compiler/deprecation/build warnings were observed; do not call the
whole registry graph warning-free. Cargo's normal registry lint capping was not
changed. TFM-owned/probe workspace strict Clippy passed. The only newly approved
upstream allowance was on the single imported trait method:

```rust
// Preserve the imported Yazi API; allowance applies only to this method.
#[allow(clippy::wrong_self_convention)]
fn as_sftp_path(self) -> SftpPath<'a>;
```

Do not rename that method, its implementations or callers. Original inherited
upstream allowances were preserved; no workspace-wide allowance or strict
Clippy `--no-deps` bypass was introduced. Review the actual future fork lint
scope rather than transferring an application-wide suppression.

## Exact HANDLE correction for the future fork

SFTP HANDLE is an opaque SSH length-prefixed **byte string**, not UTF-8. The
upstream `String` decoder rejected `[0x00, 0x80, 0xff, 0x5c]` with
`Serde("invalid utf-8 sequence of 1 bytes from index 1")`. The focused test failed
before correction and passed after; earlier M0-S also exposed the 129th held
handle. Never use `from_utf8`, lossy decoding, display text or native Windows
Path to represent this remote identity.

Change exactly these eleven upstream source files: responses/handle.rs,
fs/file.rs, fs/read_dir.rs, operator.rs, requests/{close,read,write,fstat,
set_stat,read_dir,extended}.rs. Own handles as Vec<u8>, borrow as &[u8] or
Cow<[u8]>, and keep the same wire length prefix. **Extension request names stay
strings**; only ExtendedFsync's opaque handle changes. Retain existing error,
lifetime and close handling; this patch does not harden session lifecycle.

The exact narrow correction is recorded below as documentation, not vendored
runnable source. Tabs and blank context whitespace are normalised for Markdown
display; the immutable historical delta retains original patch bytes. Reapply/review against the maintained fork revision and port
these focused regression findings during M3:

1. Decode/re-encode payload `[102,0,0,0,7,0,0,0,4,0,128,255,92]` through the real
   private packet codec. Serialization adds four packet-length bytes; require
   `encoded[4..] == payload` with no byte substitution.
2. Serialize Close, Read(offset 0/len 1), Write(offset 0/data b"x"), Fstat,
   FSetStat(default attrs), ReadDir and ExtendedFsync for `b"\0\x80\xff\\"`.
   Require handle length `[0,0,0,4]` and identical bytes. Normal framed request
   handle prefix begins at offset 9; fsync at `13 + "fsync@openssh.com".len()`.
   Require the fsync extension name itself remains `b"fsync@openssh.com"`.
3. Construct typed_path::UnixPathBuf from distinct `b"/raw-\xff\\\n"`,
   `b"/raw-\xfe\\\n"` and literal UTF-8 `"/raw-�\\\n".as_bytes()`.
   Serialize Open READ/default attrs: prefix at bytes 9..13 must equal original
   byte length, bytes 13..13+len and path.as_bytes() must equal original bytes.
   This prevents invalid UTF-8 aliases and preserves backslash/newline identity
   on Windows and Unix clients.

The temporary public probe module declaration in lib.rs was access plumbing,
not part of the required correctness patch. Do not carry it into production.

```diff
--- upstream/yazi-sftp/src/fs/file.rs
+++ snapshot/yazi-sftp/src/fs/file.rs
@@ -7,7 +7,7 @@
 pub struct File {
    session: Arc<Session>,

-   handle: String,
+   handle: Vec<u8>,
    cursor: u64,
    closed: bool,

@@ -34,7 +34,7 @@
 }

 impl File {
-   pub(crate) fn new(session: &Arc<Session>, handle: impl Into<String>) -> Self {
+   pub(crate) fn new(session: &Arc<Session>, handle: impl Into<Vec<u8>>) -> Self {
        Self {
            session: session.clone(),

--- upstream/yazi-sftp/src/fs/read_dir.rs
+++ snapshot/yazi-sftp/src/fs/read_dir.rs
@@ -5,7 +5,7 @@
 pub struct ReadDir {
    session: Arc<Session>,
    dir:     Arc<typed_path::UnixPathBuf>,
-   handle:  String,
+   handle:  Vec<u8>,

    name:   responses::Name<'static>,
    cursor: usize,
@@ -17,7 +17,7 @@
 }

 impl ReadDir {
-   pub(crate) fn new(session: &Arc<Session>, dir: SftpPath, handle: String) -> Self {
+   pub(crate) fn new(session: &Arc<Session>, dir: SftpPath, handle: Vec<u8>) -> Self {
        Self {
            session: session.clone(),
            dir: Arc::new(dir.into_owned()),
--- upstream/yazi-sftp/src/operator.rs
+++ snapshot/yazi-sftp/src/operator.rs
@@ -35,15 +35,15 @@
        Ok(File::new(&self.0, handle.handle))
    }

-   pub(crate) fn close(&self, handle: &str) -> Result<Receiver, Error> {
+   pub(crate) fn close(&self, handle: &[u8]) -> Result<Receiver, Error> {
        self.send_sync(requests::Close::new(handle))
    }

-   pub(crate) fn read(&self, handle: &str, offset: u64, len: u32) -> Result<Receiver, Error> {
+   pub(crate) fn read(&self, handle: &[u8], offset: u64, len: u32) -> Result<Receiver, Error> {
        self.send_sync(requests::Read::new(handle, offset, len))
    }

-   pub(crate) fn write(&self, handle: &str, offset: u64, data: &[u8]) -> Result<Receiver, Error> {
+   pub(crate) fn write(&self, handle: &[u8], offset: u64, data: &[u8]) -> Result<Receiver, Error> {
        self.send_sync(requests::Write::new(handle, offset, data))
    }

@@ -55,7 +55,7 @@
        Ok(attrs.attrs)
    }

-   pub(crate) async fn fstat(&self, handle: &str) -> Result<Attrs, Error> {
+   pub(crate) async fn fstat(&self, handle: &[u8]) -> Result<Attrs, Error> {
        let attrs: responses::Attrs = self.send(requests::Fstat::new(handle)).await?;
        Ok(attrs.attrs)
    }
@@ -68,7 +68,7 @@
        status.into()
    }

-   pub(crate) async fn fsetstat(&self, handle: &str, attrs: &Attrs) -> Result<(), Error> {
+   pub(crate) async fn fsetstat(&self, handle: &[u8], attrs: &Attrs) -> Result<(), Error> {
        let status: responses::Status = self.send(requests::FSetStat::new(handle, attrs)).await?;
        status.into()
    }
@@ -172,7 +172,7 @@
        status.into()
    }

-   pub(crate) fn fsync(&self, handle: &str) -> Result<Receiver, Error> {
+   pub(crate) fn fsync(&self, handle: &[u8]) -> Result<Receiver, Error> {
        if self.extensions.lock().get("fsync@openssh.com").is_none_or(|s| s != "1") {
            return Err(Error::Unsupported);
        }
--- upstream/yazi-sftp/src/requests/close.rs
+++ snapshot/yazi-sftp/src/requests/close.rs
@@ -5,11 +5,11 @@
 #[derive(Debug, Deserialize, Serialize)]
 pub struct Close<'a> {
    pub(crate) id: u32,
-   handle:        Cow<'a, str>,
+   handle:        Cow<'a, [u8]>,
 }

 impl<'a> Close<'a> {
-   pub(crate) fn new(handle: impl Into<Cow<'a, str>>) -> Self {
+   pub(crate) fn new(handle: impl Into<Cow<'a, [u8]>>) -> Self {
        Self { id: 0, handle: handle.into() }
    }

--- upstream/yazi-sftp/src/requests/extended.rs
+++ snapshot/yazi-sftp/src/requests/extended.rs
@@ -53,11 +53,11 @@
 // --- Fsync
 #[derive(Debug, Deserialize, Serialize)]
 pub struct ExtendedFsync<'a> {
-   handle: Cow<'a, str>,
+   handle: Cow<'a, [u8]>,
 }

 impl<'a> ExtendedFsync<'a> {
-   pub(crate) fn new(handle: impl Into<Cow<'a, str>>) -> Self { Self { handle: handle.into() } }
+   pub(crate) fn new(handle: impl Into<Cow<'a, [u8]>>) -> Self { Self { handle: handle.into() } }
 }

 impl ExtendedData for ExtendedFsync<'_> {
--- upstream/yazi-sftp/src/requests/fstat.rs
+++ snapshot/yazi-sftp/src/requests/fstat.rs
@@ -5,11 +5,11 @@
 #[derive(Debug, Deserialize, Serialize)]
 pub struct Fstat<'a> {
    pub(crate) id: u32,
-   handle:        Cow<'a, str>,
+   handle:        Cow<'a, [u8]>,
 }

 impl<'a> Fstat<'a> {
-   pub(crate) fn new(handle: impl Into<Cow<'a, str>>) -> Self {
+   pub(crate) fn new(handle: impl Into<Cow<'a, [u8]>>) -> Self {
        Self { id: 0, handle: handle.into() }
    }

--- upstream/yazi-sftp/src/requests/read.rs
+++ snapshot/yazi-sftp/src/requests/read.rs
@@ -5,7 +5,7 @@
 #[derive(Debug, Deserialize, Serialize)]
 pub struct Read<'a> {
    pub(crate) id: u32,
-   handle:        Cow<'a, str>,
+   handle:        Cow<'a, [u8]>,
    offset:        u64,
    len:           u32,
 }
@@ -13,7 +13,7 @@
 impl<'a> Read<'a> {
    pub(crate) fn new<H>(handle: H, offset: u64, len: u32) -> Self
    where
-       H: Into<Cow<'a, str>>,
+       H: Into<Cow<'a, [u8]>>,
    {
        Self { id: 0, handle: handle.into(), offset, len }
    }
--- upstream/yazi-sftp/src/requests/read_dir.rs
+++ snapshot/yazi-sftp/src/requests/read_dir.rs
@@ -5,11 +5,11 @@
 #[derive(Debug, Deserialize, Serialize)]
 pub struct ReadDir<'a> {
    pub(crate) id: u32,
-   handle:        Cow<'a, str>,
+   handle:        Cow<'a, [u8]>,
 }

 impl<'a> ReadDir<'a> {
-   pub(crate) fn new(handle: &'a str) -> Self { Self { id: 0, handle: handle.into() } }
+   pub(crate) fn new(handle: &'a [u8]) -> Self { Self { id: 0, handle: handle.into() } }

    pub(crate) fn len(&self) -> usize { size_of_val(&self.id) + 4 + self.handle.len() }
 }
--- upstream/yazi-sftp/src/requests/set_stat.rs
+++ snapshot/yazi-sftp/src/requests/set_stat.rs
@@ -27,12 +27,12 @@
 #[derive(Debug, Deserialize, Serialize)]
 pub struct FSetStat<'a> {
    pub(crate) id: u32,
-   handle:        Cow<'a, str>,
+   handle:        Cow<'a, [u8]>,
    attrs:         Cow<'a, Attrs>,
 }

 impl<'a> FSetStat<'a> {
-   pub(crate) fn new(handle: impl Into<Cow<'a, str>>, attrs: &'a Attrs) -> Self {
+   pub(crate) fn new(handle: impl Into<Cow<'a, [u8]>>, attrs: &'a Attrs) -> Self {
        Self { id: 0, handle: handle.into(), attrs: Cow::Borrowed(attrs) }
    }

--- upstream/yazi-sftp/src/requests/write.rs
+++ snapshot/yazi-sftp/src/requests/write.rs
@@ -5,7 +5,7 @@
 #[derive(Debug, Deserialize, Serialize)]
 pub struct Write<'a> {
    pub(crate) id: u32,
-   handle:        Cow<'a, str>,
+   handle:        Cow<'a, [u8]>,
    offset:        u64,
    data:          Cow<'a, [u8]>,
 }
@@ -13,7 +13,7 @@
 impl Write<'_> {
    pub(crate) fn new<'a, H, D>(handle: H, offset: u64, data: D) -> Write<'a>
    where
-       H: Into<Cow<'a, str>>,
+       H: Into<Cow<'a, [u8]>>,
        D: Into<Cow<'a, [u8]>>,
    {
        Write { id: 0, handle: handle.into(), offset, data: data.into() }
--- upstream/yazi-sftp/src/responses/handle.rs
+++ snapshot/yazi-sftp/src/responses/handle.rs
@@ -3,7 +3,7 @@
 #[derive(Debug, Deserialize, Serialize)]
 pub struct Handle {
    pub(crate) id:     u32,
-   pub(crate) handle: String,
+   pub(crate) handle: Vec<u8>,
 }

 impl Handle {
```

## Practical lessons and real implementation obligations

Keep remote POSIX path identity and HANDLE bytes separate from UI display and
native local paths. Do not inherit Yazi workspace defaults, RSA/aws-lc or Tokio
full accidentally. Re-evaluate exact resolved features/duplicates for the actual
fork/application graph. Prove native execution separately from cross-linking.
Rustup's explicit RUSTUP_TOOLCHAIN prevented the product toolchain override;
Windows core.autocrlf=false before checkout preserved the reviewed lock bytes;
macOS assembler inventory requires as --version. Workflow portability failures
before Cargo were not demonstrated dependency incompatibilities.

Manual workflow dispatch requires the file on the default branch; a temporary
path-filtered draft-PR trigger with exact head checkout enabled bounded proof
without merging. Do not keep that temporary workflow or redesign ordinary CI.
Use explicit text formatting for mixed PowerShell inventory shapes in future
ordinary implementation work; no M0 SDK rerun is required.

M3 must own the narrow fork/full revision, config semantics (aliases/Include/
Match/ProxyJump/ProxyCommand and unsupported directives), host trust, agent/key/
password/passphrase handling, no insecure host acceptance/secret persistence,
identity-aware bounded pooling, frame/request bounds, fatal-error propagation,
short writes, acknowledged close, teardown/cancellation/reconnect and uncertainty.
Exercise real isolated SFTP servers and raw remote names from native Windows and
Unix clients. M4 must retain explicit relay consent, safe staging/source removal
and honest sent-request uncertainty. M8 must revalidate the actual shipping
features/licences/notices/ABI/runtime floors and immutable delivery graph.

## Historical evidence retrieval

- [Frozen lockfile](https://github.com/seapagan/terminal-file-manager/blob/1cf54087729748f087d05d911abab5d603fc7629/probes/m0-dependencies/Cargo.lock) at the immutable accepted revision.
- [Resolved metadata/features](https://github.com/seapagan/terminal-file-manager/blob/1cf54087729748f087d05d911abab5d603fc7629/probes/m0-dependencies/evidence/graph.json) at the immutable accepted revision.
- [Original/current source hashes](https://github.com/seapagan/terminal-file-manager/blob/1cf54087729748f087d05d911abab5d603fc7629/probes/m0-dependencies/evidence/source-hashes.json) at the immutable accepted revision.
- [Full reviewed snapshot delta](https://github.com/seapagan/terminal-file-manager/blob/1cf54087729748f087d05d911abab5d603fc7629/probes/m0-dependencies/evidence/snapshot-delta.txt) at the immutable accepted revision.
- [Byte-preserving codec regressions](https://github.com/seapagan/terminal-file-manager/blob/1cf54087729748f087d05d911abab5d603fc7629/probes/m0-dependencies/vendor/yazi-sftp/src/probe.rs) at the immutable accepted revision.
- [Policy reverse paths](https://github.com/seapagan/terminal-file-manager/blob/1cf54087729748f087d05d911abab5d603fc7629/probes/m0-dependencies/evidence/duplicate-paths.json) at the immutable accepted revision.
- [Original provenance](https://github.com/seapagan/terminal-file-manager/blob/1cf54087729748f087d05d911abab5d603fc7629/probes/m0-dependencies/PROVENANCE.md) at the immutable accepted revision.

Hosted run/job links above preserve native results; uploaded artifacts were short-lived.
Post-run curated log hashes are retained below, not as runnable probe files.

| Target | Full job log SHA256 | Native evidence artifact SHA256 |
| --- | --- | --- |
| x86_64-unknown-linux-gnu | `fe288e094b9a1fff1199659151af2b998f2e06b9411e1a8a18ac4419944d5d46` | `dd71c7c875ae0d36fdfe53e023efc80c89869f7de1da7d3a8d155010fecd1e21` |
| aarch64-unknown-linux-gnu | `43dbb174338a50cdb8595d3a22085f85bb42e5eb8ebc45f527ff3a8a8d53b3f7` | `7f2f04c616ebb4f8f782787489801e9f7c84ab98348b94603d3cdc6fafa0a27e` |
| x86_64-unknown-linux-musl | `dd4b9b355f951c0863df35e9cb52801e965f8ac58701e486f46419cf5dc00ac6` | `dc3b6d994f5b32f2484edfcf6630fd98a7d283d28175413aee596e63d41c3f51` |
| aarch64-unknown-linux-musl | `6cc63ad7f7dff312786737a884aa44db2def07ee4e9c32845bbc83426bae49db` | `da446de2257204ec41283d9d760ce8350a5f7a0287bbfb5640bf453cda874874` |
| x86_64-apple-darwin | `fd7be0f6f89099dbc5b3587ead6b3e79fb5833e4009cf7dde76e9dc12ff4e8ff` | `72e67e53ef033dfc93b1a480c7135b29c35b5effbb809200401d8e6c601bb1e1` |
| aarch64-apple-darwin | `a3be5de317c6607bab6507566bd2244a6096d0e7fa80a53fcc79b5e4a07e0548` | `823e5bce0a4b34a849890ffdac4d36d85410fb82c50e2bc5979a96ae2d97c832` |
| x86_64-pc-windows-msvc | `ee4d327b53f0e30331c66f20cc75b48fb55210d0ccc6049df6050c641b82777b` | `5a4b4b7dc12d5e207890656c34a0c2ebcb6245816011286525046d8087f020df` |
| aarch64-pc-windows-msvc | `648aee275746b77500dd9f8ed585bc4382ac6f2d02c78f9300f60814d0f73953` | `618c86c2d5b0e468faec0a3690dcfc7475abfd953c99ccb6b7e540ba1a8b525e` |

## Exact duplicate reverse edges (historical, probe-only)

| Resolved crate | Parent | Requirement | Target condition |
| --- | --- | --- | --- |
| getrandom 0.2.17 | ring 0.17.14 | `^0.2.10` | all |
| getrandom 0.4.3 | crypto-bigint 0.7.5 | `^0.4` | all |
| getrandom 0.4.3 | crypto-common 0.2.2 | `^0.4` | all |
| getrandom 0.4.3 | rand 0.10.3 | `^0.4.0` | all |
| getrandom 0.4.3 | russh 0.64.1 | `^0.4` | all |
| hashbrown 0.16.1 | kasuari 0.4.12 | `^0.16` | all |
| hashbrown 0.17.1 | lru 0.18.5 | `^0.17.0` | all |
| hashbrown 0.17.1 | ratatui-core 0.1.2 | `^0.17` | all |
| hashbrown 0.17.1 | ratatui-widgets 0.3.2 | `^0.17` | all |
| hashbrown 0.17.1 | yazi-sftp 26.9.1 | `=0.17.1` | all |
| sha3 0.11.0 | ml-kem 0.3.2 | `^0.11` | all |
| sha3 0.12.0 | russh 0.64.1 | `^0.12.0` | all |
| syn 2.0.119 | curve25519-dalek-derive 0.1.1 | `^2.0.27` | all |
| syn 2.0.119 | delegate 0.13.5 | `^2` | all |
| syn 2.0.119 | derive_more-impl 2.1.1 | `^2.0.45` | all |
| syn 2.0.119 | enum_dispatch 0.3.13 | `^2.0` | all |
| syn 2.0.119 | palette_derive 0.7.7 | `^2.0.13` | all |
| syn 2.0.119 | strum_macros 0.28.0 | `^2.0` | all |
| syn 2.0.119 | windows-implement 0.60.2 | `^2.0` | all |
| syn 2.0.119 | windows-interface 0.59.3 | `^2.0` | all |
| syn 3.0.8 | darling_core 0.24.1 | `^3.0.0` | all |
| syn 3.0.8 | darling_macro 0.24.1 | `^3.0.0` | all |
| syn 3.0.8 | futures-macro 0.3.34 | `^3.0` | all |
| syn 3.0.8 | instability 0.3.14 | `^3.0` | all |
| syn 3.0.8 | serde_derive 1.0.229 | `^3` | all |
| syn 3.0.8 | thiserror-impl 2.0.21 | `^3` | all |
| syn 3.0.8 | tokio-macros 2.7.2 | `^3.0` | all |
| syn 3.0.8 | wasm-bindgen-macro-support 0.2.129 | `^3.0` | all |
| windows-sys 0.52.0 | ring 0.17.14 | `^0.52` | cfg(all(all(target_arch = "aarch64", target_endian = "little"), target_os = "windows")) |
| windows-sys 0.61.2 | errno 0.3.14 | `>=0.52, <0.62` | cfg(windows) |
| windows-sys 0.61.2 | mio 1.2.4 | `^0.61` | cfg(windows) |
| windows-sys 0.61.2 | russh-cryptovec 0.62.0 | `^0.61` | cfg(target_os = "windows") |
| windows-sys 0.61.2 | rustix 1.1.5 | `>=0.52, <0.62` | cfg(windows) |
| windows-sys 0.61.2 | socket2 0.6.5 | `>=0.60, <0.62` | cfg(windows) |
| windows-sys 0.61.2 | tokio 1.53.2 | `^0.61` | cfg(windows) |
| windows-sys 0.61.2 | tokio 1.53.2 | `^0.61` | cfg(windows) |

## Corrected source hash ledger

Hashes refer to the extracted yazi-sftp snapshot; original bytes come from the
exact upstream revision above. Unmodified files are recoverable from the
historical source-hashes link. This table includes every changed or added file.

| Snapshot file | Original SHA256 | Corrected SHA256 |
| --- | --- | --- |
| Cargo.toml | `f14c49b15f01f5d281a75769384962040b8feb43b8e908430d93233c696046b1` | `ea8823948e963aba051471cc6c28177e04eea4bd03948464d983562cde34cd76` |
| src/fs/file.rs | `1485c7661a634ea4b3f0c93f36168c8f8f870463ba87aaa941424957ac39ddf5` | `8d6a3ba32383eb58c69c12d929b87561b917b2b39a0fb6638e91331527b36702` |
| src/fs/read_dir.rs | `d0e399df87c2e850ef47509eae8900f388dcad7e93c55a5c05fd0e3a30b29b98` | `07d8497729663a810fbdce063526571fabdccde59ae5b23dd39e668048a9a917` |
| src/lib.rs | `550107a27bc24ee2e74894943711f085a2bd07915cbdd6cca018548b6beb219b` | `cf2bf03806e6b3c01675a89d3ae7a298489be8999d01646a0a66bb21f50a401d` |
| src/operator.rs | `66f599e0a2a5fb6e987f6b3b186c1e7d57dd53c07ab30e5dbd3b4ec6fef4e046` | `db838059b2fc1472a0dd65f438767b61ab9a40a74718a0cf47add8a86d0a68d1` |
| src/path.rs | `dd2a051c85c8b4e2469af7fe7f14f1b1bd72628428bb4630407d3ca55650eb01` | `5634b23531f081f5ecb5fcdb3527816916b86bee4030733cbf02f5e76bd836b7` |
| src/probe.rs | `added` | `3b3ed360d145f3246774aadcfcb1c6ec716594d55f77215c7c5ff07bf09c8187` |
| src/requests/close.rs | `e7267070e773801847bfa5598ef5e1356f7988217ee0fdb45387c87e1a180f32` | `3544becf5d272bd93b1c1ef822bd86db2ccd1a1de9fa1c1df16ba4f2b8de4804` |
| src/requests/extended.rs | `6f9bdcabf5de34e7403563352295a675d0b5869de3641fc78a3af31fdd8d7b2f` | `6daebf7c10f700c17053e2dc390fd13905672a63370bcaba497109a59913e7a2` |
| src/requests/fstat.rs | `c0ba404f96da6cbc2f0af67de43f870e94221156ddf09d2e0149f51c7b8fc721` | `b9442c643275b879a64b48f49fd6fede7f8a9ea9ebdf6bc2c7267a43ef0efb2f` |
| src/requests/read.rs | `56a1df05e7d69fb010ad53dbf259c51039b5ee61a05a7e8c42eb1d8f637fff85` | `1347b5b32d9cccfb497b22051cff137fef701a2d010899f805de7b837febd06d` |
| src/requests/read_dir.rs | `566d97bf492744100ca95d277c7d2a1a31f6a4bd26ef3578a2fb3448a3ad7190` | `cd344703f33d9b57076caaa55ac921e3849edb0d5338ff554e159e04fe0e4b60` |
| src/requests/set_stat.rs | `3cd1baf4495ca6b98542a59b768a5bb95a219e8c448cddb7f1b7069d19fef0ae` | `cdfb66768f3b4114a8c93f34ec6946a41ecce0ea22b82e1827ca1f03cb4d6b4e` |
| src/requests/write.rs | `8901609a212949a556f9625969868e7335220b48bbb089696f9bd9fe92c6ed6d` | `d7e0749e6b38fa6ab9e8fb1b4d0549bd252110eaf59383264286cac0e360c78d` |
| src/responses/handle.rs | `f415404bfa251b707450e0bfc935812da2241449b95f2b2c74d5e8425ffbbc21` | `e8a63fe2abf860ce1484511c4e670e3f54c2a5ecac4409c76232922dcecdfdb6` |

## Frozen resolved versions and unified features

This historical metadata lists 246 packages including the two local probe
workspace packages. All other packages use the checksummed crates.io registry
source recorded in the historical Cargo.lock. Features are unified metadata
features, not a promise that all conditional target edges compile on every OS.

| Package | Version | Resolved features |
| --- | --- | --- |
| aead | 0.6.1 | rand_core |
| aes | 0.9.3 | zeroize |
| aes-gcm | 0.11.1 | aes, zeroize |
| allocator-api2 | 0.2.21 | alloc |
| android_system_properties | 0.1.6 | none |
| approx | 0.5.1 | std |
| argon2 | 0.6.0 | alloc |
| autocfg | 1.5.1 | none |
| base16ct | 1.0.0 | alloc |
| base64ct | 1.8.3 | alloc |
| bcrypt-pbkdf | 0.11.0 | alloc |
| bitflags | 2.13.2 | serde, serde_core, std |
| blake2 | 0.11.0 | none |
| block-buffer | 0.12.1 | zeroize |
| block-padding | 0.4.2 | none |
| blowfish | 0.10.0 | bcrypt |
| bumpalo | 3.20.3 | default |
| by_address | 1.2.1 | none |
| byteorder | 1.5.0 | default, std |
| bytes | 1.12.1 | default, std |
| castaway | 0.2.4 | alloc |
| cbc | 0.2.1 | block-padding, default |
| cc | 1.8.0 | none |
| cfg-if | 1.0.5 | none |
| cfg_aliases | 0.2.2 | none |
| chacha20 | 0.10.2 | cipher, legacy, rng, zeroize |
| chrono | 0.4.45 | alloc, clock, default, iana-time-zone, js-sys, now, oldtime, std, wasm-bindgen, wasmbind, winapi, windows-link |
| cipher | 0.5.2 | block-buffer, block-padding, rand_core, stream-wrapper, zeroize |
| cmov | 0.5.4 | none |
| compact_str | 0.9.1 | std |
| const-oid | 0.10.2 | none |
| convert_case | 0.10.0 | none |
| core-foundation-sys | 0.8.7 | default, link |
| cpubits | 0.1.1 | none |
| cpufeatures | 0.3.1 | none |
| crossterm | 0.29.0 | bracketed-paste, default, derive-more, events, windows |
| crossterm_winapi | 0.9.1 | none |
| crypto-bigint | 0.7.5 | alloc, default, getrandom, hybrid-array, rand_core, subtle, zeroize |
| crypto-common | 0.2.2 | getrandom, rand_core, zeroize |
| ctr | 0.10.1 | none |
| ctutils | 0.4.3 | subtle |
| curve25519-dalek | 5.0.0 | alloc, default, digest, precomputed-tables, rand_core, zeroize |
| curve25519-dalek-derive | 0.1.1 | none |
| darling | 0.24.1 | default, suggestions |
| darling_core | 0.24.1 | strsim, suggestions |
| darling_macro | 0.24.1 | none |
| data-encoding | 2.11.1 | alloc, default, std |
| delegate | 0.13.5 | none |
| der | 0.8.2 | alloc, oid, pem, std, zeroize |
| deranged | 0.5.8 | default |
| derive_more | 2.1.1 | default, is_variant, std |
| derive_more-impl | 2.1.1 | default, is_variant |
| des | 0.9.0 | zeroize |
| digest | 0.11.3 | alloc, block-api, default, mac, oid, rand_core |
| document-features | 0.2.12 | default |
| ecdsa | 0.17.0 | algorithm, alloc, default, der, digest, getrandom, pem, pkcs8, spki, std |
| ed25519 | 3.0.0 | alloc, pkcs8 |
| ed25519-dalek | 3.0.0 | alloc, default, fast, pkcs8, rand_core, signature, zeroize |
| either | 1.19.0 | std, use_std |
| elliptic-curve | 0.14.1 | alloc, arithmetic, default, digest, ecdh, ff, getrandom, group, pem, pkcs8, sec1, std |
| enum_dispatch | 0.3.13 | none |
| equivalent | 1.0.2 | none |
| errno | 0.3.14 | default, std |
| ff | 0.14.0 | alloc |
| fiat-crypto | 0.3.0 | none |
| find-msvc-tools | 0.1.14 | none |
| foldhash | 0.2.0 | none |
| futures | 0.3.34 | alloc, async-await, default, executor, futures-executor, std |
| futures-channel | 0.3.34 | alloc, futures-sink, sink, std |
| futures-core | 0.3.34 | alloc, std |
| futures-executor | 0.3.34 | std |
| futures-io | 0.3.34 | std |
| futures-macro | 0.3.34 | none |
| futures-sink | 0.3.34 | alloc, std |
| futures-task | 0.3.34 | alloc, std |
| futures-util | 0.3.34 | alloc, async-await, async-await-macro, channel, futures-channel, futures-io, futures-macro, futures-sink, io, memchr, sink, slab, std |
| generic-array | 0.14.9 | none |
| generic-array | 1.4.5 | compat-0_14 |
| getrandom | 0.2.17 | none |
| getrandom | 0.4.3 | std, sys_rng, wasm_js |
| ghash | 0.6.0 | zeroize |
| group | 0.14.0 | alloc |
| hashbrown | 0.16.1 | allocator-api2, default, default-hasher, equivalent, inline-more, raw-entry |
| hashbrown | 0.17.1 | allocator-api2, default, default-hasher, equivalent, inline-more, raw-entry, serde |
| heck | 0.5.0 | none |
| hex | 0.4.3 | alloc |
| hex-literal | 1.1.0 | none |
| hkdf | 0.13.0 | none |
| hmac | 0.13.0 | none |
| hybrid-array | 0.4.15 | alloc, ctutils, extra-sizes, subtle, zeroize |
| iana-time-zone | 0.1.65 | fallback |
| iana-time-zone-haiku | 0.1.2 | none |
| ident_case | 1.0.1 | none |
| indoc | 2.0.8 | none |
| inout | 0.2.2 | block-padding |
| instability | 0.3.14 | none |
| itertools | 0.14.0 | use_alloc, use_std |
| itoa | 1.0.18 | none |
| js-sys | 0.3.106 | default, std, unsafe-eval |
| kasuari | 0.4.12 | std |
| keccak | 0.2.2 | none |
| kem | 0.3.0 | none |
| libc | 0.2.190 | default, extra_traits, std |
| libm | 0.2.16 | none |
| line-clipping | 0.3.8 | none |
| linux-raw-sys | 0.12.1 | auxvec, elf, errno, general, ioctl, no_std |
| litrs | 1.0.0 | none |
| lock_api | 0.4.14 | atomic_usize, default |
| log | 0.4.34 | none |
| lru | 0.18.5 | default, hashbrown |
| md5 | 0.8.1 | default, std |
| memchr | 2.8.3 | alloc, default, std |
| mio | 1.2.4 | default, log, net, os-ext, os-poll |
| ml-kem | 0.3.2 | alloc, default |
| module-lattice | 0.2.3 | alloc, ctutils |
| nix | 0.31.3 | default, mman |
| num-bigint | 0.5.1 | default, rand_0_10, rand_core_0_10, std |
| num-conv | 0.2.2 | none |
| num-integer | 0.1.47 | i128, std |
| num-traits | 0.2.19 | i128, std |
| num_threads | 0.1.7 | none |
| once_cell | 1.21.4 | alloc, default, race, std |
| p256 | 0.14.0 | alloc, arithmetic, default, digest, ecdh, ecdsa, ecdsa-core, getrandom, pem, pkcs8, precomputed-tables, sha2, sha256, std |
| p384 | 0.14.0 | alloc, arithmetic, default, digest, ecdh, ecdsa, ecdsa-core, getrandom, pem, pkcs8, precomputed-tables, sha2, sha384, std |
| p521 | 0.14.0 | alloc, arithmetic, default, digest, ecdh, ecdsa, ecdsa-core, getrandom, pem, pkcs8, precomputed-tables, sha512, std |
| pageant | 0.2.4 | default, namedpipes, wmmessage |
| palette | 0.7.7 | alloc, libm, std |
| palette_derive | 0.7.7 | none |
| palette_math | 0.7.7 | alloc, libm, std |
| parking_lot | 0.12.5 | default |
| parking_lot_core | 0.9.12 | none |
| password-hash | 0.6.1 | alloc, phc |
| pbkdf2 | 0.13.0 | default, hmac |
| pem-rfc7468 | 1.0.0 | alloc |
| phc | 0.6.1 | alloc |
| pin-project-lite | 0.2.17 | none |
| pkcs5 | 0.8.1 | alloc, pbes2, rand_core |
| pkcs8 | 0.11.0 | alloc, encryption, pem, pkcs5, std |
| poly1305 | 0.9.1 | zeroize |
| polyval | 0.7.3 | hazmat, zeroize |
| portable-atomic | 1.15.0 | require-cas, std |
| powerfmt | 0.2.1 | none |
| primefield | 0.14.0 | none |
| primeorder | 0.14.0 | alloc, basepoint-table, std |
| proc-macro2 | 1.0.107 | default, proc-macro |
| quote | 1.0.47 | default, proc-macro |
| r-efi | 6.0.0 | none |
| rand | 0.10.3 | alloc, default, std, std_rng, sys_rng, thread_rng |
| rand_core | 0.10.1 | none |
| ratatui | 0.30.2 | crossterm, crossterm_0_29, std |
| ratatui-core | 0.1.2 | default, std, underline-color |
| ratatui-crossterm | 0.1.2 | crossterm_0_29, default, underline-color |
| ratatui-widgets | 0.3.2 | std |
| redox_syscall | 0.5.18 | default, userspace |
| rfc6979 | 0.6.0 | none |
| ring | 0.17.14 | alloc, default, dev_urandom_fallback |
| russh | 0.64.1 | ring |
| russh-cryptovec | 0.62.0 | ssh-encoding |
| russh-util | 0.52.0 | none |
| rustc_version | 0.4.1 | none |
| rustix | 1.1.5 | alloc, std, stdio, termios |
| rustversion | 1.0.23 | none |
| ryu | 1.0.23 | none |
| salsa20 | 0.11.0 | none |
| scopeguard | 1.2.0 | none |
| scrypt | 0.12.0 | none |
| sec1 | 0.8.1 | alloc, ctutils, default, der, pem, point, std, subtle, zeroize |
| semver | 1.0.28 | default, std |
| serde | 1.0.229 | alloc, default, derive, serde_derive, std |
| serde_core | 1.0.229 | alloc, result, std |
| serde_derive | 1.0.229 | default |
| serdect | 0.4.3 | alloc |
| sha1 | 0.11.0 | alloc, default, oid |
| sha2 | 0.11.0 | alloc, default, oid |
| sha3 | 0.11.0 | none |
| sha3 | 0.12.0 | alloc, default, oid |
| shlex | 2.0.1 | default, std |
| signal-hook | 0.3.18 | channel, default, iterator |
| signal-hook-mio | 0.2.5 | mio-1_0, support-v1_0 |
| signal-hook-registry | 1.4.8 | none |
| signature | 3.0.0 | alloc, digest, rand_core |
| slab | 0.4.12 | std |
| smallvec | 1.16.3 | none |
| socket2 | 0.6.5 | all |
| spki | 0.8.1 | alloc, pem, std |
| sponge-cursor | 0.1.0 | none |
| ssh-cipher | 0.3.0 | aes, chacha20poly1305, encoding, rand_core, zeroize |
| ssh-encoding | 0.3.0 | alloc, base64, bytes, ctutils, digest, pem, zeroize |
| ssh-key | 0.7.0-rc.11 | alloc, default, ecdsa, ed25519, encryption, p256, p384, p521, ppk, rand_core, sha1, std |
| static_assertions | 1.1.0 | none |
| strsim | 0.11.1 | none |
| strum | 0.28.0 | derive, std, strum_macros |
| strum_macros | 0.28.0 | none |
| subtle | 2.6.1 | const-generics, default, i128, std |
| syn | 2.0.119 | clone-impls, default, derive, extra-traits, full, parsing, printing, proc-macro, visit-mut |
| syn | 3.0.8 | clone-impls, default, derive, extra-traits, full, parsing, printing, proc-macro, visit, visit-mut |
| tfm-m0-dependency-probe | 0.0.0 | none |
| thiserror | 2.0.21 | default, std |
| thiserror-impl | 2.0.21 | none |
| time | 0.3.55 | alloc, local-offset, std |
| time-core | 0.1.9 | none |
| tokio | 1.53.2 | bytes, default, io-util, libc, macros, mio, net, rt, rt-multi-thread, socket2, sync, time, tokio-macros, windows-sys |
| tokio-macros | 2.7.2 | none |
| typed-path | 0.12.3 | default, std |
| typenum | 1.20.1 | const-generics |
| unicode-ident | 1.0.26 | none |
| unicode-segmentation | 1.13.3 | none |
| unicode-truncate | 2.0.1 | std |
| unicode-width | 0.2.2 | cjk, default |
| universal-hash | 0.6.1 | none |
| untrusted | 0.9.0 | none |
| version_check | 0.9.5 | none |
| wasi | 0.11.1+wasi-snapshot-preview1 | default, std |
| wasm-bindgen | 0.2.129 | default, std |
| wasm-bindgen-futures | 0.4.79 | default, std |
| wasm-bindgen-macro | 0.2.129 | none |
| wasm-bindgen-macro-support | 0.2.129 | none |
| wasm-bindgen-shared | 0.2.129 | none |
| winapi | 0.3.9 | consoleapi, handleapi, impl-default, processenv, synchapi, winbase, winerror, winuser |
| winapi-i686-pc-windows-gnu | 0.4.0 | none |
| winapi-x86_64-pc-windows-gnu | 0.4.0 | none |
| windows | 0.62.2 | Win32, Win32_Foundation, Win32_Security, Win32_Security_Authentication, Win32_Security_Authentication_Identity, Win32_Security_Cryptography, Win32_System, Win32_System_DataExchange, Win32_System_Memory, Win32_System_Threading, Win32_System_WindowsProgramming, Win32_UI, Win32_UI_WindowsAndMessaging, default, std |
| windows-collections | 0.3.2 | std |
| windows-core | 0.62.2 | default, std |
| windows-future | 0.3.2 | std |
| windows-implement | 0.60.2 | none |
| windows-interface | 0.59.3 | none |
| windows-link | 0.2.1 | none |
| windows-numerics | 0.3.1 | std |
| windows-result | 0.4.1 | std |
| windows-strings | 0.5.1 | default, std |
| windows-sys | 0.52.0 | Win32, Win32_Foundation, Win32_System, Win32_System_Threading, default |
| windows-sys | 0.61.2 | Wdk, Wdk_Foundation, Wdk_Storage, Wdk_Storage_FileSystem, Wdk_System, Wdk_System_IO, Win32, Win32_Foundation, Win32_Networking, Win32_Networking_WinSock, Win32_Security, Win32_Storage, Win32_Storage_FileSystem, Win32_System, Win32_System_Diagnostics, Win32_System_Diagnostics_Debug, Win32_System_IO, Win32_System_Memory, Win32_System_Pipes, Win32_System_SystemInformation, Win32_System_SystemServices, Win32_System_Threading, Win32_System_WindowsProgramming, default |
| windows-targets | 0.52.6 | none |
| windows-threading | 0.2.1 | none |
| windows_aarch64_gnullvm | 0.52.6 | none |
| windows_aarch64_msvc | 0.52.6 | none |
| windows_i686_gnu | 0.52.6 | none |
| windows_i686_gnullvm | 0.52.6 | none |
| windows_i686_msvc | 0.52.6 | none |
| windows_x86_64_gnu | 0.52.6 | none |
| windows_x86_64_gnullvm | 0.52.6 | none |
| windows_x86_64_msvc | 0.52.6 | none |
| wnaf | 0.14.1 | none |
| yazi-sftp | 26.9.1 | none |
| zeroize | 1.9.1 | alloc, default |

## Probe closeout verification (2026-10-11)

The temporary workflow and entire isolated probe tree (88 tracked files plus
local temporary evidence) were removed. No references to their runnable paths
remain in active CI/task/script/test configuration. Product Cargo.toml/Cargo.lock,
Makefile.toml (including verify-fast), installers, accepted release rehearsal,
CHANGELOG.md and project deny.toml are unchanged. No probe duplicate skip leaks
into project policy. The approved licence allowlist remained intact at this
dated closeout; the subsequent product-policy change is recorded above.

`rtk proxy cargo make verify` passed, including mandatory Linux xwin check,
strict Clippy and build, exact declared product MSRV gate, audit/deny, Python
Ruff/strict mypy/tests, complexity, workflow actionlint/ShellCheck/pedantic Zizmor,
Rust tests/docs/package and the existing local release/installer regressions.
The initial sandbox attempt stopped at a read-only advisory-cache lock; the
identical elevated retry passed. Deny reported only license-not-encountered
warnings for the retained approved licences unused by the std-only product at
that checkpoint.
No waiver was added. `rtk proxy cargo make verify-fast` passed unchanged, and
`rtk proxy git diff --check` passed after documentation patch-display whitespace
normalisation. Documentation preservation was checked against the existing
source, graph, reverse-edge and hosted evidence before deletion.

The signed cleanup commit delivered this handover and the authorized removals.
Local handover/spec/plan files stayed outside Git. Routine hosted checks passed
at `e681a368`; the closeout did not rerun the dependency matrix, SDK inventory or
distribution rehearsal. The later distribution checkpoint is recorded above.
