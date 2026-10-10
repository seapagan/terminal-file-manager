# Terminal File Manager

`terminal-file-manager` is the Rust package; `tfm` is its executable.
This repository currently contains the M0-B engineering foundation. The
executable prints help or its version and rejects unsupported arguments.
File browsing, TUI, filesystem operations and SSH are future milestones.

```console
cargo run --locked -- --help
cargo run --locked -- --version
```

No-argument invocation prints help. Help/version return 0; unsupported
arguments print a diagnostic to stderr and return 2.

The package uses edition 2024, exact Rust 1.95.0 as the approved scaffold MSRV,
and MIT OR Apache-2.0 licensing. Development uses pinned Rust 1.99.0.
See [CONTRIBUTING](CONTRIBUTING.md) for tools and `cargo make verify`.

The manual [release rehearsal](.github/workflows/release-rehearsal.yml)
retains all eight Linux GNU/MUSL, macOS and Windows x64/ARM64 targets. It never
publishes. Executable verifiers/installers and real native/ABI proof belong
to M0-C; hosted proof follows separately authorized Git delivery. No binary
release or installer is offered at this checkpoint.

- [Architecture and future safety contracts](docs/architecture.md)
- [Acceptance gates and milestone boundaries](docs/acceptance.md)
- [Reference provenance](docs/provenance.md)
- [Post-v1 work](TODO.md)
- [Security reporting](SECURITY.md)
