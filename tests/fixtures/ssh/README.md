# SSH/SFTP fixtures

See [SSH/SFTP interoperability](../../../CONTRIBUTING.md#sshsftp-interoperability)
for prerequisites, commands and when to run the matrix.

The [shared contract](../../ssh/interoperability.rs) covers RSA keys, encrypted
keys, agent and password authentication, credential and host-key rejection,
directory listings, raw filenames, metadata, symlinks, errors and disconnection.
The terminal driver enters and leaves a remote directory in the TFM pane.
AsyncSSH omits optional metadata on one entry to test its absence.

The CSV lists the server name, pinned base image and any required architecture.
Distribution builds install OpenSSH from the distribution's package repositories.
