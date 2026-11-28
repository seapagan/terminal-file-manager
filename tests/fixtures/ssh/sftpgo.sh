#!/bin/sh
set -eu

sh /fixture/layout.sh /tmp/tfm-home
cp /fixture/host /tmp/tfm-host
chmod 600 /tmp/tfm-host
sftpgo --version
export SFTPGO_SFTPD__HOST_KEYS=/tmp/tfm-host
export SFTPGO_HTTPD__BINDINGS__0__PORT=0
exec sftpgo serve --loaddata-from /fixture/users.json
