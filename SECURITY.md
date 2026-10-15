# Security Policy

## Supported versions

TFM has no public release yet. Report issues against the development branch.
Once releases begin, the maintainer will support the latest release line.

## Security properties

The application handles help, version and rejected arguments. It does not browse
or mutate user files, open SSH connections or access credentials. The repository
also contains Unix and PowerShell installers, which can replace an installed
binary after archive, checksum, architecture and version validation.
Future file-manager security contracts and their required proof are documented
in [architecture](docs/architecture.md) and [acceptance](docs/acceptance.md).
They are not implemented guarantees of this scaffold.

The build-only release rehearsal has no publishing job or write permissions.
Its Rustup bootstrap verifies pinned executable downloads before execution.
The consumer installers use HTTPS for remote release downloads and verify
SHA-256 sidecars before extraction. Checksums establish byte integrity; users
must trust the publisher and any configured Windows release mirror.
Dependency and workflow policy checks remain mandatory. Native binary and
installer proof passed for the recorded M0 checkpoint. Final shipping-graph
verification and publication remain separate release gates.

## Reporting a vulnerability

Please report privately via the GitHub security advisories feature for
[seapagan/terminal-file-manager](https://github.com/seapagan/terminal-file-manager/security/advisories),
or email the maintainer. Do not open a public issue for security problems.
