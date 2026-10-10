# Security Policy

## Supported versions

Only the latest release line is supported.

## Security properties

M0-B only handles help, version and rejected arguments. It does not browse or
mutate user files, open SSH connections, access credentials or install software.
Future file-manager security contracts and their required proof are documented
in [architecture](docs/architecture.md) and [acceptance](docs/acceptance.md).
They are not implemented guarantees of this scaffold.

The build-only release framework has no publishing job or write permissions.
Executable downloads are version-pinned and SHA-256 verified before execution.
Dependency and workflow policy checks are mandatory. Installer and native
binary verification remain M0-C gates; no release-readiness claim follows from
this framework alone.

## Reporting a vulnerability

Please report privately via the GitHub security advisories feature for
[seapagan/terminal-file-manager](https://github.com/seapagan/terminal-file-manager/security/advisories),
or email the maintainer. Do not open a public issue for security problems.
