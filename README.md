# Terminal File Manager

TFM is a cross-platform, keyboard-first terminal file manager in development for
Linux, macOS and Windows. The planned application will let you browse local and
SSH/SFTP directories in one or two panes, copy and move files with clear
overwrite and deletion safeguards, and manage transfers. Shell integration,
clipboard support and directory jumping will help you work from the terminal.

These file-manager features are planned. The current `tfm` executable prints
help or its version and rejects unsupported arguments. You can try that CLI
foundation from source:

```console
cargo run --locked -- --help
cargo run --locked -- --version
```

Running without arguments also prints help. The Rust package name is
`terminal-file-manager`; the command name is `tfm`.

## Progress

Completed:

- The CLI foundation, build tooling, security checks and native CI for Linux,
  macOS and Windows.
- Unix and PowerShell installers, with native build and installation rehearsals
  across eight platform targets.
- An eight-target feasibility check for the planned embedded SSH/SFTP stack.

Upcoming milestones:

- Local browsing, dual panes, sorting, filters and selection.
- Safe file operations, Trash, conflict handling and a transfer queue.
- SSH/SFTP browsing, local/remote transfers and labelled client relay between
  remote servers. Direct server-to-server transfers remain post-v1 work.
- Bookmarks, history, shell and clipboard integration, and directory jumping.
- Text viewing, filename search, directory comparison and terminal compatibility.
- Verification of the finished application and its release artifacts.

TFM has no public release yet. The completed engineering foundation does not
include file browsing or file operations. See [CONTRIBUTING](CONTRIBUTING.md)
for building and verification, and [acceptance](docs/acceptance.md) for recorded
proof and remaining release gates. TFM uses the MIT OR Apache-2.0 licence.

- [Architecture and future safety contracts](docs/architecture.md)
- [Acceptance gates and milestone boundaries](docs/acceptance.md)
- [Reference provenance](docs/provenance.md)
- [Post-v1 work](TODO.md)
- [Security reporting](SECURITY.md)
