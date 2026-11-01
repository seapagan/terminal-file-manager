# Security Policy

## Supported versions

TFM is in development and has no public releases. Report vulnerabilities in the
current `main` branch. Once releases begin, the maintainer will support the
latest release line.

## Security and your files

TFM browses and manages local files on Linux, macOS and Windows. It can copy,
move, rename, create and delete files, and change supported file attributes.
It uses your account's permissions; avoid running it as root or administrator.

Review the selected files and destination before starting an operation. Permanent
deletion cannot be undone. TFM does not fall back to permanent deletion when
Trash fails. Keep backups of important files: an error or cancellation can leave
part of an operation complete. Check Jobs for the result and any recovery details.

Enter navigates into directories; it does not open or execute files. SSH/SFTP
connections are not available yet.

## Reporting a vulnerability

[Report a vulnerability privately on GitHub](https://github.com/seapagan/terminal-file-manager/security/advisories/new).
Do not open a public issue for security problems.

Include your operating system, the TFM version or commit, steps to reproduce the
problem and its potential impact. Use disposable test files for examples. Do not
include passwords, private keys or personal files.
