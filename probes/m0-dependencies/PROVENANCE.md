# M0 dependency probe provenance — local implementation

This isolated, unpublished workspace is a feasibility input, not application code.
Policy passes with the separately approved licence additions and probe-only
exact-version duplicate exceptions. Local smoke tests exercise the selected
crypto/SSH-banner/terminal APIs and SFTP codec. Rust 1.95.0 strict Clippy, full local gates and native x86_64 GNU/MUSL
release proof pass. Foreign native proof remains pending. The approved licence
restoration passes exact-byte provenance, packaging and dependency-policy checks.

- Yazi tag: `v26.9.1`; commit: `8dd895c695a5950330c2623eb43debf323b60654`.
- Archive: https://codeload.github.com/sxyazi/yazi/tar.gz/8dd895c695a5950330c2623eb43debf323b60654
- Archive SHA256: `ff68e645d81c5b275e12f7be5493e8cae170026fce9537f40c67a786cfac235c`.
- Extracted only `yazi-sftp/`. Its original Apache-2.0 `LICENSE` is restored
  unchanged (SHA256
  `c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4`).
  The upstream workspace root MIT `LICENSE` is retained unchanged separately
  as `LICENSE-MIT` (SHA256
  `e66519d3d006dc76760d00d40361d24d2b97ba80ab93438885e78bc5a70bdd9c`).
  Upstream `license.workspace = true` inherits MIT despite the package's
  Apache licence text. The isolated manifest retains that upstream metadata;
  this records the inconsistency without inventing a new dual-licence grant.
  An earlier extraction replaced the Apache text with MIT; the approved
  restoration corrects that mistake. Both current texts match the archive,
  exact revision's live raw files and verified package contents byte-for-byte.
  See evidence/licence-provenance-restored.json. Shipping ownership/notices
  interpretation remains for M3/M8; neither upstream text was modified.
- Eleven upstream files have the narrow HANDLE correction: responses/handle.rs,
  fs/file.rs, fs/read_dir.rs, operator.rs, and requests/{close,read,write,fstat,
  set_stat,read_dir,extended}.rs. Owned handles are Vec<u8>; borrowed handles
  are Cow<[u8]> or &[u8]. Extension request names remain strings.
- lib.rs has one appended probe module declaration; added src/probe.rs exercises
  the private wire codec. evidence/snapshot-delta.txt records modified existing files; added files are hashed separately.
- Replaced workspace-inherited manifest entries with explicit package metadata
  and dependencies. Preserved upstream workspace Clippy entries exactly,
  including inherited allowances. The sole newly approved lint allowance is
  `#[allow(clippy::wrong_self_convention)]` on the imported trait method
  `AsSftpPath::as_sftp_path(self)` in src/path.rs. Its implementations/callers
  retain the upstream name. This preserves the API without weakening any
  TFM-owned/probe Clippy scope; no crate/workspace allowance or --no-deps bypass.
- Both russh edges use `=0.64.1`, defaults off, only `ring`. The corresponding
  upstream release commit is `e5d80a07dc554480b6ea3b2e14adfc190407b627`.
  Cargo consumes the checksummed crates.io release, not a Git dependency.
- Replaced upstream Tokio `full` with runtime/I/O/net/sync/time/macros features;
  pinned the approved 1.53.2 patch. Pinned compatible necessary snapshot libraries
  after live crates.io stable/MSRV checks; no other Yazi workspace crate or patch
  was imported. Ratatui uses only its Crossterm 0.29 backend at the top level;
  transitive feature unification remains visible in `evidence/features.txt`.
- Direct ring access executes an AES-256-GCM round trip; russh Ed25519 signing
  verifies valid payloads and rejects changed ones. They use public fixture
  material only. The same ring 0.17.14 instance serves russh; no second backend.
- The original HANDLE regression failed on invalid UTF-8 before correction and
  passes afterward. Other checks serialize all seven handle-consuming operations
  and distinct invalid-UTF8/literal replacement-character paths on every OS.
  Other upstream behaviour, including lifecycle limitations, is preserved.

`evidence/source-hashes.json` records every extracted/added file and its original
and current hash. Each original file has its upstream path; LICENSE-MIT points
to the upstream root LICENSE. Both licence hashes now match their originals.
`evidence/graph.json`, `features.txt`, `audit.json`, `deny.txt`, `handle-red.txt`
and the workspace lockfile retain the observed graph and local results.
Long-term shipping ownership and lifecycle hardening remain open for M3.

The project policy adds only approved licences. This workspace's separate deny.toml
adds five exact older-version duplicate skips after checking every reverse path
and its disjoint semver requirement in evidence/duplicate-paths.json. Global
multiple-versions denial is retained. Unification requires upstream changes.
The exceptions apply only to this frozen lockfile; re-review any lockfile change.
