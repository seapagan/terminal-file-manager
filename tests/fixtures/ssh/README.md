# SSH/SFTP fixtures

See [SSH/SFTP interoperability](../../../CONTRIBUTING.md#sshsftp-interoperability)
for prerequisites, commands and when to run the matrix.

The [shared contract](../../ssh/interoperability.rs) covers RSA keys, encrypted
keys, agent and password authentication, credential and host-key rejection,
directory listings, raw filenames, metadata, symlinks, errors and disconnection.
The terminal driver enters and leaves a remote directory in the TFM pane.
AsyncSSH omits optional metadata on one entry to test its absence.

The CSV lists the server name, pinned base image and any required architecture.
Both launchers rebuild the distribution and AsyncSSH images from pinned bases
without cached install layers. SFTPGo uses its pinned upstream image.
Distribution builds refresh their package indexes and install the current
OpenSSH package from those repositories. Each row logs the installed daemon
version. The Linux client keeps its Cargo caches.
